//! Рантайм-настройки: всё, что раньше было зашито в код.
//!
//! Это НЕ настройки истории (те живут в чате и меняются на каждую партию). Здесь железо и
//! движки: сколько контекста давать рассказчику, сколько слоёв класть на карту, сколько
//! шагов делать кадру, включена ли озвучка и можно ли считать её параллельно.
//!
//! Хранится одним файлом рядом с моделями, поэтому переживает обновление приложения и
//! правится руками, если до интерфейса не дотянуться.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Runtime {
    // ── Где считать ───────────────────────────────────────────────────────────
    // Один общий выбор и по одному переопределению на стадию. Пустая строка означает
    // «как у всех», `auto` — «решай по железу», `gpu`/`cpu` — прямое указание. Раньше это
    // решалось тремя разными способами вразнобой, и игрок не понимал, что где считается.
    /// Общий выбор для локальных стадий: `auto`, `gpu` или `cpu`.
    pub local_backend: String,
    /// Переопределение для рассказчика; пусто — как общий.
    pub narrator_backend: String,
    /// Переопределение для картинок.
    pub image_backend: String,
    /// Переопределение для озвучки.
    pub tts_backend: String,
    /// Переопределение для распознавания речи.
    pub asr_backend: String,
    /// Каким движком распознавать речь: `parakeet` (transcribe.cpp в процессе, быстрый) или `crisperwhisper`
    /// ([`ASR_CRISPERWHISPER`], в audiocpp_server: почти все языки мира, дословная запись).
    pub asr_engine: String,
    /// Кто пишет текст игры: пусто — своя карта или облако по флагам ниже, `agent` — агент,
    /// подключённый к MCP игры (Claude Code и т.п.): каждый проход хода уходит ему вопросом.
    pub narrator_provider: String,
    /// Вариант Parakeet: `int8` (по умолчанию) или `ultra`.
    pub asr_model: String,
    /// Номер карты NVIDIA (в порядке шины PCI), на которой считает игра; пусто — первая. Действует с запуска игры:
    /// CUDA читает выбор один раз на процесс.
    pub gpu_index: Option<u32>,

    /// Контекст рассказчика в токенах. Это главный потребитель памяти под KV-кэш: в промпт
    /// уезжает около 48 тысяч символов истории, то есть примерно 13 тысяч токенов, поэтому
    /// больший контекст просто отнимает карту у картинок и озвучки.
    pub narrator_ctx: u32,
    /// Слоёв рассказчика на карте; −1 — все.
    pub narrator_gpu_layers: i32,
    /// Шагов сэмплера на кадр. Krea-2 Turbo дистиллирована под восемь.
    pub image_steps: i32,
    /// Сила следования промпту. У дистиллированной модели должна оставаться единицей.
    pub image_cfg: f32,
    /// Держать веса картинок в оперативной памяти и подавать их на карту по мере надобности.
    pub image_offload_to_cpu: bool,
    /// Бюджет VRAM для посегментного прохода; «-1» — авто с запасом.
    pub image_max_vram: String,
    /// Озвучивать ход.
    pub tts_enabled: bool,
    /// Считать озвучку параллельно с механикой и кадром. Замер на 4090: пик около 23.7 ГБ
    /// из 24.5, то есть влезает впритык — на картах поменьше это надо выключать.
    pub tts_parallel: bool,
    /// Сколько секунд эталонного клипа отдавать движку голоса.
    pub tts_reference_seconds: u32,
    /// Квант Higgs (`q8_0`, `q6_k`, `q4_k_m`); пусто — по памяти карты (`acpp::higgs_quant`).
    pub tts_quant: String,

    // ── Облако ────────────────────────────────────────────────────────────────
    // Каждая стадия уводится в облако отдельно: так можно играть и совсем без карты, и
    // разгрузить её частично, оставив на ней только то, что важно по скорости.
    /// Ключ OpenRouter. Пустой — облако выключено целиком, чем бы ни были помечены стадии.
    pub openrouter_key: String,
    pub openrouter_narrator_on: bool,
    pub openrouter_narrator_model: String,
    pub openrouter_image_on: bool,
    pub openrouter_image_model: String,
    pub openrouter_tts_on: bool,
    pub openrouter_tts_model: String,
    pub openrouter_tts_voice: String,
    pub openrouter_asr_on: bool,
    pub openrouter_asr_model: String,
    /// Кто озвучивает в облаке, когда облачная озвучка включена: `openrouter` или `google`
    /// (Gemini напрямую, ключ — `GEMINI_API_KEY` или сохранённый в игре).
    pub cloud_tts_provider: String,
    /// Модель озвучки Gemini.
    pub google_tts_model: String,
    /// Голос рассказчика у Gemini; пусто — первый подходящий по языку истории.
    pub google_tts_voice: String,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            // Пусто у стадий = «как общий», а общий «auto» = решать по железу: на машине с
            // картой всё считает карта, без неё — процессор, и ничего настраивать не нужно.
            local_backend: "auto".to_string(),
            narrator_backend: String::new(),
            image_backend: String::new(),
            tts_backend: String::new(),
            asr_backend: String::new(),
            asr_engine: "parakeet".to_string(),
            narrator_provider: String::new(),
            asr_model: "int8".to_string(),
            gpu_index: None,
            narrator_ctx: 16_384,
            narrator_gpu_layers: -1,
            image_steps: 8,
            image_cfg: 1.0,
            image_offload_to_cpu: true,
            image_max_vram: "-1".to_string(),
            tts_enabled: true,
            tts_parallel: true,
            tts_reference_seconds: 12,
            tts_quant: String::new(),
            openrouter_key: String::new(),
            openrouter_narrator_on: false,
            openrouter_narrator_model: String::new(),
            openrouter_image_on: false,
            openrouter_image_model: String::new(),
            openrouter_tts_on: false,
            openrouter_tts_model: String::new(),
            openrouter_tts_voice: String::new(),
            openrouter_asr_on: false,
            openrouter_asr_model: String::new(),
            cloud_tts_provider: "openrouter".to_string(),
            google_tts_model: crate::google_tts::MODELS[0].to_string(),
            google_tts_voice: String::new(),
        }
    }
}

