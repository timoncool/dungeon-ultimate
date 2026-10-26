//! Игра как MCP-сервер: Streamable HTTP, JSON-RPC без сессий на `/mcp` — устройство YuE2 Studio.
//!
//! Каждый тул — маршрут API игры, вызванный внутри процесса через тот же роутер, с которым
//! говорит окно: агент делает ровно то же, что кнопки, тем же кодом. Тулы `ui_*` видят окно и
//! работают с ним как человек. Тулы `gm_*` дают агенту вести партию: инструменты ведущего,
//! ход рассказчика, механика. Когда игрок выбрал рассказчиком агента, каждый проход хода
//! приходит ему вопросом (`gm_requests_wait` → `gm_request_answer`) через локальный
//! OpenAI-совместимый вход `/agent/v1/chat/completions`.

use std::collections::HashMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{header, HeaderMap, Method, Request, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use serde_json::{json, Value};
use tower::ServiceExt;

/// Роутер API игры; ставится, когда сервер его собрал.
static API: OnceLock<Router> = OnceLock::new();

pub fn install(api: Router) {
    let _ = API.set(api);
}

const GAME: &str = "Dungeon Ultimate";
/// Ответы длиннее обрезаются: история в сотни сообщений — больше, чем агент читает за раз.
const LIMIT: usize = 60_000;
const WAIT_DEFAULT: u64 = 30;
const WAIT_LONGEST: u64 = 55;

// ---------------------------------------------------------------- что отправляет тул

enum Payload {
    None,
    Json(Value),
    /// Сырые байты файла с диска (запись голоса для распознавания).
    File(PathBuf),
    /// Multipart, как его шлёт окно: поля и файл.
    Form { fields: Vec<(String, String)>, file: Option<(String, PathBuf)> },
    /// Команда окну: что на экране, клики, ввод. Отвечает сама страница.
    Window { command: &'static str, args: Value, seconds: u64 },
    /// Тул, который сервер собирает сам из нескольких шагов.
    Composite(&'static str),
}

struct Call {
    method: Method,
    path: String,
    payload: Payload,
}

fn get(path: String) -> Result<Call, String> {
    Ok(Call { method: Method::GET, path, payload: Payload::None })
}

fn send(method: Method, path: String, body: Value) -> Result<Call, String> {
    Ok(Call { method, path, payload: Payload::Json(body) })
}

fn window(command: &'static str, args: &Value, seconds: u64) -> Result<Call, String> {
    Ok(Call { method: Method::GET, path: String::new(), payload: Payload::Window { command, args: args.clone(), seconds } })
}

fn composite(kind: &'static str) -> Result<Call, String> {
    Ok(Call { method: Method::GET, path: String::new(), payload: Payload::Composite(kind) })
}

/// Обязательный текстовый аргумент или сообщение, какого не хватает.
fn text(args: &Value, name: &str) -> Result<String, String> {
    args.get(name).and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()).map(str::to_string).ok_or_else(|| format!("'{name}' is required"))
}

/// Сегмент пути, экранированный.
fn segment(value: &str) -> String {
    value.bytes().map(|byte| if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) { (byte as char).to_string() } else { format!("%{byte:02X}") }).collect()
}

/// Аргументы без тех, что ушли в путь.
fn body_without(args: &Value, taken: &[&str]) -> Value {
    let mut body = args.as_object().cloned().unwrap_or_default();
    for name in taken {
        body.remove(*name);
    }
    Value::Object(body)
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

fn nothing() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn chat_only() -> Value {
    object(json!({ "chat_id": { "type": "string", "description": "the story's id from chats_list" } }), &["chat_id"])
}

fn query(pairs: &[(&str, Option<String>)]) -> String {
    let parts: Vec<String> = pairs.iter().filter_map(|(key, value)| value.as_ref().map(|value| format!("{key}={}", segment(value)))).collect();
    if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) }
}

// ---------------------------------------------------------------- тулы

struct Tool {
    name: &'static str,
    description: &'static str,
    schema: Value,
}

fn tool(name: &'static str, description: &'static str, schema: Value) -> Tool {
    Tool { name, description, schema }
}

/// Тулы ведущего — инструменты рассказчика из `game_tools`, отданные агенту.
fn gm_tools() -> Vec<Tool> {
    crate::game_tools::all()
        .into_iter()
        .map(|game| {
            let mut schema = game.parameters.clone();
            if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
                properties.insert("chat_id".into(), json!({ "type": "string", "description": "the story's id from chats_list" }));
            }
            let mut required: Vec<Value> = schema.get("required").and_then(Value::as_array).cloned().unwrap_or_default();
            required.push(json!("chat_id"));
            schema["required"] = Value::Array(required);
            if let Some(map) = schema.as_object_mut() {
                map.remove("additionalProperties");
            }
            let name: &'static str = Box::leak(format!("gm_{}", game.name).into_boxed_str());
            let description: &'static str = Box::leak(
                format!("Game-master tool of the story's engine (the same the narrator model uses during a turn): {}", game.description).into_boxed_str(),
            );
            Tool { name, description, schema }
        })
        .collect()
}

