//! Персонажи чата и загрузка вложений.

use axum::extract::{Multipart, Path, State};
use axum::Json;
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};

use du_core::{Attachment, StoryCharacter};

use crate::routes::ApiError;
use crate::state::AppState;

type ApiResult<T> = Result<T, ApiError>;

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Отличить «поле не прислали» от «прислали `null`». Обычный `Option<Option<T>>` этого не
/// умеет: внешний `Option` сам поглощает `null`, и снятие портрета выглядит как отсутствие правки.
fn present_even_if_null<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

/// Поля листа персонажа. Все НЕОБЯЗАТЕЛЬНЫЕ: правка приходит частичной — например, только
/// портрет, — и не должна затирать остальное пустыми строками.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CharacterInput {
    pub name: Option<String>,
    pub details: Option<String>,
    pub inventory: Option<String>,
    pub skills: Option<String>,
    pub spells: Option<String>,
    /// `null` явно снимает портрет, отсутствие поля — оставляет как есть.
    #[serde(deserialize_with = "present_even_if_null")]
    pub portrait: Option<Option<Attachment>>,
    #[serde(deserialize_with = "present_even_if_null")]
    pub voice: Option<Option<String>>,
}

pub async fn list(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "characters": state.store.list_characters(&chat_id)? })))
}

pub async fn create(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
    Json(body): Json<CharacterInput>,
) -> ApiResult<Json<Value>> {
    let name = body.name.unwrap_or_default().trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("у персонажа должно быть имя"));
    }
    let now = now_iso();
    let character = StoryCharacter {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat_id.clone(),
        name,
        details: body.details.unwrap_or_default(),
        inventory: body.inventory.unwrap_or_default(),
        skills: body.skills.unwrap_or_default(),
        spells: body.spells.unwrap_or_default(),
        portrait: body.portrait.flatten(),
        voice: body.voice.flatten().filter(|voice| !voice.trim().is_empty()),
        created_at: now.clone(),
        updated_at: now,
    };
    state.store.create_character(&character)?;
    Ok(Json(json!({ "character": character })))
}

pub async fn update(
    State(state): State<AppState>,
    Path((chat_id, character_id)): Path<(String, String)>,
    Json(body): Json<CharacterInput>,
) -> ApiResult<Json<Value>> {
    let existing = state
        .store
        .list_characters(&chat_id)?
        .into_iter()
        .find(|character| character.id == character_id)
        .ok_or_else(|| ApiError::not_found("персонаж не найден"))?;

    // Сливаем: пришедшее поле заменяет, отсутствующее — оставляет прежнее. Иначе правка
    // одного портрета стирала бы весь лист персонажа.
    let character = StoryCharacter {
        name: body
            .name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or(existing.name),
        details: body.details.unwrap_or(existing.details),
        inventory: body.inventory.unwrap_or(existing.inventory),
        skills: body.skills.unwrap_or(existing.skills),
        spells: body.spells.unwrap_or(existing.spells),
        portrait: match body.portrait {
            Some(portrait) => portrait,
            None => existing.portrait,
        },
        voice: match body.voice {
            Some(voice) => voice.filter(|voice| !voice.trim().is_empty()),
            None => existing.voice,
        },
        updated_at: now_iso(),
        ..existing
    };
    state.store.update_character(&character)?;
    Ok(Json(json!({ "character": character })))
}

pub async fn remove(
    State(state): State<AppState>,
    Path((chat_id, character_id)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    state.store.delete_character(&chat_id, &character_id)?;
    Ok(Json(json!({ "ok": true })))
}

/// Загрузка картинки игроком: портрет персонажа или референс к ходу.
pub async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<Json<Attachment>> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        let name = field.file_name().unwrap_or("upload").to_string();
        let content_type = field.content_type().unwrap_or("image/png").to_string();
        let bytes = field
            .bytes()
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if bytes.is_empty() {
            continue;
        }

        // Имя файла берём своё: пользовательское может содержать что угодно, вплоть до
        // сегментов пути.
        let extension = match content_type.as_str() {
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            "image/gif" => "gif",
            _ => "png",
        };
        let stored = format!("{}.{extension}", uuid::Uuid::new_v4());
        std::fs::write(state.uploads.join(&stored), &bytes)
            .map_err(|error| ApiError::internal(error.to_string()))?;

        return Ok(Json(Attachment {
            id: stored.clone(),
            name,
            kind: content_type,
            url: format!("/uploads/{stored}"),
            data_url: None,
        }));
    }
    Err(ApiError::bad_request("в запросе нет файла"))
}

/// Что лежит на диске у игрока и сколько это занимает.
pub async fn local_data(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let size_of = |dir: &std::path::Path| -> u64 {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter_map(|entry| entry.metadata().ok())
                    .filter(|meta| meta.is_file())
                    .map(|meta| meta.len())
                    .sum()
            })
            .unwrap_or(0)
    };
    Ok(Json(json!({
        "chats": state.store.list_chats()?.len(),
        "generatedBytes": size_of(&state.generated),
        "uploadedBytes": size_of(&state.uploads),
    })))
}

/// Полное стирание локальных данных по кнопке игрока.
pub async fn wipe(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    state.store.delete_everything()?;
    for dir in [&state.generated, &state.uploads] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    Ok(Json(json!({ "ok": true })))
}

/// Настройки истории по умолчанию для нового чата.
pub async fn default_settings() -> Json<Value> {
    Json(json!({ "settings": du_core::StorySettings::default() }))
}
