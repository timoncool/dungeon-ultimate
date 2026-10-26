//! Ошибки, которые видит игрок.
//!
//! У каждой есть код и поля: окно переводит её на язык игры по коду (`frontend/src/lib/ui-text-errors.ts`),
//! а русский текст остаётся для журнала и статистики хаба — по нему сбои и группируются, поэтому
//! он совпадает с прежним дословно. Ошибка без своего вида едет как `Raw`: окно показывает её
//! текст как есть, и это видно, а не прячется.

use std::fmt;

use serde_json::{json, Map, Value};

/// Стадия, которую игра умеет считать в облаке.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudStage {
    Narrator,
    Image,
    Tts,
    Asr,
}

impl CloudStage {
    pub fn key(self) -> &'static str {
        match self {
            CloudStage::Narrator => "narrator",
            CloudStage::Image => "image",
            CloudStage::Tts => "tts",
            CloudStage::Asr => "asr",
        }
    }

    fn ru(self) -> &'static str {
        match self {
            CloudStage::Narrator => "рассказчик",
            CloudStage::Image => "картинки",
            CloudStage::Tts => "озвучка",
            CloudStage::Asr => "распознавание речи",
        }
    }

    /// Как стадию называла прежняя строка ошибки облачного провайдера.
    fn ru_call(self) -> &'static str {
        match self {
            CloudStage::Narrator => "облачный рассказчик",
            CloudStage::Image => "облачный кадр",
            CloudStage::Tts => "облачная озвучка",
            CloudStage::Asr => "облачное распознавание",
        }
    }
}

impl From<crate::cloud::Stage> for CloudStage {
    fn from(stage: crate::cloud::Stage) -> Self {
        match stage {
            crate::cloud::Stage::Narrator => CloudStage::Narrator,
            crate::cloud::Stage::Image => CloudStage::Image,
            crate::cloud::Stage::Tts => CloudStage::Tts,
            crate::cloud::Stage::Asr => CloudStage::Asr,
        }
    }
}

