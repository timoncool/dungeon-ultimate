//! Генерация картинок на движке [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp).
//!
//! Библиотека подключается ДИНАМИЧЕСКИ (`stable-diffusion.dll` рядом с `ggml-cuda.dll`),
//! как `audiocpp_engine.dll` в Dub Studio: cmake и CUDA-тулчейн в нашей сборке не нужны,
//! а версию движка можно менять, не пересобирая приложение.
//!
//! Модель одна — Krea-2 Turbo: она же рисует сцену с нуля, она же редактирует предыдущий
//! кадр по референсу (пресеты `krea2_ostris_edit` / `krea2_edit` плюс edit-LoRA), поэтому
//! второй набор весов на GPU держать не нужно.

pub mod ffi;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ffi::*;

/// Коммит движка, под который написан слой FFI. Раскладка структур обязана совпадать,
/// поэтому на старте фактический коммит сверяется и расхождение попадает в лог.
pub const PINNED_COMMIT: &str = "de298c2";

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("движок картинок не найден: {0}")]
    LibraryMissing(PathBuf),
    #[error("не удалось загрузить движок картинок: {0}")]
    Library(#[from] libloading::Error),
    #[error("в пути к модели недопустимый нулевой байт: {0}")]
    BadPath(String),
    #[error("не удалось создать контекст модели — проверь пути к весам, энкодеру и VAE")]
    ContextCreation,
    #[error("генерация не удалась")]
    GenerationFailed,
    #[error("генерация отменена")]
    Cancelled,
}

/// Готовый кадр в виде сырых пикселей.
#[derive(Debug, Clone)]
pub struct RawImage {
    pub width: u32,
    pub height: u32,
    /// Обычно 3 (RGB).
    pub channels: u32,
    pub data: Vec<u8>,
}

/// Куда движку класть веса и как экономить VRAM.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Трансформер диффузии в GGUF (квант Q4_K_M — тот же, что у текстовой модели в Dub Studio).
    pub diffusion_model: PathBuf,
    /// Текстовый энкодер (для Krea-2 — Qwen3-VL-4B; abliterated снимает отказы).
    pub llm: PathBuf,
    pub vae: PathBuf,
    pub vae_format: SdVaeFormat,
    /// Flash-attention в диффузии: на CUDA даёт и меньше памяти, и выше скорость.
    pub diffusion_flash_attn: bool,
    /// Держать веса в RAM и подавать их в бэкенд по мере надобности.
    pub offload_to_cpu: bool,
    /// Бюджет VRAM в ГиБ для посегментного прохода; «-1» — авто с запасом ~1 ГиБ.
    pub max_vram: Option<String>,
    /// Потоковая подача блоков трансформера — включается только вместе с offload.
    pub stream_layers: bool,
    pub n_threads: i32,
}

impl ModelConfig {
    /// Раскладка Krea-2 Turbo: VAE от Wan 2.1, энкодер Qwen3-VL-4B.
    pub fn krea2(diffusion_model: PathBuf, llm: PathBuf, vae: PathBuf) -> Self {
        Self {
            diffusion_model,
            llm,
            vae,
            vae_format: SdVaeFormat::Wan,
            diffusion_flash_attn: true,
            offload_to_cpu: true,
            max_vram: Some("-1".to_string()),
            stream_layers: false,
            n_threads: -1,
        }
    }
}

/// LoRA поверх базовой модели. Для INT8-весов применяется в рантайме и сами веса не трогает.
#[derive(Debug, Clone)]
pub struct LoraSpec {
    pub path: PathBuf,
    pub multiplier: f32,
}

/// Параметры одного кадра.
#[derive(Debug, Clone)]
pub struct GenParams {
    pub prompt: String,
    pub negative_prompt: String,
    pub width: i32,
    pub height: i32,
    pub steps: i32,
    /// Krea-2 Turbo дистиллирована, ей нужен cfg 1.0.
    pub cfg_scale: f32,
    pub seed: i64,
    pub sample_method: SampleMethod,
    pub scheduler: Scheduler,
    /// Референсы для редактирования: предыдущий кадр локации, портрет героя, предмет.
    pub ref_images: Vec<RawImage>,
    /// Пресет обработки референсов, например `preset=krea2_ostris_edit`.
    pub ref_image_args: Option<String>,
    pub loras: Vec<LoraSpec>,
}

