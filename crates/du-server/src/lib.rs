//! Сервер Dungeon Ultimate: один процесс отдаёт и приложение, и его API.
//!
//! Всё, что трогает видеокарту, проходит через единственную очередь, поэтому две модели
//! на карте одновременно оказаться не могут. Прежде порядок держал клиент, и одного
//! совпадения хватало, чтобы карта встала.

pub mod assist;
pub mod bytes;
mod casting;
mod characters;
mod chats;
mod dialogue;
pub mod cloud;
pub mod cloud_models;
pub mod runtime;
pub mod gpu;
pub mod hw;
pub mod jobs;
mod model_caps;
pub mod resolve;
pub mod release;
mod routes;
pub mod scene;
pub mod setup;
pub mod spa;
pub mod state;
pub mod story;
pub mod tts;
pub mod voice;
pub mod voice_catalog;
mod voice_tags;
pub mod turn;

use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::Router;

use state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(routes::health))
        .route("/api/release", get(routes::release_check))
        .route("/api/chats", get(chats::list_chats).post(chats::create_chat))
        .route("/api/chats/{chat_id}", get(chats::get_chat))
        .route("/api/chats/{chat_id}", patch(chats::patch_chat))
        .route("/api/chats/{chat_id}", delete(chats::delete_chat))
        .route("/api/chats/{chat_id}/events", get(chats::list_events))
        .route("/api/chats/{chat_id}/items", get(chats::list_items))
        .route("/api/chats/{chat_id}/quests", get(chats::list_quests))
        .route("/api/chats/{chat_id}/quests/{quest_id}", patch(chats::patch_quest))
        .route("/api/chats/{chat_id}/items/{item_id}", patch(chats::equip_item))
        .route("/api/chats/{chat_id}/items/{item_id}/image", post(routes::item_image))
        .route("/api/image-worker", post(routes::image_worker))
        .route("/api/tts-voice", post(tts::upload_voice))
        // Пути сообщений и персонажей — те же, что были у прежней версии, чтобы
        // перенесённый интерфейс работал без правок контракта.
        .route("/api/chats/{chat_id}/messages/{message_id}", delete(chats::delete_message))
        .route("/api/chats/{chat_id}/messages/{message_id}", patch(chats::edit_message))
        .route("/api/chats/{chat_id}/characters", get(characters::list).post(characters::create))
        .route("/api/chats/{chat_id}/characters/{character_id}", patch(characters::update))
        .route("/api/chats/{chat_id}/characters/{character_id}", delete(characters::remove))
        .route("/api/actions", post(assist::actions))
        .route("/api/suggest", post(assist::suggest))
        .route("/api/story-setup", post(assist::story_setup))
        .route("/api/image-style", post(assist::image_style))
        .route("/api/character", post(assist::character))
        .route("/api/upload", post(characters::upload))
        .route("/api/local-data", get(characters::local_data).delete(characters::wipe))
        .route("/api/settings/defaults", get(characters::default_settings))
        .route("/api/runtime", get(routes::runtime_get).put(routes::runtime_put))
        .route("/api/hw", get(routes::hardware))
        .route("/api/setup/status", get(routes::setup_status))
        .route("/api/setup/download", post(routes::setup_download))
        .route("/api/asr", post(voice::transcribe))
        .route("/api/voices", get(voice::voices))
        .route("/api/cloud/voices", get(routes::cloud_voices))
        .route("/api/cloud/models", get(routes::cloud_model_list))
        .route("/api/tts", post(tts::speak))
        .route("/api/images", post(routes::create_image))
        .route("/api/story", post(story::story))
        .fallback(serve_static)
        .with_state(state)
}

/// Сгенерированное и загруженное отдаём как файлы, всё остальное — приложению.
async fn serve_static(State(state): State<AppState>, request: axum::extract::Request) -> Response {
    let path = request.uri().path().to_string();
    if let Some(name) = path.strip_prefix("/generated/") {
        return spa::serve_file(&state.generated, name).await;
    }
    if let Some(name) = path.strip_prefix("/uploads/") {
        return spa::serve_file(&state.uploads, name).await;
    }
    if path.starts_with("/api/") {
        return (StatusCode::NOT_FOUND, "нет такого метода").into_response();
    }
    spa::serve_spa(state.web_root.as_deref(), &path).await
}

/// Поднять сервер на 127.0.0.1 и работать до завершения процесса. Вызывается из оболочки
/// на фоновом потоке, поэтому внутри создаётся собственный рантайм.
pub fn serve_blocking(root: &Path, port: u16) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(async move {
        let state = AppState::new(root)?;
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let listener = tokio::net::TcpListener::bind(address).await?;
        tracing::info!("сервер слушает http://{address}");
        axum::serve(listener, router(state)).await?;
        Ok::<_, anyhow::Error>(())
    })
}

#[cfg(test)]
mod tests;
