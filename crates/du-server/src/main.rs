//! Запуск сервера отдельным процессом — для разработки и для портативной сборки без оболочки.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let root = std::env::var("DU_ROOT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    let port = std::env::var("DU_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8770);
    du_server::serve_blocking(&root, port)
}