impl Default for GenParams {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: String::new(),
            width: 1024,
            height: 1024,
            steps: 8,
            cfg_scale: 1.0,
            seed: -1,
            sample_method: SampleMethod::Euler,
            scheduler: Scheduler::Simple,
            ref_images: Vec::new(),
            ref_image_args: None,
            loras: Vec::new(),
        }
    }
}

type ProgressSink = Box<dyn Fn(i32, i32, f32) + Send + Sync>;

/// Колбэк прогресса у движка глобальный, поэтому и слот здесь один. Генерации всё равно
/// идут строго по одной — GPU делится с текстовой моделью и озвучкой.
static PROGRESS: Mutex<Option<ProgressSink>> = Mutex::new(None);

extern "C" fn progress_trampoline(step: c_int, steps: c_int, time: f32, _data: *mut c_void) {
    if let Ok(guard) = PROGRESS.lock() {
        if let Some(sink) = guard.as_ref() {
            sink(step, steps, time);
        }
    }
}

extern "C" fn log_trampoline(level: SdLogLevel, text: *const c_char, _data: *mut c_void) {
    if text.is_null() {
        return;
    }
    let message = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    let message = message.trim_end();
    match level {
        SdLogLevel::Debug => tracing::debug!(target: "sd", "{message}"),
        SdLogLevel::Info => tracing::info!(target: "sd", "{message}"),
        SdLogLevel::Warn => tracing::warn!(target: "sd", "{message}"),
        SdLogLevel::Error => tracing::error!(target: "sd", "{message}"),
    }
}

macro_rules! symbols {
    ($($name:ident: $ty:ty),* $(,)?) => {
        struct Symbols {
            $($name: $ty,)*
        }
        impl Symbols {
            unsafe fn load(lib: &libloading::Library) -> Result<Self, libloading::Error> {
                Ok(Self {
                    $($name: *lib.get(concat!(stringify!($name), "\0").as_bytes())?,)*
                })
            }
        }
    };
}

symbols! {
    sd_version: unsafe extern "C" fn() -> *const c_char,
    sd_commit: unsafe extern "C" fn() -> *const c_char,
    sd_list_devices: unsafe extern "C" fn(*mut c_char, usize) -> usize,
    sd_set_log_callback: unsafe extern "C" fn(SdLogCb, *mut c_void),
    sd_set_progress_callback: unsafe extern "C" fn(SdProgressCb, *mut c_void),
    sd_ctx_params_init: unsafe extern "C" fn(*mut SdCtxParams),
    sd_img_gen_params_init: unsafe extern "C" fn(*mut SdImgGenParams),
    new_sd_ctx: unsafe extern "C" fn(*const SdCtxParams) -> *mut sd_ctx_t,
    free_sd_ctx: unsafe extern "C" fn(*mut sd_ctx_t),
    generate_image: unsafe extern "C" fn(*mut sd_ctx_t, *const SdImgGenParams, *mut *mut SdImage, *mut c_int) -> bool,
    free_sd_images: unsafe extern "C" fn(*mut SdImage, c_int),
    sd_cancel_generation: unsafe extern "C" fn(*mut sd_ctx_t, SdCancelMode),
}

/// Загруженная библиотека движка. Держать её нужно ровно столько, сколько живут контексты.
pub struct Engine {
    // Библиотеки обязаны пережить символы, поэтому лежат перед ними и роняются последними.
    _ggml: libloading::Library,
    _lib: libloading::Library,
    symbols: Symbols,
}

