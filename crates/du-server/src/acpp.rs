//! audiocpp_server игры (`acpp_server::SERVERS`, по процессу на бэкенд): голос Higgs и распознавание CrisperWhisper.
//! Сборка сервера — по машине (`accel`), как у рассказчика: CUDA 13, CUDA 12, Vulkan или процессор.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use acpp_server::asset::Build;
use acpp_server::{Backend, ModelEntry, Server, ServerConfig};

use crate::errors::UserError;

pub const HIGGS: &str = "higgs";
pub const CRISPERWHISPER: &str = "crisperwhisper";

/// Папка CrisperWhisper в `models` — репозиторий Hugging Face как есть, путь Dub Studio.
pub const CRISPERWHISPER_DIR: &str = "crisperwhisper2-turbo";
/// Веса Higgs игры (Higgs-Audio-v3-Studio, q4_k_m) — те же файлы, что читала прежняя библиотека голоса.
pub const HIGGS_GGUF: &str = "models/tts/q4_k_m.gguf";

/// Сервер моделей на старте не грузит, поэтому отвечает быстро.
const READY_WITHIN: Duration = Duration::from_secs(60);

/// Сборка сервера для этой машины.
pub fn build(accel: crate::accel::Accel) -> Build {
    match accel {
        crate::accel::Accel::Cuda13 => Build::Cuda13,
        crate::accel::Accel::Cuda12 => Build::Cuda12,
        crate::accel::Accel::Vulkan => Build::Vulkan,
        crate::accel::Accel::Cpu => Build::Cpu,
    }
}

/// Программа сервера этой сборки.
pub fn exe(root: &Path, build: Build) -> PathBuf {
    let name = if cfg!(windows) { "audiocpp_server.exe" } else { "audiocpp_server" };
    root.join("tools").join(build.folder()).join(name)
}

/// Где лежат библиотеки CUDA, которых нет в архиве сервера: CUDA 13 — в `models/tts-engine` (туда их кладут
/// движок озвучки и «Parakeet на видеокарте»), CUDA 12.9 — в `models/cuda12-runtime`.
pub fn library_dir(root: &Path, build: Build) -> Option<PathBuf> {
    match build {
        Build::Cuda13 => Some(root.join("models").join("tts-engine")),
        Build::Cuda12 => Some(root.join("models").join("cuda12-runtime")),
        Build::Vulkan | Build::Cpu => None,
    }
}

/// Модели, файлы которых на диске: сервер читает контракт каждой настроенной модели при старте, и один
/// недостающий файл остановил бы все.
pub fn models(root: &Path) -> Vec<ModelEntry> {
    let mut out = Vec::new();
    let higgs = root.join(HIGGS_GGUF);
    if higgs.is_file() {
        out.push(ModelEntry::higgs_tts(HIGGS, &higgs));
    }
    let crisper = root.join("models").join(CRISPERWHISPER_DIR);
    if crisper.join("model.safetensors").is_file() {
        out.push(ModelEntry::crisperwhisper(CRISPERWHISPER, &crisper));
    }
    out
}

fn config(root: &Path, build: Build, backend: Backend) -> ServerConfig {
    let name = match backend {
        Backend::Cuda => "cuda",
        Backend::Vulkan => "vulkan",
        Backend::Cpu => "cpu",
    };
    ServerConfig {
        exe: exe(root, build),
        backend,
        device: 0,
        threads: std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(8).min(16),
        models: models(root),
        data_dir: root.join("logs").join(format!("audiocpp_server-{name}")),
        log: false,
    }
}

/// Работающий сервер, несущий `model`: тот, что есть, или новый, если модели на диске поменялись или процесс
/// кончился. `on_cpu` — стадия уведена на процессор: тот же сервер этой машины с процессорным бэкендом.
pub fn server(root: &Path, model: &str, on_cpu: bool) -> Result<Arc<Server>, UserError> {
    let runtime = crate::runtime::load(root);
    let accel = crate::accel::accel(&runtime);
    let build = build(accel);
    let backend = if on_cpu { Backend::Cpu } else { build.backend() };
    if backend != Backend::Cpu && accel.nvidia() {
        if let Some(problem) = crate::accel::missing_chosen_card(&runtime) {
            return Err(problem);
        }
    }
    dub_tcpp::cpu_floor("audiocpp_server").map_err(UserError::CpuFloor)?;
    let config = config(root, build, backend);
    if !config.exe.is_file() {
        return Err(UserError::AcppMissing(config.exe.display().to_string()));
    }
    let exe_dir = config.exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let libraries = library_dir(root, build);
    let dirs: Vec<&Path> = std::iter::once(exe_dir.as_path()).chain(libraries.as_deref()).collect();
    let missing = acpp_server::asset::missing_libraries(build, &dirs);
    if !missing.is_empty() {
        return Err(UserError::AcppLibrariesMissing(missing.join(", ")));
    }
    if !config.models.iter().any(|entry| entry.id == model) {
        return Err(match model {
            HIGGS => UserError::VoiceModelMissing(root.join(HIGGS_GGUF).display().to_string()),
            _ => UserError::CrisperWhisperMissing(root.join("models").join(CRISPERWHISPER_DIR).display().to_string()),
        });
    }
    let prepare = |command: &mut std::process::Command| {
        // Windows ищет импортируемые библиотеки в PATH: CUDA-библиотеки лежат не рядом с программой.
        if let Some(dir) = &libraries {
            let mut paths = vec![dir.clone()];
            if let Some(current) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&current));
            }
            if let Ok(joined) = std::env::join_paths(paths) {
                command.env("PATH", joined);
            }
        }
    };
    let server = acpp_server::SERVERS.get(&config, READY_WITHIN, &prepare).map_err(UserError::Acpp)?;
    tracing::info!(
        "audiocpp_server {} ({:?}, {}) несёт {:?}",
        server.pid(),
        backend,
        build.folder(),
        config.models.iter().map(|entry| entry.id.as_str()).collect::<Vec<_>>()
    );
    Ok(server)
}

/// Снять модели с памяти работающих серверов: следующий запрос загрузит их снова, процесс остаётся.
pub fn unload(ids: &[&str]) {
    for (pid, error) in acpp_server::SERVERS.unload(ids) {
        tracing::error!("[ERROR] audiocpp_server {pid} не выгрузил {ids:?}: {error}");
    }
}

/// Остановить все серверы (один завис); следующая надобность поднимет новый.
pub fn stop() {
    acpp_server::SERVERS.stop_all();
}
