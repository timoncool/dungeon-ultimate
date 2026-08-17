//! Кто сейчас занимает видеокарту.
//!
//! Моделей четыре — нарратор, картинка, озвучка, распознавание, — а карта одна. Поэтому
//! резидент ровно один, и смена резидента означает выгрузку предыдущего: у текстовой модели
//! это убийство процесса сайдкара, у картинки — снятие контекста движка. Обе операции
//! синхронные, так что к моменту возврата память уже свободна.
//!
//! Все обращения к карте идут через [`crate::jobs::JobQueue`], поэтому смена резидента не
//! может случиться посреди чужой работы.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use du_image::{Engine, GenParams, ModelConfig, RawImage};
use du_llm::{LlamaServer, ServerOpts};

/// Кто держит карту прямо сейчас.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resident {
    Nobody,
    Text,
    Image,
}

/// Где лежат веса и рантайм-библиотеки.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Каталог с `stable-diffusion.dll` и бэкендами ggml.
    pub image_runtime: PathBuf,
    /// Каталог с весами картинок.
    pub image_models: PathBuf,
    /// Каталог с `llama-server`.
    pub llama_tools: PathBuf,
    /// GGUF нарратора.
    pub text_model: PathBuf,
    /// Проектор для мультимодальности; без него модель не увидит портреты.
    pub text_mmproj: Option<PathBuf>,
}

impl Paths {
    pub fn under(root: &Path) -> Self {
        let models = root.join("models");
        Self {
            image_runtime: models.join("runtime").join("sd"),
            image_models: models.join("image"),
            llama_tools: root.join("tools").join("llama"),
            text_model: models.join("text").join("narrator.gguf"),
            text_mmproj: Some(models.join("text").join("mmproj.gguf")),
        }
    }

    fn krea2(&self) -> ModelConfig {
        ModelConfig::krea2(
            self.image_models.join("krea2-turbo-Q4_K_M.gguf"),
            self.image_models.join("qwen3-vl-4b-abliterated-Q4_K_M.gguf"),
            self.image_models.join("wan_2.1_vae.safetensors"),
        )
    }

    /// LoRA, которая включает правку по референсу.
    pub fn edit_lora(&self) -> PathBuf {
        self.image_models.join("krea2-identity-edit-r128.safetensors")
    }

    /// Все ли веса картинок на месте.
    pub fn image_ready(&self) -> bool {
        let config = self.krea2();
        config.diffusion_model.is_file() && config.llm.is_file() && config.vae.is_file()
    }

    pub fn text_ready(&self) -> bool {
        self.text_model.is_file() && du_llm::resolve_llama_bin(&self.llama_tools).is_file()
    }
}

/// Владелец видеокарты. Ленивый: движок картинок и сайдкар поднимаются при первом обращении.
pub struct Gpu {
    paths: Paths,
    /// Корень приложения — отсюда читаются рантайм-настройки на каждый запуск модели.
    root: PathBuf,
    /// Движок держим отдельно от контекста: библиотека грузится один раз, а вот контекст
    /// (то есть веса в памяти) снимается и поднимается по мере смены резидента.
    engine: Mutex<Option<Engine>>,
    text: Mutex<Option<LlamaServer>>,
    /// Движок озвучки держим загруженным между ходами: его загрузка занимает секунды, и
    /// платить их на каждой реплике незачем. Освобождается, когда карта нужна картинке.
    speech: Mutex<Option<Arc<du_tts::AudiocppEngine>>>,
    resident: Mutex<Resident>,
}

