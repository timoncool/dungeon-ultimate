//! Маршруты REST. Пути и форма ответов те же, что отдавали роуты Next.js, чтобы фронт
//! переезжал без правок контракта.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use du_core::{dimensions_for_image, Attachment, GeneratedImage, ImageRequest, StorySettings};
use du_image::{GenParams, LoraSpec, RawImage};
use du_prompts::{apply_edit_continuity, finalize_scene_prompt};

use crate::scene;
use crate::state::AppState;

/// Ошибка запроса в том же виде, в каком её ждал прежний фронт: `{ "error": "…" }`.
pub struct ApiError(StatusCode, String);

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self(StatusCode::NOT_FOUND, message.into())
    }

    /// Действие противоречит текущему состоянию игры — например, ход после гибели героя.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self(StatusCode::CONFLICT, message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, message.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<du_db::DbError> for ApiError {
    fn from(error: du_db::DbError) -> Self {
        ApiError::internal(error.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

// ── Состояние ──────────────────────────────────────────────────────────────────

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    let paths = state.gpu.paths();
    // Про свой движок картинок спрашиваем, только если кадры и правда рисует своя карта.
    // Кадры в облаке — местный движок к делу не относится, и трогать его незачем.
    let runtime = crate::runtime::load(&state.root);
    let in_cloud = crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Image);
    let (engine_version, devices) =
        if in_cloud { Default::default() } else { state.gpu.image_info().unwrap_or_default() };
    Json(json!({
        "ok": true,
        "image": {
            "ready": in_cloud || paths.image_ready(),
            "inCloud": in_cloud,
            "engine": engine_version,
            "devices": devices,
        },
        "text": { "ready": paths.text_ready() },
        "gpu": format!("{:?}", state.gpu.resident()),
    }))
}

/// Где НА САМОМ ДЕЛЕ считается каждая стадия — облако, видеокарта или процессор.
///
/// Настройки показывают выбор игрока, а этот ответ — итог: пустой выбор стадии подменяется
/// общим, «как есть» превращается в конкретное железо, а включённое облако отменяет и то, и
/// другое. Раньше это можно было только угадывать, и игроки жаловались, что непонятно, чем
/// считается ход.
pub async fn where_it_runs(State(state): State<AppState>) -> Json<Value> {
    use crate::backend::{stage_backend, Backend, Stage};
    let runtime = crate::runtime::load(&state.root);
    let decide = |stage: Stage, cloud: crate::cloud::Stage, model: &str| {
        if crate::cloud::stage_enabled(&runtime, cloud) {
            return json!({ "where": "cloud", "detail": model });
        }
        let device = match stage_backend(&runtime, stage) {
            Backend::Gpu => "gpu",
            Backend::Cpu => "cpu",
        };
        json!({ "where": device, "detail": "" })
    };
    Json(json!({
        "gpuPresent": crate::backend::gpu_present(),
        "stages": {
            "narrator": decide(Stage::Narrator, crate::cloud::Stage::Narrator, &runtime.openrouter_narrator_model),
            "image": decide(Stage::Image, crate::cloud::Stage::Image, &runtime.openrouter_image_model),
            "tts": decide(Stage::Tts, crate::cloud::Stage::Tts, &runtime.openrouter_tts_model),
            "asr": decide(Stage::Asr, crate::cloud::Stage::Asr, &runtime.openrouter_asr_model),
        }
    }))
}

/// Есть ли новая версия. В сеть ходит не чаще раза в полдня, отказ сети не считается ошибкой.
pub async fn release_check() -> Json<Value> {
    let release = tokio::task::spawn_blocking(crate::release::check)
        .await
        .unwrap_or_else(|_| crate::release::Release {
            current: crate::release::CURRENT.to_string(),
            latest: String::new(),
            newer: false,
            url: "https://github.com/timoncool/dungeon-ultimate/releases".to_string(),
            notes: String::new(),
        });
    Json(json!({ "release": release }))
}

