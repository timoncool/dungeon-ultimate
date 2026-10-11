//! Агент — ведущий партии: инструменты рассказчика, ход рассказчика и механика хода тем же
//! кодом, что у хода истории. Окно перечитывает историю и проигрывает озвучку.

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use du_core::{ImageRequest, StoryMessage, StoryRole};

use crate::routes::{ApiError, ApiResult};
use crate::state::AppState;

/// Инструмент ведущего из `game_tools` — тот же, что зовёт рассказчик во время хода.
pub async fn tool(
    State(state): State<AppState>,
    Path((chat_id, name)): Path<(String, String)>,
    Json(arguments): Json<Value>,
) -> ApiResult<Json<Value>> {
    if !crate::game_tools::all().iter().any(|tool| tool.name == name) {
        return Err(ApiError::not_found(format!("нет инструмента ведущего {name}")));
    }
    state.store.get_chat(&chat_id)?.ok_or_else(|| ApiError::not_found("чат не найден"))?;
    let inner = state.0.clone();
    let answer = tokio::task::spawn_blocking(move || crate::game_tools::run(&inner, &chat_id, &name, &arguments))
        .await
        .map_err(|_| ApiError::internal("задача пропала"))?;
    Ok(Json(answer))
}

#[derive(Deserialize)]
pub struct Narrate {
    text: String,
    /// Озвучить в окне голосами истории.
    #[serde(default)]
    voice: bool,
    /// Промпт кадра по-английски: заявка кадра на сообщении.
    #[serde(default)]
    image_prompt: Option<String>,
    /// Нарисовать кадр сразу (в очереди к карте, за идущей работой).
    #[serde(default)]
    draw: bool,
}

/// Ход рассказчика от агента: сообщение рассказчика в истории, по желанию — озвучка в окне и кадр.
pub async fn narrate(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
    Json(body): Json<Narrate>,
) -> ApiResult<Json<Value>> {
    let text = body.text.trim().to_string();
    if text.is_empty() {
        return Err(ApiError::bad_request("пустой текст рассказчика"));
    }
    let chat = state.store.get_chat(&chat_id)?.ok_or_else(|| ApiError::not_found("чат не найден"))?;
    let image_request = body
        .image_prompt
        .as_deref()
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
        .map(|prompt| ImageRequest { needed: true, prompt: Some(prompt.to_string()), ..Default::default() });
    let message = StoryMessage {
        id: uuid::Uuid::new_v4().to_string(),
        role: StoryRole::Assistant,
        content: text.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        attachments: vec![],
        image_request: image_request.clone(),
        generated_image: None,
        rpg_snapshot: None,
    };
    state.store.add_message(&chat_id, &message)?;
    crate::mcp::announce("chat");

    let mut voice_job = None;
    if body.voice {
        let inner = state.0.clone();
        let settings = chat.summary.settings.clone();
        let key = format!("{chat_id}-gm-{}", message.id);
        let chat_for_play = chat_id.clone();
        // Озвучка — в общей очереди к карте: своя карта одна, и голос не должен пересечься с кадром.
        let chat_for_log = chat_id.clone();
        let job = state
            .queue
            .enqueue(Box::new(move |_, abort| {
                let runtime = crate::runtime::load(&inner.root);
                let voice = if crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Tts) {
                    crate::cloud::tts_voice(&runtime).to_string()
                } else if settings.voice.trim().is_empty() {
                    crate::tts::default_voice(&inner.root).unwrap_or_default()
                } else {
                    settings.voice.clone()
                };
                let mut first = None;
                for (index, sentence) in crate::tts::split_sentences(&text).into_iter().enumerate() {
                    if abort.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    let clip = crate::tts::synthesize_with(&inner.root, &inner.generated, &runtime, &sentence, &voice, &format!("{key}-{index}"));
                    match clip {
                        Ok(url) => {
                            crate::mcp::tell_windows(json!({ "play": url, "chat_id": chat_for_play }));
                            first.get_or_insert(url);
                        }
                        Err(error) => {
                            crate::hub::ended(crate::hub::Work::Tts, Err(&error));
                            background_failed("gm_narrate voice", &chat_for_log, &error);
                            return Err(error);
                        }
                    }
                }
                crate::hub::ended(crate::hub::Work::Tts, Ok(()));
                Ok(json!({ "first": first }))
            }))
            .await;
        voice_job = Some(job);
    }

    if body.draw {
        if let Some(prompt) = image_request.and_then(|request| request.prompt) {
            let settings = chat.summary.settings.clone();
            let message_id = message.id.clone();
            let chat_for_image = chat_id.clone();
            tokio::spawn(async move {
                let answer = crate::mcp::call_json(
                    "/api/images",
                    json!({ "prompt": prompt, "chatId": chat_for_image, "messageId": message_id, "settings": settings }),
                )
                .await;
                if let Err(error) = answer {
                    tracing::warn!("кадр ведущего не нарисовался: {error}");
                    background_failed("gm_narrate draw", &chat_for_image, &error);
                }
                crate::mcp::announce("chat");
            });
        }
    }

    Ok(Json(json!({ "message": message, "voicing": body.voice, "voice_job": voice_job, "drawing": body.draw })))
}

/// Голос и кадр ведущего идут после ответа агенту: их сбой он видит в журнале активности.
fn background_failed(what: &str, chat_id: &str, error: &str) {
    crate::activity::record(what, &json!({ "chat_id": chat_id }), false, error);
    crate::mcp::announce("activity");
}

#[derive(Deserialize)]
pub struct Apply {
    update: Value,
}

/// Механика хода от ведущего-агента: объявления — движку, тот считает и сохраняет.
pub async fn apply(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
    Json(body): Json<Apply>,
) -> ApiResult<Json<Value>> {
    let chat = state.store.get_chat(&chat_id)?.ok_or_else(|| ApiError::not_found("чат не найден"))?;
    let inner = state.0.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let characters = inner.store.list_characters(&chat_id).map_err(|e| e.to_string())?;
        let enemies = inner.store.combatants(&chat_id).map_err(|e| e.to_string())?;
        let mut rpg_states = Vec::new();
        for character in &characters {
            let state_of = inner.store.character_rpg(&chat_id, &character.id).map_err(|e| e.to_string())?.unwrap_or_default();
            rpg_states.push((character.id.clone(), state_of));
        }
        let quests = inner.store.list_quests(&chat_id).unwrap_or_default();
        let open_quests: Vec<du_rpg::Quest> = quests.iter().filter(|quest| quest.status.is_open()).cloned().collect();
        let hero_id = characters.first().map(|character| character.id.clone());
        let mut actors = crate::turn::build_actors(&characters, &rpg_states, &enemies);
        let (events, items) = crate::story::apply_mechanics(
            &inner,
            &chat_id,
            &chat.summary.settings,
            &body.update,
            &mut actors,
            &crate::story::Mechanics {
                characters: &characters,
                enemies: &enemies,
                open_quests: &open_quests,
                hero_id: hero_id.as_deref(),
                turn: chat.messages.len() as i64,
            },
        );
        inner.store.add_events(&chat_id, &events).map_err(|e| e.to_string())?;
        Ok(json!({ "events": events, "items": items }))
    })
    .await
    .map_err(|_| ApiError::internal("задача пропала"))?
    .map_err(ApiError::internal)?;
    crate::mcp::announce("chat");
    Ok(Json(result))
}
