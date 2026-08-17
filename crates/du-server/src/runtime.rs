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
    /// Каким движком распознавать речь: `parakeet` (в процессе, быстрый) или `whisper`
    /// (отдельная программа, точнее на шумной записи и на смеси языков).
    pub asr_engine: String,

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
            narrator_ctx: 16_384,
            narrator_gpu_layers: -1,
            image_steps: 8,
            image_cfg: 1.0,
            image_offload_to_cpu: true,
            image_max_vram: "-1".to_string(),
            tts_enabled: true,
            tts_parallel: true,
            tts_reference_seconds: 12,
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

/// Привести пришедшие извне значения к работоспособным.
pub fn sanitize(mut runtime: Runtime) -> Runtime {
    runtime.narrator_ctx = runtime.narrator_ctx.clamp(2_048, 131_072);
    runtime.narrator_gpu_layers = runtime.narrator_gpu_layers.max(-1);
    runtime.image_steps = runtime.image_steps.clamp(1, 60);
    runtime.image_cfg = runtime.image_cfg.clamp(0.5, 20.0);
    runtime.tts_reference_seconds = runtime.tts_reference_seconds.clamp(3, 60);
    if runtime.image_max_vram.trim().is_empty() {
        runtime.image_max_vram = "-1".into();
    }
    runtime
}

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
    fn settings_survive_a_round_trip_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut runtime = Runtime::default();
        runtime.narrator_ctx = 32_768;
        runtime.tts_parallel = false;
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