// ── Кадр ───────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageBody {
    /// Сообщение, к которому относится кадр: по нему восстанавливается контекст сцены.
    #[serde(default)]
    pub message_id: Option<String>,
    pub prompt: String,
    #[serde(default)]
    pub chat_id: Option<String>,
    #[serde(default)]
    pub settings: StorySettings,
    // Интерфейс шлёт эти поля отдельно, когда рисует портрет или предмет: настройки чата
    // там не при чём, важны только размер и пропорции конкретного кадра.
    #[serde(default)]
    pub mode: Option<du_core::ImageMode>,
    #[serde(default)]
    pub aspect: Option<du_core::AspectPreset>,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub references: Vec<Attachment>,
}

pub async fn create_image(
    State(state): State<AppState>,
    Json(body): Json<ImageBody>,
) -> ApiResult<Json<GeneratedImage>> {
    let context = match body.message_id.as_deref() {
        Some(id) => state.store.message_context(id)?,
        None => None,
    };
    let chat_id = context
        .as_ref()
        .map(|(chat_id, _)| chat_id.clone())
        .or_else(|| body.chat_id.clone());
    let request: Option<ImageRequest> = context.and_then(|(_, request)| request);

    let decision = match chat_id.as_deref() {
        Some(chat_id) => scene::decide(
            &state.store,
            chat_id,
            request.as_ref(),
            &body.prompt,
            &body.references,
        )?,
        None => scene::SceneDecision::default(),
    };

    // Правка получает указание сохранить всё, кроме описанного; свежий кадр — только
    // стилевой замок и запрет текста в кадре.
    let styled = finalize_scene_prompt(&body.prompt, &body.settings.image_style_prefix);
    let prompt = if decision.is_edit() { apply_edit_continuity(&styled) } else { styled };

    let (width, height) = dimensions_for_image(
        body.mode.unwrap_or(body.settings.image_mode),
        body.aspect.unwrap_or(body.settings.aspect),
    );
    let references = load_references(&state, &decision.references);
    let is_edit = decision.is_edit();
    let lora = state.gpu.paths().edit_lora();

    let params = GenParams {
        prompt: prompt.clone(),
        width: width as i32,
        height: height as i32,
        seed: body.seed.unwrap_or(-1),
        ref_images: references,
        // Пресет и LoRA включаются только в режиме правки: на свежем кадре референсов нет.
        ref_image_args: is_edit.then(|| "preset=krea2_edit".to_string()),
        loras: if is_edit && lora.is_file() {
            vec![LoraSpec { path: lora, multiplier: 1.0 }]
        } else {
            Vec::new()
        },
        ..Default::default()
    };

    let gpu = state.0.clone();
    let runtime = crate::runtime::load(&state.root);
    let in_cloud = crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Image);
    let started = std::time::Instant::now();

    // Облачный кадр рисует чужой сервер — очередь своей карты он не занимает и в неё НЕ
    // ставится: раньше он попадал и сюда, и в очередь, то есть рисовался дважды — двойная
    // задержка и двойная плата.
    // Чем подписать кадр: имя облачной модели или своего движка — но не то, что осталось
    // в настройках карты.
    let rendered_by = if in_cloud {
        runtime.openrouter_image_model.clone()
    } else {
        format!("{:?}", body.settings.image_backend).to_lowercase()
    };
    let value = if in_cloud {
        // Прошлый кадр этой же сцены уходит в облако референсом: без него каждая
        // иллюстрация рисуется с нуля и сцена «переезжает» из кадра в кадр.
        let refs = reference_paths(&state, &decision.references);
        draw_in_cloud(gpu, runtime, params.prompt.clone(), (width, height), refs)
            .await
            .map_err(ApiError::internal)?
    } else {
        let (_, result) = state
            .queue
            .enqueue_awaitable(Box::new(move |progress| {
                let sink = progress.clone();
                let images = gpu
                    .gpu
                    .generate_image(
                        &params,
                        Some(Box::new(move |step, steps, _| {
                            sink(json!({ "stage": "image", "step": step, "steps": steps }));
                        })),
                    )
                    .map_err(|error| error.to_string())?;
                let image = images.into_iter().next().ok_or("движок не вернул кадр")?;
                let name = format!("{}.png", uuid::Uuid::new_v4());
                let path = gpu.generated.join(&name);
                write_png(&path, &image).map_err(|error| error.to_string())?;
                Ok(json!({ "url": format!("/generated/{name}"), "width": image.width, "height": image.height }))
            }))
            .await;
        result
            .await
            .map_err(|_| ApiError::internal("задача пропала"))?
            .map_err(ApiError::internal)?
    };

    let url = value["url"].as_str().unwrap_or_default().to_string();
    let image = GeneratedImage {
        id: uuid::Uuid::new_v4().to_string(),
        url: url.clone(),
        prompt,
        mode: body.settings.image_mode,
        backend: Some(body.settings.image_backend),
        aspect: body.settings.aspect,
        width: value["width"].as_u64().unwrap_or(width as u64) as u32,
        height: value["height"].as_u64().unwrap_or(height as u64) as u32,
        elapsed_seconds: Some(started.elapsed().as_secs_f32()),
        seed: body.seed,
        warnings: Vec::new(),
        scene_location: (!decision.location.is_empty()).then(|| decision.location.clone()),
        edited_from: decision.edit_from.as_ref().map(|from| from.url.clone()),
        rendered_by: Some(rendered_by),
    };

    if let Some(message_id) = body.message_id.as_deref() {
        state.store.set_message_image(message_id, &image)?;
    }

    // Учёт непрерывности делаем после выдачи кадра и не роняем ответ, если он не удался.
    if let Some(chat_id) = chat_id.as_deref() {
        record_continuity(&state, chat_id, &decision, &image, &body.prompt);
    }

    Ok(Json(image))
}