fn tools() -> &'static Vec<Tool> {
    static TOOLS: OnceLock<Vec<Tool>> = OnceLock::new();
    TOOLS.get_or_init(|| {
        let mut list = vec![
            tool("game_status", "What the game is doing now: server health, where each stage runs (card, processor, cloud, agent), what is still to download, whether the window and an agent are connected and how many narrator requests wait for you. Start here.", nothing()),
            tool("game_release", "Whether a newer Dungeon Ultimate is out.", nothing()),
            tool("game_where", "Where each stage (narrator, image, tts, asr) runs right now: gpu, cpu, cloud.", nothing()),
            tool("hardware", "Graphics card, video memory, RAM and their use now.", nothing()),
            tool("game_wait", "Wait for a story turn started by story_turn: its stages, prose so far, voice clips, and at the end the saved message, the game events and the frame request. Returns when the turn ends or after seconds (30 by default, at most 55); call again to keep waiting.", object(json!({ "job_id": { "type": "string" }, "seconds": { "type": "integer" } }), &["job_id"])),
            tool("chats_list", "The stories, newest first: id, title, when changed.", nothing()),
            tool("chat_create", "Start a new story. settings is the story's settings object (world, rules, language, image and voice switches) as settings_defaults returns it; omitted fields take the defaults.", object(json!({ "title": { "type": "string" }, "settings": { "type": "object" } }), &[])),
            tool("chat_get", "One story with its messages, settings and characters.", chat_only()),
            tool("chat_update", "Rename a story or change its settings (a whole settings object).", object(json!({ "chat_id": { "type": "string" }, "title": { "type": "string" }, "settings": { "type": "object" } }), &["chat_id"])),
            tool("chat_delete", "Delete a story with its pictures.", chat_only()),
            tool("chat_events", "The story's game journal: rolls, damage, items, quests.", chat_only()),
            tool("items_list", "The party's items in a story.", chat_only()),
            tool("quests_list", "The story's quests: offered, taken, done, failed.", chat_only()),
            tool("achievements_list", "Achievements earned in all stories.", nothing()),
            tool("quest_update", "Take, refuse, complete or fail a quest: status offered|active|done|failed|declined.", object(json!({ "chat_id": { "type": "string" }, "quest_id": { "type": "string" }, "status": { "type": "string" } }), &["chat_id", "quest_id", "status"])),
            tool("item_equip", "Put an item on or take it off.", object(json!({ "chat_id": { "type": "string" }, "item_id": { "type": "string" }, "equipped": { "type": "boolean" } }), &["chat_id", "item_id", "equipped"])),
            tool("item_image", "Draw an item's picture (a graphics-card job behind any running turn).", object(json!({ "chat_id": { "type": "string" }, "item_id": { "type": "string" } }), &["chat_id", "item_id"])),
            tool("message_delete", "Delete a message; include_after (true by default) also deletes everything after it and rolls back its game consequences.", object(json!({ "chat_id": { "type": "string" }, "message_id": { "type": "string" }, "include_after": { "type": "boolean" } }), &["chat_id", "message_id"])),
            tool("message_edit", "Rewrite a message's text.", object(json!({ "chat_id": { "type": "string" }, "message_id": { "type": "string" }, "content": { "type": "string" } }), &["chat_id", "message_id", "content"])),
            tool("characters_list", "The story's characters: hero first, with details, inventory, skills, spells, voice and portrait.", chat_only()),
            tool("character_create", "Add a character to a story. details holds appearance and 'Пол: мужской/женский'; voice is a name from voices_list or cloud_voices.", object(json!({ "chat_id": { "type": "string" }, "name": { "type": "string" }, "details": { "type": "string" }, "inventory": { "type": "string" }, "skills": { "type": "string" }, "spells": { "type": "string" }, "voice": { "type": "string" } }), &["chat_id", "name"])),
            tool("character_update", "Change a character's fields (only those given).", object(json!({ "chat_id": { "type": "string" }, "character_id": { "type": "string" }, "name": { "type": "string" }, "details": { "type": "string" }, "inventory": { "type": "string" }, "skills": { "type": "string" }, "spells": { "type": "string" }, "voice": { "type": "string" } }), &["chat_id", "character_id"])),
            tool("character_delete", "Remove a character from a story.", object(json!({ "chat_id": { "type": "string" }, "character_id": { "type": "string" } }), &["chat_id", "character_id"])),
            tool("actions_suggest", "Action chips for the player after a passage, written by the game's narrator.", object(json!({ "passage": { "type": "string" }, "language": { "type": "string" } }), &["passage"])),
            tool("story_suggest", "Fill one field of the new-story form (world, style, character, opening) by the game's narrator.", object(json!({ "field": { "type": "string" }, "context": { "type": "string" }, "language": { "type": "string" } }), &["field"])),
            tool("story_setup", "A whole new-story setup (world, hero, opening) by the game's narrator.", object(json!({ "setting": { "type": "string" }, "gender": { "type": "string" }, "language": { "type": "string" } }), &[])),
            tool("image_style", "A picture style prefix for a setting.", object(json!({ "setting": { "type": "string" }, "wish": { "type": "string" } }), &[])),
            tool("character_generate", "A generated hero card from a hint.", object(json!({ "hint": { "type": "string" }, "language": { "type": "string" } }), &[])),
            tool("file_upload", "Upload a picture from a path on this computer (a portrait, a reference); returns the attachment to put into a character or a frame request.", object(json!({ "path": { "type": "string" } }), &["path"])),
            tool("local_data", "How much the stories, pictures and uploads take on disk.", nothing()),
            tool("local_data_wipe", "Delete every story, picture and upload. Irreversible.", nothing()),
            tool("settings_defaults", "The default story settings object.", nothing()),
            tool("tools_try", "Run the narrator's tool layer on a passage once and see which game tools it calls (cloud or agent narrator only).", object(json!({ "chat_id": { "type": "string" }, "passage": { "type": "string" } }), &["chat_id", "passage"])),
            tool("job_stop", "Stop a running job: a story turn (its job id from story_turn) or a download.", object(json!({ "job_id": { "type": "string" } }), &["job_id"])),
            tool("runtime_get", "The engine settings: where each stage runs, cloud switches and models, recognition variant, narrator provider (empty or 'agent'), Google voice. Keys are never shown.", nothing()),
            tool("runtime_set", "Change engine settings: settings is an object with only the fields to change, as runtime_get names them (narratorProvider 'agent' makes you the game's narrator; asrEngine 'parakeet' or 'crisperwhisper' and asrModel pick recognition; cloudTtsProvider 'google' voices through Gemini).", object(json!({ "settings": { "type": "object" } }), &["settings"])),
            tool("models_status", "What the game downloads on first run: each component, whether it is on disk, its size and the selected recognition variant.", nothing()),
            tool("models_download", "Download components by id (from models_status); no ids = everything required that is missing. Runs in the background.", object(json!({ "ids": { "type": "array", "items": { "type": "string" } } }), &[])),
            tool("google_key_set", "Store or clear (key null) the Google AI Studio key for Gemini voices.", object(json!({ "key": { "type": ["string", "null"] } }), &[])),
            tool("image_worker", "The picture engine: action start or status.", object(json!({ "action": { "type": "string" } }), &["action"])),
            tool("voices_list", "Local voices (reference clips) the voice engine reads with.", nothing()),
            tool("voice_upload", "Add a local voice from an audio file on this computer.", object(json!({ "path": { "type": "string" }, "name": { "type": "string" } }), &["path"])),
            tool("cloud_voices", "Cloud voices of a TTS model with gender, age and Russian support; no model = the models in the catalog.", object(json!({ "model": { "type": "string" }, "russian": { "type": "boolean" }, "gender": { "type": "string" } }), &[])),
            tool("cloud_models", "Live OpenRouter models for a kind: text, image, tts, asr.", object(json!({ "kind": { "type": "string" } }), &["kind"])),
            tool("tts_speak", "Voice a text with a voice and get the clip's URL; ui_play plays it in the window.", object(json!({ "text": { "type": "string" }, "voice": { "type": "string" }, "message_id": { "type": "string" } }), &["text"])),
            tool("asr_transcribe", "Recognise speech in a 16 kHz mono WAV file on this computer with the selected recogniser.", object(json!({ "path": { "type": "string" } }), &["path"])),
            tool("image_create", "Draw a frame: prompt in English, chat_id and message_id attach it to a message and keep the scene's continuity; references are attachments. A graphics-card job behind any running turn.", object(json!({ "prompt": { "type": "string" }, "chat_id": { "type": "string" }, "message_id": { "type": "string" }, "settings": { "type": "object" }, "mode": { "type": "string" }, "aspect": { "type": "string" }, "seed": { "type": "integer" }, "references": { "type": "array", "items": { "type": "object" } } }), &["prompt"])),
            tool("story_turn", "Play a turn as the player: input is what the player does or says. The game's narrator (card, cloud or you, if you are the narrator) writes it, the engine applies the mechanics, the frame and the voice follow. Returns the job id; game_wait follows it. The window shows the result and plays the voice.", object(json!({ "chat_id": { "type": "string" }, "input": { "type": "string" } }), &["chat_id", "input"])),
            tool("gm_narrate", "As the game master, write the narrator's message into a story yourself: it is saved as the narrator's turn, voiced in the window when voice is true (voices by the story's settings), and image_prompt (English) is stored as its frame request and drawn when draw is true. Voice and frame run after the answer; a failure of either shows in activity_list.", object(json!({ "chat_id": { "type": "string" }, "text": { "type": "string" }, "voice": { "type": "boolean" }, "image_prompt": { "type": "string" }, "draw": { "type": "boolean" } }), &["chat_id", "text"])),
            tool("gm_apply", "As the game master, declare a turn's mechanics and let the engine compute them: update has the shape of the engine's schema (attacks, checks, hpDeltas, items, spawns, offerQuests, completeQuests, failQuests, xp); actors are named as in the story ('hero' or a name). Returns the events.", object(json!({ "chat_id": { "type": "string" }, "update": { "type": "object" } }), &["chat_id", "update"])),
            tool("gm_engine_schema", "The JSON schema of gm_apply's update.", nothing()),
            tool("gm_requests_wait", "When the player made you the game's narrator (runtime_set narratorProvider 'agent'), every narrator request of a turn waits here: the chat messages, the tools you may call and the JSON schema an answer must match. Returns the waiting requests as soon as there is one, or after seconds (30 by default, at most 55). Answer each with gm_request_answer; keep calling while the player plays.", object(json!({ "seconds": { "type": "integer" } }), &[])),
            tool("gm_request_answer", "Answer a narrator request: answer is plain text (prose), a JSON object matching its answer_schema, or {\"tool_calls\":[{\"name\":...,\"arguments\":{...}}]} to call the offered tools.", object(json!({ "request_id": { "type": "string" }, "answer": {} }), &["request_id", "answer"])),
            tool("hub_status", "Studio Hub in this game: the statistics choice, the feed source and the notices that fit now.", object(json!({ "lang": { "type": "string" } }), &[])),
            tool("hub_telemetry_set", "Turn the anonymous statistics on or off.", object(json!({ "enabled": { "type": "boolean" } }), &["enabled"])),
            tool("activity_list", "The Activity journal: what agents did in the game.", nothing()),
            tool("setup_update_plan", "The update screen's plan of this release: what is updated (old → new, size), what is new and what is no longer needed and removed (space freed), each with its reason code, and whether the window shows it at start.", nothing()),
            tool("setup_update_apply", "The update screen's button: download what is updated and new, then remove what former releases left; with nothing to download it removes them at once. The plan counts as shown. Returns the download job (if any) and the new plan.", nothing()),
            tool("setup_update_later", "«Later» on the update screen: this release's plan is not shown at start again; «What to download» keeps it.", nothing()),
            tool("diagnostics", "The game's diagnostics as Support shows them: version, system, cards and the builds the game runs on, memory, every component and whether it is on disk, what the voice server's libraries miss, and the latest log lines; as text to paste into an issue and as data.", nothing()),
            tool("diagnostics_report", "Pack a bug report into the data folder (reports/*.zip: the diagnostics, the activity journal and the log, keys masked) and answer its path.", nothing()),
            tool("diagnostics_report_reveal", "Show the folder of a report from diagnostics_report in the computer's file manager.", object(json!({ "path": { "type": "string" } }), &["path"])),
            tool("diagnostics_logs_reveal", "Show the folder with the game's and its helpers' logs in the computer's file manager.", nothing()),
            tool("ui_read_page", "What is on the game's window now: every visible control, one line each, with a ref the other ui_ tools take.", nothing()),
            tool("ui_screenshot", "A picture of the game's window as it is now.", object(json!({ "max_width": { "type": "integer" } }), &[])),
            tool("ui_click", "Click a control by ref (from ui_read_page) or by its visible text.", object(json!({ "ref": { "type": "string" }, "text": { "type": "string" } }), &[])),
            tool("ui_type", "Type into a field by ref or label; submit presses Enter.", object(json!({ "ref": { "type": "string" }, "text": { "type": "string" }, "value": { "type": "string" }, "submit": { "type": "boolean" } }), &["value"])),
            tool("ui_press_key", "Press a key or a combination (Enter, Escape, Ctrl+Enter) in the window.", object(json!({ "key": { "type": "string" } }), &["key"])),
            tool("ui_scroll", "Scroll the window, or to a control by ref or text.", object(json!({ "direction": { "type": "string" }, "amount": { "type": "integer" }, "ref": { "type": "string" }, "text": { "type": "string" } }), &[])),
            tool("ui_select", "Pick a value in a drop-down list by ref or label.", object(json!({ "ref": { "type": "string" }, "text": { "type": "string" }, "value": { "type": "string" } }), &["value"])),
            tool("ui_console", "The window's recent errors and warnings.", nothing()),
            tool("ui_navigate", "Open a part of the window: to 'chat' with chat_id opens a story, 'book' or 'feed' switches the story view, a side panel (chats, characters, achievements, story, images, voice, engines, textModel, setup, news, activity, localData, support), the book, or the new-story dialog (new).", object(json!({ "to": { "type": "string" }, "chat_id": { "type": "string" } }), &["to"])),
            tool("ui_play", "Play an audio clip (a URL from tts_speak or a turn) in the window.", object(json!({ "url": { "type": "string" } }), &["url"])),
        ];
        list.extend(gm_tools());
        list
    })
}

