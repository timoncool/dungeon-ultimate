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

/// Сколько ждать озвучку после того, как всё остальное готово.
const AUDIO_WAIT_LIMIT: u64 = 90;

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

pub fn now_iso() -> String {
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

    // Погибшим героем история кончается. Раньше можно было писать дальше как ни в чём не
    // бывало: движок помечал героя мёртвым, а ход шёл своим чередом. Отменить роковой ход
    // по-прежнему можно — «Стереть» и «Повтор» откатывают его последствия.
    if chat.summary.settings.rpg_enabled && hero_is_dead(&state, &body.chat_id) {
        return Err(ApiError::conflict(
            "Герой погиб — история окончена. Отмени последний ход или начни новую историю.",
        )
        .into_response());
    }

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
        .enqueue(Box::new(move |progress, abort| {
            run_turn(&inner, &chat_id, &input, &chat, progress, abort)
        }))
        .await;


    let (mut receiver, terminal) = state
        .queue
        .subscribe(&job_id)
        .await
        .ok_or_else(|| ApiError::internal("задача потерялась").into_response())?;

    // Контракт потока — ИМЕНОВАННЫЕ события: интерфейс различает их по имени, а безымянные
    // просто игнорирует, из-за чего экран оставался пустым при уже готовом ходе.
    let told_job = job_id.clone();
    let stream = async_stream::stream! {
        // Первым делом сообщаем номер хода: по нему интерфейс сможет его прервать.
        yield Ok(Event::default().event("job").data(json!({ "job": told_job }).to_string()));
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

/// Погиб ли герой этой истории.
///
/// Герой — самый ранний персонаж чата, как и везде в игре. Ошибку чтения считаем «жив»:
/// молчание базы не повод запирать игрока.
fn hero_is_dead(state: &AppState, chat_id: &str) -> bool {
    let Ok(characters) = state.store.list_characters(chat_id) else { return false };
    let Some(hero) = characters.into_iter().next() else { return false };
    state
        .store
        .character_rpg(chat_id, &hero.id)
        .ok()
        .flatten()
        .map(|rpg| rpg.dead)
        .unwrap_or(false)
}

/// Разложить готовую фразу на куски с голосами. Контекстом для поиска говорящего служит
/// хвост уже прочитанного: имя часто называют фразой раньше самой реплики.
/// Забрать САМЫЙ СТАРЫЙ из запущенных запросов озвучки и отдать клип игроку.
///
/// Порядок важнее скорости: фразы должны звучать так, как написаны, поэтому ждём именно
/// первый в очереди, даже если следующий уже готов.
fn take_ready(
    flying: &mut Vec<std::thread::JoinHandle<(usize, String, Result<String, String>)>>,
    sink: &crate::jobs::ProgressFn,
    first: &mut Option<String>,
) {
    if flying.is_empty() {
        return;
    }
    match flying.remove(0).join() {
        Ok((index, voice, Ok(url))) => {
            sink(json!({ "stage": "озвучка", "clip": url, "index": index, "voice": voice }));
            first.get_or_insert(url);
        }
        Ok((_, _, Err(error))) => {
            tracing::warn!("фраза не озвучилась: {error}");
            sink(json!({ "stage": "озвучка", "voiceError": error }));
        }
        Err(_) => tracing::warn!("поток озвучки не дожил до ответа"),
    }
}

fn voiced_lines(
    sentence: &str,
    settings: &du_core::StorySettings,
    cast: &[du_core::StoryCharacter],
    pool: &[String],
    spoken: &str,
    casting: &std::sync::Mutex<Option<crate::casting::Casting>>,
    narrator_voice: &str,
) -> Vec<(String, String)> {
    // Далёкий контекст только путает: смотрим на последние пару сотен символов.
    let context: String = spoken.chars().rev().take(220).collect::<Vec<_>>().into_iter().rev().collect();
    let context = crate::voice_tags::strip(&context);
    let ready = casting.lock().ok().and_then(|guard| guard.clone()).unwrap_or_default();
    // Рассказчик сам помечает реплики: `[[V:eve]]` перед прямой речью. Пометка режет фразу
    // на куски, и это работает и с кавычками, и с тире — искать одни кавычки бессмысленно,
    // русская проза оформляет речь и так, и так.
    let mut lines: Vec<(String, String)> = Vec::new();
    for (tag, part) in crate::voice_tags::runs(sentence) {
        // Выдуманный голос провайдер не знает и ответил бы отказом — берём только известные.
        let tagged = tag.and_then(|name| {
            pool.iter().find(|voice| voice.eq_ignore_ascii_case(name.trim())).cloned()
        });
        let Some(tagged) = tagged else {
            lines.extend(plain_lines(&part, settings, cast, pool, &context, narrator_voice, &ready));
            continue;
        };
        // Слова автора («— буркнул старик») читает рассказчик: голосом персонажа они
        // звучали бы как продолжение его же речи.
        let (speech, tail) = crate::voice_tags::speech_and_tail(&part);
        if !speech.is_empty() {
            lines.push((speech, tagged));
        }
        if !tail.trim().is_empty() {
            lines.push((tail, narrator_voice.to_string()));
        }
    }
    lines
}

/// Раздать голоса куску без пометок — по заведённым персонажам и по словам самого текста.
#[allow(clippy::too_many_arguments)]
fn plain_lines(
    part: &str,
    settings: &du_core::StorySettings,
    cast: &[du_core::StoryCharacter],
    pool: &[String],
    context: &str,
    narrator_voice: &str,
    ready: &crate::casting::Casting,
) -> Vec<(String, String)> {
    crate::dialogue::split_dialogue_segments(part, settings, cast, pool, context, narrator_voice, &[])
        .into_iter()
        .map(|segment| {
            // Раскладка от модели важнее подбора по словам: она видела персонажа целиком.
            let voice = segment
                .character_id
                .as_ref()
                .and_then(|id| cast.iter().find(|character| &character.id == id))
                .and_then(|character| ready.get(&character.name).cloned())
                .unwrap_or(segment.voice);
            (segment.text, voice)
        })
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
                let voice = event.get("voice").and_then(Value::as_str).unwrap_or_default();
                return Some(
                    Event::default().event("clip").data(
                        json!({ "url": clip, "index": index, "voice": voice }).to_string(),
                    ),
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
    // Поднят, когда игрок нажал «Прервать» или ушёл со страницы.
    abort: crate::jobs::AbortFlag,
) -> Result<Value, String> {
    let settings = &chat.summary.settings;
    let stopped = || abort.load(std::sync::atomic::Ordering::Relaxed);
    // Инструменты включаем там, где они проверены: в облаке. Своя Гемма их тоже умеет, но
    // её вызовы приходят иначе — включим, когда проверю живьём, а не на веру.
    let tools_client_is_local =
        !crate::cloud::stage_enabled(&crate::runtime::load(&state.root), crate::cloud::Stage::Narrator);

    // Рассказчик идёт либо на своей карте, либо в облако — тогда карта вообще не нужна.
    let runtime = crate::runtime::load(&state.root);
    let client = if crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Narrator) {
        progress(json!({ "stage": "облако" }));
        // Размышления ПРЯЧЕМ, а не запрещаем: запрет часть моделей отвергает отказом, а
        // если рассуждения попадают в ответ, структурный проход возвращает их вместо JSON —
        // и механика хода не срабатывает вовсе.
        let caps = crate::model_caps::caps(
            &runtime.openrouter_narrator_model,
            &runtime.openrouter_key,
        );
        // Отключать можно только там, где схема это разрешает; иначе прячем и просим
        // самый слабый уровень из объявленных — глубокие размышления тут лишние.
        let reasoning = match (caps.reasoning, caps.reasoning_mandatory) {
            (true, false) => du_llm::Reasoning::Off,
            (true, true) => du_llm::Reasoning::Hide,
            _ => du_llm::Reasoning::AsIs,
        };
        let effort = du_llm::Effort(caps.cheapest_effort().map(str::to_string));
        ChatClient::new("https://openrouter.ai/api", std::time::Duration::from_secs(600))
            .map_err(|error| error.to_string())?
            .with_api_key(Some(runtime.openrouter_key.clone()))
            .with_model(Some(runtime.openrouter_narrator_model.clone()))
            .with_reasoning(reasoning)
            .with_effort(effort)
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

    // Открытые задания: взятые уходят в состояние хода, предложенные нужны, чтобы модель
    // не предлагала то же самое ещё раз.
    let quests = if settings.rpg_enabled {
        state.store.list_quests(chat_id).unwrap_or_default()
    } else {
        Vec::new()
    };
    let open_quests: Vec<du_rpg::Quest> =
        quests.iter().filter(|quest| quest.status.is_open()).cloned().collect();

    let rpg_section = if settings.rpg_enabled {
        turn::build_rpg_section(
            &actors,
            &items,
            &enemies,
            &open_quests,
            du_prompts::prompts_for(settings.language),
        )
    } else {
        String::new()
    };

    let (summary, _) = state.store.story_summary(chat_id).map_err(|e| e.to_string())?;
    let mut messages = turn::build_story_messages(
        &chat.messages,
        input,
        settings,
        &characters,
        &summary,
        &rpg_section,
    );

    // Озвучку готовим ЗАРАНЕЕ, чтобы первая же дописанная фраза сразу ушла в синтез.
    // Голос рассказчика зависит от того, ГДЕ считается озвучка: на карте это имя файла с
    // эталоном, в облаке — имя голоса модели. Смешивать нельзя: локальное имя провайдер не
    // знает и отвечает отказом, а голос модели не найдётся среди файлов на диске.
    let voice = if crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Tts) {
        let chosen = runtime.openrouter_tts_voice.trim();
        if chosen.is_empty() {
            crate::voice_catalog::suitable(&runtime.openrouter_tts_model, true, None)
                .first()
                .map(|voice| voice.name.clone())
                .unwrap_or_default()
        } else {
            chosen.to_string()
        }
    } else if settings.voice.trim().is_empty() {
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
    // Раскладку голосов спрашивает отдельный проход — он идёт ниже, когда есть текст.
    // Набор голосов зависит от того, где считается озвучка: на карте это файлы эталонов,
    // в облаке — голоса выбранной модели. Иначе персонажам достались бы имена, которых у
    // провайдера нет, и озвучка молча падала бы на каждой реплике.
    let voice_pool = if crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Tts) {
        crate::voice_catalog::suitable(&runtime.openrouter_tts_model, settings.language == du_core::Language::Ru, None)
            .into_iter()
            .map(|voice| voice.name.clone())
            .collect()
    } else {
        crate::tts::available_voices(&state.root)
    };
    // Голоса отдаём рассказчику: он один знает, КТО произносит каждую реплику, — а список
    // с пометками пола и возраста позволяет ему выбрать женский голос женщине и детский
    // ребёнку. Без этого автокастинг работал бы только на заведённых персонажах, которых в
    // истории почти нет: вдов и стражников игрок персонажами не заводит.
    if settings.multi_voice && speaking_on && !voice_pool.is_empty() {
        let hints: Vec<crate::voice_tags::VoiceHint> = voice_pool
            .iter()
            .filter(|name| *name != &voice)
            .map(|name| crate::voice_tags::VoiceHint {
                name: name.clone(),
                gender: crate::voice_catalog::gender_of(name)
                    .map(str::to_string)
                    .unwrap_or_else(|| match crate::dialogue::voice_gender(name) {
                        Some(crate::dialogue::Gender::Female) => "женский".to_string(),
                        Some(crate::dialogue::Gender::Male) => "мужской".to_string(),
                        None => "неизвестен".to_string(),
                    }),
                age: crate::voice_catalog::age_of(name).unwrap_or("взрослый").to_string(),
            })
            .collect();
        let asked =
            crate::voice_tags::instruction(&hints, settings.language == du_core::Language::Ru);
        if !asked.is_empty() {
            tracing::debug!("голоса рассказчику: {} шт.", hints.len());
            // Ставим СРАЗУ за главной подсказкой, а не в самый конец: в конце указание
            // оказывается после реплики игрока, и модель его попросту не выполняет —
            // проверено живым ходом, пометок не было ни одной.
            let at = messages.len().min(1);
            messages.insert(at, Message::system(asked));
        }
    } else {
        tracing::debug!(
            "пометки голосов не запрошены: многоголосие={}, озвучка={}, голосов={}",
            settings.multi_voice,
            speaking_on,
            voice_pool.len()
        );
    }

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
            // Один путь для обоих режимов: фразы приходят по очереди и озвучиваются каждая
            // своим голосом. Раньше облако склеивало ход в один запрос — и весь текст,
            // включая реплики персонажей, читался голосом рассказчика.
            let in_cloud = crate::cloud::stage_enabled(&runtime_for_tts, crate::cloud::Stage::Tts);
            let engine = if in_cloud {
                None
            } else {
                match gpu.speech_engine() {
                    Ok(engine) => Some(engine),
                    Err(error) => {
                        tracing::warn!("озвучка не удалась: {error}");
                        sink(json!({ "stage": "озвучка", "voiceError": error.to_string() }));
                        return None;
                    }
                }
            };
            // Эталонный клип на голос готовится один раз: перекодировать его на каждую
            // реплику — это лишние секунды посреди речи.
            let mut references: std::collections::HashMap<String, Option<(std::path::PathBuf, Option<String>)>> =
                std::collections::HashMap::new();
            let mut first = None;
            // В облаке фразы синтезируются ОДНОВРЕМЕННО: это независимые запросы к чужому
            // серверу, и ждать, пока договорит предыдущая, незачем. На своей карте так нельзя —
            // движок там один, и очередь обязательна.
            let side_by_side = in_cloud && runtime_for_tts.tts_parallel;
            let mut flying: Vec<std::thread::JoinHandle<(usize, String, Result<String, String>)>> =
                Vec::new();
            for (index, sentence, line_voice) in sentences_rx {
                if side_by_side {
                    // Больше четырёх запросов разом провайдер не любит, а игрок всё равно
                    // столько вперёд не прослушает.
                    while flying.len() >= 4 {
                        take_ready(&mut flying, &sink, &mut first);
                    }
                    let root = root.clone();
                    let generated = generated.clone();
                    let mut runtime_for_line = runtime_for_tts.clone();
                    runtime_for_line.openrouter_tts_voice = line_voice.clone();
                    let key = key.clone();
                    flying.push(std::thread::spawn(move || {
                        let spoken = crate::tts::synthesize_with(
                            &root,
                            &generated,
                            &runtime_for_line,
                            &sentence,
                            &line_voice,
                            &format!("{key}-{index}"),
                        );
                        (index, line_voice, spoken)
                    }));
                    continue;
                }
                let spoken = if in_cloud {
                    // В облаке голос — это имя из набора модели, эталон не нужен.
                    let mut runtime_for_line = runtime_for_tts.clone();
                    runtime_for_line.openrouter_tts_voice = line_voice.clone();
                    crate::tts::synthesize_with(
                        &root,
                        &generated,
                        &runtime_for_line,
                        &sentence,
                        &line_voice,
                        &format!("{key}-{index}"),
                    )
                } else {
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
                    match (engine.as_ref(), prepared.as_ref()) {
                        (Some(engine), Some((reference, transcript))) => crate::tts::speak_sentence(
                            engine,
                            &generated,
                            reference,
                            transcript.as_deref(),
                            &sentence,
                            &key,
                            index,
                        ),
                        _ => continue,
                    }
                };
                match spoken {
                    Ok(url) => {
                        // Фраза готова — игрок слушает её, пока хвост ещё пишется.
                        // Голос отдаём вместе с клипом: по нему видно, что реплики читают РАЗНЫЕ голоса,
                        // а не один рассказчик.
                        sink(json!({
                            "stage": "озвучка",
                            "clip": url,
                            "index": index,
                            "voice": line_voice,
                        }));
                        first.get_or_insert(url);
                    }
                    Err(error) => {
                        tracing::warn!("фраза не озвучилась: {error}");
                        sink(json!({ "stage": "озвучка", "voiceError": error.to_string() }));
                    }
                }
            }
            // Хвост: последние запросы ещё в полёте, их клипы игрок ждёт.
            while !flying.is_empty() {
                take_ready(&mut flying, &sink, &mut first);
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
    // Раскладка «персонаж → голос» на этот ход. Считается лениво: пока не появилась первая
    // фраза с прямой речью, спрашивать не о чем.
    let casting: std::sync::Arc<std::sync::Mutex<Option<crate::casting::Casting>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    // Хвост куска, который может оказаться НАЧАЛОМ пометки: показывать его нельзя, пока
    // не пришло закрытие, иначе игрок увидит «[[V:» на середине фразы.
    let mut held = String::new();
    let narration = client
        .chat_stream(&messages, &sampling, |delta| {
            // Игрок прервал — прекращаем писать прямо здесь, не досчитывая ход до конца.
            if stopped() {
                return false;
            }
            held.push_str(delta);
            let keep = crate::voice_tags::dangling(&held);
            let show: String = held[..held.len() - keep].to_string();
            held = held[held.len() - keep..].to_string();
            let show = crate::voice_tags::strip(&show);
            if !show.is_empty() {
                sink(json!({ "stage": "проза", "delta": show }));
            }
            if speaking_on {
                pending.push_str(delta);
                // Отдаём в озвучку каждую ЗАКОНЧЕННУЮ фразу, не дожидаясь конца хода.
                let ready = crate::tts::split_sentences(&pending);
                if ready.len() > 1 {
                    for sentence in &ready[..ready.len() - 1] {
                        for (text, line_voice) in voiced_lines(
                            sentence,
                            &settings,
                            &cast,
                            &voice_pool,
                            &spoken,
                            &casting,
                            &voice,
                        ) {
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
    // Хвост озвучки считаем по СЫРОМУ тексту: `spoken` тоже сырой, с пометками, и вычесть
    // одно из другого можно только пока они в одном виде.
    let narration_raw = narration.clone();
    let narration = crate::voice_tags::strip(&strip_image_artifacts(&narration));
    // Раскладку заказываем один раз за ход и только если в отрывке есть прямая речь.
    if settings.multi_voice && !cast.is_empty() && narration.contains(['«', '"', '\u{201c}']) {
        let described: Vec<(String, String, String)> = voice_pool
            .iter()
            .map(|name| {
                let gender = crate::voice_catalog::gender_of(name)
                    .map(str::to_string)
                    .unwrap_or_else(|| match crate::dialogue::voice_gender(name) {
                        Some(crate::dialogue::Gender::Female) => "female".to_string(),
                        Some(crate::dialogue::Gender::Male) => "male".to_string(),
                        None => "unknown".to_string(),
                    });
                let age = crate::voice_catalog::age_of(name).unwrap_or("adult").to_string();
                (name.clone(), gender, age)
            })
            .collect();
        let ready = crate::casting::assign(&client, &narration, &cast, &described);
        if !ready.is_empty() {
            if let Ok(mut guard) = casting.lock() {
                *guard = Some(ready);
            }
        }
    }
    if stopped() {
        return Err("ход прерван".into());
    }
    if narration.trim().is_empty() {
        return Err("нарратор вернул пустой текст".into());
    }
    if speaking_on {
        // Хвост после чистки артефактов озвучиваем тем же порядком.
        let tail =
            narration_raw.strip_prefix(spoken.as_str()).unwrap_or(&pending).trim().to_string();
        if !tail.is_empty() {
            for sentence in crate::tts::split_sentences(&tail) {
                for (text, line_voice) in
                    voiced_lines(&sentence, &settings, &cast, &voice_pool, &spoken, &casting, &voice)
                {
                    let _ = sentences_tx.send((sentence_index, text, line_voice));
                    sentence_index += 1;
                }
                spoken.push_str(&sentence);
            }
        }
    }
    drop(sentences_tx);

    // Инструменты: модель сама смотрит состояние и сама закрывает то, что выполнено.
    //
    // Это отдельный короткий проход ПОСЛЕ прозы: подсказка со списком заданий не работала —
    // модель её не помнила, и задания висели открытыми сотню ходов. Теперь она спрашивает.
    // Проход не обязателен: не вышел — ход идёт дальше, как раньше.
    // Что инструменты успели записать в журнал: броски, награды, закрытые задания. Их надо
    // отдать игроку вместе с ходом — иначе кубик не крутится и строки в журнале появляются
    // только после перезагрузки.
    let mut tool_events: Vec<du_rpg::GameEvent> = Vec::new();
    if settings.rpg_enabled && !stopped() && !tools_client_is_local {
        progress(json!({ "stage": "инструменты" }));
        let known: std::collections::HashSet<String> = state
            .store
            .list_events(chat_id, 60)
            .map(|events| events.into_iter().map(|event| event.id).collect())
            .unwrap_or_default();
        let talk = vec![
            json!({ "role": "system", "content":
                "Ты ведёшь эту партию. Перед тем как решать, посмотри состояние инструментами:                  открытые задания, лист персонажей, журнал. Если в отрывке ВЫПОЛНЕНО условие                  взятого задания — закрой его. Если заговорил тот, кого нет в листе, — заведи                  его character_add с полом и возрастом, чтобы у него был свой голос. Ничего                  не выдумывай: числа и исходы берутся инструментами. В конце ответь одним                  коротким предложением о том, что ты сделал." }),
            json!({ "role": "user", "content": narration.clone() }),
        ];
        let sink = progress.clone();
        // Инструменты надо ВЫДАТЬ: клиент рассказчика их не несёт, и без этого модель просто
        // не знает, что у неё есть руки.
        let with_tools = client.clone().with_tools(crate::game_tools::all());
        match crate::game_tools::converse(
            state,
            chat_id,
            &with_tools,
            &turn::structured_sampling(),
            talk,
            &|name, _| sink(json!({ "stage": "инструменты", "tool": name })),
        ) {
            Ok(_) => {}
            Err(error) => tracing::warn!("проход инструментов не удался: {error}"),
        }
        if let Ok(after) = state.store.list_events(chat_id, 60) {
            tool_events = after.into_iter().filter(|event| !known.contains(&event.id)).collect();
        }
    }

    // Механика хода: модель объявляет, движок считает.
    // Порядок событий — хронологический: инструменты отработали до этого прохода.
    let mut events = tool_events;
    let mut granted = Vec::new();
    if settings.rpg_enabled && !stopped() {
        progress(json!({ "stage": "механика" }));
        let ask = vec![
            Message::system(
                "Ты — детерминированный движок правил для ОДНОГО хода. По описанию сцены выдай \
                 ТОЛЬКО объявления механики. Все броски и урон считает движок, ты их НЕ \
                 придумываешь. Персонажей называй именами из состояния игры.\n\n\
                 ЗАДАНИЯ. offerQuests заполняй РЕДКО и только тогда, когда в отрывке живой \
                 персонаж ПРЯМО обратился к герою с просьбой или поручением: сказал, что \
                 нужно сделать, и к нему можно вернуться с ответом. Обязательно укажи \
                 giver — имя того, кто дал. Случайная находка, слух, надпись на стене и \
                 собственные намерения героя заданиями НЕ являются. Нет такой просьбы в \
                 отрывке — оставляй offerQuests пустым; это обычное состояние хода.\n\
                 completeQuests заполняй, только когда в отрывке выполнено условие задания \
                 из раздела ВЗЯТЫЕ ЗАДАНИЯ, и называй его тем же заголовком."
                    .to_string(),
            ),
            Message::user(narration.clone()),
        ];
        // Взятые задания ОБЯЗАНЫ дойти до движка: он видит только текст отрывка, а раздел
        // состояния уходит рассказчику. Без этого списка ему нечего называть в
        // completeQuests — выполнение не засчитывалось никогда.
        let taken: Vec<&du_rpg::Quest> = open_quests
            .iter()
            .filter(|quest| quest.status == du_rpg::QuestStatus::Active)
            .collect();
        let mut ask = ask;
        if !taken.is_empty() {
            let rows = taken
                .iter()
                .map(|quest| {
                    let conditions = if quest.conditions.is_empty() {
                        String::new()
                    } else {
                        format!("
    условия: {}", quest.conditions.join("; "))
                    };
                    let giver = quest
                        .giver
                        .as_deref()
                        .map(|giver| format!(" (дал: {giver})"))
                        .unwrap_or_default();
                    format!("• «{}»{giver}{conditions}", quest.title)
                })
                .collect::<Vec<_>>()
                .join("
");
            ask.insert(
                1,
                Message::system(format!(
                    "ВЗЯТЫЕ ЗАДАНИЯ героя:
{rows}

Если в отрывке выполнено условие                      одного из них — верни его в completeQuests с ТЕМ ЖЕ заголовком в                      кавычках. Если задание стало невыполнимым — в failQuests. Ничего из                      этого в отрывке не произошло — оставь оба списка пустыми."
                )),
            );
        }
        match client.chat_json(&ask, &turn::structured_sampling(), &turn::engine_schema()) {
            Err(error) => tracing::warn!("проход механики не удался: {error}"),
            Ok(raw) => {
            let update = resolve_engine_update(&raw, &actors, hero_id.as_deref());
            let opts = ApplyOptions {
                hero_id: hero_id.clone(),
                random_events: settings.random_events,
                quests: open_quests.clone(),
                // Номер хода: по нему движок держит редкость заданий.
                turn: chat.messages.len() as i64,
                // Журнал приключения пишется на языке игры, а не по-русски при любом языке.
                journal: du_prompts::prompts_for(settings.language).journal.clone(),
            };
            let applied = apply_game_update(&update, &mut actors, &opts);
            // Дописываем, а НЕ заменяем: иначе броски и награды инструментов пропадут.
            events.extend(applied.events);
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

            // Задания: новые кладём предложенными — решение за игроком; закрытые меняют
            // состояние там, где лежали.
            let now = now_iso();
            let offered: Vec<du_rpg::Quest> = applied
                .quests_offered
                .into_iter()
                .map(|mut quest| {
                    quest.created_at = now.clone();
                    quest.updated_at = now.clone();
                    quest
                })
                .collect();
            let _ = state.store.put_quests(chat_id, &offered);
            for (quest_id, status) in &applied.quests_closed {
                let _ = state.store.set_quest_status(chat_id, quest_id, *status, &now);
            }

            let _ = state.store.add_events(chat_id, &events);
            }
        }
    }

    // Кадр: оператор выбирает ОДИН ключевой момент этой сцены.
    let mut image_request: Option<ImageRequest> = None;
    if settings.image_generation_enabled && !stopped() {
        progress(json!({ "stage": "кадр" }));
        let ask = vec![
            Message::system(
                "You are the cinematographer for an illustrated roleplay. Decide the ONE key \
                 image that best illustrates THIS passage.\n\n\
                 WRITE A FULL PROMPT, NOT A LABEL — a short prompt gives a weak, generic \
                 picture. In English, as flowing comma-separated phrases, in this order:\n\
                 1) the subject and what they are DOING at this exact instant — posture, \
                 hands, gaze, expression;\n\
                 2) how they look and what they wear — fabric, wear and tear, colour;\n\
                 3) the place and the concrete things visible in it, near and far;\n\
                 4) the light — its source, direction, hardness, what it touches and what \
                 stays in shadow;\n\
                 5) the camera — shot size, angle, lens feel, depth of field;\n\
                 6) palette and mood, in a few words.\n\
                 Prefer specific nouns to adjectives: not \"an old room\" but \"a low room of \
                 soot-blackened beams, a cracked clay pot on the sill\". No proper names — \
                 describe people by appearance. No text or lettering in the frame.\n\n\
                 Set location to a short STABLE label for the physical place and reuse it \
                 verbatim while the scene stays there; set sameLocation to true when this \
                 shot is the same place as the previous illustrated turn.\n\
                 Set reference to what this picture should grow from: \"scene\" when the \
                 action continues in a place already drawn, \"characters\" when a known face \
                 or figure must stay recognisable, \"none\" for a new place with nobody \
                 familiar in it."
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
                    reference: raw.get("reference").and_then(Value::as_str).map(str::to_string),
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
    // Ждём озвучку ОГРАНИЧЕННО. Зависший синтез не должен держать ход: текст и картинка
    // важнее звука, а поток пусть договаривает сам по себе — его клипы уже ушли игроку.
    let narration_audio = speaker.and_then(|handle| {
        // Пока ждём голос, игрок должен видеть именно это: иначе над ходом висит стадия
        // кадра, хотя кадр давно выбран, и кажется, что всё зависло.
        progress(json!({ "stage": "озвучка" }));
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done_tx.send(handle.join().unwrap_or(None));
        });
        match done_rx.recv_timeout(std::time::Duration::from_secs(AUDIO_WAIT_LIMIT)) {
            Ok(url) => url,
            Err(_) => {
                tracing::warn!("озвучка не уложилась в {AUDIO_WAIT_LIMIT} с — ход идёт без неё");
                None
            }
        }
    });

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