#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error("движок картинок: {0}")]
    Image(#[from] du_image::ImageError),
    #[error("текстовая модель: {0}")]
    Text(#[from] du_llm::LlmError),
    #[error("не хватает файлов модели: {0}")]
    Missing(String),
}

impl Gpu {
    pub fn new(root: &Path, paths: Paths) -> Self {
        Self {
            paths,
            root: root.to_path_buf(),
            engine: Mutex::new(None),
            text: Mutex::new(None),
            speech: Mutex::new(None),
            resident: Mutex::new(Resident::Nobody),
        }
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn resident(&self) -> Resident {
        *self.resident.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Освободить карту от текстовой модели. Убийство процесса — единственный надёжный
    /// способ: у llama-server нет выгрузки весов без завершения.
    fn release_text(&self) {
        let mut text = self.text.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut server) = text.take() {
            tracing::info!("освобождаю карту: глушу текстовую модель");
            server.stop();
        }
    }

    /// Поднять текстовую модель, предварительно освободив карту.
    pub fn text_base_url(&self) -> Result<String, GpuError> {
        {
            let text = self.text.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(server) = text.as_ref() {
                return Ok(server.base_url().to_string());
            }
        }
        if !self.paths.text_ready() {
            return Err(GpuError::Missing(format!(
                "нет модели нарратора ({}) или llama-server",
                self.paths.text_model.display()
            )));
        }
        // Картинка обязана уйти с карты до загрузки нарратора.
        self.drop_image_context();

        // Настройки читаем при КАЖДОМ запуске: изменил контекст — следующий ход уже с ним.
        let runtime = crate::runtime::load(&self.root);
        let bin = du_llm::resolve_llama_bin(&self.paths.llama_tools);
        let mut opts = ServerOpts::new(bin, self.paths.text_model.clone())
            .with_mmproj(self.paths.text_mmproj.clone().filter(|path| path.is_file()))
            .with_ctx(runtime.narrator_ctx);
        opts.n_gpu_layers = narrator_layers(&runtime);
        let server = LlamaServer::start(&opts)?;
        let url = server.base_url().to_string();
        *self.text.lock().unwrap_or_else(|e| e.into_inner()) = Some(server);
        *self.resident.lock().unwrap_or_else(|e| e.into_inner()) = Resident::Text;
        Ok(url)
    }

    fn drop_image_context(&self) {
        // Контекст живёт внутри вызова generate, отдельного состояния он не оставляет:
        // достаточно того, что мы его не удерживаем между задачами.
        let mut resident = self.resident.lock().unwrap_or_else(|e| e.into_inner());
        if *resident == Resident::Image {
            *resident = Resident::Nobody;
        }
    }

    /// Загруженный движок озвучки. Первый вызов грузит веса, дальше отдаётся тот же.
    pub fn speech_engine(&self) -> Result<Arc<du_tts::AudiocppEngine>, GpuError> {
        {
            let speech = self.speech.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(engine) = speech.as_ref() {
                return Ok(engine.clone());
            }
        }
        let dll = self.root.join("models").join("tts-engine").join("audiocpp_engine.dll");
        let model_root = self.root.join("models").join("tts");
        if !dll.is_file() {
            return Err(GpuError::Missing("движок озвучки не установлен".into()));
        }
        let engine = du_tts::AudiocppEngine::load(&dll)
            .map_err(|error| GpuError::Missing(error.to_string()))?;
        engine
            .load_model(&model_root, "cuda", 0, 8, None)
            .map_err(|error| GpuError::Missing(error.to_string()))?;
        let engine = Arc::new(engine);
        *self.speech.lock().unwrap_or_else(|e| e.into_inner()) = Some(engine.clone());
        Ok(engine)
    }

    /// Освободить карту от озвучки.
    fn release_speech(&self) {
        let mut speech = self.speech.lock().unwrap_or_else(|e| e.into_inner());
        if speech.take().is_some() {
            tracing::info!("освобождаю карту: выгружаю движок озвучки");
        }
    }

    /// Нарисовать кадр. Перед загрузкой весов карта освобождается от текстовой модели.
    ///
    /// Контекст создаётся на время вызова и снимается сразу после: держать 13 ГБ весов между
    /// ходами нельзя — следующий ход нарратора не получит памяти.
    pub fn generate_image(
        &self,
        params: &GenParams,
        progress: Option<Box<dyn Fn(i32, i32, f32) + Send + Sync>>,
    ) -> Result<Vec<RawImage>, GpuError> {
        if !self.paths.image_ready() {
            return Err(GpuError::Missing(format!(
                "нет весов картинок в {}",
                self.paths.image_models.display()
            )));
        }
        self.release_text();
        // Картинке нужна вся карта: и текст, и озвучка уходят с неё.
        self.release_speech();
        *self.resident.lock().unwrap_or_else(|e| e.into_inner()) = Resident::Image;

        let mut engine_slot = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        if engine_slot.is_none() {
            *engine_slot = Some(Engine::load(&self.paths.image_runtime)?);
        }
        let engine = engine_slot.as_ref().expect("движок только что загружен");

        let runtime = crate::runtime::load(&self.root);
        let config = image_config(self.paths.krea2(), &runtime);
        let mut context = engine.new_context(&config)?;
        let images = context.generate(params, progress)?;
        drop(context); // веса уходят с карты немедленно

        *self.resident.lock().unwrap_or_else(|e| e.into_inner()) = Resident::Nobody;
        Ok(images)
    }

    /// Сведения о движке картинок — ТОЛЬКО если он уже загружен.
    ///
    /// Раньше эта справка сама грузила движок: страница состояния опрашивает её при каждом
    /// открытии, и `sd.cpp` поднимал CUDA прямо в процессе сервера — видеокарта занята
    /// движком, которым игрок не пользуется, а кадры при этом рисует облако. Ради подписи
    /// в интерфейсе грузить движок незачем: не загружен — так и скажем.
    pub fn image_info(&self) -> Option<(String, Vec<String>)> {
        let engine_slot = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        engine_slot
            .as_ref()
            .map(|engine| (engine.version(), engine.devices()))
    }
}


/// Сколько слоёв рассказчика отдать карте.
///
/// Выбор устройства — общий, а не отдельный тумблер про слои: на процессоре слоёв на карте
/// ноль, иначе столько, сколько попрошено (−1 — все).
pub fn narrator_layers(runtime: &crate::runtime::Runtime) -> i32 {
    match crate::backend::stage_backend(runtime, crate::backend::Stage::Narrator) {
        crate::backend::Backend::Cpu => 0,
        crate::backend::Backend::Gpu => runtime.narrator_gpu_layers,
    }
}

/// Настройки движка картинок под выбранное устройство.
pub fn image_config(
    mut config: du_image::ModelConfig,
    runtime: &crate::runtime::Runtime,
) -> du_image::ModelConfig {
    let where_to = crate::backend::stage_backend(runtime, crate::backend::Stage::Image);
    config.backend = Some(where_to.engine_name().to_string());
    // Выгрузка весов в память имеет смысл только когда считает карта: на процессоре они и
    // так там, а поблочная подача лишь замедляет.
    config.offload_to_cpu = !where_to.is_cpu() && runtime.image_offload_to_cpu;
    config.max_vram = Some(runtime.image_max_vram.clone());
    config
}

#[cfg(test)]
mod tests {

    #[test]
    fn the_narrator_follows_the_chosen_device() {
        use crate::runtime::Runtime;
        let on_gpu = Runtime { local_backend: "gpu".into(), narrator_gpu_layers: -1, ..Default::default() };
        assert_eq!(super::narrator_layers(&on_gpu), -1, "на карте — все слои");

        let on_cpu = Runtime { local_backend: "cpu".into(), narrator_gpu_layers: -1, ..Default::default() };
        assert_eq!(super::narrator_layers(&on_cpu), 0, "на процессоре карте не достаётся ничего");

        // Своя настройка стадии перебивает общую.
        let mixed = Runtime {
            local_backend: "cpu".into(),
            narrator_backend: "gpu".into(),
            narrator_gpu_layers: 20,
            ..Default::default()
        };
        assert_eq!(super::narrator_layers(&mixed), 20);
    }

    #[test]
    fn the_image_engine_gets_the_chosen_device() {
        use crate::runtime::Runtime;
        let base = || du_image::ModelConfig::krea2("d".into(), "l".into(), "v".into());

        let on_gpu = Runtime { local_backend: "gpu".into(), image_offload_to_cpu: true, ..Default::default() };
        let config = super::image_config(base(), &on_gpu);
        assert_eq!(config.backend.as_deref(), Some("cuda"));
        assert!(config.offload_to_cpu, "на карте выгрузка весов в память сохраняется");

        let on_cpu = Runtime { local_backend: "cpu".into(), image_offload_to_cpu: true, ..Default::default() };
        let config = super::image_config(base(), &on_cpu);
        assert_eq!(config.backend.as_deref(), Some("cpu"));
        assert!(!config.offload_to_cpu, "на процессоре выгружать некуда");

        // Стадия важнее общего: картинки на карте, всё остальное на процессоре.
        let mixed = Runtime { local_backend: "cpu".into(), image_backend: "gpu".into(), ..Default::default() };
        assert_eq!(super::image_config(base(), &mixed).backend.as_deref(), Some("cuda"));
    }
    use super::*;

    #[test]
    fn paths_are_derived_from_the_application_root() {
        let paths = Paths::under(Path::new("D:/приложение"));
        assert!(paths.image_models.ends_with("models/image") || paths.image_models.ends_with("models\\image"));
        assert!(paths.edit_lora().to_string_lossy().contains("identity-edit"));
    }

    #[test]
    fn a_missing_checkpoint_is_reported_before_the_card_is_touched() {
        let root = Path::new("D:/нет-такого-каталога");
        let gpu = Gpu::new(root, Paths::under(root));
        assert!(!gpu.paths().image_ready());
        let error = gpu.generate_image(&GenParams::default(), None).unwrap_err();
        assert!(matches!(error, GpuError::Missing(_)));
        assert_eq!(gpu.resident(), Resident::Nobody, "карта не должна была занятья");
    }

    #[test]
    fn a_missing_narrator_is_reported_too() {
        let root = Path::new("D:/нет-такого-каталога");
        let gpu = Gpu::new(root, Paths::under(root));
        assert!(matches!(gpu.text_base_url(), Err(GpuError::Missing(_))));
    }
}