/// Тул в маршрут.
fn route(name: &str, args: &Value) -> Result<Call, String> {
    let chat = || text(args, "chat_id").map(|id| segment(&id));
    Ok(match name {
        "game_status" => return composite("status"),
        "game_release" => get("/api/release".into())?,
        "game_where" => get("/api/where".into())?,
        "hardware" => get("/api/hw".into())?,
        "game_wait" => return composite("wait"),
        "chats_list" => get("/api/chats".into())?,
        "chat_create" => send(Method::POST, "/api/chats".into(), args.clone())?,
        "chat_get" => get(format!("/api/chats/{}", chat()?))?,
        "chat_update" => send(Method::PATCH, format!("/api/chats/{}", chat()?), body_without(args, &["chat_id"]))?,
        "chat_delete" => send(Method::DELETE, format!("/api/chats/{}", chat()?), json!({}))?,
        "chat_events" => get(format!("/api/chats/{}/events", chat()?))?,
        "items_list" => get(format!("/api/chats/{}/items", chat()?))?,
        "quests_list" => get(format!("/api/chats/{}/quests", chat()?))?,
        "achievements_list" => get("/api/achievements".into())?,
        "quest_update" => send(Method::PATCH, format!("/api/chats/{}/quests/{}", chat()?, segment(&text(args, "quest_id")?)), json!({ "status": text(args, "status")? }))?,
        "item_equip" => send(Method::PATCH, format!("/api/chats/{}/items/{}", chat()?, segment(&text(args, "item_id")?)), json!({ "equipped": args.get("equipped").and_then(Value::as_bool).unwrap_or(true) }))?,
        "item_image" => send(Method::POST, format!("/api/chats/{}/items/{}/image", chat()?, segment(&text(args, "item_id")?)), json!({}))?,
        "message_delete" => {
            let after = args.get("include_after").and_then(Value::as_bool).unwrap_or(true);
            send(Method::DELETE, format!("/api/chats/{}/messages/{}?include_after={after}", chat()?, segment(&text(args, "message_id")?)), json!({}))?
        }
        "message_edit" => send(Method::PATCH, format!("/api/chats/{}/messages/{}", chat()?, segment(&text(args, "message_id")?)), json!({ "content": text(args, "content")? }))?,
        "characters_list" => get(format!("/api/chats/{}/characters", chat()?))?,
        "character_create" => send(Method::POST, format!("/api/chats/{}/characters", chat()?), body_without(args, &["chat_id"]))?,
        "character_update" => send(Method::PATCH, format!("/api/chats/{}/characters/{}", chat()?, segment(&text(args, "character_id")?)), body_without(args, &["chat_id", "character_id"]))?,
        "character_delete" => send(Method::DELETE, format!("/api/chats/{}/characters/{}", chat()?, segment(&text(args, "character_id")?)), json!({}))?,
        "actions_suggest" => send(Method::POST, "/api/actions".into(), args.clone())?,
        "story_suggest" => send(Method::POST, "/api/suggest".into(), args.clone())?,
        "story_setup" => send(Method::POST, "/api/story-setup".into(), args.clone())?,
        "image_style" => send(Method::POST, "/api/image-style".into(), args.clone())?,
        "character_generate" => send(Method::POST, "/api/character".into(), args.clone())?,
        "file_upload" => {
            let path = PathBuf::from(text(args, "path")?);
            Call { method: Method::POST, path: "/api/upload".into(), payload: Payload::Form { fields: Vec::new(), file: Some(("file".into(), path)) } }
        }
        "local_data" => get("/api/local-data".into())?,
        "local_data_wipe" => send(Method::DELETE, "/api/local-data".into(), json!({}))?,
        "settings_defaults" => get("/api/settings/defaults".into())?,
        "tools_try" => send(Method::POST, "/api/tools/try".into(), json!({ "chatId": text(args, "chat_id")?, "passage": text(args, "passage")? }))?,
        "job_stop" => send(Method::POST, format!("/api/jobs/{}/stop", segment(&text(args, "job_id")?)), json!({}))?,
        "runtime_get" => get("/api/runtime".into())?,
        "runtime_set" => send(Method::PUT, "/api/runtime".into(), args.get("settings").cloned().filter(Value::is_object).ok_or("'settings' must be an object")?)?,
        "models_status" => get("/api/setup/status".into())?,
        "models_download" => send(Method::POST, "/api/setup/download".into(), json!({ "ids": args.get("ids").cloned().unwrap_or_else(|| json!([])) }))?,
        "google_key_set" => send(Method::PUT, "/api/google/key".into(), json!({ "key": args.get("key").cloned().unwrap_or(Value::Null) }))?,
        "image_worker" => send(Method::POST, "/api/image-worker".into(), json!({ "action": text(args, "action")? }))?,
        "voices_list" => get("/api/voices".into())?,
        "voice_upload" => {
            let path = PathBuf::from(text(args, "path")?);
            let name = args.get("name").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default());
            Call { method: Method::POST, path: "/api/tts-voice".into(), payload: Payload::Form { fields: vec![("name".into(), name)], file: Some(("file".into(), path)) } }
        }
        "cloud_voices" => get(format!(
            "/api/cloud/voices{}",
            query(&[
                ("model", args.get("model").and_then(Value::as_str).map(str::to_string)),
                ("russian", args.get("russian").and_then(Value::as_bool).map(|value| value.to_string())),
                ("gender", args.get("gender").and_then(Value::as_str).map(str::to_string)),
            ])
        ))?,
        "cloud_models" => get(format!("/api/cloud/models?kind={}", segment(&text(args, "kind")?)))?,
        "tts_speak" => send(Method::POST, "/api/tts".into(), json!({ "text": text(args, "text")?, "voice": args.get("voice").cloned().unwrap_or(json!("")), "messageId": args.get("message_id").cloned().unwrap_or(Value::Null) }))?,
        "asr_transcribe" => Call { method: Method::POST, path: "/api/asr".into(), payload: Payload::File(PathBuf::from(text(args, "path")?)) },
        "image_create" => {
            let mut body = json!({ "prompt": text(args, "prompt")? });
            for (from, to) in [("chat_id", "chatId"), ("message_id", "messageId"), ("settings", "settings"), ("mode", "mode"), ("aspect", "aspect"), ("seed", "seed"), ("references", "references")] {
                if let Some(value) = args.get(from).filter(|value| !value.is_null()) {
                    body[to] = value.clone();
                }
            }
            send(Method::POST, "/api/images".into(), body)?
        }
        "story_turn" => return composite("turn"),
        "gm_narrate" => send(Method::POST, format!("/api/chats/{}/narrate", chat()?), body_without(args, &["chat_id"]))?,
        "gm_apply" => send(Method::POST, format!("/api/chats/{}/apply", chat()?), json!({ "update": args.get("update").cloned().filter(Value::is_object).ok_or("'update' must be an object")? }))?,
        "gm_engine_schema" => return composite("engine_schema"),
        "gm_requests_wait" => return composite("questions"),
        "gm_request_answer" => return composite("answer"),
        "hub_status" => get(format!("/v1/hub/state{}", query(&[("lang", args.get("lang").and_then(Value::as_str).map(str::to_string))])))?,
        "hub_telemetry_set" => send(Method::POST, "/v1/hub/telemetry".into(), json!({ "enabled": args.get("enabled").and_then(Value::as_bool).ok_or("'enabled' is required")?, "acknowledge": true }))?,
        "activity_list" => get("/api/activity".into())?,
        "setup_update_plan" => get("/api/setup/update".into())?,
        "setup_update_apply" => send(Method::POST, "/api/setup/update/apply".into(), json!({}))?,
        "setup_update_later" => send(Method::POST, "/api/setup/update/seen".into(), json!({}))?,
        "diagnostics" => get("/api/diagnostics".into())?,
        "diagnostics_report" => send(Method::POST, "/api/diagnostics/report".into(), json!({}))?,
        "diagnostics_report_reveal" => send(Method::POST, "/api/diagnostics/report/reveal".into(), json!({ "path": text(args, "path")? }))?,
        "diagnostics_logs_reveal" => send(Method::POST, "/api/diagnostics/logs/reveal".into(), json!({}))?,
        "ui_read_page" => return window("read_page", args, 15),
        "ui_screenshot" => return window("screenshot", args, 30),
        "ui_click" => return window("click", args, 15),
        "ui_type" => return window("type", args, 15),
        "ui_press_key" => return window("press_key", args, 15),
        "ui_scroll" => return window("scroll", args, 15),
        "ui_select" => return window("select", args, 15),
        "ui_console" => return window("console", args, 15),
        "ui_navigate" => return window("navigate", args, 15),
        "ui_play" => return window("play", args, 15),
        other => match other.strip_prefix("gm_") {
            Some(game) if crate::game_tools::all().iter().any(|tool| tool.name == game) => {
                send(Method::POST, format!("/api/chats/{}/gm/{}", chat()?, segment(game)), body_without(args, &["chat_id"]))?
            }
            _ => return Err(format!("Unknown tool: {other}; tools/list names them all.")),
        },
    })
}