fn path_of(root: &Path) -> PathBuf {
    root.join("models").join("runtime.json")
}

pub fn load(root: &Path) -> Runtime {
    // Битый или отсутствующий файл не должен мешать запуску: берём значения по умолчанию.
    std::fs::read_to_string(path_of(root))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .map(sanitize)
        .unwrap_or_default()
}

pub fn save(root: &Path, runtime: &Runtime) -> Result<(), String> {
    let path = path_of(root);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let text = serde_json::to_string_pretty(runtime).map_err(|error| error.to_string())?;
    // Пишем через временный файл: обрыв записи не должен оставить пустой конфиг.
    let temp = path.with_extension("json.part");
    std::fs::write(&temp, text).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, &path).map_err(|error| error.to_string())
}

pub fn sanitize(mut runtime: Runtime) -> Runtime {
    runtime.openrouter_key = runtime.openrouter_key.trim().to_string();
    runtime.narrator_ctx = runtime.narrator_ctx.clamp(2_048, 131_072);
    runtime.narrator_gpu_layers = runtime.narrator_gpu_layers.max(-1);
    runtime.image_steps = runtime.image_steps.clamp(1, 60);
    runtime.image_cfg = runtime.image_cfg.clamp(0.5, 20.0);
    runtime.tts_reference_seconds = runtime.tts_reference_seconds.clamp(3, 60);
    if runtime.image_max_vram.trim().is_empty() {
        runtime.image_max_vram = "-1".into();
    }
    // faster-whisper ушёл: выбравший Whisper распознаёт CrisperWhisper — тот же Whisper в audiocpp_server
    if runtime.asr_engine.trim().eq_ignore_ascii_case("whisper") {
        runtime.asr_engine = ASR_CRISPERWHISPER.into();
    }
    runtime
}

/// Распознавание CrisperWhisper в audiocpp_server.
pub const ASR_CRISPERWHISPER: &str = "crisperwhisper";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_fit_a_shared_card() {
        let runtime = Runtime::default();
        assert_eq!(runtime.narrator_ctx, 16_384, "больший контекст даром съедает карту");
        assert_eq!(runtime.image_steps, 8, "модель дистиллирована под восемь шагов");
        assert_eq!(runtime.image_cfg, 1.0);
        assert!(runtime.image_offload_to_cpu);
    }

    #[test]
    fn a_saved_whisper_choice_reads_as_crisperwhisper() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &Runtime { asr_engine: "whisper".into(), ..Runtime::default() }).unwrap();
        assert_eq!(load(dir.path()).asr_engine, ASR_CRISPERWHISPER);
        let mut runtime = Runtime::default();
        assert!(crate::setup::select_variant(&mut runtime, "asr-crisperwhisper"));
        assert!(crate::setup::variant_selected(&runtime, "asr-crisperwhisper"));
        assert!(!crate::setup::variant_selected(&runtime, "asr"));
        assert!(crate::setup::select_variant(&mut runtime, "asr-ultra"));
        assert_eq!((runtime.asr_engine.as_str(), runtime.asr_model.as_str()), ("parakeet", "ultra"));
    }

    #[test]
    fn settings_survive_a_round_trip_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = Runtime { narrator_ctx: 32_768, tts_parallel: false, ..Runtime::default() };
        save(dir.path(), &runtime).unwrap();

        let back = load(dir.path());
        assert_eq!(back.narrator_ctx, 32_768);
        assert!(!back.tts_parallel);
    }

    #[test]
    fn a_broken_file_falls_back_to_defaults_instead_of_blocking_startup() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("models")).unwrap();
        std::fs::write(path_of(dir.path()), "не json").unwrap();
        assert_eq!(load(dir.path()).narrator_ctx, Runtime::default().narrator_ctx);
    }

    #[test]
    fn absurd_values_are_clamped_rather_than_accepted() {
        let runtime = sanitize(Runtime {
            narrator_ctx: 1,
            image_steps: 5_000,
            image_cfg: -3.0,
            tts_reference_seconds: 0,
            image_max_vram: "  ".into(),
            ..Runtime::default()
        });
        assert_eq!(runtime.narrator_ctx, 2_048);
        assert_eq!(runtime.image_steps, 60);
        assert_eq!(runtime.image_cfg, 0.5);
        assert_eq!(runtime.tts_reference_seconds, 3);
        assert_eq!(runtime.image_max_vram, "-1");
    }

    #[test]
    fn unknown_fields_do_not_break_an_older_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("models")).unwrap();
        std::fs::write(path_of(dir.path()), r#"{"narratorCtx": 8192, "чтоТоНовое": 1}"#).unwrap();
        assert_eq!(load(dir.path()).narrator_ctx, 8_192);
    }
}
