//! Ход истории целиком: от реплики игрока до сохранённой сцены.
//!
//! Порядок проходов важен. Сначала нарратор потоком пишет прозу — игрок видит текст сразу.
//! Потом по этой прозе идут структурные проходы: что произошло механически и какой кадр
//! иллюстрирует момент. Так модель не смешивает служебные данные с текстом истории.
//!
//! Весь ход выполняется ОДНОЙ задачей очереди, поэтому карта занята им целиком и никакая
//! генерация картинки не влезет в середину.

use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::stream::Stream;
use serde::Deserialize;
use serde_json::{json, Value};

use du_core::{ImageRequest, StoryMessage, StoryRole};
use du_llm::{ChatClient, Message, Sampling};
use du_prompts::strip_image_artifacts;
use du_rpg::{apply_game_update, ApplyOptions, RpgSnapshot};

use crate::resolve::resolve_engine_update;
use crate::routes::ApiError;
use crate::state::AppState;
use crate::turn;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryBody {
    pub chat_id: String,
    pub input: String,
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Ход отдаётся потоком событий: проза приходит кусками, в конце — итог с механикой и кадром.
pub async fn story(
    State(state): State<AppState>,
    Json(body): Json<StoryBody>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Response> {
    let chat = state
        .store
        .get_chat(&body.chat_id)
        .map_err(|error| ApiError::internal(error.to_string()).into_response())?
        .ok_or_else(|| ApiError::not_found("чат не найден").into_response())?;

    // Реплику игрока сохраняем сразу: если ход сорвётся, она не потеряется.
    let user_message = StoryMessage {
        id: new_id(),
        role: StoryRole::User,
        content: body.input.clone(),
        created_at: now_iso(),
        attachments: vec![],
        image_request: None,
        generated_image: None,
        rpg_snapshot: None,
    };
    state
        .store
        .add_message(&body.chat_id, &user_message)
        .map_err(|error| ApiError::internal(error.to_string()).into_response())?;

    let inner = state.0.clone();
    let chat_id = body.chat_id.clone();
    let input = body.input.clone();

    let job_id = state
        .queue
        .enqueue(Box::new(move |progress| {
            run_turn(&inner, &chat_id, &input, &chat, progress)
        }))
        .await;

    let (mut receiver, terminal) = state
        .queue
        .subscribe(&job_id)
        .await
        .ok_or_else(|| ApiError::internal("задача потерялась").into_response())?;

    // Контракт потока — ИМЕНОВАННЫЕ события: интерфейс различает их по имени, а безымянные
    // просто игнорирует, из-за чего экран оставался пустым при уже готовом ходе.
    let stream = async_stream::stream! {
        if let Some(event) = terminal {
            if let Some(named) = to_named(&event) {
                yield Ok(named);
            }
            return;
        }
        while let Ok(event) = receiver.recv().await {
            let is_terminal =
                matches!(event.get("type").and_then(Value::as_str), Some("done") | Some("error"));
            if let Some(named) = to_named(&event) {
                yield Ok(named);
            }
            if is_terminal {
                break;
            }
        }
    };
    Ok(Sse::new(stream))
}

/// Разложить готовую фразу на куски с голосами. Контекстом для поиска говорящего служит
/// хвост уже прочитанного: имя часто называют фразой раньше самой реплики.
fn voiced_lines(
    sentence: &str,
    settings: &du_core::StorySettings,
    cast: &[du_core::StoryCharacter],
    pool: &[String],
    spoken: &str,
) -> Vec<(String, String)> {
    // Далёкий контекст только путает: смотрим на последние пару сотен символов.
    let context: String = spoken.chars().rev().take(220).collect::<Vec<_>>().into_iter().rev().collect();
    crate::dialogue::split_dialogue_segments(sentence, settings, cast, pool, &context)
        .into_iter()
        .map(|segment| (segment.text, segment.voice))
        .collect()
}

/// Переложить событие очереди в именованное событие потока.
///
/// `delta` — кусок прозы, `clip` — готовая фраза озвучки, `stage` — что сейчас считается,
/// `done` — итог хода, `error` — обрыв. Неизвестные события интерфейс игнорирует, поэтому
/// добавлять новые безопасно.
fn to_named(event: &Value) -> Option<Event> {
    let kind = event.get("type").and_then(Value::as_str).unwrap_or_default();
    match kind {
        "progress" => {
            if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                return Some(Event::default().event("delta").data(json!({ "text": delta }).to_string()));
            }
            if let Some(clip) = event.get("clip").and_then(Value::as_str) {
                let index = event.get("index").and_then(Value::as_u64).unwrap_or(0);
                return Some(
                    Event::default()
                        .event("clip")
                        .data(json!({ "url": clip, "index": index }).to_string()),
                );
            }
            if let Some(reason) = event.get("voiceError").and_then(Value::as_str) {
                return Some(
                    Event::default()
                        .event("notice")
                        .data(json!({ "text": format!("Озвучка не удалась: {reason}") }).to_string()),
                );
            }
            let stage = event.get("stage").and_then(Value::as_str)?;
            Some(Event::default().event("stage").data(json!({ "stage": stage }).to_string()))
        }
        "done" => {
            let result = event.get("result").cloned().unwrap_or(Value::Null);
            let message = result.get("message").cloned().unwrap_or(Value::Null);
            Some(
                Event::default().event("done").data(
                    json!({
                        "id": message.get("id"),
                        "content": message.get("content"),
                        "imageRequest": result.get("imageRequest"),
                        "events": result.get("events"),
                        "audioUrl": result.get("audioUrl"),
                    })
                    .to_string(),
                ),
            )
        }
        "error" => {
            let error = event.get("error").and_then(Value::as_str).unwrap_or("ход прервался");
            Some(Event::default().event("error").data(json!({ "error": error }).to_string()))
        }
        _ => None,
    }
}

/// Тело хода. Возвращает итог: сохранённое сообщение, журнал и заявку на кадр.
fn run_turn(
    state: &crate::state::Inner,
    chat_id: &str,
    input: &str,
    chat: &du_core::StoryChat,
    progress: crate::jobs::ProgressFn,
) -> Result<Value, String> {
    let settings = &chat.summary.settings;

    // Рассказчик идёт либо на своей карте, либо в облако — тогда карта вообще не нужна.
    let runtime = crate::runtime::load(&state.root);
    let client = if crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Narrator) {
        progress(json!({ "stage": "облако" }));
        ChatClient::new("https://openrouter.ai/api", std::time::Duration::from_secs(600))
            .map_err(|error| error.to_string())?
            .with_api_key(Some(runtime.openrouter_key.clone()))
            .with_model(Some(runtime.openrouter_narrator_model.clone()))
    } else {
        progress(json!({ "stage": "загрузка модели" }));
        let base_url = state.gpu.text_base_url().map_err(|error| error.to_string())?;
        ChatClient::new(&base_url, std::time::Duration::from_secs(600))
            .map_err(|error| error.to_string())?
    };

    // Состав хода и снимок ДО него: по снимку Повтор откатит последствия.
    let characters = state.store.list_characters(chat_id).map_err(|e| e.to_string())?;
    let items = state.store.list_items(chat_id).map_err(|e| e.to_string())?;
    let enemies = state.store.combatants(chat_id).map_err(|e| e.to_string())?;
    let mut rpg_states = Vec::new();
    for character in &characters {
        let state_of = state
            .store
            .character_rpg(chat_id, &character.id)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        rpg_states.push((character.id.clone(), state_of));
    }
    let hero_id = characters.first().map(|character| character.id.clone());
    let mut actors = turn::build_actors(&characters, &rpg_states, &enemies);
    let snapshot_chars: std::collections::BTreeMap<String, du_rpg::CharacterRpg> =
        rpg_states.iter().cloned().collect();

    let rpg_section = if settings.rpg_enabled {
        turn::build_rpg_section(&actors, &items, &enemies, du_prompts::prompts_for(settings.language))
    } else {
        String::new()
    };

    let (summary, _) = state.store.story_summary(chat_id).map_err(|e| e.to_string())?;
    let messages = turn::build_story_messages(
        &chat.messages,
        input,
        settings,
        &characters,
        &summary,
        &rpg_section,
    );

    // Озвучку готовим ЗАРАНЕЕ, чтобы первая же дописанная фраза сразу ушла в синтез.
    let voice = if settings.voice.trim().is_empty() {
        crate::tts::default_voice(&state.root).unwrap_or_default()
    } else {
        settings.voice.clone()
    };
    // Озвучка не зависит от того, где считается рассказчик: в облаке она вообще не трогает
    // карту, а локально голос нужен только свой.
    let speaking_on = settings.autoplay
        && runtime.tts_enabled
        && (!voice.is_empty() || crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Tts));
    // Кто говорит в ходе и какими голосами: реплики персонажей читаются их голосами, а не
    // голосом рассказчика. Раньше это делал браузер, получив ход целиком; теперь озвучка
    // начинается во время письма, поэтому голоса раздаёт сервер.
    let cast = if settings.multi_voice {
        state.store.list_characters(&chat_id).map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let voice_pool = crate::tts::available_voices(&state.root);
    // Канал несёт (номер, текст, голос): номер держит порядок воспроизведения.
    let (sentences_tx, sentences_rx) = std::sync::mpsc::channel::<(usize, String, String)>();
    let speaker = if speaking_on {
        let root = state.root.clone();
        let generated = state.generated.clone();
        let runtime_for_tts = runtime.clone();
        let voice = voice.clone();
        let key = format!("{chat_id}-{}", chat.messages.len());
        let sink = progress.clone();
        let gpu = state.gpu_handle();
        Some(std::thread::spawn(move || -> Option<String> {
            // Облачная озвучка отдаёт весь ход одним файлом — её не режем на фразы.
            if crate::cloud::stage_enabled(&runtime_for_tts, crate::cloud::Stage::Tts) {
                let whole: String = sentences_rx
                    .iter()
                    .map(|(_, sentence, _)| sentence)
                    .collect::<Vec<_>>()
                    .join(" ");
                if whole.trim().is_empty() {
                    return None;
                }
                return match crate::tts::synthesize_with(
                    &root, &generated, &runtime_for_tts, &whole, &voice, &key,
                ) {
                    Ok(url) => Some(url),
                    Err(error) => {
                        tracing::warn!("облачная озвучка не удалась: {error}");
                        sink(json!({ "stage": "озвучка", "voiceError": error.to_string() }));
                        None
                    }
                };
            }

            let engine = match gpu.speech_engine() {
                Ok(engine) => engine,
                Err(error) => {
                    tracing::warn!("озвучка не удалась: {error}");
                    // Игрок должен УЗНАТЬ, что звука не будет, а не гадать, почему тихо.
                    sink(json!({ "stage": "озвучка", "voiceError": error.to_string() }));
                    return None;
                }
            };
            // Эталонный клип на голос готовится один раз: перекодировать его на каждую
            // реплику — это лишние секунды посреди речи.
            let mut references: std::collections::HashMap<String, Option<(std::path::PathBuf, Option<String>)>> =
                std::collections::HashMap::new();
            let mut first = None;
            for (index, sentence, line_voice) in sentences_rx {
                let prepared = references.entry(line_voice.clone()).or_insert_with(|| {
                    match crate::tts::prepared_reference(
                        &root,
                        &line_voice,
                        runtime_for_tts.tts_reference_seconds,
                    ) {
                        Ok(pair) => Some(pair),
                        Err(error) => {
                            tracing::warn!("голос «{line_voice}» недоступен: {error}");
                            None
                        }
                    }
                });
                let Some((reference, transcript)) = prepared.as_ref() else { continue };
                match crate::tts::speak_sentence(
                    &engine,
                    &generated,
                    reference,
                    transcript.as_deref(),
                    &sentence,
                    &key,
                    index,
                ) {
                    Ok(url) => {
                        // Фраза готова — игрок слушает её, пока хвост ещё пишется.
                        sink(json!({ "stage": "озвучка", "clip": url, "index": index }));
                        first.get_or_insert(url);
                    }
                    Err(error) => tracing::warn!("фраза не озвучилась: {error}"),
                }
            }
            first
        }))
    } else {
        drop(sentences_rx);
        None
    };

    // Проза идёт потоком: игрок читает её, пока модель ещё пишет.
    progress(json!({ "stage": "проза" }));
    let sink = progress.clone();
    let sampling = Sampling::new(0.9, max_tokens_for(settings));
    let mut spoken = String::new();
    let mut pending = String::new();
    let mut sentence_index = 0usize;
    let narration = client
        .chat_stream(&messages, &sampling, |delta| {
            sink(json!({ "stage": "проза", "delta": delta }));
            if speaking_on {
                pending.push_str(delta);
                // Отдаём в озвучку каждую ЗАКОНЧЕННУЮ фразу, не дожидаясь конца хода.
                let ready = crate::tts::split_sentences(&pending);
                if ready.len() > 1 {
                    for sentence in &ready[..ready.len() - 1] {
                        for (text, line_voice) in
                            voiced_lines(sentence, &settings, &cast, &voice_pool, &spoken)
                        {
                            let _ = sentences_tx.send((sentence_index, text, line_voice));
                            sentence_index += 1;
                        }
                        spoken.push_str(sentence);
                    }
                    pending = ready[ready.len() - 1].clone();
                }
            }
            true
        })
        .map_err(|error| error.to_string())?;
    let narration = strip_image_artifacts(&narration);
    if narration.trim().is_empty() {
        return Err("нарратор вернул пустой текст".into());
    }
    if speaking_on {
        // Хвост после чистки артефактов озвучиваем тем же порядком.
        let tail = narration.strip_prefix(spoken.as_str()).unwrap_or(&pending).trim().to_string();
        if !tail.is_empty() {
            for sentence in crate::tts::split_sentences(&tail) {
                for (text, line_voice) in
                    voiced_lines(&sentence, &settings, &cast, &voice_pool, &spoken)
                {
                    let _ = sentences_tx.send((sentence_index, text, line_voice));
                    sentence_index += 1;
                }
                spoken.push_str(&sentence);
            }
        }
    }
    drop(sentences_tx);

    // Механика хода: модель объявляет, движок считает.
    let mut events = Vec::new();
    let mut granted = Vec::new();
    if settings.rpg_enabled {
        progress(json!({ "stage": "механика" }));
        let ask = vec![
            Message::system(
                "Ты — детерминированный движок правил для ОДНОГО хода. По описанию сцены выдай \
                 ТОЛЬКО объявления механики. Все броски и урон считает движок, ты их НЕ \
                 придумываешь. Персонажей называй именами из состояния игры."
                    .to_string(),
            ),
            Message::user(narration.clone()),
        ];
        match client.chat_json(&ask, &turn::structured_sampling(), &turn::engine_schema()) {
            Err(error) => tracing::warn!("проход механики не удался: {error}"),
            Ok(raw) => {
            let update = resolve_engine_update(&raw, &actors, hero_id.as_deref());
            let opts = ApplyOptions {
                hero_id: hero_id.clone(),
                random_events: settings.random_events,
                // Журнал приключения пишется на языке игры, а не по-русски при любом языке.
                journal: du_prompts::prompts_for(settings.language).journal.clone(),
            };
            let applied = apply_game_update(&update, &mut actors, &opts);
            events = applied.events;
            granted = applied.items;

            // Сохраняем всё, что изменил ход.
            for id in &applied.changed {
                if let Some(actor) = actors.get(id) {
                    if characters.iter().any(|character| &character.id == id) {
                        let _ = state.store.save_character_rpg(id, &actor.rpg);
                    }
                }
            }
            let mut roster = enemies.clone();
            roster.extend(applied.spawned_enemies.clone());
            for enemy in roster.iter_mut() {
                if let Some(actor) = actors.get(&enemy.id) {
                    enemy.rpg = actor.rpg.clone();
                }
            }
            let _ = state.store.set_combatants(chat_id, &roster);
            let _ = state.store.add_items(chat_id, &granted);
            let _ = state.store.add_events(chat_id, &events);
            }
        }
    }

    // Кадр: оператор выбирает ОДИН ключевой момент этой сцены.
    let mut image_request: Option<ImageRequest> = None;
    if settings.image_generation_enabled {
        progress(json!({ "stage": "кадр" }));
        let ask = vec![
            Message::system(
                "You are the cinematographer for an illustrated roleplay. Decide the ONE key \
                 image that best illustrates THIS passage. The prompt must be English, concrete \
                 and cinematic. Set location to a short STABLE label for the physical place and \
                 reuse it verbatim while the scene stays there; set sameLocation to true when \
                 this shot is the same place as the previous illustrated turn."
                    .to_string(),
            ),
            Message::user(narration.clone()),
        ];
        match client.chat_json(&ask, &turn::structured_sampling(), &turn::image_schema()) {
            Err(error) => tracing::warn!("проход кадра не удался: {error}"),
            Ok(raw) => {
            let prompt = raw.get("prompt").and_then(Value::as_str).unwrap_or_default().trim().to_string();
            // Пустой промпт означает, что кадра фактически нет: заявку не сохраняем, иначе
            // в интерфейсе появится кнопка «нарисовать», которая ничего не нарисует.
            if raw.get("needed").and_then(Value::as_bool) == Some(true) && !prompt.is_empty() {
                image_request = Some(ImageRequest {
                    needed: true,
                    prompt: Some(prompt),
                    location: raw.get("location").and_then(Value::as_str).map(str::to_string),
                    same_location: raw.get("sameLocation").and_then(Value::as_bool),
                    shot: serde_json::from_value(raw.get("shot").cloned().unwrap_or(Value::Null)).ok(),
                    character_ids: raw
                        .get("characters")
                        .and_then(Value::as_array)
                        .map(|list| {
                            list.iter()
                                .filter_map(|value| value.as_str())
                                // Модель называет персонажей именами — переводим в идентификаторы.
                                .filter_map(|name| {
                                    characters
                                        .iter()
                                        .find(|character| {
                                            character.id == name
                                                || character.name.eq_ignore_ascii_case(name)
                                        })
                                        .map(|character| character.id.clone())
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    ..Default::default()
                });
            }
            }
        }
    }

    // Дожидаемся озвучки: она шла параллельно механике и кадру. Не удалась — ход всё равно
    // состоялся: текст и картинка важнее звука.
    let narration_audio = speaker.and_then(|handle| handle.join().unwrap_or(None));

    let assistant = StoryMessage {
        id: new_id(),
        role: StoryRole::Assistant,
        content: narration,
        created_at: now_iso(),
        attachments: vec![],
        image_request: image_request.clone(),
        generated_image: None,
        // Снимок ДО хода — по нему Повтор и Стереть откатят последствия вместо повторного применения.
        rpg_snapshot: serde_json::to_value(RpgSnapshot {
            chars: snapshot_chars,
            combatants: enemies,
            item_ids: granted.iter().map(|item| item.id.clone()).collect(),
            event_ids: events.iter().map(|event| event.id.clone()).collect(),
        })
        .ok(),
    };
    state.store.add_message(chat_id, &assistant).map_err(|e| e.to_string())?;

    Ok(json!({
        "message": assistant,
        "events": events,
        "imageRequest": image_request,
        "audioUrl": narration_audio,
    }))
}

/// Сколько токенов дать нарратору под ответ выбранной длины.
fn max_tokens_for(settings: &du_core::StorySettings) -> u32 {
    match settings.response_length {
        du_core::ResponseLength::Short => 700,
        du_core::ResponseLength::Medium => 1400,
        du_core::ResponseLength::Long => 2600,
        du_core::ResponseLength::Epic => 4096,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use du_core::{ResponseLength, StorySettings};

    #[test]
    fn a_longer_answer_gets_a_bigger_budget() {
        let budget = |length| {
            max_tokens_for(&StorySettings { response_length: length, ..Default::default() })
        };
        assert!(budget(ResponseLength::Short) < budget(ResponseLength::Medium));
        assert!(budget(ResponseLength::Medium) < budget(ResponseLength::Long));
        assert!(budget(ResponseLength::Long) < budget(ResponseLength::Epic));
    }
}