#[derive(Debug)]
pub enum UserError {
    /// Ошибка без своего вида: окно показывает её текст как есть.
    Raw(String),
    JobLost,
    JobCrashed(String),
    TurnAborted,
    HeroDead,
    NarratorOnlyArtifacts,
    /// Сбой текстовой модели; `starting` — при её запуске на своей карте.
    Llm { error: du_llm::LlmError, starting: bool },
    TextModelMissing(String),
    LocalServerUrlEmpty,
    AgentNotConnected,
    CloudKeyMissing(CloudStage),
    CloudModelMissing(CloudStage),
    /// Провайдер облака ответил отказом; `message` — его собственные слова.
    CloudRefused { stage: CloudStage, status: u16, message: Option<String> },
    CloudNetwork { stage: CloudStage, detail: String },
    /// Провайдер ответил без кода ошибки, но словами о сбое.
    CloudSaid { stage: CloudStage, message: String },
    CloudEmpty(CloudStage),
    CloudTooSmall { stage: CloudStage, bytes: usize },
    GoogleKeyMissing,
    OpenRouterKeyNotSet,
    VoiceMissing(String),
    NoVoices,
    TtsEngineMissing,
    Tts(du_tts::EngineError),
    VoiceReference(String),
    TtsEmptyText,
    TtsNothing,
    ImageWeightsMissing(String),
    Image(du_image::ImageError),
    ImageNoFrame,
    AsrFailed(String),
    WhisperBinMissing(String),
    WhisperModelMissing { model: String, path: String },
    WhisperCudaMissing,
    ParakeetModelMissing(String),
    /// Нет сборки transcribe.cpp; `gpu` — нужна была сборка для видеокарты.
    AsrEngineMissing { path: String, gpu: bool },
    ParakeetCudaLibMissing { name: String, path: String },
    /// Стадия считается только на видеокарте, а её нет.
    NeedsGpu(CloudStage),
    /// Движок стадии собран только под CUDA, а карта не NVIDIA (или CUDA на ней не идёт).
    NeedsNvidia(CloudStage),
    /// Карты из настроек нет в компьютере: CUDA осталась бы без устройства, и движок молча ушёл бы на процессор.
    /// `cards` — карты NVIDIA, которые есть («0: NVIDIA GeForce RTX 4090»).
    GpuMissing { index: u32, cards: Vec<String> },
    /// Процессор без набора команд, под который собран движок; текст называет недостающие.
    CpuFloor(String),
    RecordingEmpty,
    NoAudioFile,
    NoFile,
    ChatNotFound,
    ChatNotCreated,
    MessageNotFound,
    ItemNotFound,
    ItemGone,
    QuestNotFound,
    CharacterNotFound,
    CharacterNameRequired,
    UnknownField(String),
    UnknownAction(String),
    BadSetting(String),
    /// Закачка сорвалась; `kind` — тот же код, что уходит в хаб (`setup::failure_code`).
    Download { kind: &'static str, detail: String },
    /// Места на томе игры меньше, чем займут выбранные компоненты: байты нужно и свободно, путь тома.
    DiskSpace { need: u64, free: u64, path: String },
}

impl UserError {
    /// Код, по которому окно находит перевод.
    pub fn code(&self) -> &'static str {
        match self {
            UserError::Raw(_) => "raw",
            UserError::JobLost => "job_lost",
            UserError::JobCrashed(_) => "job_crashed",
            UserError::TurnAborted => "turn_aborted",
            UserError::HeroDead => "hero_dead",
            UserError::NarratorOnlyArtifacts => "narrator_only_artifacts",
            UserError::Llm { error, .. } => llm_code(error),
            UserError::TextModelMissing(_) => "text_model_missing",
            UserError::LocalServerUrlEmpty => "local_server_url_empty",
            UserError::AgentNotConnected => "agent_not_connected",
            UserError::CloudKeyMissing(_) => "cloud_key_missing",
            UserError::CloudModelMissing(_) => "cloud_model_missing",
            UserError::CloudRefused { message: Some(_), .. } => "cloud_refused",
            UserError::CloudRefused { message: None, .. } => "cloud_refused_silent",
            UserError::CloudNetwork { .. } => "cloud_network",
            UserError::CloudSaid { .. } => "cloud_said",
            UserError::CloudEmpty(_) => "cloud_empty",
            UserError::CloudTooSmall { .. } => "cloud_too_small",
            UserError::GoogleKeyMissing => "google_key_missing",
            UserError::OpenRouterKeyNotSet => "openrouter_key_not_set",
            UserError::VoiceMissing(_) => "voice_missing",
            UserError::NoVoices => "no_voices",
            UserError::TtsEngineMissing => "tts_engine_missing",
            UserError::Tts(error) => match error {
                du_tts::EngineError::LibraryLoad(_) => "tts_engine_load",
                du_tts::EngineError::Cancelled => "tts_cancelled",
                du_tts::EngineError::Generation(_) => "tts_failed",
                du_tts::EngineError::InvalidParam(_) => "tts_bad_param",
            },
            UserError::VoiceReference(_) => "voice_reference",
            UserError::TtsEmptyText => "tts_empty_text",
            UserError::TtsNothing => "tts_nothing",
            UserError::ImageWeightsMissing(_) => "image_weights_missing",
            UserError::Image(error) => match error {
                du_image::ImageError::LibraryMissing(_) => "image_engine_missing",
                du_image::ImageError::Library { .. } => "image_engine_load",
                du_image::ImageError::BadPath(_) => "image_bad_path",
                du_image::ImageError::ContextCreation(_) => "image_context",
                du_image::ImageError::GenerationFailed(_) => "image_failed",
            },
            UserError::ImageNoFrame => "image_no_frame",
            UserError::AsrFailed(_) => "asr_failed",
            UserError::WhisperBinMissing(_) => "whisper_bin_missing",
            UserError::WhisperModelMissing { .. } => "whisper_model_missing",
            UserError::WhisperCudaMissing => "whisper_cuda_missing",
            UserError::ParakeetModelMissing(_) => "parakeet_model_missing",
            UserError::AsrEngineMissing { gpu: false, .. } => "asr_engine_missing",
            UserError::AsrEngineMissing { gpu: true, .. } => "asr_engine_gpu_missing",
            UserError::ParakeetCudaLibMissing { .. } => "parakeet_cuda_lib_missing",
            UserError::CpuFloor(_) => "cpu_floor",
            UserError::NeedsGpu(_) => "needs_gpu",
            UserError::NeedsNvidia(_) => "needs_nvidia",
            UserError::GpuMissing { cards, .. } if cards.is_empty() => "gpu_missing_no_nvidia",
            UserError::GpuMissing { .. } => "gpu_missing",
            UserError::RecordingEmpty => "recording_empty",
            UserError::NoAudioFile => "no_audio_file",
            UserError::NoFile => "no_file",
            UserError::ChatNotFound => "chat_not_found",
            UserError::ChatNotCreated => "chat_not_created",
            UserError::MessageNotFound => "message_not_found",
            UserError::ItemNotFound => "item_not_found",
            UserError::ItemGone => "item_gone",
            UserError::QuestNotFound => "quest_not_found",
            UserError::CharacterNotFound => "character_not_found",
            UserError::CharacterNameRequired => "character_name_required",
            UserError::UnknownField(_) => "unknown_field",
            UserError::UnknownAction(_) => "unknown_action",
            UserError::BadSetting(_) => "bad_setting",
            UserError::Download { kind, .. } => match *kind {
                "hash_mismatch" => "download_hash_mismatch",
                "network" => "download_network",
                "archive" => "download_archive",
                "disk" => "download_disk",
                "disk_space" => "download_disk_space",
                _ => "download_other",
            },
            UserError::DiskSpace { .. } => "disk_space",
        }
    }