impl Engine {
    /// Загрузить движок. `dir` — каталог с `stable-diffusion.dll` и её зависимостями
    /// (`ggml-cuda.dll` и прочими); они грузятся оттуда же.
    pub fn load(dir: &Path) -> Result<Self, ImageError> {
        let dll = dir.join("stable-diffusion.dll");
        if !dll.is_file() {
            return Err(ImageError::LibraryMissing(dll));
        }
        // У бэкенда CUDA свои зависимости (`cudart64_12.dll`, `cublas64_12.dll`), а их
        // загрузчик ищет по PATH и рядом с EXE процесса, но НЕ рядом с самой библиотекой.
        // Без этого ggml-cuda.dll молча не грузится и остаётся только CPU.
        augment_search_path(dir);

        // Бэкенды ggml (CUDA, CPU) подключаются динамически и по умолчанию ищутся рядом с
        // EXE процесса. У нас движок лежит отдельным каталогом, поэтому регистрируем их
        // явно оттуда — иначе в процессе не будет видно ни одного устройства.
        let ggml = unsafe { load_library(&dir.join("ggml.dll"))? };
        unsafe {
            let load_all: unsafe extern "C" fn(*const c_char) =
                *ggml.get(b"ggml_backend_load_all_from_path\0")?;
            if let Ok(dir_c) = CString::new(dir.to_string_lossy().into_owned()) {
                load_all(dir_c.as_ptr());
            }
        }

        let lib = unsafe { load_library(&dll)? };
        let symbols = unsafe { Symbols::load(&lib)? };
        let engine = Engine { _ggml: ggml, _lib: lib, symbols };
        unsafe {
            (engine.symbols.sd_set_log_callback)(log_trampoline, std::ptr::null_mut());
            (engine.symbols.sd_set_progress_callback)(progress_trampoline, std::ptr::null_mut());
        }
        let commit = engine.commit();
        if !commit.starts_with(PINNED_COMMIT) {
            tracing::warn!(
                "движок картинок собран из {commit}, а слой FFI написан под {PINNED_COMMIT} — \
                 сверь раскладку структур в stable-diffusion.h"
            );
        }
        Ok(engine)
    }

    pub fn version(&self) -> String {
        unsafe { cstr_to_string((self.symbols.sd_version)()) }
    }

    pub fn commit(&self) -> String {
        unsafe { cstr_to_string((self.symbols.sd_commit)()) }
    }

    /// Список доступных устройств: строки вида `name<TAB>description`.
    pub fn devices(&self) -> Vec<String> {
        unsafe {
            let needed = (self.symbols.sd_list_devices)(std::ptr::null_mut(), 0);
            if needed == 0 {
                return Vec::new();
            }
            let mut buffer = vec![0u8; needed + 1];
            (self.symbols.sd_list_devices)(buffer.as_mut_ptr() as *mut c_char, buffer.len());
            let text = CStr::from_ptr(buffer.as_ptr() as *const c_char).to_string_lossy().into_owned();
            text.lines().filter(|line| !line.trim().is_empty()).map(str::to_string).collect()
        }
    }

    /// Загрузить модель в память. Дорого — контекст живёт до явного `drop`, который и
    /// освобождает VRAM под текстовую модель.
    pub fn new_context(&self, config: &ModelConfig) -> Result<Context<'_>, ImageError> {
        // Строки обязаны пережить вызов, поэтому держим их до конца функции.
        let diffusion = path_to_cstring(&config.diffusion_model)?;
        let llm = path_to_cstring(&config.llm)?;
        let vae = path_to_cstring(&config.vae)?;
        let max_vram = match config.max_vram.as_deref() {
            Some(value) => Some(CString::new(value).map_err(|_| ImageError::BadPath(value.into()))?),
            None => None,
        };
        // `*=cpu` — тот же смысл, что у флага --offload-to-cpu в CLI движка.
        let params_backend = if config.offload_to_cpu { Some(CString::new("*=cpu").unwrap()) } else { None };

        let mut params = unsafe {
            let mut params = std::mem::zeroed::<SdCtxParams>();
            (self.symbols.sd_ctx_params_init)(&mut params);
            params
        };
        params.diffusion_model_path = diffusion.as_ptr();
        params.llm_path = llm.as_ptr();
        params.vae_path = vae.as_ptr();
        params.vae_format = config.vae_format;
        params.diffusion_flash_attn = config.diffusion_flash_attn;
        params.n_threads = config.n_threads;
        params.stream_layers = config.stream_layers && config.offload_to_cpu;
        // LoRA поверх INT8 обязана применяться в рантайме, иначе движку пришлось бы
        // деквантовать и переквантовать веса.
        params.lora_apply_mode = LoraApplyMode::Auto;
        if let Some(value) = max_vram.as_ref() {
            params.max_vram = value.as_ptr();
        }
        if let Some(value) = params_backend.as_ref() {
            params.params_backend = value.as_ptr();
        }

