//! Раздача собранного фронта и файлов, которые породило само приложение.
//!
//! Защита от выхода за корень обязательна: без канонизации пути сегменты `..%2f` вычитали бы
//! произвольные файлы с диска пользователя.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

/// Интерфейс, вшитый в сам бинарь.
///
/// Благодаря этому игра — ОДИН файл: рядом с ним не нужна папка `frontend/dist`. Каталог на
/// диске всё равно имеет приоритет: так правку интерфейса видно без пересборки сервера.
#[derive(rust_embed::RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../frontend/dist"]
struct Ui;

/// Отдать файл из корня или, если такого нет, точку входа приложения. Возврат точки входа
/// вместо ошибки нужен, чтобы обновление страницы на любом маршруте не давало 404.
pub async fn serve_spa(web_root: Option<&Path>, path: &str) -> Response {
    if let Some(root) = web_root.and_then(|root| root.canonicalize().ok()) {
        if let Some(file) = resolve_inside(&root, path) {
            return file_response(&file).await;
        }
        let entry = root.join("index.html");
        if entry.is_file() {
            return file_response(&entry).await;
        }
    }
    embedded(path).unwrap_or_else(|| {
        embedded("index.html")
            .unwrap_or_else(|| (StatusCode::NOT_FOUND, "интерфейс не собран").into_response())
    })
}

/// Файл из вшитого интерфейса.
fn embedded(path: &str) -> Option<Response> {
    let path = path.trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let file = Ui::get(path)?;
    let kind = mime_guess::from_path(path).first_or_octet_stream();
    Some(
        (
            [(header::CONTENT_TYPE, kind.as_ref())],
            Body::from(file.data.into_owned()),
        )
            .into_response(),
    )
}

/// Отдать файл из каталога сгенерированного (картинки, озвучка). Здесь подмена на точку
/// входа неуместна: отсутствующий файл — это именно 404, а не страница приложения.
pub async fn serve_file(root: &Path, path: &str) -> Response {
    let Some(root) = root.canonicalize().ok() else {
        return (StatusCode::NOT_FOUND, "нет каталога").into_response();
    };
    match resolve_inside(&root, path) {
        Some(file) => file_response(&file).await,
        None => (StatusCode::NOT_FOUND, "не найдено").into_response(),
    }
}

/// Разрешить путь запроса в существующий файл ВНУТРИ корня. Канонизация раскрывает `..` и
/// символические ссылки, поэтому проверка вхождения делается уже по реальному пути.
fn resolve_inside(root: &Path, path: &str) -> Option<PathBuf> {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return None;
    }
    let candidate = root.join(path).canonicalize().ok()?;
    if candidate.starts_with(root) && candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

async fn file_response(path: &Path) -> Response {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], Body::from(bytes)).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "не найдено").into_response(),
    }
}

/// Найти собранный фронт рядом с корнем приложения.
pub fn find_web_root(root: &Path) -> Option<PathBuf> {
    let dist = root.join("frontend").join("dist");
    dist.join("index.html").is_file().then_some(dist)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<html>приложение</html>").unwrap();
        std::fs::create_dir_all(dir.path().join("assets")).unwrap();
        std::fs::write(dir.path().join("assets/app.js"), "console.log(1)").unwrap();
        dir
    }

    #[tokio::test]
    async fn a_real_file_is_served_with_its_type() {
        let dir = sandbox();
        let response = serve_spa(Some(dir.path()), "/assets/app.js").await;
        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response.headers().get(header::CONTENT_TYPE).unwrap();
        assert!(content_type.to_str().unwrap().contains("javascript"));
    }

    #[tokio::test]
    async fn an_unknown_route_falls_back_to_the_entry_point() {
        let dir = sandbox();
        let response = serve_spa(Some(dir.path()), "/chat/42").await;
        assert_eq!(response.status(), StatusCode::OK, "обновление страницы не должно давать 404");
    }

    #[tokio::test]
    async fn escaping_the_root_is_impossible() {
        let dir = sandbox();
        let secret = dir.path().parent().unwrap().join("секрет.txt");
        std::fs::write(&secret, "нельзя читать").unwrap();

        for attempt in ["../секрет.txt", "..%2f..%2fсекрет.txt", "assets/../../секрет.txt"] {
            let response = serve_spa(Some(dir.path()), attempt).await;
            let body = axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap();
            let text = String::from_utf8_lossy(&body);
            assert!(!text.contains("нельзя читать"), "утечка файла через {attempt}");
        }
        let _ = std::fs::remove_file(secret);
    }

    #[tokio::test]
    async fn generated_files_report_a_missing_one_instead_of_the_entry_point() {
        let dir = sandbox();
        let response = serve_file(dir.path(), "/нет-такого.png").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn an_unbuilt_frontend_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_web_root(dir.path()).is_none());
    }
}
