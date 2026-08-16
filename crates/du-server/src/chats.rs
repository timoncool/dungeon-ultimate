//! Маршруты историй: список, чтение, правка, удаление, а также журнал, инвентарь и
//! отдельные сообщения. Пути и форма ответов те же, что отдавали роуты Next.js.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use du_core::StorySettings;

use crate::routes::{ApiError, ApiResult};
use crate::state::AppState;

pub async fn list_chats(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "chats": state.store.list_chats()? })))
}

#[derive(Deserialize)]
pub struct CreateChat {
    #[serde(default)]
    settings: StorySettings,
    #[serde(default)]
    title: Option<String>,
}

pub async fn create_chat(
    State(state): State<AppState>,
    Json(body): Json<CreateChat>,
) -> ApiResult<Json<Value>> {
    let title = body.title.unwrap_or_else(|| "Новая история".to_string());
    let id = state.store.create_chat(body.settings, &title)?;
    let chat = state.store.get_chat(&id)?.ok_or_else(|| ApiError::internal("чат не создался"))?;
    Ok(Json(json!({ "chat": chat })))
}

pub async fn get_chat(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let chat = state.store.get_chat(&chat_id)?.ok_or_else(|| ApiError::not_found("чат не найден"))?;
    // В режиме D&D вместе с чатом отдаём состояние героя, инвентарь и журнал: иначе после
    // перезагрузки они пусты — живут-то они в событиях хода. На этом же ответе держится и
    // портрет героя: без `heroId` интерфейсу некому его рисовать.
    if !chat.summary.settings.rpg_enabled {
        return Ok(Json(json!({ "chat": chat })));
    }
    // Герой — САМЫЙ РАННИЙ персонаж, как и в движке с картинками: список отсортирован по
    // времени создания, поэтому берём первого.
    let hero_id = state.store.list_characters(&chat_id)?.into_iter().next().map(|hero| hero.id);
    // Отдаём БАЗОВЫЕ характеристики: надетое снаряжение интерфейс складывает сам, и
    // производные значения тут посчитались бы дважды.
    let hero_rpg = match hero_id.as_deref() {
        Some(id) => state.store.character_rpg(&chat_id, id)?,
        None => None,
    };
    Ok(Json(json!({
        "chat": chat,
        "heroId": hero_id,
        "heroRpg": hero_rpg,
        "items": state.store.list_items(&chat_id)?,
        "events": state.store.list_events(&chat_id, EVENT_HISTORY_LIMIT)?,
    })))
}

/// Сколько записей журнала поднимаем вместе с чатом — столько же, сколько прежняя версия.
const EVENT_HISTORY_LIMIT: i64 = 200;

#[derive(Deserialize)]
pub struct PatchChat {
    settings: Option<StorySettings>,
    title: Option<String>,
}

pub async fn patch_chat(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
    Json(body): Json<PatchChat>,
) -> ApiResult<Json<Value>> {
    if let Some(settings) = body.settings {
        state.store.update_settings(&chat_id, &settings)?;
    }
    if let Some(title) = body.title {
        state.store.set_title(&chat_id, &title)?;
    }
    let chat = state.store.get_chat(&chat_id)?.ok_or_else(|| ApiError::not_found("чат не найден"))?;
    Ok(Json(json!({ "chat": chat })))
}

pub async fn delete_chat(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<Value>> {
    // Записи в базе удалить мало: кадры и озвучка этой истории остались бы на диске
    // навсегда — и как мусор, и как след удалённой игры.
    let files = chat_files(&state, &chat_id);
    state.store.delete_chat(&chat_id)?;
    for path in files {
        let _ = std::fs::remove_file(path);
    }
    Ok(Json(json!({ "ok": true })))
}

/// Файлы, которые породила эта история: кадры из сообщений и клипы озвучки.
///
/// Берём только то, что лежит В КАТАЛОГЕ сгенерированного и является файлом: ссылка в
/// сообщении может указывать куда угодно, и удалять по ней вслепую нельзя.
fn chat_files(state: &AppState, chat_id: &str) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut keep = |url: &str| {
        if let Some(name) = url.strip_prefix("/generated/") {
            // Имя должно быть именно именем: ни каталогов, ни выхода наверх.
            if !name.is_empty() && !name.contains('/') && !name.contains('\\') && name != ".." {
                let path = state.generated.join(name);
                if path.is_file() {
                    files.push(path);
                }
            }
        }
    };

    if let Ok(Some(chat)) = state.store.get_chat(chat_id) {
        for message in &chat.messages {
            if let Some(image) = &message.generated_image {
                keep(&image.url);
            }
            for attachment in &message.attachments {
                keep(&attachment.url);
            }
        }
        for character in &chat.characters {
            if let Some(portrait) = &character.portrait {
                keep(&portrait.url);
            }
        }
    }

    // Клипы озвучки в базе не значатся — их имя начинается с идентификатора истории.
    let prefix = format!("tts-{}", crate::tts::safe_name(chat_id, 80));
    if let Ok(entries) = std::fs::read_dir(&state.generated) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            if path.is_file() && name.starts_with(&prefix) {
                files.push(path);
            }
        }
    }
    files
}

pub async fn list_events(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "events": state.store.list_events(&chat_id, 200)? })))
}

pub async fn list_items(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "items": state.store.list_items(&chat_id)? })))
}

#[derive(Deserialize)]
pub struct EquipBody {
    equipped: bool,
}

pub async fn equip_item(
    State(state): State<AppState>,
    Path((chat_id, item_id)): Path<(String, String)>,
    Json(body): Json<EquipBody>,
) -> ApiResult<Json<Value>> {
    let item = state
        .store
        .set_item_equipped(&chat_id, &item_id, body.equipped)?
        .ok_or_else(|| ApiError::not_found("предмет не найден"))?;
    Ok(Json(json!({ "item": item })))
}

#[derive(Deserialize)]
pub struct DeleteMessage {
    /// Удалять ли всё, что было после. По умолчанию да: ход сносится целиком, иначе
    /// повтор задвоил бы его последствия.
    #[serde(default = "yes")]
    include_after: bool,
}

fn yes() -> bool {
    true
}

pub async fn delete_message(
    State(state): State<AppState>,
    Path((_chat_id, message_id)): Path<(String, String)>,
    Query(query): Query<DeleteMessage>,
) -> ApiResult<Json<Value>> {
    let removed = state.store.delete_message_and_after(&message_id, query.include_after)?;
    if !removed {
        return Err(ApiError::not_found("сообщение не найдено"));
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct EditMessage {
    content: String,
}

pub async fn edit_message(
    State(state): State<AppState>,
    Path((_chat_id, message_id)): Path<(String, String)>,
    Json(body): Json<EditMessage>,
) -> ApiResult<Json<Value>> {
    if !state.store.set_message_content(&message_id, &body.content)? {
        return Err(ApiError::not_found("сообщение не найдено"));
    }
    Ok(Json(json!({ "ok": true })))
}

