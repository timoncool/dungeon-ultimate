//! Запуск сервера отдельным процессом — для разработки и для портативной сборки без оболочки.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let root = std::env::var("DU_ROOT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    // onnxruntime игры — та, что скачана в её корень; без явного пути распознавание искало бы её
    // рядом с бинарём. Так же делает оболочка.
    if std::env::var_os("ORT_DYLIB_PATH").is_none() {
        std::env::set_var("ORT_DYLIB_PATH", du_core::runtime::ort_library(&root.join("models")));
    }
    // Порт MCP и окна — тот же, что у оболочки.
    let port = std::env::var("DU_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8795);
    du_server::serve_blocking(&root, port)
}