    /// Поля, которые подставляются в перевод: имена, пути и собственные слова движков.
    pub fn params(&self) -> Map<String, Value> {
        let mut params = Map::new();
        let mut put = |key: &str, value: Value| {
            params.insert(key.to_string(), value);
        };
        match self {
            UserError::Raw(text) => put("text", json!(text)),
            UserError::JobCrashed(detail)
            | UserError::VoiceReference(detail)
            | UserError::AsrFailed(detail)
            | UserError::BadSetting(detail)
            | UserError::CpuFloor(detail) => put("detail", json!(detail)),
            UserError::Llm { error, .. } => llm_params(error, &mut put),
            UserError::TextModelMissing(path)
            | UserError::ImageWeightsMissing(path)
            | UserError::WhisperBinMissing(path)
            | UserError::ParakeetModelMissing(path)
            | UserError::AsrEngineMissing { path, .. } => put("path", json!(path)),
            UserError::WhisperModelMissing { model, path } => {
                put("model", json!(model));
                put("path", json!(path));
            }
            UserError::ParakeetCudaLibMissing { name, path } => {
                put("name", json!(name));
                put("path", json!(path));
            }
            UserError::CloudKeyMissing(stage)
            | UserError::CloudModelMissing(stage)
            | UserError::CloudEmpty(stage)
            | UserError::NeedsGpu(stage)
            | UserError::NeedsNvidia(stage) => {
                put("stage", json!(stage.key()))
            }
            UserError::GpuMissing { index, cards } => {
                put("index", json!(index));
                put("cards", json!(cards.join(", ")));
            }
            UserError::CloudSaid { stage, message } => {
                put("stage", json!(stage.key()));
                put("detail", json!(message));
            }
            UserError::CloudTooSmall { stage, bytes } => {
                put("stage", json!(stage.key()));
                put("bytes", json!(bytes));
            }
            UserError::CloudRefused { stage, status, message } => {
                put("stage", json!(stage.key()));
                put("status", json!(status));
                if let Some(message) = message {
                    put("detail", json!(message));
                }
            }
            UserError::CloudNetwork { stage, detail } => {
                put("stage", json!(stage.key()));
                put("detail", json!(detail));
            }
            UserError::VoiceMissing(voice) => put("voice", json!(voice)),
            UserError::Tts(error) => match error {
                du_tts::EngineError::LibraryLoad(detail)
                | du_tts::EngineError::Generation(detail)
                | du_tts::EngineError::InvalidParam(detail) => put("detail", json!(detail)),
                du_tts::EngineError::Cancelled => {}
            },
            UserError::Image(error) => match error {
                du_image::ImageError::LibraryMissing(path) => put("path", json!(path.display().to_string())),
                du_image::ImageError::Library { dll, detail } => put("detail", json!(format!("{dll}: {detail}"))),
                du_image::ImageError::BadPath(detail)
                | du_image::ImageError::ContextCreation(detail)
                | du_image::ImageError::GenerationFailed(detail) => put("detail", json!(detail)),
            },
            UserError::UnknownField(field) => put("field", json!(field)),
            UserError::UnknownAction(action) => put("action", json!(action)),
            UserError::Download { detail, .. } => put("detail", json!(detail)),
            UserError::DiskSpace { need, free, path } => {
                put("need", json!(gigabytes(*need)));
                put("free", json!(gigabytes(*free)));
                put("path", json!(path));
            }
            _ => {}
        }
        params
    }

