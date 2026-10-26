//! Таблицы импорта библиотек движков: всё, что грузят llama-server, sd.cpp, Higgs и transcribe.cpp, должно лежать
//! рядом с ними, рядом с exe игры (VC++ runtime кладёт установщик) или приходить с Windows и драйвером видеокарты.
//! Разбор PE и правила — те же, что у Dub Studio (`dub-server/src/dll_imports.rs`); проверка настоящей установки —
//! `scripts/check-dll-imports.ps1`.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Имена DLL из таблицы импорта PE (PE32 и PE32+), включая отложенную загрузку (delay-load).
pub fn imported_libraries(binary: &Path) -> Result<Vec<String>, String> {
    let data = std::fs::read(binary).map_err(|e| format!("{}: {e}", binary.display()))?;
    let at = |offset: usize| -> Result<u32, String> {
        let b = data.get(offset..offset + 4).ok_or("truncated PE header")?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let short = |offset: usize| -> Result<u16, String> {
        let b = data.get(offset..offset + 2).ok_or("truncated PE header")?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    };
    let pe = at(0x3c)? as usize;
    if data.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err(format!("{} is not a PE file", binary.display()));
    }
    let sections = short(pe + 6)? as usize;
    let optional_size = short(pe + 20)? as usize;
    let optional = pe + 24;
    // 0x20b — PE32+, его каталоги данных на 16 байт дальше.
    let directories = optional + if short(optional)? == 0x20b { 112 } else { 96 };
    let headers: Vec<(u32, u32, u32)> = (0..sections)
        .map(|i| {
            let base = optional + optional_size + i * 40;
            Ok((at(base + 12)?, at(base + 16)?, at(base + 20)?))
        })
        .collect::<Result<_, String>>()?;
    let offset_of = |rva: u32| -> Option<usize> {
        headers.iter().find(|(va, size, _)| rva >= *va && rva < va + size).map(|(va, _, raw)| (raw + (rva - va)) as usize)
    };
    let name_at = |rva: u32| -> Result<String, String> {
        let start = offset_of(rva).ok_or("import name outside the file")?;
        let end = data[start..].iter().position(|b| *b == 0).ok_or("import name without a terminator")? + start;
        Ok(String::from_utf8_lossy(&data[start..end]).into_owned())
    };
    let mut names = Vec::new();
    // (каталог данных, размер дескриптора, смещение RVA имени в дескрипторе): импорт (1) и отложенный импорт (13).
    for (dir_index, descriptor, name_field) in [(1usize, 20usize, 12usize), (13, 32, 4)] {
        let table_rva = at(directories + dir_index * 8)?;
        if table_rva == 0 {
            continue;
        }
        let mut entry = offset_of(table_rva).ok_or("import table outside the file")?;
        loop {
            let d = data.get(entry..entry + descriptor).ok_or("truncated import table")?;
            if d.iter().all(|b| *b == 0) {
                break;
            }
            let f = &d[name_field..name_field + 4];
            names.push(name_at(u32::from_le_bytes([f[0], f[1], f[2], f[3]]))?);
            entry += descriptor;
        }
    }
    Ok(names)
}

/// Библиотеки, которые есть в любой Windows 10/11 или приходят с драйвером видеокарты.
pub fn is_provided_by_the_system(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("api-ms-win-")
        || name.starts_with("ext-ms-win-")
        // Библиотеки драйверов: CUDA и Vulkan ставит драйвер видеокарты, не игра.
        || name == "nvcuda.dll"
        || name == "vulkan-1.dll"
        || [
            "kernel32.dll", "kernelbase.dll", "ntdll.dll", "user32.dll", "gdi32.dll", "advapi32.dll", "shell32.dll",
            "shlwapi.dll", "ole32.dll", "oleaut32.dll", "combase.dll", "ws2_32.dll", "crypt32.dll", "bcrypt.dll",
            "ncrypt.dll", "secur32.dll", "rpcrt4.dll", "setupapi.dll", "cfgmgr32.dll", "version.dll", "dbghelp.dll",
            "powrprof.dll", "psapi.dll", "userenv.dll", "winmm.dll", "msvcrt.dll", "comdlg32.dll", "comctl32.dll",
            "imm32.dll", "iphlpapi.dll", "winhttp.dll", "wininet.dll", "dxgi.dll", "d3d11.dll", "d3d12.dll",
            "dxcore.dll", "dwmapi.dll", "uxtheme.dll", "normaliz.dll", "wldap32.dll",
        ]
        .contains(&name.as_str())
}

/// VC++ runtime, который установщик игры кладёт рядом с её exe (`bundleVCRuntime` и `vcomp140.dll` в
/// `desktop/src-tauri/tauri.conf.json`).
pub const BESIDE_THE_GAME: &[&str] = &[
    "concrt140.dll",
    "msvcp140.dll",
    "msvcp140_1.dll",
    "msvcp140_2.dll",
    "msvcp140_atomic_wait.dll",
    "msvcp140_codecvt_ids.dll",
    "vccorlib140.dll",
    "vcomp140.dll",
    "vcruntime140.dll",
    "vcruntime140_1.dll",
    "vcruntime140_threads.dll",
];