/// Подсказки MCP по тулу: читает ли он, стирает ли, идёт ли в сеть.
fn annotations(name: &str) -> Value {
    const READS: &[&str] = &["_list", "_get", "_status", "_where", "_events", "_defaults", "_schema", "_open", "_sheet", "_recent", "_recall", "_search", "_page", "_screenshot", "_console", "_release"];
    const CHANGES: &[&str] = &["create", "update", "delete", "wipe", "set", "upload", "download", "speak", "transcribe", "turn", "narrate", "apply", "grant", "complete", "fail", "add", "assign", "equip", "edit", "stop", "click", "type", "press", "select", "navigate", "play", "scroll", "image", "frame", "check", "answer", "worker", "suggest", "setup", "style", "generate", "try"];
    let changes = CHANGES.iter().any(|verb| name.split('_').any(|word| word == *verb));
    let read_only = matches!(name, "game_status" | "hardware" | "game_wait" | "gm_requests_wait" | "local_data" | "cloud_voices" | "cloud_models" | "runtime_get" | "gm_voices_list" | "gm_icons_search" | "gm_inventory_list" | "gm_quests_open" | "gm_achievements_list")
        || (!changes && READS.iter().any(|part| name.ends_with(part)));
    let destructive = name.ends_with("_delete") || name == "local_data_wipe" || name == "message_edit" || name == "gm_quest_fail";
    let open_world = matches!(name, "cloud_models" | "models_download" | "game_release" | "hub_status");
    json!({ "title": name.replace('_', " "), "readOnlyHint": read_only, "destructiveHint": destructive, "idempotentHint": read_only, "openWorldHint": open_world })
}

// ---------------------------------------------------------------- маршруты внутри процесса

async fn call_route(call: Call) -> Result<(StatusCode, String), String> {
    let api = API.get().ok_or("the game is still starting")?.clone();
    let builder = Request::builder().method(call.method).uri(&call.path).header(header::HOST, "127.0.0.1");
    let request = match call.payload {
        Payload::None | Payload::Window { .. } | Payload::Composite(_) => builder.body(Body::empty()),
        Payload::Json(body) => builder.header(header::CONTENT_TYPE, "application/json").body(Body::from(body.to_string())),
        Payload::File(path) => {
            let bytes = tokio::fs::read(&path).await.map_err(|error| format!("read {}: {error}", path.display()))?;
            builder.header(header::CONTENT_TYPE, "audio/wav").body(Body::from(bytes))
        }
        Payload::Form { fields, file } => {
            let boundary = format!("du-mcp-{}", uuid::Uuid::new_v4().simple());
            let mut body: Vec<u8> = Vec::new();
            for (name, value) in fields {
                body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
            }
            if let Some((name, path)) = file {
                let bytes = tokio::fs::read(&path).await.map_err(|error| format!("read {}: {error}", path.display()))?;
                let file_name = path.file_name().map(|name| name.to_string_lossy().replace('"', "'")).unwrap_or_else(|| "file".into());
                let mime = mime_guess::from_path(&path).first_or_octet_stream();
                body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\nContent-Type: {mime}\r\n\r\n").as_bytes());
                body.extend_from_slice(&bytes);
                body.extend_from_slice(b"\r\n");
            }
            body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
            builder.header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}")).body(Body::from(body))
        }
    }
    .map_err(|error| error.to_string())?;
    let response = api.oneshot(request).await.map_err(|error| error.to_string())?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024 * 1024).await.map_err(|error| error.to_string())?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if text.trim().is_empty() {
        text = if status.is_success() { "Done.".into() } else { status.to_string() };
    }
    Ok((status, text))
}

