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

use crate::errors::UserError;

/// Кто держит карту прямо сейчас.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resident {
    Nobody,
    Text,
    Image,
    /// Движок озвучки: он тоже занимает карту, и это должно быть видно снаружи.
    Speech,
}

/// Где лежат веса и рантайм-библиотеки.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Корень игры: сборки движков под разные карты лежат в своих папках под ним.
    pub root: PathBuf,
    /// Каталог с весами картинок.
    pub image_models: PathBuf,
    /// GGUF нарратора.
    pub text_model: PathBuf,
    /// Проектор для мультимодальности; без него модель не увидит портреты.
    pub text_mmproj: Option<PathBuf>,
}

/// Сообщение о ходе рисования: шаг, всего шагов, доля готовности.
pub type ImageProgressFn = Box<dyn Fn(i32, i32, f32) + Send + Sync>;

impl Paths {
    pub fn under(root: &Path) -> Self {
        let models = root.join("models");
        Self {
            root: root.to_path_buf(),
            image_models: models.join("image"),
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
        let accel = crate::accel::accel(&crate::runtime::load(&self.root));
        self.text_model.is_file() && du_llm::resolve_llama_bin(&llama_dir(&self.root, accel)).is_file()
    }
}

/// Сборка llama.cpp под эту машину: CUDA 13, CUDA 12, Vulkan (AMD, Intel) или процессор — у каждой своя папка, и
/// одноимённые `ggml-*.dll` разных сборок не смешиваются.
pub fn llama_dir(root: &Path, accel: crate::accel::Accel) -> PathBuf {
    let tools = root.join("tools");
    match accel {
        crate::accel::Accel::Cuda13 => tools.join("llama"),
        crate::accel::Accel::Cuda12 => tools.join("llama-cuda12"),
        crate::accel::Accel::Vulkan => tools.join("llama-vulkan"),
        crate::accel::Accel::Cpu => tools.join("llama-cpu"),
    }
}

/// Сборка sd.cpp: CUDA 12 на любой годной карте NVIDIA, Vulkan на остальных; на процессоре кадр не рисуется — кроме
/// Mac, где одна сборка с Metal и машина считается процессорной.
pub fn image_dir(root: &Path, accel: crate::accel::Accel) -> Option<PathBuf> {
    let runtime = root.join("models").join("runtime");
    match accel {
        crate::accel::Accel::Cuda13 | crate::accel::Accel::Cuda12 => Some(runtime.join("sd")),
        crate::accel::Accel::Vulkan => Some(runtime.join("sd-vulkan")),
        crate::accel::Accel::Cpu if cfg!(target_os = "macos") => Some(runtime.join("sd")),
        crate::accel::Accel::Cpu => None,
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
    speech: Mutex<Option<Arc<acpp_server::Server>>>,
    resident: Mutex<Resident>,
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
    pub fn text_base_url(&self) -> Result<String, UserError> {
        // Настройки читаем при КАЖДОМ обращении: изменил устройство или контекст — следующий
        // запрос уже идёт на сервер с ними.
        let runtime = crate::runtime::load(&self.root);
        let accel = crate::accel::accel(&runtime);
        let bin = du_llm::resolve_llama_bin(&llama_dir(&self.root, accel));
        let mut opts = ServerOpts::new(bin, self.paths.text_model.clone())
            // Зрение подключаем, только если файл кто-то положил руками: игра картинки
            // рассказчику не показывает — приложенное игроком уходит в конвейер кадра, а
            // ему достаётся лишь строка с именем файла. Качать эти 180 МБ было незачем.
            .with_mmproj(self.paths.text_mmproj.clone().filter(|path| path.is_file()))
            .with_ctx(runtime.narrator_ctx);
        opts.n_gpu_layers = narrator_layers(&runtime);
        opts.device = narrator_device(&runtime);
        // VC++ runtime установщик кладёт рядом с exe игры; на Linux cudart и cuBLAS сборки llama лежат в общей папке
        // CUDA-рантайма (Windows-сборке их приносит её собственный архив).
        opts.library_dirs = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)).into_iter().collect();
        if cfg!(target_os = "linux") {
            if let Some(dir) = crate::acpp::library_dir(&self.root, crate::acpp::build(accel)) {
                opts.library_dirs.push(dir);
            }
        }
        {
            let mut text = self.text.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(server) = text.as_mut() {
                // Живой и запущенный с теми же настройками — он и отвечает. Умерший (нехватка
                // памяти, падение) отдавал бы мёртвый адрес каждому следующему ходу.
                match server.ended() {
                    Some(why) => tracing::warn!("llama-server завершился сам: {why}; поднимаю заново"),
                    None if server.launch_key() == opts.launch_key() => return Ok(server.base_url().to_string()),
                    None => tracing::info!("рассказчик: настройки устройства изменились, перезапускаю llama-server"),
                }
                if let Some(mut old) = text.take() {
                    old.stop();
                }
            }
        }
        if !self.paths.text_ready() {
            return Err(UserError::TextModelMissing(self.paths.text_model.display().to_string()));
        }
        if accel.nvidia() && opts.device.is_none() {
            if let Some(problem) = crate::accel::missing_chosen_card(&runtime) {
                return Err(problem);
            }
        }
        // Картинка обязана уйти с карты до загрузки нарратора.
        self.drop_image_context();

        // Голос уходит с карты перед рассказчиком — по той же причине, что и наоборот.
        self.release_speech();
        let server = LlamaServer::start(&opts).map_err(|error| UserError::Llm { error, starting: true })?;
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

    /// Сервер голоса (audiocpp_server с Higgs). Первая фраза грузит веса, дальше говорит тот же сервер.
    pub fn speech_engine(&self) -> Result<Arc<acpp_server::Server>, UserError> {
        {
            let speech = self.speech.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(server) = speech.as_ref().filter(|server| server.alive()) {
                return Ok(server.clone());
            }
        }
        let runtime = crate::runtime::load(&self.root);
        let on_cpu = crate::backend::stage_backend(&runtime, crate::backend::Stage::Tts).is_cpu();
        let server = crate::acpp::server(&self.root, crate::acpp::HIGGS, on_cpu)?;
        // Голосу нужна карта — значит текстовая модель с неё уходит. Без этого Гемма на 8 ГБ
        // и голос на 4 ГБ жили на карте одновременно, всё упиралось в память и ход занимал
        // одиннадцать минут вместо десятков секунд. Кадр так делал давно, озвучка — нет.
        if !on_cpu {
            self.release_text();
        }
        *self.resident.lock().unwrap_or_else(|e| e.into_inner()) = Resident::Speech;
        *self.speech.lock().unwrap_or_else(|e| e.into_inner()) = Some(server.clone());
        Ok(server)
    }

    /// Освободить карту от озвучки: веса Higgs уходят с памяти, сам сервер остаётся до следующей фразы.
    fn release_speech(&self) {
        let mut speech = self.speech.lock().unwrap_or_else(|e| e.into_inner());
        if speech.take().is_some() {
            tracing::info!("освобождаю карту: выгружаю голос");
            crate::acpp::unload(&[crate::acpp::HIGGS]);
        }
        let mut resident = self.resident.lock().unwrap_or_else(|e| e.into_inner());
        if *resident == Resident::Speech {
            *resident = Resident::Nobody;
        }
    }

    /// Нарисовать кадр. Перед загрузкой весов карта освобождается от текстовой модели.
    ///
    /// Контекст создаётся на время вызова и снимается сразу после: держать 13 ГБ весов между
    /// ходами нельзя — следующий ход нарратора не получит памяти.
    pub fn generate_image(
        &self,
        params: &GenParams,
        progress: Option<ImageProgressFn>,
    ) -> Result<Vec<RawImage>, UserError> {
        if !self.paths.image_ready() {
            return Err(UserError::ImageWeightsMissing(self.paths.image_models.display().to_string()));
        }
        let runtime = crate::runtime::load(&self.root);
        let accel = crate::accel::accel(&runtime);
        let Some(dir) = image_dir(&self.root, accel) else {
            return Err(UserError::NeedsGpu(crate::errors::CloudStage::Image));
        };
        if accel.nvidia() {
            if let Some(problem) = crate::accel::missing_chosen_card(&runtime) {
                return Err(problem);
            }
        }
        self.release_text();
        // Картинке нужна вся карта: и текст, и озвучка уходят с неё.
        self.release_speech();
        *self.resident.lock().unwrap_or_else(|e| e.into_inner()) = Resident::Image;
        let mut engine_slot = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        if engine_slot.is_none() {
            tracing::info!("картинки: sd.cpp из {} ({})", dir.display(), accel.key());
            *engine_slot = Some(Engine::load(&dir)?);
        }
        let engine = engine_slot.as_ref().expect("движок только что загружен");

        let config = image_config(self.paths.krea2(), &runtime, accel);
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
/// `--device` рассказчика: на процессоре — `none`, иначе llama.cpp отдаёт вычисления карте и
/// при `-ngl 0`.
pub fn narrator_device(runtime: &crate::runtime::Runtime) -> Option<String> {
    crate::backend::stage_backend(runtime, crate::backend::Stage::Narrator).is_cpu().then(|| "none".to_string())
}

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
    accel: crate::accel::Accel,
) -> du_image::ModelConfig {
    let where_to = crate::backend::stage_backend(runtime, crate::backend::Stage::Image);
    // Имя реестра ggml: сборка под Vulkan знает «vulkan», под CUDA — «cuda».
    config.backend = Some(if accel == crate::accel::Accel::Vulkan { "vulkan".to_string() } else { where_to.engine_name().to_string() });
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
        let config = super::image_config(base(), &on_gpu, crate::accel::Accel::Cuda13);
        assert_eq!(config.backend.as_deref(), Some("cuda"));
        assert!(config.offload_to_cpu, "на карте выгрузка весов в память сохраняется");
        let config = super::image_config(base(), &on_gpu, crate::accel::Accel::Vulkan);
        assert_eq!(config.backend.as_deref(), Some("vulkan"));

        // Кадр не уводится на процессор ДАЖЕ по прямому указанию: там он считается десятки
        // минут вместо десятка секунд.
        let on_cpu = Runtime {
            local_backend: "cpu".into(),
            image_backend: "cpu".into(),
            image_offload_to_cpu: true,
            ..Default::default()
        };
        let config = super::image_config(base(), &on_cpu, crate::accel::Accel::Cuda13);
        assert_eq!(config.backend.as_deref(), Some("cuda"));
        assert!(config.offload_to_cpu, "выгрузка весов в память при этом сохраняется");
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
        assert!(matches!(error, UserError::ImageWeightsMissing(_)));
        assert_eq!(gpu.resident(), Resident::Nobody, "карта не должна была занятья");
    }

    #[test]
    fn a_missing_narrator_is_reported_too() {
        let root = Path::new("D:/нет-такого-каталога");
        let gpu = Gpu::new(root, Paths::under(root));
        assert!(matches!(gpu.text_base_url(), Err(UserError::TextModelMissing(_))));
    }
}
