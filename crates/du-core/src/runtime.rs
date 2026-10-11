//! Где лежит onnxruntime 1.28.2, которую скачивает «Первый запуск»: каталог под `models/runtime` — корень
//! архива релиза — и файл библиотеки в его `lib/`. ort rc.13 с api-28 работает только с 1.28.x: чужая
//! библиотека вешает создание сессии намертво.

pub const ORT_VERSION: &str = "1.28.2";

#[cfg(windows)]
pub const ORT_DIR: &str = "onnxruntime-win-x64-1.28.2";
#[cfg(not(windows))]
pub const ORT_DIR: &str = "onnxruntime-linux-x64-1.28.2";

#[cfg(windows)]
pub const ORT_LIBRARY: &str = "onnxruntime.dll";
#[cfg(not(windows))]
pub const ORT_LIBRARY: &str = "libonnxruntime.so.1.28.2";

/// Библиотека под `<models>/runtime`.
pub fn ort_library(models_root: &std::path::Path) -> std::path::PathBuf {
    models_root.join("runtime").join(ORT_DIR).join("lib").join(ORT_LIBRARY)
}