/// POST JSON внутри процесса — для серверных шагов, которые идут тем же путём, что окно.
pub(crate) async fn call_json(path: &str, body: Value) -> Result<Value, String> {
    let (status, text) = call_route(Call { method: Method::POST, path: path.into(), payload: Payload::Json(body) }).await?;
    if !status.is_success() {
        return Err(format!("{status}: {text}"));
    }
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

async fn fetch(path: &str) -> Value {
    match call_route(Call { method: Method::GET, path: path.into(), payload: Payload::None }).await {
        Ok((_, text)) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        Err(problem) => json!({ "error": problem }),
    }
}

// ---------------------------------------------------------------- окно

struct Bridge {
    commands: tokio::sync::broadcast::Sender<String>,
    pending: Mutex<HashMap<String, (u64, tokio::sync::oneshot::Sender<Result<Value, String>>)>>,
    /// Открытые окна; последним — то, к которому человек обратился последним.
    windows: Mutex<Vec<u64>>,
    sequence: AtomicU64,
}

fn bridge() -> &'static Bridge {
    static BRIDGE: OnceLock<Bridge> = OnceLock::new();
    BRIDGE.get_or_init(|| Bridge { commands: tokio::sync::broadcast::channel(256).0, pending: Mutex::new(HashMap::new()), windows: Mutex::new(Vec::new()), sequence: AtomicU64::new(0) })
}

fn open_windows() -> std::sync::MutexGuard<'static, Vec<u64>> {
    bridge().windows.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct Listening(u64);

/// What a command gets when the window it went to closes before answering.
const WINDOW_CLOSED: &str = "The game's window closed before it answered. Call the tool again: it goes to another open window, or says that none is open.";

impl Drop for Listening {
    fn drop(&mut self) {
        open_windows().retain(|window| *window != self.0);
        // a window that went away answers nothing: its commands fail now, not at their timeout
        let mut pending = bridge().pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gone: Vec<String> = pending.iter().filter(|(_, (window, _))| *window == self.0).map(|(id, _)| id.clone()).collect();
        for id in gone {
            if let Some((_, sender)) = pending.remove(&id) {
                let _ = sender.send(Err(WINDOW_CLOSED.into()));
            }
        }
    }
}

use crate::guard::{foreign_origin, local_origin};

/// Поток команд, которые исполняет окно. Первое сообщение называет окно.
pub async fn window_events(headers: HeaderMap) -> Response {
    if !local_origin(&headers) {
        return foreign_origin();
    }
    let window = bridge().sequence.fetch_add(1, Ordering::Relaxed);
    open_windows().push(window);
    let receiver = bridge().commands.subscribe();
    let hello = futures_util::stream::once(async move { Ok::<Event, Infallible>(Event::default().data(json!({ "window": window }).to_string())) });
    let commands = futures_util::stream::unfold((receiver, Listening(window)), |(mut receiver, listening)| async move {
        loop {
            match receiver.recv().await {
                Ok(command) => return Some((Ok(Event::default().data(command)), (receiver, listening))),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(futures_util::StreamExt::chain(hello, commands)).keep_alive(KeepAlive::default()).into_response()
}

#[derive(serde::Deserialize)]
pub struct WindowAnswer {
    id: String,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error: Option<String>,
}

pub async fn window_result(headers: HeaderMap, Json(answer): Json<WindowAnswer>) -> StatusCode {
    if !local_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }
    let waiting = bridge().pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&answer.id);
    match waiting {
        Some((_, sender)) => {
            let _ = sender.send(match answer.error {
                Some(error) => Err(error),
                None => Ok(answer.result),
            });
            StatusCode::NO_CONTENT
        }
        None => StatusCode::NOT_FOUND,
    }
}

#[derive(serde::Deserialize)]
pub struct WindowFocus {
    window: u64,
}

pub async fn window_focus(headers: HeaderMap, Json(focus): Json<WindowFocus>) -> StatusCode {
    if !local_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }
    let mut windows = open_windows();
    let Some(place) = windows.iter().position(|open| *open == focus.window) else { return StatusCode::NOT_FOUND };
    windows.remove(place);
    windows.push(focus.window);
    StatusCode::NO_CONTENT
}

/// Сказать окнам, что за их спиной что-то поменялось: они перечитают истории и настройки.
pub fn announce(what: &str) {
    tell_windows(json!({ "changed": what }));
}

/// Событие для всех открытых окон.
pub fn tell_windows(event: Value) {
    let _ = bridge().commands.send(event.to_string());
}

async fn ask_window(command: &str, args: Value, seconds: u64) -> Result<Value, String> {
    let Some(window) = open_windows().last().copied() else {
        return Err("The game's window is not open. Open Dungeon Ultimate and call the tool again; everything else works without it.".into());
    };
    let id = format!("w{}", bridge().sequence.fetch_add(1, Ordering::Relaxed));
    let (sender, receiver) = tokio::sync::oneshot::channel();
    bridge().pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(id.clone(), (window, sender));
    // the window may have closed after it was chosen: its Listening went before this entry was there to fail
    if !open_windows().contains(&window) {
        bridge().pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&id);
        return Err(WINDOW_CLOSED.into());
    }
    let _ = bridge().commands.send(json!({ "id": id, "window": window, "command": command, "args": args }).to_string());
    match tokio::time::timeout(Duration::from_secs(seconds), receiver).await {
        Ok(Ok(answer)) => answer,
        _ => {
            bridge().pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&id);
            Err(format!("The window did not answer '{command}' within {seconds} s."))
        }
    }
}

// ---------------------------------------------------------------- агент как рассказчик

struct Question {
    question: Value,
    answer: tokio::sync::oneshot::Sender<Value>,
}

struct Agent {
    questions: Mutex<Vec<Question>>,
    last_call: Mutex<Option<(Instant, String)>>,
    calls: AtomicU64,
}

fn agent() -> &'static Agent {
    static AGENT: OnceLock<Agent> = OnceLock::new();
    AGENT.get_or_init(|| Agent { questions: Mutex::new(Vec::new()), last_call: Mutex::new(None), calls: AtomicU64::new(0) })
}

fn questions() -> std::sync::MutexGuard<'static, Vec<Question>> {
    agent().questions.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Агент, звавший сервер не раньше этого срока, считается подключённым.
const AGENT_PRESENT: Duration = Duration::from_secs(600);
/// Сколько вопрос ждёт ответа агента.
pub const AGENT_ANSWER: Duration = Duration::from_secs(900);

fn seen(what: &str) {
    *agent().last_call.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((Instant::now(), what.to_string()));
    agent().calls.fetch_add(1, Ordering::Relaxed);
}

pub fn agent_present() -> bool {
    agent().last_call.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_ref().is_some_and(|(at, _)| at.elapsed() < AGENT_PRESENT)
}

fn open_questions() -> Vec<Value> {
    let mut waiting = questions();
    waiting.retain(|question| !question.answer.is_closed());
    waiting.iter().map(|question| question.question.clone()).collect()
}

/// Задать агенту вопрос рассказчика и дождаться ответа.
async fn ask_agent(mut question: Value) -> Result<Value, String> {
    if !agent_present() {
        return Err("рассказчик — агент, но агент не подключён к MCP игры".into());
    }
    let id = format!("q{}", bridge().sequence.fetch_add(1, Ordering::Relaxed));
    question["id"] = json!(id);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    questions().push(Question { question, answer: sender });
    let deadline = tokio::time::Instant::now() + AGENT_ANSWER;
    let mut receiver = receiver;
    let outcome = loop {
        match tokio::time::timeout(Duration::from_millis(500), &mut receiver).await {
            Ok(Ok(answer)) => break Ok(answer),
            Ok(Err(_)) => break Err("вопрос агенту снят".to_string()),
            Err(_) if crate::narrator::waiting_job_aborted() => break Err("вопрос агенту снят: ход прерван".to_string()),
            Err(_) if tokio::time::Instant::now() >= deadline => {
                break Err(format!("агент не ответил за {} минут", AGENT_ANSWER.as_secs() / 60))
            }
            Err(_) => {}
        }
    };
    if outcome.is_err() {
        questions().retain(|question| question.question["id"] != id.as_str());
    }
    outcome
}