    /// Тело ответа окну: прежнее поле `error` и рядом код с полями для перевода.
    pub fn to_json(&self) -> Value {
        json!({ "error": self.to_string(), "code": self.code(), "params": self.params() })
    }
}

fn llm_code(error: &du_llm::LlmError) -> &'static str {
    use du_llm::{EmptyStream, LlmError};
    match error {
        LlmError::BinMissing(_) => "llm_bin_missing",
        LlmError::ModelMissing(_) => "llm_model_missing",
        LlmError::Spawn(_) => "llm_spawn",
        LlmError::Http(_) | LlmError::StreamSend(_) => "llm_network",
        LlmError::StreamBroken(_) => "llm_stream_broken",
        LlmError::Status { code: 401 | 403, .. } => "llm_key_rejected",
        LlmError::Status { .. } => "llm_status",
        LlmError::Api(_) => "llm_api",
        LlmError::Empty(why) => match why {
            EmptyStream::ToolCall { .. } => "llm_tool_call",
            EmptyStream::OnlyThinking { .. } => "llm_only_thinking",
            EmptyStream::Filtered => "llm_filtered",
            EmptyStream::Finished { .. } => "llm_empty",
            EmptyStream::NoReason => "llm_empty_no_reason",
            EmptyStream::Cut => "llm_cut",
        },
    }
}

/// Байты в гигабайтах с одной цифрой после точки — так их показывает «Что скачать».
fn gigabytes(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / (1u64 << 30) as f64)
}