/// Нарисовать кадр в облаке, не занимая очередь своей карты.
///
/// Один путь на все кадры — сцену, портрет, иконку предмета: иначе какой-нибудь из них
/// продолжает молча грузить видеокарту, хотя игрок увёл всё в облако.
async fn draw_in_cloud(
    inner: std::sync::Arc<crate::state::Inner>,
    runtime: crate::runtime::Runtime,
    prompt: String,
    size: (u32, u32),
    references: Vec<std::path::PathBuf>,
) -> Result<Value, String> {
    let (width, height) = size;
    tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let path = inner.generated.join(format!("{}.png", uuid::Uuid::new_v4()));
        // Имя может смениться: формат кадра выбирает провайдер.
        let path =
            crate::cloud::draw(&inner.root, &runtime, &prompt, (width, height), &references, &path)?;
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        Ok(json!({ "url": format!("/generated/{name}"), "width": width, "height": height }))
    })
    .await
    .map_err(|_| "задача пропала".to_string())?
}

/// Запомнить кадр за локацией, обновить облик показанных персонажей и портреты предметов.
fn record_continuity(
    state: &AppState,
    chat_id: &str,
    decision: &scene::SceneDecision,
    image: &GeneratedImage,
    prompt: &str,
) {
    let attachment = Attachment {
        id: image.id.clone(),
        name: if decision.location.is_empty() {
            "кадр сцены".to_string()
        } else {
            format!("сцена: {}", decision.location)
        },
        kind: "image/png".into(),
        url: image.url.clone(),
        data_url: None,
    };

    if !decision.location.is_empty() {
        let _ = state.store.record_scene_image(
            chat_id,
            &decision.location,
            &attachment,
            !decision.is_edit(),
        );
    }

    // Облик обновляем герою всегда, а второстепенному персонажу — только когда он в кадре
    // один: групповая сцена иначе затёрла бы его чистый портрет.
    if let Ok(Some(hero)) = state.store.hero(chat_id) {
        let _ = state.store.set_character_reference(chat_id, &hero.id, &attachment);
    }

    if let Ok(items) = scene::items_mentioned_in(&state.store, chat_id, prompt) {
        for item in items {
            let _ = state.store.set_item_image(chat_id, &item.id, &image.url);
        }
    }
}

/// Пути к файлам референсов — как есть, без разбора картинки.
///
/// Локальному движку нужны распакованные пиксели, а облаку — сам файл: он уходит туда
/// строкой data-url. Разбирать и собирать картинку обратно ради этого незачем.
fn reference_paths(state: &AppState, references: &[Attachment]) -> Vec<std::path::PathBuf> {
    references
        .iter()
        .filter_map(|reference| {
            let relative = reference.url.trim_start_matches('/');
            let path = if let Some(name) = relative.strip_prefix("generated/") {
                state.generated.join(name)
            } else if let Some(name) = relative.strip_prefix("uploads/") {
                state.uploads.join(name)
            } else {
                return None;
            };
            path.is_file().then_some(path)
        })
        .collect()
}

