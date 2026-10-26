//! audiocpp_server игры (`acpp_server::SERVERS`, по процессу на бэкенд): голос Higgs и распознавание CrisperWhisper.
//! Сборка сервера — по машине (`accel`), как у рассказчика: CUDA 13, CUDA 12, Vulkan или процессор; на Mac — Metal.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use acpp_server::asset::Build;
use acpp_server::{Backend, ModelEntry, Server, ServerConfig};

use crate::errors::UserError;

pub const HIGGS: &str = "higgs";
pub const CRISPERWHISPER: &str = "crisperwhisper";
/// Parakeet TDT v3 в сервере; Parakeet Ultra считает transcribe.cpp.
pub const PARAKEET_V3: &str = "parakeet-v3";

/// Папка CrisperWhisper в `models` — репозиторий Hugging Face как есть, путь Dub Studio.
pub const CRISPERWHISPER_DIR: &str = "crisperwhisper2-turbo";
/// Кванты Higgs (Higgs-Audio-v3-Studio) от лучшего к меньшему и их GGUF: q8_0 и q6_k по путям Speech Studio (их можно
/// взять у неё), q4_k_m — прежний файл игры.
pub const HIGGS_QUANTS: [(&str, &str); 3] =
    [("q8_0", "models/higgs-q8_0/q8_0.gguf"), ("q6_k", "models/higgs-q6_k/q6_k.gguf"), ("q4_k_m", "models/tts/q4_k_m.gguf")];

/// Квант Higgs по памяти карты, как пресеты Speech Studio: от 15 ГБ — q8_0, от 11 — q6_k, меньше — q4_k_m.
pub fn recommended_quant(vram_gb: f64) -> &'static str {
    if vram_gb >= 15.0 {
        "q8_0"
    } else if vram_gb >= 11.0 {
        "q6_k"
    } else {
        "q4_k_m"
    }
}

/// Квант Higgs игры: выбранный в настройках; без выбора — рекомендованный по карте, если он скачан, иначе лучший из
/// скачанных (прежний q4_k_m играет, пока игрок не скачал рекомендованный), а не скачан ни один — рекомендованный.
pub fn higgs_quant(root: &Path, runtime: &crate::runtime::Runtime) -> &'static str {
    let on_disk = |quant: &str| HIGGS_QUANTS.iter().any(|(q, gguf)| *q == quant && root.join(gguf).is_file());
    if let Some((quant, _)) = HIGGS_QUANTS.iter().find(|(q, _)| *q == runtime.tts_quant) {
        return quant;
    }
    let vram = crate::accel::chosen_card(runtime).map(|card| card.vram_gb).unwrap_or(0.0);
    let recommended = recommended_quant(vram);
    if on_disk(recommended) {
        return recommended;
    }
    HIGGS_QUANTS.iter().map(|(q, _)| *q).find(|q| on_disk(q)).unwrap_or(recommended)
}

/// GGUF Higgs игры (`higgs_quant`).
pub fn higgs_gguf(root: &Path, runtime: &crate::runtime::Runtime) -> PathBuf {
    let quant = higgs_quant(root, runtime);
    let rel = HIGGS_QUANTS.iter().find(|(q, _)| *q == quant).map(|(_, gguf)| *gguf).unwrap_or(HIGGS_QUANTS[2].1);
    root.join(rel)
}

/// Сервер моделей на старте не грузит, поэтому отвечает быстро.
const READY_WITHIN: Duration = Duration::from_secs(60);

/// Сборка сервера для этой машины. Mac на Apple silicon — Metal-сборка: машина там считается процессорной, а Metal
/// и процессор у сервера в одной программе.
pub fn build(accel: crate::accel::Accel) -> Build {
    if let Some(mac) = Build::for_this_mac() {
        return mac;
    }
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

/// Где лежат библиотеки CUDA, которых нет в архиве сервера: CUDA 13 — в `models/tts-engine` на Windows (туда их кладут
/// движок озвучки и «Parakeet на видеокарте») и в `models/higgs-engine` на Linux (её называет RUNPATH сборок audio.cpp
/// и transcribe.cpp), CUDA 12.9 — в `models/cuda12-runtime`.
pub fn library_dir(root: &Path, build: Build) -> Option<PathBuf> {
    match build {
        Build::Cuda13 => Some(root.join("models").join(if cfg!(windows) { "tts-engine" } else { "higgs-engine" })),
        Build::Cuda12 => Some(root.join("models").join("cuda12-runtime")),
        Build::Vulkan | Build::Cpu | Build::Metal => None,
    }
}

/// Модели, файлы которых на диске: сервер читает контракт каждой настроенной модели при старте, и один
/// недостающий файл остановил бы все. Parakeet v3 читает GGUF transcribe.cpp (путь Dub Studio), как в Speech Studio.
pub fn models(root: &Path) -> Vec<ModelEntry> {
    let mut out = Vec::new();
    let higgs = higgs_gguf(root, &crate::runtime::load(root));
    if higgs.is_file() {
        out.push(ModelEntry::higgs_tts(HIGGS, &higgs));
    }
    let crisper = root.join("models").join(CRISPERWHISPER_DIR);
    if crisper.join("model.safetensors").is_file() {
        out.push(ModelEntry::crisperwhisper(CRISPERWHISPER, &crisper));
    }
    if let Some(model) = dub_tcpp::model(PARAKEET_V3) {
        let gguf = dub_tcpp::model_path(&root.join("models"), model);
        if gguf.is_file() {
            out.push(ModelEntry::parakeet_tdt(PARAKEET_V3, &gguf));
        }
    }
    out
}

fn config(root: &Path, build: Build, backend: Backend) -> ServerConfig {
    let name = match backend {
        Backend::Cuda => "cuda",
        Backend::Vulkan => "vulkan",
        Backend::Cpu => "cpu",
        Backend::Metal => "metal",
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
            HIGGS => UserError::VoiceModelMissing(higgs_gguf(root, &runtime).display().to_string()),
            PARAKEET_V3 => UserError::ParakeetModelMissing(
                dub_tcpp::model(PARAKEET_V3).map(|m| dub_tcpp::model_path(&root.join("models"), m)).unwrap_or_default().display().to_string(),
            ),
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
