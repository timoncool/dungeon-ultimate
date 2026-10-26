//! Studio Hub в Dungeon Ultimate — как в Dub Studio: анонимная статистика (только после того, как
//! экран «Что скачать» показал галочку; выключена при DO_NOT_TRACK или STUDIO_TELEMETRY=0) и лента
//! новостей и полосок поверх окна.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static HUB: OnceLock<Option<studio_hub_client::Hub>> = OnceLock::new();
/// Корень игры: по нему читается, на чём шла работа.
static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Работа, которую считает отчёт надёжности хаба: `<вид>_done` против `<вид>_failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Work {
    /// Ход истории: проза и разбор.
    Turn,
    /// Кадр сцены, портрет или предмет.
    Image,
    /// Озвучка.
    Tts,
    /// Распознавание речи.
    Asr,
}

impl Work {
    fn as_str(self) -> &'static str {
        match self {
            Work::Turn => "turn",
            Work::Image => "image",
            Work::Tts => "tts",
            Work::Asr => "asr",
        }
    }

    fn stage(self) -> &'static str {
        match self {
            Work::Turn => "narrator",
            Work::Image => "image",
            Work::Tts => "tts",
            Work::Asr => "asr",
        }
    }
}

/// Запустить хаб один раз, внутри tokio-рантайма сервера; нечитаемый файл состояния оставляет игру
/// без хаба, о чём сказано один раз.
pub fn start(root: &Path) {
    let _ = ROOT.set(root.to_path_buf());
    HUB.get_or_init(|| {
        let snapshot = crate::hw::snapshot();
        let name = snapshot.gpu_name.to_ascii_lowercase();
        let vendor = if name.contains("nvidia") || name.contains("geforce") || name.contains("rtx") {
            "nvidia"
        } else if name.contains("amd") || name.contains("radeon") {
            "amd"
        } else if name.contains("intel") {
            "intel"
        } else if name.is_empty() {
            "none"
        } else {
            "other"
        };
        let http = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build() {
            Ok(http) => http,
            Err(error) => {
                tracing::error!("Studio Hub выключен на этот запуск: HTTP-клиент не собрался: {error}");
                return None;
            }
        };
        let os_label = format!(
            "{} {}",
            sysinfo::System::name().unwrap_or_else(|| std::env::consts::OS.to_string()),
            sysinfo::System::os_version().unwrap_or_default()
        )
        .to_ascii_lowercase();
        let config = studio_hub_client::HubConfig {
            app: "dungeon".into(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            data_dir: root.to_path_buf(),
            http,
            os_label,
            gpu: studio_hub_client::Gpu {
                vendor: vendor.into(),
                vram_gb: studio_hub_client::Gpu::vram_bucket(snapshot.total_vram),
                backend: crate::accel::accel(&crate::runtime::load(root)).key().into(),
            },
            ui_lang: "en".into(),
            urls: Vec::new(),
        };
        match studio_hub_client::Hub::new(config) {
            Ok(hub) => {
                hub.spawn();
                Some(hub)
            }
            Err(error) => {
                tracing::error!("Studio Hub выключен на этот запуск: файл состояния не читается: {error}");
                None
            }
        }
    });
}

/// Язык окна — хабу: по нему лента выбирает новости, и он уходит в отчёт.
pub fn set_ui_lang(lang: &str) {
    if let Some(hub) = HUB.get().and_then(Option::as_ref) {
        hub.set_ui_lang(lang);
    }
}

/// Отчёт за день уходит при закрытии игры, а не при следующем запуске; не дольше нескольких секунд.
pub fn flush_on_exit() {
    let Some(hub) = HUB.get().and_then(Option::as_ref).cloned() else { return };
    match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime.block_on(hub.flush(std::time::Duration::from_secs(8))),
        Err(error) => tracing::error!("статистика при выходе не ушла, нет рантайма: {error}"),
    }
}

/// Маршруты хаба для окна (`/v1/hub/*`); пусто, если хаб не запустился.
pub fn router<S: Clone + Send + Sync + 'static>() -> axum::Router<S> {
    HUB.get().and_then(Option::as_ref).map(studio_hub_client::Hub::router).unwrap_or_default()
}

/// Каждый вызов API игры считается по шаблону маршрута — какими возможностями пользуются; без хаба роутер как есть.
pub fn track<S: Clone + Send + Sync + 'static>(router: axum::Router<S>) -> axum::Router<S> {
    match HUB.get().and_then(Option::as_ref) {
        Some(hub) => hub.track(router),
        None => router,
    }
}

/// Модели докачались — шаг между установкой и первым ходом.
pub fn setup_ready() {
    if let Some(hub) = HUB.get().and_then(Option::as_ref) {
        hub.setup_ready();
    }
}

/// Работа, у которой движок был один (очередь к карте пускает по задаче): секунды результата и секунды движка — для
/// скорости по классу карты.
pub fn worked(kind: &str, result_seconds: f64, work: std::time::Duration) {
    if let Some(hub) = HUB.get().and_then(Option::as_ref) {
        hub.worked(kind, result_seconds, work);
    }
}

/// Закачка моделей упала; причину обезличивает клиент хаба.
pub fn setup_failed(reason: &str) {
    if let Some(hub) = HUB.get().and_then(Option::as_ref) {
        hub.setup_failed(reason);
    }
}

/// Работа закончилась: считается `<вид>_done` или `<вид>_failed`. У сбоя остаётся причина, которую
/// клиент хаба обезличивает, у удачи — на чём она шла.
pub fn ended(work: Work, outcome: Result<(), &str>) {
    finish(work, outcome, None);
}

/// Ход истории закончился. История может писаться на своём сервере (настройка «Локальный
/// сервер»): тогда и в отчёте стоит он, а не встроенная модель.
pub fn turn_ended(settings: &du_core::StorySettings, outcome: Result<(), &str>) {
    finish(Work::Turn, outcome, Some(settings));
}

fn finish(work: Work, outcome: Result<(), &str>, story: Option<&du_core::StorySettings>) {
    let Some(hub) = HUB.get().and_then(Option::as_ref) else { return };
    match outcome {
        Ok(()) => {
            hub.count(&format!("{}_done", work.as_str()), 1);
            if let Some(root) = ROOT.get() {
                let runtime = crate::runtime::load(root);
                hub.used_models(None, &[model_in_use(&runtime, work.stage(), story)], 1);
            }
        }
        Err(reason) => {
            hub.count(&format!("{}_failed", work.as_str()), 1);
            hub.failed(work.as_str(), reason);
        }
    }
}

/// Имя модели своего сервера истории без пути: LM Studio и llama.cpp называют модель путём к
/// файлу, а путь несёт имя пользователя.
fn custom_model_name(settings: &du_core::StorySettings) -> String {
    let model = settings.custom_model.trim();
    let name = model.rsplit(['/', '\\']).next().unwrap_or(model);
    if name.is_empty() { "custom".to_string() } else { format!("custom:{name}") }
}

/// На чём сейчас идёт стадия: `стадия:модель`.
fn model_in_use(runtime: &crate::runtime::Runtime, stage: &str, story: Option<&du_core::StorySettings>) -> String {
    use crate::cloud::{stage_enabled, Stage};
    let custom = story.filter(|settings| settings.text_provider == du_core::TextProvider::Custom);
    match stage {
        "narrator" if runtime.narrator_provider == "agent" => "narrator:agent".into(),
        "narrator" if stage_enabled(runtime, Stage::Narrator) => format!("narrator:openrouter:{}", runtime.openrouter_narrator_model),
        "narrator" if custom.is_some() => format!("narrator:{}", custom.map(custom_model_name).unwrap_or_default()),
        "narrator" => "narrator:gemma-4-12b-q4_k_m".into(),
        "image" if stage_enabled(runtime, Stage::Image) => format!("image:openrouter:{}", runtime.openrouter_image_model),
        "image" => "image:krea2-turbo-q4_k_m".into(),
        "tts" if stage_enabled(runtime, Stage::Tts) && crate::cloud::tts_by_google(runtime) => format!("tts:google:{}", runtime.google_tts_model),
        "tts" if stage_enabled(runtime, Stage::Tts) => format!("tts:openrouter:{}", runtime.openrouter_tts_model),
        "tts" => "tts:higgs-q4_k_m".into(),
        "asr" if stage_enabled(runtime, Stage::Asr) => format!("asr:openrouter:{}", runtime.openrouter_asr_model),
        "asr" if runtime.asr_engine == crate::runtime::ASR_CRISPERWHISPER => "asr:crisperwhisper-2-turbo".into(),
        "asr" => format!("asr:parakeet-{}", if runtime.asr_model.is_empty() { "int8" } else { &runtime.asr_model }),
        other => format!("{other}:unknown"),
    }
}

#[cfg(test)]
mod tests {
    use super::model_in_use;
    use crate::runtime::Runtime;

    #[test]
    fn each_stage_names_what_it_runs_on() {
        let local = Runtime::default();
        assert_eq!(model_in_use(&local, "narrator", None), "narrator:gemma-4-12b-q4_k_m");
        assert_eq!(model_in_use(&local, "asr", None), "asr:parakeet-int8");
        let google = Runtime {
            openrouter_tts_on: true,
            cloud_tts_provider: "google".into(),
            google_tts_model: "gemini-3.8-flash-tts".into(),
            asr_engine: crate::runtime::ASR_CRISPERWHISPER.into(),
            narrator_provider: "agent".into(),
            ..Runtime::default()
        };
        assert_eq!(model_in_use(&google, "tts", None), "tts:google:gemini-3.8-flash-tts");
        assert_eq!(model_in_use(&google, "asr", None), "asr:crisperwhisper-2-turbo");
        assert_eq!(model_in_use(&google, "narrator", None), "narrator:agent");
    }

    #[test]
    fn a_story_on_its_own_server_names_that_server_without_the_path() {
        let local = Runtime::default();
        let mut story = du_core::StorySettings { text_provider: du_core::TextProvider::Custom, ..Default::default() };
        assert_eq!(model_in_use(&local, "narrator", Some(&story)), "narrator:custom");
        story.custom_model = "C:\\Users\\ivan\\models\\qwen3-8b-q4_k_m.gguf".into();
        assert_eq!(model_in_use(&local, "narrator", Some(&story)), "narrator:custom:qwen3-8b-q4_k_m.gguf");
        story.custom_model = "lmstudio-community/gemma-3-12b".into();
        assert_eq!(model_in_use(&local, "narrator", Some(&story)), "narrator:custom:gemma-3-12b");
        story.text_provider = du_core::TextProvider::Local;
        assert_eq!(model_in_use(&local, "narrator", Some(&story)), "narrator:gemma-4-12b-q4_k_m");
    }
}