        let ctx = unsafe { (self.symbols.new_sd_ctx)(&params) };
        if ctx.is_null() {
            return Err(ImageError::ContextCreation);
        }
        Ok(Context { engine: self, ctx })
    }
}

/// Загруженная модель. `Drop` освобождает VRAM — на этом держится правило «одна модель
/// на карте»: перед ходом нарратора контекст картинок роняется.
pub struct Context<'a> {
    engine: &'a Engine,
    ctx: *mut sd_ctx_t,
}

// Указатель контекста не разделяется между потоками одновременно: генерации сериализованы
// очередью, а `&mut self` в `generate` не даёт двум вызовам пересечься.
unsafe impl Send for Context<'_> {}

impl Context<'_> {
    /// Сгенерировать кадр. `progress` зовут на каждом шаге сэмплера.
    pub fn generate(
        &mut self,
        params: &GenParams,
        progress: Option<ProgressSink>,
    ) -> Result<Vec<RawImage>, ImageError> {
        let prompt = CString::new(params.prompt.as_str())
            .map_err(|_| ImageError::BadPath("prompt".into()))?;
        let negative = CString::new(params.negative_prompt.as_str())
            .map_err(|_| ImageError::BadPath("negative_prompt".into()))?;
        let ref_args = match params.ref_image_args.as_deref() {
            Some(value) => Some(CString::new(value).map_err(|_| ImageError::BadPath(value.into()))?),
            None => None,
        };
        let lora_paths: Vec<CString> = params
            .loras
            .iter()
            .map(|lora| path_to_cstring(&lora.path))
            .collect::<Result<_, _>>()?;
        let loras: Vec<SdLora> = params
            .loras
            .iter()
            .zip(lora_paths.iter())
            .map(|(lora, path)| SdLora { is_high_noise: false, multiplier: lora.multiplier, path: path.as_ptr() })
            .collect();

        // Референсы отдаём указателями на НАШИ буферы: движок только читает их на время вызова.
        let mut ref_buffers: Vec<Vec<u8>> = params.ref_images.iter().map(|image| image.data.clone()).collect();
        let mut refs: Vec<SdImage> = params
            .ref_images
            .iter()
            .zip(ref_buffers.iter_mut())
            .map(|(image, buffer)| SdImage {
                width: image.width,
                height: image.height,
                channel: image.channels,
                data: buffer.as_mut_ptr(),
            })
            .collect();

        let mut gen = unsafe {
            let mut gen = std::mem::zeroed::<SdImgGenParams>();
            (self.engine.symbols.sd_img_gen_params_init)(&mut gen);
            gen
        };
        gen.prompt = prompt.as_ptr();
        gen.negative_prompt = negative.as_ptr();
        gen.width = params.width;
        gen.height = params.height;
        gen.seed = params.seed;
        gen.batch_count = 1;
        gen.sample_params.sample_steps = params.steps;
        gen.sample_params.sample_method = params.sample_method;
        gen.sample_params.scheduler = params.scheduler;
        gen.sample_params.guidance.txt_cfg = params.cfg_scale;
        if !refs.is_empty() {
            gen.ref_images = refs.as_mut_ptr();
            gen.ref_images_count = refs.len() as c_int;
        }
        if let Some(args) = ref_args.as_ref() {
            gen.ref_image_args = args.as_ptr();
        }
        if !loras.is_empty() {
            gen.loras = loras.as_ptr();
            gen.lora_count = loras.len() as u32;
        }

        set_progress_sink(progress);
        let mut out: *mut SdImage = std::ptr::null_mut();
        let mut count: c_int = 0;
        let ok = unsafe { (self.engine.symbols.generate_image)(self.ctx, &gen, &mut out, &mut count) };
        set_progress_sink(None);

        if !ok || out.is_null() || count <= 0 {
            return Err(ImageError::GenerationFailed);
        }

        // Байты копируем к себе и сразу отдаём буферы движку: память выделена его CRT,
        // освобождать её обязан он сам.
        let mut images = Vec::with_capacity(count as usize);
        unsafe {
            for index in 0..count as isize {
                let raw = *out.offset(index);
                let len = (raw.width * raw.height * raw.channel) as usize;
                let data = if raw.data.is_null() || len == 0 {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(raw.data, len).to_vec()
                };
                images.push(RawImage { width: raw.width, height: raw.height, channels: raw.channel, data });
            }
            (self.engine.symbols.free_sd_images)(out, count);
        }
        Ok(images)
    }

    /// Оборвать текущую генерацию. Зовётся из другого потока, пока `generate` работает.
    pub fn cancel(&self) {
        unsafe { (self.engine.symbols.sd_cancel_generation)(self.ctx, SdCancelMode::All) };
    }
}