async fn wait_for_questions(args: &Value) -> Value {
    let seconds = args.get("seconds").and_then(Value::as_u64).unwrap_or(WAIT_DEFAULT).clamp(1, WAIT_LONGEST);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        let waiting = open_questions();
        if !waiting.is_empty() {
            return json!({ "requests": waiting });
        }
        if tokio::time::Instant::now() >= deadline {
            return json!({ "requests": [], "note": "no request yet; call gm_requests_wait again to keep listening" });
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

fn answer_question(args: &Value) -> Result<String, String> {
    let id = text(args, "request_id")?;
    let answer = args.get("answer").cloned().filter(|value| !value.is_null()).ok_or("'answer' is required")?;
    let found = {
        let mut waiting = questions();
        waiting.iter().position(|question| question.question["id"] == id.as_str()).map(|at| waiting.remove(at))
    };
    let Some(question) = found else {
        return Err(format!("No request {id} is waiting; gm_requests_wait lists the open ones."));
    };
    question.answer.send(answer).map_err(|_| format!("Request {id} was given up by the game before the answer came."))?;
    Ok("Answered; the game goes on with it.".into())
}

/// OpenAI-совместимый вход для клиента рассказчика, когда рассказчик — агент: запрос
/// становится вопросом агенту, ответ — ответом модели (текст, JSON по схеме или вызовы
/// инструментов), а поток — одним куском текста.
pub async fn agent_completions(headers: HeaderMap, Json(request): Json<Value>) -> Response {
    if !local_origin(&headers) {
        return foreign_origin();
    }
    let stream = request.get("stream").and_then(Value::as_bool).unwrap_or(false);
    let schema = request
        .pointer("/response_format/json_schema/schema")
        .or_else(|| request.pointer("/response_format/schema"))
        .cloned();
    let tools: Vec<Value> = request
        .get("tools")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(|tool| tool.get("function").cloned()).collect())
        .unwrap_or_default();
    let kind = if schema.is_some() { "json" } else if !tools.is_empty() { "tools" } else if stream { "prose" } else { "text" };
    let question = json!({
        "kind": kind,
        "messages": request.get("messages").cloned().unwrap_or_else(|| json!([])),
        "answer_schema": schema,
        "tools": tools,
        "max_tokens": request.get("max_tokens").cloned().unwrap_or(Value::Null),
    });
    let answer = match ask_agent(question).await {
        Ok(answer) => answer,
        Err(error) => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": { "message": error } }))).into_response(),
    };
    let (content, calls) = match &answer {
        Value::String(text) => (text.clone(), Vec::new()),
        Value::Object(map) if map.contains_key("tool_calls") => {
            let calls: Vec<Value> = map["tool_calls"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .map(|(index, call)| {
                    let name = call.get("name").or_else(|| call.pointer("/function/name")).cloned().unwrap_or(Value::Null);
                    let arguments = call.get("arguments").or_else(|| call.pointer("/function/arguments")).cloned().unwrap_or_else(|| json!({}));
                    let arguments = if arguments.is_string() { arguments } else { json!(arguments.to_string()) };
                    json!({ "id": format!("call_{index}"), "type": "function", "function": { "name": name, "arguments": arguments } })
                })
                .collect();
            (map.get("content").and_then(Value::as_str).unwrap_or_default().to_string(), calls)
        }
        other => (other.to_string(), Vec::new()),
    };
    if stream {
        let chunk = json!({ "choices": [{ "index": 0, "delta": { "content": content } }] });
        let body = format!("data: {chunk}\n\ndata: [DONE]\n\n");
        return ([(header::CONTENT_TYPE, "text/event-stream")], body).into_response();
    }
    let mut message = json!({ "role": "assistant", "content": content });
    let finish = if calls.is_empty() { "stop" } else { "tool_calls" };
    if !calls.is_empty() {
        message["tool_calls"] = Value::Array(calls);
    }
    Json(json!({
        "id": "agent",
        "object": "chat.completion",
        "choices": [{ "index": 0, "message": message, "finish_reason": finish }],
    }))
    .into_response()
}

/// Подключены ли агент и окно — для окна.
pub async fn status() -> Json<Value> {
    let last = agent().last_call.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    Json(json!({
        "window_open": !open_windows().is_empty(),
        "agent_connected": agent_present(),
        "agent_last_call": last.as_ref().map(|(_, what)| what.clone()),
        "agent_seconds_ago": last.as_ref().map(|(at, _)| at.elapsed().as_secs()),
        "agent_calls": agent().calls.load(Ordering::Relaxed),
        "requests_waiting": open_questions().len(),
    }))
}

// ---------------------------------------------------------------- ход истории

struct Turn {
    started: Instant,
    events: Vec<Value>,
    prose: String,
    clips: Vec<String>,
    done: Option<Value>,
    error: Option<String>,
}

/// Сколько законченных ходов помнит `game_wait`: старые уходят, когда начинается новый.
const KEEP_TURNS: usize = 16;

fn remember_turn(id: String) {
    let mut all = turns();
    all.insert(id, Turn { started: Instant::now(), events: Vec::new(), prose: String::new(), clips: Vec::new(), done: None, error: None });
    let mut finished: Vec<(Instant, String)> = all
        .iter()
        .filter(|(_, turn)| turn.done.is_some() || turn.error.is_some())
        .map(|(id, turn)| (turn.started, id.clone()))
        .collect();
    finished.sort();
    let surplus = finished.len().saturating_sub(KEEP_TURNS);
    for (_, id) in finished.into_iter().take(surplus) {
        all.remove(&id);
    }
}

fn turns() -> std::sync::MutexGuard<'static, HashMap<String, Turn>> {
    static TURNS: OnceLock<Mutex<HashMap<String, Turn>>> = OnceLock::new();
    TURNS.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Начать ход тем же маршрутом, что окно, и читать его поток в фоне: агент получает номер задачи
/// сразу, `game_wait` отдаёт ход по мере письма, окно получает клипы озвучки и в конце
/// перечитывает историю.
async fn start_turn(args: &Value) -> Result<Value, String> {
    let chat_id = text(args, "chat_id")?;
    let input = text(args, "input")?;
    let api = API.get().ok_or("the game is still starting")?.clone();
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/story")
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "chatId": chat_id, "input": input }).to_string()))
        .map_err(|error| error.to_string())?;
    let response = api.oneshot(request).await.map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap_or_default();
        return Err(String::from_utf8_lossy(&bytes).into_owned());
    }
    let mut stream = response.into_body().into_data_stream();
    let (job_sender, job_receiver) = tokio::sync::oneshot::channel::<String>();
    announce("chat");
    tokio::spawn(async move {
        use futures_util::StreamExt;
        let mut buffer = String::new();
        let mut job: Option<String> = None;
        let mut job_sender = Some(job_sender);
        let mut name = String::new();
        while let Some(chunk) = stream.next().await {
            let Ok(chunk): Result<axum::body::Bytes, _> = chunk else { break };
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(end) = buffer.find('\n') {
                let line = buffer[..end].trim_end_matches('\r').to_string();
                buffer.drain(..=end);
                if let Some(event) = line.strip_prefix("event:") {
                    name = event.trim().to_string();
                    continue;
                }
                let Some(data) = line.strip_prefix("data:") else { continue };
                let data: Value = serde_json::from_str(data.trim()).unwrap_or(Value::Null);
                if name == "job" {
                    let id = data["job"].as_str().unwrap_or_default().to_string();
                    remember_turn(id.clone());
                    if let Some(sender) = job_sender.take() {
                        let _ = sender.send(id.clone());
                    }
                    job = Some(id);
                    continue;
                }
                let Some(id) = job.clone() else { continue };
                let mut all = turns();
                let Some(turn) = all.get_mut(&id) else { continue };
                match name.as_str() {
                    "delta" => turn.prose.push_str(data["text"].as_str().unwrap_or_default()),
                    "clip" => {
                        if let Some(url) = data["url"].as_str() {
                            turn.clips.push(url.to_string());
                            tell_windows(json!({ "play": url, "chat_id": chat_id }));
                        }
                    }
                    "done" => turn.done = Some(data.clone()),
                    "error" => turn.error = Some(data["error"].as_str().or(data["message"].as_str()).unwrap_or("ход сорвался").to_string()),
                    _ => turn.events.push(json!({ "event": name, "data": data })),
                }
            }
        }
        if let Some(id) = &job {
            if let Some(turn) = turns().get_mut(id) {
                if turn.done.is_none() && turn.error.is_none() {
                    turn.error = Some("поток хода оборвался без конца".into());
                }
            }
        }
        announce("chat");
    });
    let job = tokio::time::timeout(Duration::from_secs(30), job_receiver)
        .await
        .map_err(|_| "the turn did not start within 30 s".to_string())?
        .map_err(|_| "the turn ended before it started".to_string())?;
    Ok(json!({ "job_id": job, "note": "game_wait follows the turn" }))
}