fn llm_params(error: &du_llm::LlmError, put: &mut impl FnMut(&str, Value)) {
    use du_llm::{EmptyStream, LlmError};
    match error {
        LlmError::BinMissing(path) | LlmError::ModelMissing(path) => put("path", json!(path.display().to_string())),
        LlmError::Spawn(detail)
        | LlmError::Http(detail)
        | LlmError::StreamSend(detail)
        | LlmError::StreamBroken(detail)
        | LlmError::Api(detail) => put("detail", json!(detail)),
        LlmError::Status { code, detail } => {
            put("status", json!(code));
            put("detail", json!(detail));
        }
        LlmError::Empty(why) => match why {
            EmptyStream::ToolCall { finish } | EmptyStream::Finished { finish } => put("finish", json!(finish)),
            EmptyStream::OnlyThinking { chars, finish } => {
                put("chars", json!(chars));
                put("finish", json!(finish));
            }
            EmptyStream::Filtered | EmptyStream::NoReason | EmptyStream::Cut => {}
        },
    }
}

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserError::Raw(text) => f.write_str(text),
            UserError::JobLost => f.write_str("задача пропала"),
            UserError::JobCrashed(detail) => write!(f, "задача упала: {detail}"),
            UserError::TurnAborted => f.write_str("ход прерван"),
            UserError::HeroDead => {
                f.write_str("Герой погиб — история окончена. Отмени последний ход или начни новую историю.")
            }
            UserError::NarratorOnlyArtifacts => {
                f.write_str("нарратор написал только служебные вставки, прозы после их чистки не осталось")
            }
            UserError::Llm { error, starting: true } => write!(f, "текстовая модель: {error}"),
            UserError::Llm { error, starting: false } => write!(f, "{error}"),
            UserError::TextModelMissing(path) => {
                write!(
                    f,
                    "не хватает файлов модели: нет модели нарратора ({path}) или llama-server — скачайте их в «Что скачать» или уведите рассказчика в облако в «Движки и облако»"
                )
            }
            UserError::AgentNotConnected => f.write_str(
                "рассказчик — агент, но агент не подключён к MCP игры: подключите его или выберите рассказчика на своей карте",
            ),
            UserError::LocalServerUrlEmpty => {
                f.write_str("в настройках истории выбран локальный сервер, но его адрес пуст: впишите адрес сервера")
            }
            UserError::CloudKeyMissing(stage) => write!(
                f,
                "{} включён в облаке, но ключ OpenRouter не задан — впишите ключ или выключите облако для этой стадии",
                stage.ru()
            ),
            UserError::CloudModelMissing(stage) => write!(
                f,
                "{} включён в облаке, но модель не выбрана — выберите модель или выключите облако для этой стадии",
                stage.ru()
            ),
            UserError::CloudRefused { stage, status, message: Some(message) } => {
                write!(f, "{}: {message} ({status})", stage.ru_call())
            }
            UserError::CloudRefused { stage, status, message: None } => {
                write!(f, "{}: провайдер отказал ({status})", stage.ru_call())
            }
            UserError::CloudNetwork { stage, detail } => write!(f, "{}: {detail}", stage.ru_call()),
            UserError::CloudSaid { stage, message } => write!(f, "{}: {message}", stage.ru_call()),
            UserError::CloudEmpty(CloudStage::Asr) => f.write_str("облачное распознавание вернуло пустой текст"),
            UserError::CloudEmpty(CloudStage::Image) => f.write_str("облачный кадр: модель не вернула картинку"),
            UserError::CloudEmpty(stage) => write!(f, "{}: пустой ответ", stage.ru_call()),
            UserError::CloudTooSmall { stage, bytes } => write!(f, "{}: слишком мало данных ({bytes} байт)", stage.ru_call()),
            UserError::GoogleKeyMissing => f.write_str(
                "озвучка Google включена, но ключа нет — задайте ключ Google в настройках озвучки или GEMINI_API_KEY",
            ),
            UserError::OpenRouterKeyNotSet => f.write_str("ключ OpenRouter не задан"),
            UserError::VoiceMissing(voice) => write!(
                f,
                "голоса «{voice}» нет на диске: поставьте «Пак голосов» в Настройках → Модели, добавьте этот голос или выберите другой"
            ),
            UserError::NoVoices => f.write_str(
                "голосов для озвучки нет: поставьте «Пак голосов» в Настройках → Модели или добавьте свой голос",
            ),
            UserError::TtsEngineMissing => f.write_str("движок озвучки не установлен"),
            UserError::Tts(error) => write!(f, "{error}"),
            UserError::VoiceReference(detail) => write!(f, "не удалось подготовить эталон голоса: {detail}"),
            UserError::TtsEmptyText => f.write_str("пустой текст для озвучки"),
            UserError::TtsNothing => f.write_str("нечего озвучивать"),
            UserError::ImageWeightsMissing(path) => write!(
                f,
                "не хватает файлов модели: нет весов картинок в {path} — скачайте их в «Что скачать» или уведите кадр в облако в «Движки и облако»"
            ),
            UserError::Image(error) => write!(f, "движок картинок: {error}"),
            UserError::ImageNoFrame => f.write_str("движок не вернул кадр"),
            UserError::AsrFailed(detail) => write!(f, "Parakeet не распознал запись: {detail}"),
            UserError::WhisperBinMissing(path) => write!(
                f,
                "выбран Whisper, но его программы нет в {path} — скачайте вариант Whisper в «Что скачать» или выберите Parakeet"
            ),
            UserError::WhisperModelMissing { model, path } => {
                write!(f, "выбран Whisper {model}, но модели нет в {path} — скачайте её в «Что скачать»")
            }
            UserError::WhisperCudaMissing => f.write_str(
                "Whisper должен считать на видеокарте, но библиотек CUDA для него нет — скачайте «Whisper на видеокарте» в «Что скачать» или переключите распознавание на процессор",
            ),
            UserError::ParakeetModelMissing(path) => write!(
                f,
                "нет модели распознавания {path} — либо скачайте её в «Что скачать», либо включите облако"
            ),
            UserError::AsrEngineMissing { path, gpu: false } => {
                write!(f, "нет движка распознавания в {path} — скачайте Parakeet в «Что скачать»")
            }
            UserError::AsrEngineMissing { path, gpu: true } => write!(
                f,
                "распознавание выбрано на видеокарте, а сборки для неё нет в {path} — скачайте «Parakeet на видеокарте» в «Что скачать» или переключите распознавание на процессор"
            ),
            UserError::ParakeetCudaLibMissing { name, path } => write!(
                f,
                "Parakeet на видеокарте берёт {name} у движка озвучки, а его нет в {path} — скачайте «Parakeet на видеокарте» в «Что скачать» или переключите распознавание на процессор"
            ),
            UserError::CpuFloor(detail) => write!(f, "процессор не подходит: {detail}"),
            UserError::NeedsGpu(stage) => write!(f, "{}: считается только на видеокарте, а её в компьютере нет", stage.ru()),
            UserError::NeedsNvidia(stage) => {
                write!(f, "{}: нужна видеокарта NVIDIA с драйвером от 525 — движок собран только под CUDA", stage.ru())
            }
            UserError::GpuMissing { index, cards } if cards.is_empty() => write!(
                f,
                "видеокарты номер {index}, выбранной в «Движки и облако» → «Видеокарта», в компьютере нет, и карт NVIDIA нет совсем"
            ),
            UserError::GpuMissing { index, cards } => write!(
                f,
                "видеокарты номер {index}, выбранной в «Движки и облако» → «Видеокарта», в компьютере нет (есть: {}) — выберите там другую карту",
                cards.join(", ")
            ),
            UserError::RecordingEmpty => f.write_str("пустая запись"),
            UserError::NoAudioFile => f.write_str("в запросе нет звукового файла"),
            UserError::NoFile => f.write_str("в запросе нет файла"),
            UserError::ChatNotFound => f.write_str("чат не найден"),
            UserError::ChatNotCreated => f.write_str("чат не создался"),
            UserError::MessageNotFound => f.write_str("сообщение не найдено"),
            UserError::ItemNotFound => f.write_str("предмет не найден"),
            UserError::ItemGone => f.write_str("предмет пропал"),
            UserError::QuestNotFound => f.write_str("задание не найдено"),
            UserError::CharacterNotFound => f.write_str("персонаж не найден"),
            UserError::CharacterNameRequired => f.write_str("у персонажа должно быть имя"),
            UserError::UnknownField(field) => write!(f, "неизвестное поле: {field}"),
            UserError::UnknownAction(action) => write!(f, "неизвестное действие: {action}"),
            UserError::BadSetting(detail) => write!(f, "непонятная настройка: {detail}"),
            UserError::Download { detail, .. } => f.write_str(detail),
            UserError::DiskSpace { need, free, path } => write!(
                f,
                "не хватает места на диске: нужно {} ГБ, свободно {} ГБ ({path}) — освободите место и скачайте снова",
                gigabytes(*need),
                gigabytes(*free)
            ),
        }
    }
}