/// Подгрузить пиксели референсов с диска. Недоступный файл просто пропускаем: кадр важнее.
fn load_references(state: &AppState, references: &[Attachment]) -> Vec<RawImage> {
    references
        .iter()
        .filter_map(|reference| {
            let relative = reference.url.trim_start_matches('/');
            let path = if let Some(name) = relative.strip_prefix("generated/") {
                state.generated.join(name)
            } else if let Some(name) = relative.strip_prefix("uploads/") {
                state.uploads.join(name)
            } else {
                return None;
            };
            read_png(&path).ok()
        })
        .collect()
}

pub fn read_png(path: &std::path::Path) -> Result<RawImage, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
    buffer.truncate(info.buffer_size());
    let data = match info.color_type {
        png::ColorType::Rgb => buffer,
        png::ColorType::Rgba => buffer.chunks_exact(4).flat_map(|px| px[..3].to_vec()).collect(),
        other => return Err(format!("не поддержанный цвет PNG: {other:?}")),
    };
    Ok(RawImage { width: info.width, height: info.height, channels: 3, data })
}

pub fn write_png(path: &std::path::Path, image: &RawImage) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(if image.channels == 4 { png::ColorType::Rgba } else { png::ColorType::Rgb });
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&image.data)
        .map_err(|e| e.to_string())
}

// ── Докачка моделей ────────────────────────────────────────────────────────────

pub async fn setup_status(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "components": crate::setup::status(&state.root) }))
}

#[derive(Deserialize)]
pub struct SetupDownload {
    /// Что качать; пусто — всё недостающее.
    #[serde(default)]
    ids: Vec<String>,
}

/// Закачка идёт задачей очереди и стримит прогресс: файлы измеряются гигабайтами, и
/// молчащий интерфейс выглядел бы зависшим.
pub async fn setup_download(
    State(state): State<AppState>,
    Json(body): Json<SetupDownload>,
) -> ApiResult<Json<Value>> {
    let inner = state.0.clone();
    let job = state
        .queue
        .enqueue(Box::new(move |progress| {
            let abort = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            for component in crate::setup::manifest() {
                if !body.ids.is_empty() && !body.ids.iter().any(|id| id == component.id) {
                    continue;
                }
                let title = component.title;
                let report = |have: u64, total: u64| {
                    progress(json!({
                        "stage": "загрузка",
                        "component": title,
                        "have": have,
                        "total": total,
                    }));
                };
                crate::setup::download_component(&inner.root, &component, &report, abort.clone())
                    .map_err(|error| format!("{title}: {error}"))?;
            }
            Ok(json!({ "ok": true }))
        }))
        .await;
    Ok(Json(json!({ "job": job })))
}

// ── Рантайм-настройки ──────────────────────────────────────────────────────────

pub async fn runtime_get(State(state): State<AppState>) -> Json<Value> {
    let mut runtime = crate::runtime::load(&state.root);
    // Ключ наружу не отдаём: интерфейсу достаточно знать, задан он или нет.
    let has_key = !runtime.openrouter_key.trim().is_empty();
    runtime.openrouter_key = String::new();
    Json(json!({ "runtime": runtime, "openrouterKeySet": has_key }))
}

pub async fn runtime_put(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    // Правка приходит ЧАСТИЧНОЙ: панель шлёт один тронутый переключатель. Разобрать её
    // сразу в настройки нельзя — недостающие поля встали бы умолчаниями и стёрли всё
    // остальное: так однажды слетела вся облачная настройка разом.
    let current = crate::runtime::load(&state.root);
    let mut merged = serde_json::to_value(&current)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    if let (Some(target), Some(patch)) = (merged.as_object_mut(), body.as_object()) {
        for (key, value) in patch {
            target.insert(key.clone(), value.clone());
        }
    }
    let parsed: crate::runtime::Runtime = serde_json::from_value(merged)
        .map_err(|error| ApiError::bad_request(format!("непонятная настройка: {error}")))?;
    let mut next = crate::runtime::sanitize(parsed);
    // Сменил модель озвучки — голос от прежней у новой не существует, и синтез падал бы на
    // каждой реплике. Если голос не из её набора, подставляем подходящий сами.
    let voices = crate::voice_catalog::suitable(&next.openrouter_tts_model, true, None);
    if !voices.is_empty()
        && !voices.iter().any(|voice| voice.name.eq_ignore_ascii_case(next.openrouter_tts_voice.trim()))
    {
        next.openrouter_tts_voice = voices[0].name.clone();
    }
    // Пустой ключ в запросе означает «не менять»: интерфейс его не получает и вернуть не может.
    if next.openrouter_key.trim().is_empty() {
        next.openrouter_key = crate::runtime::load(&state.root).openrouter_key;
    }
    crate::runtime::save(&state.root, &next).map_err(ApiError::internal)?;
    let has_key = !next.openrouter_key.trim().is_empty();
    next.openrouter_key = String::new();
    Ok(Json(json!({ "runtime": next, "openrouterKeySet": has_key })))
}