/// Что грузит игра: (компонент, файл). ggml-cuda и ggml-vulkan грузятся LoadLibrary'ем, поэтому проверяются
/// отдельными точками входа. llama-server — отдельный процесс: каталог exe игры ему передаётся в PATH.
pub const ENTRY_POINTS: &[(&str, &str, &[&str])] = &[
    ("narrator-engine", "tools/llama/llama-server.exe", &[]),
    ("narrator-engine", "tools/llama/ggml-cuda.dll", &[]),
    ("narrator-engine-cuda12", "tools/llama-cuda12/llama-server.exe", &[]),
    ("narrator-engine-vulkan", "tools/llama-vulkan/llama-server.exe", &[]),
    ("narrator-engine-cpu", "tools/llama-cpu/llama-server.exe", &[]),
    ("image-engine", "models/runtime/sd/stable-diffusion.dll", &[]),
    ("image-engine", "models/runtime/sd/ggml-cuda.dll", &[]),
    // CUDA-библиотеки сервер находит через PATH, который игра ставит ему на запуске (`acpp::server`)
    ("voice-engine", "tools/audiocpp/audiocpp_server.exe", &["models/tts-engine"]),
    ("voice-engine-cuda12", "tools/audiocpp-cuda12/audiocpp_server.exe", &["models/cuda12-runtime"]),
    ("voice-engine-vulkan", "tools/audiocpp-vulkan/audiocpp_server.exe", &[]),
    ("voice-engine-cpu", "tools/audiocpp-cpu/audiocpp_server.exe", &[]),
    ("asr-gpu", "models/transcribe-engine/transcribe_engine.dll", &[]),
    ("asr", "models/transcribe-engine-cpu/transcribe_engine.dll", &[]),
];

/// Итог проверки одной точки входа.
#[derive(Debug, Default)]
pub struct Resolution {
    /// Не найдено ни рядом, ни в каталогах поиска, ни в системе.
    pub missing: Vec<String>,
    /// Найдено: (имя, путь).
    pub found: Vec<(String, PathBuf)>,
}

/// Обход импортов от точки входа (exe → ggml.dll → ggml-cuda.dll → cuBLAS…): каждая библиотека ищется в каталоге
/// точки входа, затем в `dirs`.
pub fn resolve(entry: &Path, dirs: &[PathBuf]) -> Result<Resolution, String> {
    let mut search: Vec<PathBuf> = entry.parent().map(|p| vec![p.to_path_buf()]).unwrap_or_default();
    search.extend(dirs.iter().cloned());
    let mut seen: Vec<String> = Vec::new();
    let mut out = Resolution::default();
    let mut queue: VecDeque<PathBuf> = VecDeque::from([entry.to_path_buf()]);
    while let Some(bin) = queue.pop_front() {
        for import in imported_libraries(&bin)? {
            let lowered = import.to_ascii_lowercase();
            if is_provided_by_the_system(&lowered) || seen.contains(&lowered) {
                continue;
            }
            seen.push(lowered);
            match search.iter().map(|d| d.join(&import)).find(|p| p.is_file()) {
                Some(p) => {
                    out.found.push((import, p.clone()));
                    queue.push_back(p);
                }
                None => out.missing.push(import),
            }
        }
    }
    Ok(out)
}

/// Недостающее у установленных компонентов `root`: импорт, которого нет ни рядом с точкой входа, ни среди
/// библиотек рядом с exe игры, ни в системе. Не установленные компоненты пропускаются.
pub fn check_root(root: &Path) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    for (component, rel, dirs) in ENTRY_POINTS {
        let entry = root.join(rel);
        if !entry.is_file() {
            continue;
        }
        let dirs: Vec<PathBuf> = dirs.iter().map(|dir| root.join(dir)).collect();
        for name in resolve(&entry, &dirs)?.missing {
            if !BESIDE_THE_GAME.contains(&name.to_ascii_lowercase().as_str()) {
                problems.push(format!("{component}: {rel} imports {name}, and it is neither beside it, nor beside the game, nor in Windows"));
            }
        }
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Импорты самого тестового бинаря читаются: разбор PE не сломан.
    #[test]
    fn the_test_binary_imports_kernel32() {
        let exe = std::env::current_exe().unwrap();
        let names = imported_libraries(&exe).unwrap();
        assert!(names.iter().any(|n| n.eq_ignore_ascii_case("kernel32.dll")), "{names:?}");
    }

    /// Настоящая установка (`DU_DLL_ROOT` — корень игры с докачанными компонентами): у каждой точки входа есть всё,
    /// что она импортирует. `scripts/check-dll-imports.ps1`.
    #[test]
    #[ignore]
    fn an_installed_root_has_every_import() {
        let root = std::env::var("DU_DLL_ROOT").expect("DU_DLL_ROOT — корень игры с компонентами");
        let problems = check_root(Path::new(&root)).unwrap();
        assert!(problems.is_empty(), "{problems:#?}");
    }
}