impl Drop for Context<'_> {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { (self.engine.symbols.free_sd_ctx)(self.ctx) };
            self.ctx = std::ptr::null_mut();
        }
    }
}

fn set_progress_sink(sink: Option<ProgressSink>) {
    if let Ok(mut guard) = PROGRESS.lock() {
        *guard = sink;
    }
}

unsafe fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

fn path_to_cstring(path: &Path) -> Result<CString, ImageError> {
    let text = path.to_string_lossy().into_owned();
    CString::new(text.clone()).map_err(|_| ImageError::BadPath(text))
}

/// Добавить каталог движка в начало PATH процесса, чтобы зависимости бэкендов
/// находились. Идемпотентно: повторная загрузка движка не раздувает переменную.
fn augment_search_path(dir: &Path) {
    let dir = dir.to_string_lossy();
    // Канонизированный путь приходит с префиксом `\\?\`, который загрузчик в PATH не понимает.
    let dir = dir.strip_prefix(r"\\?\").unwrap_or(&dir);
    let current = std::env::var("PATH").unwrap_or_default();
    if current.split(';').any(|entry| entry.eq_ignore_ascii_case(dir)) {
        return;
    }
    std::env::set_var("PATH", format!("{dir};{current}"));
}

/// На Windows зависимости (`ggml-cuda.dll` и прочие) обязаны искаться РЯДОМ с самой
/// библиотекой, а не в каталоге процесса, — иначе загрузка падает на первой же зависимости.
#[cfg(windows)]
unsafe fn load_library(path: &Path) -> Result<libloading::Library, libloading::Error> {
    use libloading::os::windows::{Library, LOAD_WITH_ALTERED_SEARCH_PATH};
    Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH).map(Into::into)
}

#[cfg(not(windows))]
unsafe fn load_library(path: &Path) -> Result<libloading::Library, libloading::Error> {
    libloading::Library::new(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Каталог движка в репозитории; в CI без весов его может не быть.
    fn runtime_dir() -> PathBuf {
        std::env::var_os("DU_SD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../models/runtime/sd")
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from("models/runtime/sd"))
            })
    }

    #[test]
    fn engine_loads_and_reports_a_cuda_device() {
        let dir = runtime_dir();
        if !dir.join("stable-diffusion.dll").is_file() {
            eprintln!("движок не разложен в {dir:?} — проверка пропущена");
            return;
        }
        let engine = Engine::load(&dir).expect("движок обязан загрузиться");
        let version = engine.version();
        let commit = engine.commit();
        assert!(!version.is_empty(), "версия движка пустая");
        assert!(
            commit.starts_with(PINNED_COMMIT),
            "движок собран из {commit}, а FFI написан под {PINNED_COMMIT}"
        );
        let devices = engine.devices();
        assert!(!devices.is_empty(), "движок не увидел ни одного устройства");
        assert!(
            devices.iter().any(|device| device.to_uppercase().contains("CUDA")),
            "среди устройств нет CUDA: {devices:?}"
        );
    }

    #[test]
    fn missing_library_is_reported_as_such() {
        let result = Engine::load(Path::new("не-существует"));
        assert!(matches!(result.err(), Some(ImageError::LibraryMissing(_))));
    }
}
