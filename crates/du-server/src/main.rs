//! Запуск сервера отдельным процессом — для разработки и для портативной сборки без оболочки.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    du_server::diagnostics::init_logging();

    let root = std::env::var("DU_ROOT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    du_server::diagnostics::log_to_file(&root);
    // Порт MCP и окна — тот же, что у оболочки.
    let port = std::env::var("DU_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8795);
    du_server::serve_blocking(&root, port)
}