/// Снимок железа для монитора: карта, её память и загрузка, оперативная память.
pub async fn hardware() -> Json<Value> {
    Json(serde_json::to_value(crate::hw::snapshot()).unwrap_or_else(|_| json!({})))
}

/// Голоса облачной модели с полом, возрастом и пригодностью для русского. Справочник вшит в
/// приложение, поэтому список есть ещё до первого обращения к сети.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudVoicesQuery {
    #[serde(default)]
    model: String,
    #[serde(default)]
    russian: bool,
    #[serde(default)]
    gender: Option<String>,
}

pub async fn cloud_voices(Query(query): Query<CloudVoicesQuery>) -> Json<Value> {
    if query.model.trim().is_empty() {
        return Json(json!({ "models": crate::voice_catalog::models() }));
    }
    let suitable = crate::voice_catalog::suitable(
        &query.model,
        query.russian,
        query.gender.as_deref().filter(|gender| !gender.is_empty()),
    );
    Json(json!({
        "model": query.model,
        "supportsRussian": crate::voice_catalog::supports_russian(&query.model),
        "voices": suitable,
    }))
}

/// Живой список облачных моделей под назначение. Вшивать его нельзя — он протухает: в
/// перенесённом из даб-студии справочнике ни одной из одиннадцати моделей озвучки уже нет.
#[derive(Deserialize)]
pub struct CloudModelsQuery {
    #[serde(default)]
    kind: String,
}

pub async fn cloud_model_list(
    State(state): State<AppState>,
    Query(query): Query<CloudModelsQuery>,
) -> ApiResult<Json<Value>> {
    let kind = crate::cloud_models::Kind::parse(&query.kind)
        .ok_or_else(|| ApiError::bad_request("укажи назначение: text, image, tts или stt"))?;
    let key = crate::runtime::load(&state.root).openrouter_key;
    if key.trim().is_empty() {
        return Err(ApiError::bad_request("ключ OpenRouter не задан"));
    }
    let models = crate::cloud_models::list(&key, kind).map_err(ApiError::internal)?;
    let voices = if matches!(kind, crate::cloud_models::Kind::Tts) {
        json!(models
            .iter()
            .map(|model| (model.id.clone(), crate::cloud_models::voices_for(&model.id)))
            .collect::<std::collections::BTreeMap<_, _>>())
    } else {
        Value::Null
    };
    Ok(Json(json!({ "models": models, "voices": voices })))
}

// ── Портрет предмета ───────────────────────────────────────────────────────────