impl std::error::Error for UserError {}

impl From<String> for UserError {
    fn from(text: String) -> Self {
        UserError::Raw(text)
    }
}

impl From<&str> for UserError {
    fn from(text: &str) -> Self {
        UserError::Raw(text.to_string())
    }
}

impl From<du_llm::LlmError> for UserError {
    fn from(error: du_llm::LlmError) -> Self {
        UserError::Llm { error, starting: false }
    }
}

impl From<du_tts::EngineError> for UserError {
    fn from(error: du_tts::EngineError) -> Self {
        UserError::Tts(error)
    }
}

impl From<du_image::ImageError> for UserError {
    fn from(error: du_image::ImageError) -> Self {
        UserError::Image(error)
    }
}

impl From<du_db::DbError> for UserError {
    fn from(error: du_db::DbError) -> Self {
        UserError::Raw(error.to_string())
    }
}

/// Внутренние пути, что ещё говорят строкой, получают русский текст — как раньше.
impl From<UserError> for String {
    fn from(error: UserError) -> Self {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// По одной ошибке каждого кода. `match` без подстановочного знака не даст забыть новый вид.
    fn one_of_each() -> Vec<UserError> {
        use du_llm::{EmptyStream, LlmError};
        let all = vec![
            UserError::Raw("x".into()),
            UserError::JobLost,
            UserError::JobCrashed("x".into()),
            UserError::TurnAborted,
            UserError::HeroDead,
            UserError::NarratorOnlyArtifacts,
            UserError::Llm { error: LlmError::BinMissing("x".into()), starting: true },
            UserError::Llm { error: LlmError::ModelMissing("x".into()), starting: true },
            UserError::Llm { error: LlmError::Spawn("x".into()), starting: true },
            UserError::Llm { error: LlmError::Http("x".into()), starting: false },
            UserError::Llm { error: LlmError::StreamBroken("x".into()), starting: false },
            UserError::Llm { error: LlmError::Status { code: 401, detail: "x".into() }, starting: false },
            UserError::Llm { error: LlmError::Status { code: 500, detail: "x".into() }, starting: false },
            UserError::Llm { error: LlmError::Api("x".into()), starting: false },
            UserError::Llm { error: LlmError::Empty(EmptyStream::ToolCall { finish: "x".into() }), starting: false },
            UserError::Llm {
                error: LlmError::Empty(EmptyStream::OnlyThinking { chars: 1, finish: "x".into() }),
                starting: false,
            },
            UserError::Llm { error: LlmError::Empty(EmptyStream::Filtered), starting: false },
            UserError::Llm { error: LlmError::Empty(EmptyStream::Finished { finish: "x".into() }), starting: false },
            UserError::Llm { error: LlmError::Empty(EmptyStream::NoReason), starting: false },
            UserError::Llm { error: LlmError::Empty(EmptyStream::Cut), starting: false },
            UserError::TextModelMissing("x".into()),
            UserError::LocalServerUrlEmpty,
            UserError::AgentNotConnected,
            UserError::CloudKeyMissing(CloudStage::Narrator),
            UserError::CloudModelMissing(CloudStage::Image),
            UserError::CloudRefused { stage: CloudStage::Tts, status: 400, message: Some("x".into()) },
            UserError::CloudRefused { stage: CloudStage::Asr, status: 400, message: None },
            UserError::CloudNetwork { stage: CloudStage::Image, detail: "x".into() },
            UserError::CloudSaid { stage: CloudStage::Asr, message: "x".into() },
            UserError::CloudEmpty(CloudStage::Asr),
            UserError::CloudTooSmall { stage: CloudStage::Tts, bytes: 1 },
            UserError::GoogleKeyMissing,
            UserError::OpenRouterKeyNotSet,
            UserError::VoiceMissing("x".into()),
            UserError::NoVoices,
            UserError::TtsEngineMissing,
            UserError::Tts(du_tts::EngineError::LibraryLoad("x".into())),
            UserError::Tts(du_tts::EngineError::Cancelled),
            UserError::Tts(du_tts::EngineError::Generation("x".into())),
            UserError::Tts(du_tts::EngineError::InvalidParam("x".into())),
            UserError::VoiceReference("x".into()),
            UserError::TtsEmptyText,
            UserError::TtsNothing,
            UserError::ImageWeightsMissing("x".into()),
            UserError::Image(du_image::ImageError::LibraryMissing("x".into())),
            UserError::Image(du_image::ImageError::Library { dll: "x".into(), detail: "x".into() }),
            UserError::Image(du_image::ImageError::BadPath("x".into())),
            UserError::Image(du_image::ImageError::ContextCreation("x".into())),
            UserError::Image(du_image::ImageError::GenerationFailed("x".into())),
            UserError::ImageNoFrame,
            UserError::AsrFailed("x".into()),
            UserError::WhisperBinMissing("x".into()),
            UserError::WhisperModelMissing { model: "x".into(), path: "x".into() },
            UserError::WhisperCudaMissing,
            UserError::ParakeetModelMissing("x".into()),
            UserError::AsrEngineMissing { path: "x".into(), gpu: false },
            UserError::AsrEngineMissing { path: "x".into(), gpu: true },
            UserError::ParakeetCudaLibMissing { name: "x".into(), path: "x".into() },
            UserError::CpuFloor("x".into()),
            UserError::NeedsGpu(CloudStage::Image),
            UserError::NeedsNvidia(CloudStage::Tts),
            UserError::GpuMissing { index: 1, cards: vec!["0: x".into()] },
            UserError::GpuMissing { index: 1, cards: Vec::new() },
            UserError::RecordingEmpty,
            UserError::NoAudioFile,
            UserError::NoFile,
            UserError::ChatNotFound,
            UserError::ChatNotCreated,
            UserError::MessageNotFound,
            UserError::ItemNotFound,
            UserError::ItemGone,
            UserError::QuestNotFound,
            UserError::CharacterNotFound,
            UserError::CharacterNameRequired,
            UserError::UnknownField("x".into()),
            UserError::UnknownAction("x".into()),
            UserError::BadSetting("x".into()),
            UserError::Download { kind: "hash_mismatch", detail: "x".into() },
            UserError::Download { kind: "network", detail: "x".into() },
            UserError::Download { kind: "archive", detail: "x".into() },
            UserError::Download { kind: "disk", detail: "x".into() },
            UserError::Download { kind: "other", detail: "x".into() },
            UserError::Download { kind: "disk_space", detail: "x".into() },
            UserError::DiskSpace { need: 3_000_000_000, free: 1_000_000_000, path: "x".into() },
        ];
        for error in &all {
            match error {
                UserError::Raw(_)
                | UserError::JobLost
                | UserError::JobCrashed(_)
                | UserError::TurnAborted
                | UserError::HeroDead
                | UserError::NarratorOnlyArtifacts
                | UserError::Llm { .. }
                | UserError::TextModelMissing(_)
                | UserError::LocalServerUrlEmpty
                | UserError::AgentNotConnected
                | UserError::CloudKeyMissing(_)
                | UserError::CloudModelMissing(_)
                | UserError::CloudRefused { .. }
                | UserError::CloudNetwork { .. }
                | UserError::CloudSaid { .. }
                | UserError::CloudEmpty(_)
                | UserError::CloudTooSmall { .. }
                | UserError::GoogleKeyMissing
                | UserError::OpenRouterKeyNotSet
                | UserError::VoiceMissing(_)
                | UserError::NoVoices
                | UserError::TtsEngineMissing
                | UserError::Tts(_)
                | UserError::VoiceReference(_)
                | UserError::TtsEmptyText
                | UserError::TtsNothing
                | UserError::ImageWeightsMissing(_)
                | UserError::Image(_)
                | UserError::ImageNoFrame
                | UserError::AsrFailed(_)
                | UserError::WhisperBinMissing(_)
                | UserError::WhisperModelMissing { .. }
                | UserError::WhisperCudaMissing
                | UserError::ParakeetModelMissing(_)
                | UserError::AsrEngineMissing { .. }
                | UserError::ParakeetCudaLibMissing { .. }
                | UserError::CpuFloor(_)
                | UserError::NeedsGpu(_)
                | UserError::NeedsNvidia(_)
                | UserError::GpuMissing { .. }
                | UserError::RecordingEmpty
                | UserError::NoAudioFile
                | UserError::NoFile
                | UserError::ChatNotFound
                | UserError::ChatNotCreated
                | UserError::MessageNotFound
                | UserError::ItemNotFound
                | UserError::ItemGone
                | UserError::QuestNotFound
                | UserError::CharacterNotFound
                | UserError::CharacterNameRequired
                | UserError::UnknownField(_)
                | UserError::UnknownAction(_)
                | UserError::BadSetting(_)
                | UserError::Download { .. }
                | UserError::DiskSpace { .. } => {}
            }
        }
        all
    }

    /// Каждый код переведён на все семь языков окна: новый код без перевода роняет тесты.
    #[test]
    fn every_code_has_a_translation_in_every_language() {
        let dictionary = include_str!("../../../frontend/src/lib/ui-text-errors.ts");
        let mut missing = Vec::new();
        for error in one_of_each() {
            let code = error.code();
            if code == "raw" {
                continue;
            }
            let found = dictionary.matches(&format!("\n  {code}:")).count();
            if found != 7 {
                missing.push(format!("{code}: {found} из 7"));
            }
        }
        assert!(missing.is_empty(), "без перевода: {missing:?}");
    }

    #[test]
    fn the_russian_text_stays_what_the_hub_already_groups_by() {
        assert_eq!(
            UserError::VoiceMissing("RU_Male".into()).to_string(),
            "голоса «RU_Male» нет на диске: поставьте «Пак голосов» в Настройках → Модели, добавьте этот голос или выберите другой"
        );
        let refused = UserError::CloudRefused { stage: CloudStage::Image, status: 400, message: Some("Invalid size".into()) };
        assert_eq!(refused.to_string(), "облачный кадр: Invalid size (400)");
        let body = refused.to_json();
        assert_eq!(body["code"], "cloud_refused");
        assert_eq!(body["params"]["stage"], "image");
        assert_eq!(body["params"]["status"], 400);
    }
}