async fn wait_turn(args: &Value) -> Result<Value, String> {
    let job = text(args, "job_id")?;
    let seconds = args.get("seconds").and_then(Value::as_u64).unwrap_or(WAIT_DEFAULT).clamp(1, WAIT_LONGEST);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        let state = {
            let all = turns();
            let turn = all.get(&job).ok_or_else(|| format!("No turn {job}: job_id is what story_turn returned."))?;
            let finished = turn.done.is_some() || turn.error.is_some();
            let stages: Vec<Value> = turn.events.iter().filter(|event| event["event"] == "stage").map(|event| event["data"].clone()).collect();
            (finished, json!({
                "done": finished,
                "stages": stages,
                "prose": turn.prose,
                "clips": turn.clips,
                "result": turn.done,
                "error": turn.error,
            }))
        };
        if state.0 || tokio::time::Instant::now() >= deadline {
            return Ok(state.1);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn status_summary() -> Value {
    let setup = fetch("/api/setup/status").await;
    let missing: Vec<Value> = setup["components"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|component| component["present"] == false && component["requirement"] == "required")
        .map(|component| json!(component["id"]))
        .collect();
    let runtime = fetch("/api/runtime").await;
    json!({
        "game": GAME,
        "version": env!("CARGO_PKG_VERSION"),
        "health": fetch("/api/health").await,
        "where": fetch("/api/where").await["stages"],
        "narrator_provider": runtime["runtime"]["narratorProvider"],
        "required_missing": missing,
        "connection": status().await.0,
    })
}

// ---------------------------------------------------------------- JSON-RPC

const MODERN: &[&str] = &["2026-07-28"];
const LEGACY: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];
const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const LIST_TTL_MS: u64 = 300_000;
const HEADER_MISMATCH: i64 = -32020;
const UNSUPPORTED_VERSION: i64 = -32022;

fn server_info() -> Value {
    json!({ "name": "dungeon-ultimate", "title": GAME, "version": env!("CARGO_PKG_VERSION") })
}

fn supported_versions() -> Vec<&'static str> {
    MODERN.iter().chain(LEGACY).copied().collect()
}

fn rpc(id: Value, mut result: Value) -> Response {
    if let Some(fields) = result.as_object_mut() {
        fields.entry("resultType").or_insert_with(|| "complete".into());
        let meta = fields.entry("_meta").or_insert_with(|| json!({}));
        meta["io.modelcontextprotocol/serverInfo"] = server_info();
    }
    Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })).into_response()
}

fn rpc_error(id: Value, code: i64, message: String) -> Response {
    rpc_failure(StatusCode::OK, id, code, message, None)
}

fn rpc_failure(status: StatusCode, id: Value, code: i64, message: String, data: Option<Value>) -> Response {
    let mut error = json!({ "code": code, "message": message });
    if let Some(data) = data {
        error["data"] = data;
    }
    (status, Json(json!({ "jsonrpc": "2.0", "id": id, "error": error }))).into_response()
}

fn cacheable(mut result: Value) -> Value {
    result["ttlMs"] = LIST_TTL_MS.into();
    result["cacheScope"] = "public".into();
    result
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(name)?.to_str().ok().map(str::to_string)
}

fn refused(headers: &HeaderMap, method: &str, params: &Value, version: &str) -> Option<(i64, String, Option<Value>)> {
    if !MODERN.contains(&version) {
        return Some((UNSUPPORTED_VERSION, "Unsupported protocol version".into(), Some(json!({ "supported": supported_versions(), "requested": version }))));
    }
    let mismatch = |what: String| Some((HEADER_MISMATCH, format!("Header mismatch: {what}"), None));
    match header_value(headers, "mcp-protocol-version") {
        Some(value) if value == version => {}
        Some(value) => return mismatch(format!("MCP-Protocol-Version header value '{value}' does not match body value '{version}'")),
        None => return mismatch("the MCP-Protocol-Version header is missing".into()),
    }
    match header_value(headers, "mcp-method") {
        Some(value) if value == method => {}
        Some(value) => return mismatch(format!("Mcp-Method header value '{value}' does not match body value '{method}'")),
        None => return mismatch("the Mcp-Method header is missing".into()),
    }
    let named = match method {
        "tools/call" | "prompts/get" => params.get("name"),
        "resources/read" => params.get("uri"),
        _ => return None,
    }
    .and_then(Value::as_str)
    .unwrap_or_default();
    match header_value(headers, "mcp-name") {
        Some(value) if value == named => None,
        Some(value) => mismatch(format!("Mcp-Name header value '{value}' does not match body value '{named}'")),
        None => mismatch("the Mcp-Name header is missing".into()),
    }
}

fn cut(mut text: String) -> String {
    if text.len() > LIMIT {
        let mut end = LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n... (cut: ask for one story with chat_get)");
    }
    text
}

fn tool_result(id: Value, text: String, structured: Option<Value>, error: bool) -> Response {
    let mut result = json!({ "content": [{ "type": "text", "text": cut(text) }], "isError": error });
    if let Some(structured) = structured.filter(|value| value.is_object()) {
        result["structuredContent"] = structured;
    }
    rpc(id, result)
}

fn tool_json(id: Value, value: Value) -> Response {
    let text = serde_json::to_string_pretty(&value).unwrap_or_default();
    let structured = (text.len() <= LIMIT).then_some(value);
    tool_result(id, text, structured, false)
}

/// Что агент видит при подключении.
const INSTRUCTIONS: &str = "You drive Dungeon Ultimate, a local AI role-playing game, on this computer. Every tool runs the same code as the window's buttons, the player sees what you do, and every change is written into the window's Activity journal. Start with game_status. Play as the player with story_turn and game_wait. To be the game master yourself: write the narrator's turns with gm_narrate, let the engine compute the mechanics with gm_apply (gm_engine_schema shows the shape) and use the gm_ tools (rolls, quests, characters, frames, achievements); draw frames with image_create, voice with tts_speak and ui_play. To be the game's language model: runtime_set {\"settings\":{\"narratorProvider\":\"agent\"}}, then answer every narrator request with gm_requests_wait and gm_request_answer while the player plays (prose as plain text, structured passes as JSON by answer_schema, tool calls as {\"tool_calls\":[...]}). The graphics card runs one heavy job at a time: frames and voices queue behind a running turn. Look ids up instead of guessing them: chats_list, chat_get, characters_list, models_status. ui_read_page and ui_screenshot see the window.";