/// Нарисовать иконку предмета и привязать её к нему. Портрет потом переиспользуется как
/// референс, чтобы знакомая вещь выглядела в сценах одинаково.
pub async fn item_image(
    State(state): State<AppState>,
    Path((chat_id, item_id)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let item = state
        .store
        .get_item(&chat_id, &item_id)?
        .ok_or_else(|| ApiError::not_found("предмет не найден"))?;
    if item.image_url.is_some() {
        return Ok(Json(json!({ "item": item, "cached": true })));
    }

    // Промпт даёт модель при выдаче лута; если его нет — собираем из названия и описания.
    let prompt = item.image_prompt_en.clone().unwrap_or_else(|| {
        let description = item.description.clone().unwrap_or_default();
        format!(
            "a single {} item on a plain dark background, game inventory icon, centered,              dramatic rim light, painterly fantasy illustration. {description}",
            item.name
        )
    });
    let prompt = finalize_scene_prompt(&prompt, "");

    let inner = state.0.clone();
    // Иконка мелкая: гнать её в полный размер сцены незачем.
    let params = GenParams { prompt, width: 768, height: 768, ..Default::default() };

    // Иконка предмета — такой же кадр, как и все: если кадры уведены в облако, рисовать её
    // на своей карте нельзя. Раньше этот путь про облако не знал вовсе, и видеокарта
    // просыпалась на каждой находке, хотя игрок выбрал «всё в облаке».
    let runtime = crate::runtime::load(&state.root);
    if crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Image) {
        // Иконке предмета референс не нужен: это отдельная вещь на пустом фоне.
        let value = draw_in_cloud(inner, runtime, params.prompt.clone(), (768, 768), Vec::new())
            .await
            .map_err(ApiError::internal)?;
        return finish_item_image(state, &chat_id, &item_id, value).await;
    }

    let (_, result) = state
        .queue
        .enqueue_awaitable(Box::new(move |_| {
            let images = inner
                .gpu
                .generate_image(&params, None)
                .map_err(|error| error.to_string())?;
            let image = images.into_iter().next().ok_or("движок не вернул кадр")?;
            let name = format!("item-{}.png", uuid::Uuid::new_v4());
            write_png(&inner.generated.join(&name), &image).map_err(|error| error.to_string())?;
            Ok(json!({ "url": format!("/generated/{name}") }))
        }))
        .await;

    let value = result
        .await
        .map_err(|_| ApiError::internal("задача пропала"))?
        .map_err(ApiError::internal)?;
    finish_item_image(state, &chat_id, &item_id, value).await
}

/// Сохранить готовую иконку за предметом — общий хвост для облака и своей карты.
async fn finish_item_image(
    state: AppState,
    chat_id: &str,
    item_id: &str,
    value: Value,
) -> ApiResult<Json<Value>> {
    let url = value["url"].as_str().unwrap_or_default().to_string();
    let item = state
        .store
        .set_item_image(chat_id, item_id, &url)?
        .ok_or_else(|| ApiError::not_found("предмет пропал"))?;
    Ok(Json(json!({ "item": item, "cached": false })))
}

// ── Движок картинок ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct WorkerAction {
    action: String,
}

/// Управление движком картинок. Отдельного процесса у нас нет — движок живёт в приложении,
/// поэтому «запустить» означает прогреть его и честно ответить, чего не хватает.
pub async fn image_worker(
    State(state): State<AppState>,
    Json(body): Json<WorkerAction>,
) -> ApiResult<Json<Value>> {
    let paths = state.gpu.paths();
    match body.action.as_str() {
        "start" | "status" => {
            let runtime = crate::runtime::load(&state.root);
            // Кадры уведены в облако — свой движок не поднимаем даже ради ответа.
            if crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Image) {
                return Ok(Json(json!({
                    "message": format!("Кадры рисует облако: {}.", runtime.openrouter_image_model),
                    "health": { "ok": true, "loaded": false, "inCloud": true },
                })));
            }
            let (engine, devices) = state.gpu.image_info().unwrap_or_default();
            let ready = paths.image_ready();
            Ok(Json(json!({
                "message": if ready {
                    "Движок картинок готов."
                } else {
                    "Весов картинок нет — скачайте их на вкладке загрузки."
                },
                "health": {
                    "ok": ready,
                    // «Загружен» — это про то, занята ли карта, а не про наличие весов.
                    "loaded": !engine.is_empty(),
                    "engine": engine,
                    "devices": devices,
                },
            })))
        }
        "stop" => Ok(Json(json!({
            "message": "Движок освобождает карту после каждого кадра — останавливать нечего.",
            "health": { "ok": paths.image_ready(), "loaded": false },
        }))),
        "open-model-folder" => {
            let path = paths.image_models.clone();
            // Открываем каталог тем, чем открывает система: свой файловый менеджер у нас не нужен.
            #[cfg(windows)]
            let _ = std::process::Command::new("explorer").arg(&path).spawn();
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(&path).spawn();
            #[cfg(all(unix, not(target_os = "macos")))]
            let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
            Ok(Json(json!({ "path": path.to_string_lossy(), "message": "Папка модели открыта." })))
        }
        other => Err(ApiError::bad_request(format!("неизвестное действие: {other}"))),
    }
}
