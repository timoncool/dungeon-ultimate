//! Сырой C-ABI слой `stable-diffusion.h`.
//!
//! Раскладка структур обязана СОВПАДАТЬ с той, под которую собрана DLL, поэтому поля
//! перечислены здесь в том же порядке и с теми же типами, что в заголовке пиннутого
//! релиза (см. `PINNED_COMMIT` в `lib.rs`). Значения по умолчанию мы не выдумываем —
//! их заполняют сами функции `*_params_init` из библиотеки, а мы поверх меняем нужные
//! поля. Так расхождение с апстримом не приводит к молчаливому мусору в параметрах.

#![allow(non_camel_case_types)]

use std::os::raw::{c_char, c_int, c_void};

pub type sd_ctx_t = c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdLogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

/// Совпадает с `enum ggml_type`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdType {
    F32 = 0,
    F16 = 1,
    Q4_0 = 2,
    Q5_0 = 6,
    Q8_0 = 8,
    Q4_K = 12,
    Q5_K = 13,
    Q6_K = 14,
    /// Автовыбор: тип берётся из самого файла весов.
    Count = 42,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RngType {
    StdDefault = 0,
    Cuda = 1,
    Cpu = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleMethod {
    Euler = 0,
    EulerA = 1,
    Heun = 2,
    Dpm2 = 3,
    DpmPp2sA = 4,
    DpmPp2m = 5,
    Lcm = 11,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheduler {
    Discrete = 0,
    Karras = 1,
    Simple = 6,
    Flux2 = 13,
    Flux = 14,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prediction {
    Eps = 0,
    V = 1,
    Count = 7,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoraApplyMode {
    /// Для INT8-весов сам выбирает применение в рантайме, не трогая квантованные веса.
    Auto = 0,
    Immediately = 1,
    AtRuntime = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdVaeFormat {
    Auto = -1,
    Flux = 0,
    Sd3 = 1,
    Flux2 = 2,
    Wan = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    None = 0,
    Proj = 1,
    Tae = 2,
    Vae = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdCancelMode {
    /// Оборвать текущую генерацию как можно скорее.
    All = 0,
    NewLatents = 1,
    Reset = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdImage {
    pub width: u32,
    pub height: u32,
    pub channel: u32,
    pub data: *mut u8,
}

impl Default for SdImage {
    fn default() -> Self {
        Self { width: 0, height: 0, channel: 0, data: std::ptr::null_mut() }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdAudio {
    pub sample_rate: u32,
    pub channels: u32,
    pub sample_count: u64,
    pub data: *mut f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdEmbedding {
    pub name: *const c_char,
    pub path: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdTilingParams {
    pub enabled: bool,
    pub temporal_tiling: bool,
    pub tile_size_x: c_int,
    pub tile_size_y: c_int,
    pub target_overlap: f32,
    pub rel_size_x: f32,
    pub rel_size_y: f32,
    pub extra_tiling_args: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdCtxParams {
    pub model_path: *const c_char,
    pub clip_l_path: *const c_char,
    pub clip_g_path: *const c_char,
    pub clip_vision_path: *const c_char,
    pub t5xxl_path: *const c_char,
    pub llm_path: *const c_char,
    pub llm_vision_path: *const c_char,
    pub diffusion_model_path: *const c_char,
    pub high_noise_diffusion_model_path: *const c_char,
    pub uncond_diffusion_model_path: *const c_char,
    pub embeddings_connectors_path: *const c_char,
    pub vae_path: *const c_char,
    pub audio_vae_path: *const c_char,
    pub taesd_path: *const c_char,
    pub control_net_path: *const c_char,
    pub ip_adapter_path: *const c_char,
    pub motion_module_path: *const c_char,
    pub embeddings: *const SdEmbedding,
    pub embedding_count: u32,
    pub photo_maker_path: *const c_char,
    pub pulid_weights_path: *const c_char,
    pub tensor_type_rules: *const c_char,
    pub n_threads: c_int,
    pub wtype: SdType,
    pub rng_type: RngType,
    pub sampler_rng_type: RngType,
    pub prediction: Prediction,
    pub lora_apply_mode: LoraApplyMode,
    pub enable_mmap: bool,
    pub flash_attn: bool,
    pub diffusion_flash_attn: bool,
    pub tae_preview_only: bool,
    pub diffusion_conv_direct: bool,
    pub vae_conv_direct: bool,
    pub force_sdxl_vae_conv_scale: bool,
    pub vae_format: SdVaeFormat,
    /// Бюджет VRAM в ГиБ для посегментного прохода («0» выключено, «-1» авто).
    pub max_vram: *const c_char,
    pub stream_layers: bool,
    pub eager_load: bool,
    pub backend: *const c_char,
    pub params_backend: *const c_char,
    pub split_mode: *const c_char,
    pub auto_fit: bool,
    pub rpc_servers: *const c_char,
    pub model_args: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdSlgParams {
    pub layers: *mut c_int,
    pub layer_count: usize,
    pub layer_start: f32,
    pub layer_end: f32,
    pub scale: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdGuidanceParams {
    pub txt_cfg: f32,
    pub img_cfg: f32,
    pub distilled_guidance: f32,
    pub slg: SdSlgParams,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdSampleParams {
    pub guidance: SdGuidanceParams,
    pub scheduler: Scheduler,
    pub sample_method: SampleMethod,
    pub sample_steps: c_int,
    pub eta: f32,
    pub shifted_timestep: c_int,
    pub custom_sigmas: *mut f32,
    pub custom_sigmas_count: c_int,
    pub flow_shift: f32,
    pub extra_sample_args: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdPmParams {
    pub id_images: *mut SdImage,
    pub id_images_count: c_int,
    pub id_embed_path: *const c_char,
    pub style_strength: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdPulidParams {
    pub id_embedding_path: *const c_char,
    pub id_weight: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdCacheParams {
    pub mode: c_int,
    pub reuse_threshold: f32,
    pub start_percent: f32,
    pub end_percent: f32,
    pub error_decay_rate: f32,
    pub use_relative_threshold: bool,
    pub reset_error_on_compute: bool,
    pub fn_compute_blocks: c_int,
    pub bn_compute_blocks: c_int,
    pub residual_diff_threshold: f32,
    pub max_warmup_steps: c_int,
    pub max_cached_steps: c_int,
    pub max_continuous_cached_steps: c_int,
    pub taylorseer_n_derivatives: c_int,
    pub taylorseer_skip_interval: c_int,
    pub scm_mask: *const c_char,
    pub scm_policy_dynamic: bool,
    pub spectrum_w: f32,
    pub spectrum_m: c_int,
    pub spectrum_lam: f32,
    pub spectrum_window_size: c_int,
    pub spectrum_flex_window: f32,
    pub spectrum_warmup_steps: c_int,
    pub spectrum_stop_percent: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdLora {
    pub is_high_noise: bool,
    pub multiplier: f32,
    pub path: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdHiresParams {
    pub enabled: bool,
    pub upscaler: c_int,
    pub model_path: *const c_char,
    pub scale: f32,
    pub target_width: c_int,
    pub target_height: c_int,
    pub steps: c_int,
    pub denoising_strength: f32,
    pub upscale_tile_size: c_int,
    pub custom_sigmas: *mut f32,
    pub custom_sigmas_count: c_int,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SdImgGenParams {
    pub loras: *const SdLora,
    pub lora_count: u32,
    pub prompt: *const c_char,
    pub negative_prompt: *const c_char,
    pub clip_skip: c_int,
    pub init_image: SdImage,
    pub ref_images: *mut SdImage,
    pub ref_images_count: c_int,
    pub ref_image_args: *const c_char,
    pub mask_image: SdImage,
    pub width: c_int,
    pub height: c_int,
    pub sample_params: SdSampleParams,
    pub strength: f32,
    pub seed: i64,
    pub batch_count: c_int,
    pub control_image: SdImage,
    pub control_strength: f32,
    pub ip_adapter_image: SdImage,
    pub ip_adapter_strength: f32,
    pub pm_params: SdPmParams,
    pub pulid_params: SdPulidParams,
    pub vae_tiling_params: SdTilingParams,
    pub cache: SdCacheParams,
    pub hires: SdHiresParams,
    pub qwen_image_layers: c_int,
    pub circular_x: bool,
    pub circular_y: bool,
}

pub type SdLogCb = extern "C" fn(level: SdLogLevel, text: *const c_char, data: *mut c_void);
pub type SdProgressCb = extern "C" fn(step: c_int, steps: c_int, time: f32, data: *mut c_void);