pub async fn handle(headers: HeaderMap, body: axum::body::Bytes) -> Response {
    if !local_origin(&headers) {
        return foreign_origin();
    }
    let Ok(message) = serde_json::from_slice::<Value>(&body) else {
        return rpc_failure(StatusCode::BAD_REQUEST, Value::Null, -32700, "Parse error".into(), None);
    };
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return rpc_failure(StatusCode::BAD_REQUEST, Value::Null, -32600, "Invalid request: one JSON-RPC request or notification per POST".into(), None);
    };
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    if message.get("id").is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    if let Some(version) = params.get("_meta").and_then(|meta| meta.get(META_VERSION)).and_then(Value::as_str) {
        if let Some((code, text, data)) = refused(&headers, method, &params, version) {
            return rpc_failure(StatusCode::BAD_REQUEST, id, code, text, data);
        }
    }
    if method != "tools/call" {
        seen(method);
    }
    match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
            let protocol = LEGACY.iter().find(|version| **version == asked).copied().unwrap_or(LEGACY[0]);
            rpc(id, json!({
                "protocolVersion": protocol,
                "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
                "serverInfo": server_info(),
                "instructions": INSTRUCTIONS,
            }))
        }
        "server/discover" => rpc(id, cacheable(json!({ "supportedVersions": supported_versions(), "capabilities": { "tools": {}, "resources": {}, "prompts": {} }, "instructions": INSTRUCTIONS }))),
        "ping" => rpc(id, json!({})),
        "resources/list" => rpc(id, cacheable(json!({ "resources": [] }))),
        "resources/templates/list" => rpc(id, cacheable(json!({ "resourceTemplates": [] }))),
        "prompts/list" => rpc(id, cacheable(json!({ "prompts": [] }))),
        "tools/list" => {
            let list: Vec<Value> = tools().iter().map(|tool| json!({ "name": tool.name, "title": tool.name.replace('_', " "), "description": tool.description, "inputSchema": tool.schema, "annotations": annotations(tool.name) })).collect();
            rpc(id, cacheable(json!({ "tools": list })))
        }
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
            seen(&name);
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if !tools().iter().any(|tool| tool.name == name) {
                return rpc_error(id, -32602, format!("Unknown tool: {name}; tools/list names them all."));
            }
            let changes = annotations(&name)["readOnlyHint"] == false;
            let outcome = run_tool(&name, &args).await;
            let (text, structured, error) = match outcome {
                Ok((text, structured, image)) => {
                    if let Some(image) = image {
                        let content = json!([{ "type": "image", "data": image, "mimeType": "image/png" }, { "type": "text", "text": text }]);
                        return rpc(id, json!({ "content": content, "isError": false }));
                    }
                    (text, structured, false)
                }
                Err(problem) => (problem, None, true),
            };
            if changes && !name.starts_with("ui_") && name != "gm_request_answer" {
                crate::activity::record(&name, &args, !error, &text);
                announce(&name);
            }
            match structured {
                Some(value) if !error => tool_json(id, value),
                _ => tool_result(id, text, None, error),
            }
        }
        _ => rpc_failure(StatusCode::NOT_FOUND, id, -32601, format!("Method not found: {method}"), None),
    }
}

/// Исполнить тул: текст ответа, тот же ответ структурой и картинка (скриншот окна).
async fn run_tool(name: &str, args: &Value) -> Result<(String, Option<Value>, Option<String>), String> {
    let call = route(name, args)?;
    let json_answer = |value: Value| Ok((serde_json::to_string_pretty(&value).unwrap_or_default(), Some(value), None));
    match call.payload {
        Payload::Composite("status") => json_answer(status_summary().await),
        Payload::Composite("wait") => json_answer(wait_turn(args).await?),
        Payload::Composite("turn") => json_answer(start_turn(args).await?),
        Payload::Composite("engine_schema") => json_answer(crate::turn::engine_schema()),
        Payload::Composite("questions") => json_answer(wait_for_questions(args).await),
        Payload::Composite("answer") => answer_question(args).map(|text| (text, None, None)),
        Payload::Composite(other) => Err(format!("unknown composite {other}")),
        Payload::Window { command, args, seconds } => {
            let result = ask_window(command, args, seconds).await?;
            let image = result.get("image").and_then(Value::as_str).map(str::to_string);
            let text = result.get("text").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| if result.is_null() { "Done.".into() } else { serde_json::to_string_pretty(&result).unwrap_or_default() });
            Ok((text, None, image))
        }
        _ => {
            let (status, text) = call_route(call).await?;
            if !status.is_success() {
                let message = serde_json::from_str::<Value>(&text).ok().and_then(|value| value["error"].as_str().map(str::to_string)).unwrap_or(text);
                return Err(format!("{status}: {message}"));
            }
            match serde_json::from_str::<Value>(&text) {
                Ok(value) if value.is_object() || value.is_array() => {
                    let pretty = serde_json::to_string_pretty(&value).unwrap_or_default();
                    let structured = if value.is_object() { Some(value) } else { Some(json!({ "items": value })) };
                    Ok((pretty, structured, None))
                }
                _ => Ok((text, None, None)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_command_to_a_window_that_closed_fails_at_once() {
        // a command is sent only while some window listens: without a receiver it would fail as "not open"
        let _commands = bridge().commands.subscribe();
        let window = bridge().sequence.fetch_add(1, Ordering::Relaxed);
        open_windows().push(window);
        let listening = Listening(window);
        let asked = tokio::spawn(ask_window("read_page", json!({}), 300));
        while !bridge().pending.lock().unwrap().values().any(|(to, _)| *to == window) {
            tokio::task::yield_now().await;
        }
        drop(listening);
        let answer = tokio::time::timeout(Duration::from_secs(5), asked).await.expect("an answer before the timeout").unwrap();
        assert!(answer.unwrap_err().contains("closed"));
    }

    #[test]
    fn every_tool_has_a_unique_name_an_object_schema_and_a_route() {
        let mut names: Vec<&str> = tools().iter().map(|tool| tool.name).collect();
        let count = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), count, "two tools share a name");
        let sample = json!({
            "chat_id": "c", "quest_id": "q", "status": "done", "item_id": "i", "message_id": "m", "content": "x",
            "character_id": "h", "name": "n", "passage": "p", "field": "world", "path": "a.wav", "job_id": "j",
            "settings": {}, "action": "status", "kind": "text", "text": "t", "prompt": "p", "input": "i",
            "update": {}, "request_id": "q1", "answer": "a", "enabled": true, "value": "v", "key": "Enter", "to": "chats",
            "url": "/generated/a.wav",
        });
        for tool in tools() {
            assert_eq!(tool.schema["type"], "object", "{} has no object schema", tool.name);
            assert!(!tool.description.is_empty(), "{} has no description", tool.name);
            assert!(route(tool.name, &sample).is_ok(), "{} has no route", tool.name);
        }
    }

    #[test]
    fn every_api_route_of_the_game_has_a_tool() {
        // Маршруты API игры (crates/du-server/src/lib.rs) и тулы, которые их зовут.
        let covered = [
            "/api/health", "/api/release", "/api/chats", "/api/chats/c", "/api/chats/c/events", "/api/chats/c/items",
            "/api/chats/c/quests", "/api/achievements", "/api/chats/c/quests/q", "/api/chats/c/items/i",
            "/api/chats/c/items/i/image", "/api/image-worker", "/api/tts-voice", "/api/chats/c/messages/m",
            "/api/chats/c/characters", "/api/chats/c/characters/h", "/api/actions", "/api/suggest", "/api/story-setup",
            "/api/image-style", "/api/character", "/api/upload", "/api/local-data", "/api/settings/defaults", "/api/where",
            "/api/tools/try", "/api/jobs/j/stop", "/api/runtime", "/api/hw", "/api/setup/status", "/api/setup/download",
            "/api/asr", "/api/voices", "/api/cloud/voices", "/api/cloud/models", "/api/tts", "/api/images",
            "/api/setup/update", "/api/setup/update/apply", "/api/setup/update/seen",
            "/api/diagnostics", "/api/diagnostics/report", "/api/diagnostics/report/reveal", "/api/diagnostics/logs/reveal",
        ];
        let sample = json!({
            "chat_id": "c", "quest_id": "q", "status": "done", "item_id": "i", "message_id": "m", "content": "x",
            "character_id": "h", "name": "n", "passage": "p", "field": "world", "path": "a.wav", "job_id": "j",
            "settings": {}, "action": "status", "kind": "text", "text": "t", "prompt": "p",
        });
        let mut reached: Vec<String> = tools()
            .iter()
            .filter_map(|tool| route(tool.name, &sample).ok())
            .map(|call| call.path.split('?').next().unwrap_or_default().to_string())
            .collect();
        reached.push("/api/health".into());
        for path in covered {
            assert!(reached.iter().any(|reached| reached == path), "no tool reaches {path}");
        }
    }

    #[test]
    fn an_agent_answer_becomes_what_the_narrator_client_reads() {
        assert_eq!(segment("a b/ц"), "a%20b%2F%D1%86");
        assert_eq!(query(&[("model", Some("x/y".into())), ("gender", None)]), "?model=x%2Fy");
    }
}
