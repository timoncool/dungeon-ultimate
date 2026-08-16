//! Хранилище Dungeon Ultimate поверх SQLite.
//!
//! Схема и формат JSON-колонок совпадают с версией на better-sqlite3, поэтому старый
//! `local-roleplay.sqlite` открывается как есть — переезд на порт не теряет сохранёнки.

mod schema;

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use du_core::{
    normalize_location, Attachment, GeneratedImage, ImageRequest, Scene, StoryChat, StoryChatSummary,
    StoryCharacter, StoryMessage, StoryRole, StorySettings,
};
use du_rpg::{CharacterRpg, Enemy, GameEvent, Item, RpgState};
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("ошибка базы: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("повреждённый JSON в колонке {column}: {source}")]
    Json { column: &'static str, source: serde_json::Error },
    #[error("не найдено: {0}")]
    NotFound(String),
}

pub type DbResult<T> = Result<T, DbError>;

/// Сколько правок подряд можно накатить на одну локацию, прежде чем сцена
/// переустанавливается свежим кадром. Ограничивает накопление дрейфа при
/// многократном редактировании одной и той же картинки.
pub const MAX_EDIT_HOPS: i64 = 6;

/// Соединение с базой. Клонируется дёшево; запросы сериализуются мьютексом, чего
/// хватает: обращения идут из одного сервера, а тяжёлое время уходит на GPU.
#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Разобрать JSON-колонку, молча падая на значение по умолчанию: строки, записанные
/// прошлыми версиями, не должны ронять чат целиком.
fn parse_or_default<T: serde::de::DeserializeOwned + Default>(raw: Option<String>) -> T {
    raw.filter(|text| !text.trim().is_empty())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn parse_opt<T: serde::de::DeserializeOwned>(raw: Option<String>) -> Option<T> {
    raw.filter(|text| !text.trim().is_empty())
        .and_then(|text| serde_json::from_str(&text).ok())
}

impl Store {
    /// Открыть (при необходимости создав) базу по пути.
    pub fn open(path: &Path) -> DbResult<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        schema::ensure(&conn)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// База в памяти — для тестов.
    pub fn open_in_memory() -> DbResult<Self> {
        let conn = Connection::open_in_memory()?;
        schema::ensure(&conn)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        // Отравленный мьютекс означает панику в другом запросе; соединение при этом
        // остаётся валидным, поэтому работаем дальше, а не роняем весь сервер.
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    // ── Чаты ───────────────────────────────────────────────────────────────────

    pub fn list_chats(&self) -> DbResult<Vec<StoryChatSummary>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT c.id, c.title, c.settings_json, c.created_at, c.updated_at,
                    (SELECT COUNT(*) FROM messages m WHERE m.chat_id = c.id) AS message_count,
                    (SELECT m.content FROM messages m WHERE m.chat_id = c.id
                      ORDER BY m.created_at DESC, m.rowid DESC LIMIT 1) AS last_preview
               FROM chats c ORDER BY c.updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StoryChatSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                settings: parse_or_default(row.get::<_, Option<String>>(2)?),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                message_count: row.get::<_, i64>(5)? as usize,
                last_message_preview: row
                    .get::<_, Option<String>>(6)?
                    .map(|text| text.chars().take(160).collect()),
            })
        })?;
        rows.collect::<Result<_, _>>().map_err(Into::into)
    }

    pub fn create_chat(&self, settings: StorySettings, title: &str) -> DbResult<String> {
        let id = new_id();
        let now = now_iso();
        let settings_json = serde_json::to_string(&settings).unwrap_or_else(|_| "{}".into());
        self.lock().execute(
            "INSERT INTO chats (id, title, settings_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
            params![id, title, settings_json, now, now],
        )?;
        Ok(id)
    }

    pub fn get_chat(&self, chat_id: &str) -> DbResult<Option<StoryChat>> {
        let summary = {
            let conn = self.lock();
            conn.query_row(
                "SELECT id, title, settings_json, created_at, updated_at FROM chats WHERE id = ?",
                params![chat_id],
                |row| {
                    Ok(StoryChatSummary {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        settings: parse_or_default(row.get::<_, Option<String>>(2)?),
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        message_count: 0,
                        last_message_preview: None,
                    })
                },
            )
            .optional()?
        };
        let Some(mut summary) = summary else { return Ok(None) };
        let messages = self.list_messages(chat_id)?;
        let characters = self.list_characters(chat_id)?;
        summary.message_count = messages.len();
        summary.last_message_preview =
            messages.last().map(|m| m.content.chars().take(160).collect());
        Ok(Some(StoryChat { summary, messages, characters }))
    }

    pub fn update_settings(&self, chat_id: &str, settings: &StorySettings) -> DbResult<()> {
        let json = serde_json::to_string(settings).unwrap_or_else(|_| "{}".into());
        self.lock().execute(
            "UPDATE chats SET settings_json = ?, updated_at = ? WHERE id = ?",
            params![json, now_iso(), chat_id],
        )?;
        Ok(())
    }

    pub fn set_title(&self, chat_id: &str, title: &str) -> DbResult<()> {
        self.lock().execute(
            "UPDATE chats SET title = ?, updated_at = ? WHERE id = ?",
            params![title, now_iso(), chat_id],
        )?;
        Ok(())
    }

    pub fn delete_chat(&self, chat_id: &str) -> DbResult<()> {
        self.lock().execute("DELETE FROM chats WHERE id = ?", params![chat_id])?;
        Ok(())
    }

    /// Стереть всё локальное: игрок нажал «удалить мои данные».
    pub fn delete_everything(&self) -> DbResult<()> {
        self.lock().execute_batch(
            "DELETE FROM items; DELETE FROM events; DELETE FROM scenes;
             DELETE FROM characters; DELETE FROM messages; DELETE FROM chats;",
        )?;
        Ok(())
    }

    /// Свёрнутая «история до сих пор» и сколько старых сообщений она покрывает.
    pub fn story_summary(&self, chat_id: &str) -> DbResult<(String, i64)> {
        let conn = self.lock();
        let row = conn
            .query_row(
                "SELECT story_summary, story_summary_count FROM chats WHERE id = ?",
                params![chat_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        Ok(row.unwrap_or_default())
    }

    pub fn set_story_summary(&self, chat_id: &str, summary: &str, covered: i64) -> DbResult<()> {
        self.lock().execute(
            "UPDATE chats SET story_summary = ?, story_summary_count = ? WHERE id = ?",
            params![summary, covered, chat_id],
        )?;
        Ok(())
    }

    // ── Сообщения ──────────────────────────────────────────────────────────────

    pub fn list_messages(&self, chat_id: &str) -> DbResult<Vec<StoryMessage>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, role, content, attachments_json, image_request_json,
                    generated_image_json, rpg_snapshot_json, created_at
               FROM messages WHERE chat_id = ? ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map(params![chat_id], |row| {
            let role: String = row.get(1)?;
            Ok(StoryMessage {
                id: row.get(0)?,
                role: if role == "assistant" { StoryRole::Assistant } else { StoryRole::User },
                content: row.get(2)?,
                attachments: parse_or_default::<Vec<Attachment>>(row.get::<_, Option<String>>(3)?),
                image_request: parse_opt::<ImageRequest>(row.get::<_, Option<String>>(4)?),
                generated_image: parse_opt::<GeneratedImage>(row.get::<_, Option<String>>(5)?),
                rpg_snapshot: parse_opt::<serde_json::Value>(row.get::<_, Option<String>>(6)?),
                created_at: row.get(7)?,
            })
        })?;
        rows.collect::<Result<_, _>>().map_err(Into::into)
    }

    pub fn add_message(&self, chat_id: &str, message: &StoryMessage) -> DbResult<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO messages (id, chat_id, role, content, attachments_json,
                                   image_request_json, generated_image_json, rpg_snapshot_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                message.id,
                chat_id,
                match message.role {
                    StoryRole::Assistant => "assistant",
                    StoryRole::User => "user",
                },
                message.content,
                serde_json::to_string(&message.attachments).unwrap_or_else(|_| "[]".into()),
                message.image_request.as_ref().and_then(|v| serde_json::to_string(v).ok()),
                message.generated_image.as_ref().and_then(|v| serde_json::to_string(v).ok()),
                message.rpg_snapshot.as_ref().and_then(|v| serde_json::to_string(v).ok()),
                message.created_at,
            ],
        )?;
        conn.execute(
            "UPDATE chats SET updated_at = ? WHERE id = ?",
            params![now_iso(), chat_id],
        )?;
        Ok(())
    }

    /// Чат и заявка на кадр, к которым относится сообщение: по ним движок картинок
    /// восстанавливает контекст сцены.
    pub fn message_context(&self, message_id: &str) -> DbResult<Option<(String, Option<ImageRequest>)>> {
        let conn = self.lock();
        let row = conn
            .query_row(
                "SELECT chat_id, image_request_json FROM messages WHERE id = ?",
                params![message_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        Ok(row.map(|(chat_id, raw)| (chat_id, parse_opt::<ImageRequest>(raw))))
    }

    pub fn set_message_image(&self, message_id: &str, image: &GeneratedImage) -> DbResult<()> {
        let json = serde_json::to_string(image).map_err(|source| DbError::Json {
            column: "generated_image_json",
            source,
        })?;
        self.lock().execute(
            "UPDATE messages SET generated_image_json = ? WHERE id = ?",
            params![json, message_id],
        )?;
        Ok(())
    }

    pub fn set_message_content(&self, message_id: &str, content: &str) -> DbResult<bool> {
        let changed = self.lock().execute(
            "UPDATE messages SET content = ? WHERE id = ?",
            params![content, message_id],
        )?;
        Ok(changed > 0)
    }

    /// Удалить сообщение и, при `include_after`, всё, что было после него в этом чате.
    /// Именно на этом держатся «Повторить» и «Стереть»: ход убирается целиком.
    pub fn delete_message_and_after(&self, message_id: &str, include_after: bool) -> DbResult<bool> {
        let conn = self.lock();
        let found = conn
            .query_row(
                "SELECT chat_id, created_at, rowid FROM messages WHERE id = ?",
                params![message_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)),
            )
            .optional()?;
        let Some((chat_id, created_at, rowid)) = found else { return Ok(false) };

        if include_after {
            // Порядок тот же, что и при чтении: сначала по времени, при равенстве — по rowid.
            conn.execute(
                "DELETE FROM messages WHERE chat_id = ?
                   AND (created_at > ? OR (created_at = ? AND rowid >= ?))",
                params![chat_id, created_at, created_at, rowid],
            )?;
        } else {
            conn.execute("DELETE FROM messages WHERE id = ?", params![message_id])?;
        }
        conn.execute("UPDATE chats SET updated_at = ? WHERE id = ?", params![now_iso(), chat_id])?;
        Ok(true)
    }

    // ── Персонажи ──────────────────────────────────────────────────────────────

    pub fn list_characters(&self, chat_id: &str) -> DbResult<Vec<StoryCharacter>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, name, details, inventory, skills, spells, portrait_json,
                    voice, created_at, updated_at
               FROM characters WHERE chat_id = ? ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![chat_id], |row| {
            Ok(StoryCharacter {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                name: row.get(2)?,
                details: row.get(3)?,
                inventory: row.get(4)?,
                skills: row.get(5)?,
                spells: row.get(6)?,
                portrait: parse_opt::<Attachment>(row.get::<_, Option<String>>(7)?),
                voice: row.get::<_, Option<String>>(8)?.filter(|v| !v.trim().is_empty()),
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;
        rows.collect::<Result<_, _>>().map_err(Into::into)
    }

    pub fn create_character(&self, character: &StoryCharacter) -> DbResult<()> {
        self.lock().execute(
            "INSERT INTO characters (id, chat_id, name, details, inventory, skills, spells,
                                     portrait_json, voice, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                character.id,
                character.chat_id,
                character.name,
                character.details,
                character.inventory,
                character.skills,
                character.spells,
                character.portrait.as_ref().and_then(|v| serde_json::to_string(v).ok()),
                character.voice,
                character.created_at,
                character.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_character(&self, character: &StoryCharacter) -> DbResult<()> {
        self.lock().execute(
            "UPDATE characters SET name = ?, details = ?, inventory = ?, skills = ?, spells = ?,
                                   portrait_json = ?, voice = ?, updated_at = ?
             WHERE id = ? AND chat_id = ?",
            params![
                character.name,
                character.details,
                character.inventory,
                character.skills,
                character.spells,
                character.portrait.as_ref().and_then(|v| serde_json::to_string(v).ok()),
                character.voice,
                now_iso(),
                character.id,
                character.chat_id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_character(&self, chat_id: &str, character_id: &str) -> DbResult<()> {
        self.lock().execute(
            "DELETE FROM characters WHERE id = ? AND chat_id = ?",
            params![character_id, chat_id],
        )?;
        Ok(())
    }

    /// Протагонист — самый первый заведённый персонаж чата.
    pub fn hero(&self, chat_id: &str) -> DbResult<Option<StoryCharacter>> {
        Ok(self.list_characters(chat_id)?.into_iter().next())
    }

    /// Эволюционирующий референс внешности: как персонаж выглядит СЕЙЧАС. Отличается от
    /// статичного портрета, заданного игроком: портрет задаёт исходный облик, а этот
    /// снимок обновляется последней иллюстрированной сценой.
    pub fn character_reference(&self, chat_id: &str, character_id: &str) -> DbResult<Option<Attachment>> {
        let conn = self.lock();
        let row = conn
            .query_row(
                "SELECT reference_json, portrait_json FROM characters WHERE id = ? AND chat_id = ?",
                params![character_id, chat_id],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let Some((reference, portrait)) = row else { return Ok(None) };
        // Пока сцена ни разу не рисовалась, референсом служит заданный игроком портрет.
        Ok(parse_opt::<Attachment>(reference).or_else(|| parse_opt::<Attachment>(portrait)))
    }

    pub fn set_character_reference(
        &self,
        chat_id: &str,
        character_id: &str,
        reference: &Attachment,
    ) -> DbResult<()> {
        let json = serde_json::to_string(reference).map_err(|source| DbError::Json {
            column: "reference_json",
            source,
        })?;
        self.lock().execute(
            "UPDATE characters SET reference_json = ? WHERE id = ? AND chat_id = ?",
            params![json, character_id, chat_id],
        )?;
        Ok(())
    }

    // ── Состояние RPG ──────────────────────────────────────────────────────────

    pub fn rpg_state(&self, chat_id: &str) -> DbResult<RpgState> {
        let conn = self.lock();
        let raw = conn
            .query_row("SELECT rpg_state_json FROM chats WHERE id = ?", params![chat_id], |row| {
                row.get::<_, Option<String>>(0)
            })
            .optional()?
            .flatten();
        Ok(parse_or_default(raw))
    }

    pub fn set_rpg_state(&self, chat_id: &str, state: &RpgState) -> DbResult<()> {
        let json = serde_json::to_string(state).unwrap_or_else(|_| "{}".into());
        self.lock().execute(
            "UPDATE chats SET rpg_state_json = ? WHERE id = ?",
            params![json, chat_id],
        )?;
        Ok(())
    }

    pub fn combatants(&self, chat_id: &str) -> DbResult<Vec<Enemy>> {
        let conn = self.lock();
        let raw = conn
            .query_row("SELECT combatants_json FROM chats WHERE id = ?", params![chat_id], |row| {
                row.get::<_, Option<String>>(0)
            })
            .optional()?
            .flatten();
        Ok(parse_or_default(raw))
    }

    pub fn set_combatants(&self, chat_id: &str, enemies: &[Enemy]) -> DbResult<()> {
        let json = serde_json::to_string(enemies).unwrap_or_else(|_| "[]".into());
        self.lock().execute(
            "UPDATE chats SET combatants_json = ? WHERE id = ?",
            params![json, chat_id],
        )?;
        Ok(())
    }

    pub fn character_rpg(&self, chat_id: &str, character_id: &str) -> DbResult<Option<CharacterRpg>> {
        let conn = self.lock();
        let raw = conn
            .query_row(
                "SELECT rpg_json FROM characters WHERE id = ? AND chat_id = ?",
                params![character_id, chat_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;
        Ok(raw.map(|raw| parse_or_default::<CharacterRpg>(raw).sanitized()))
    }

    pub fn save_character_rpg(&self, character_id: &str, rpg: &CharacterRpg) -> DbResult<()> {
        let json = serde_json::to_string(rpg).unwrap_or_else(|_| "{}".into());
        self.lock().execute(
            "UPDATE characters SET rpg_json = ?, updated_at = ? WHERE id = ?",
            params![json, now_iso(), character_id],
        )?;
        Ok(())
    }

    // ── Журнал приключения ─────────────────────────────────────────────────────

    pub fn add_events(&self, chat_id: &str, events: &[GameEvent]) -> DbResult<()> {
        if events.is_empty() {
            return Ok(());
        }
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        for event in events {
            tx.execute(
                "INSERT OR REPLACE INTO events (id, chat_id, kind, text, data_json, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    event.id,
                    chat_id,
                    serde_json::to_value(event.kind)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_else(|| "note".into()),
                    event.text,
                    event.data.as_ref().and_then(|v| serde_json::to_string(v).ok()),
                    event.created_at,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_events(&self, chat_id: &str, limit: i64) -> DbResult<Vec<GameEvent>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, kind, text, data_json, created_at FROM events
              WHERE chat_id = ? ORDER BY created_at DESC, rowid DESC LIMIT ?",
        )?;
        let rows = stmt.query_map(params![chat_id, limit], |row| {
            let kind: String = row.get(1)?;
            Ok(GameEvent {
                id: row.get(0)?,
                kind: serde_json::from_value(serde_json::Value::String(kind))
                    .unwrap_or(du_rpg::EventKind::Note),
                text: row.get(2)?,
                data: parse_opt::<serde_json::Value>(row.get::<_, Option<String>>(3)?),
                created_at: row.get(4)?,
            })
        })?;
        let mut events: Vec<GameEvent> = rows.collect::<Result<_, _>>()?;
        // В базе выбираем свежие, а игроку журнал читается сверху вниз по времени.
        events.reverse();
        Ok(events)
    }

    pub fn delete_events(&self, ids: &[String]) -> DbResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute("DELETE FROM events WHERE id = ?", params![id])?;
        }
        tx.commit()?;
        Ok(())
    }

    // ── Предметы ───────────────────────────────────────────────────────────────

    pub fn add_items(&self, chat_id: &str, items: &[Item]) -> DbResult<()> {
        if items.is_empty() {
            return Ok(());
        }
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        for item in items {
            tx.execute(
                "INSERT OR REPLACE INTO items (id, chat_id, data_json, created_at) VALUES (?, ?, ?, ?)",
                params![
                    item.id,
                    chat_id,
                    serde_json::to_string(item).unwrap_or_else(|_| "{}".into()),
                    item.created_at
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_items(&self, chat_id: &str) -> DbResult<Vec<Item>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT data_json FROM items WHERE chat_id = ? ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map(params![chat_id], |row| row.get::<_, String>(0))?;
        let mut items = Vec::new();
        for raw in rows {
            if let Ok(item) = serde_json::from_str::<Item>(&raw?) {
                items.push(item);
            }
        }
        Ok(items)
    }

    pub fn get_item(&self, chat_id: &str, item_id: &str) -> DbResult<Option<Item>> {
        let conn = self.lock();
        let raw = conn
            .query_row(
                "SELECT data_json FROM items WHERE id = ? AND chat_id = ?",
                params![item_id, chat_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(raw.and_then(|raw| serde_json::from_str::<Item>(&raw).ok()))
    }

    fn put_item(&self, chat_id: &str, item: &Item) -> DbResult<()> {
        self.lock().execute(
            "UPDATE items SET data_json = ? WHERE id = ? AND chat_id = ?",
            params![
                serde_json::to_string(item).unwrap_or_else(|_| "{}".into()),
                item.id,
                chat_id
            ],
        )?;
        Ok(())
    }

    /// Надеть или снять предмет. Надевание СНИМАЕТ то, что занимало тот же слот у того
    /// же владельца, — правило «один предмет на слот» держится и здесь, и в выводе статов.
    pub fn set_item_equipped(&self, chat_id: &str, item_id: &str, equipped: bool) -> DbResult<Option<Item>> {
        let Some(mut item) = self.get_item(chat_id, item_id)? else { return Ok(None) };
        if equipped {
            for mut other in self.list_items(chat_id)? {
                if other.id != item.id
                    && other.equipped
                    && other.slot == item.slot
                    && other.owner_id == item.owner_id
                {
                    other.equipped = false;
                    self.put_item(chat_id, &other)?;
                }
            }
        }
        item.equipped = equipped;
        self.put_item(chat_id, &item)?;
        Ok(Some(item))
    }

    /// Привязать к предмету сгенерированный портрет, чтобы дальше он переиспользовался
    /// как референс. Ищем по id, а при его отсутствии — по имени.
    pub fn set_item_image(&self, chat_id: &str, id_or_name: &str, image_url: &str) -> DbResult<Option<Item>> {
        let items = self.list_items(chat_id)?;
        let needle = id_or_name.trim().to_lowercase();
        let found = items
            .into_iter()
            .find(|item| item.id == id_or_name || item.name.trim().to_lowercase() == needle);
        let Some(mut item) = found else { return Ok(None) };
        item.image_url = Some(image_url.to_string());
        self.put_item(chat_id, &item)?;
        Ok(Some(item))
    }

    pub fn delete_items(&self, ids: &[String]) -> DbResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute("DELETE FROM items WHERE id = ?", params![id])?;
        }
        tx.commit()?;
        Ok(())
    }

    // ── Сцены ──────────────────────────────────────────────────────────────────

    pub fn scene(&self, chat_id: &str, location: &str) -> DbResult<Option<Scene>> {
        let key = normalize_location(location);
        if key.is_empty() {
            return Ok(None);
        }
        let conn = self.lock();
        conn.query_row(
            "SELECT location, anchor_json, last_json, hops, updated_at FROM scenes
              WHERE chat_id = ? AND location = ?",
            params![chat_id, key],
            |row| {
                Ok(Scene {
                    location: row.get(0)?,
                    anchor: parse_opt::<Attachment>(row.get::<_, Option<String>>(1)?),
                    last: parse_opt::<Attachment>(row.get::<_, Option<String>>(2)?),
                    hops: row.get::<_, i64>(3)? as u32,
                    updated_at: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    /// Сцена, в которой чат находится прямо сейчас.
    pub fn active_scene(&self, chat_id: &str) -> DbResult<Option<Scene>> {
        let current: Option<String> = {
            let conn = self.lock();
            conn.query_row("SELECT current_scene FROM chats WHERE id = ?", params![chat_id], |row| {
                row.get::<_, Option<String>>(0)
            })
            .optional()?
            .flatten()
        };
        let Some(current) = current.filter(|key| !key.is_empty()) else { return Ok(None) };
        self.scene(chat_id, &current)
    }

    /// Запомнить только что нарисованный кадр за локацией и сделать её активной сценой.
    ///
    /// `anchor = true` (пере)устанавливает место: кадр становится и якорем, и последним,
    /// счётчик правок обнуляется. `anchor = false` записывает правку: двигается только
    /// последний кадр и счётчик, а исходный якорь сохраняется для будущего возврата.
    pub fn record_scene_image(
        &self,
        chat_id: &str,
        location: &str,
        image: &Attachment,
        anchor: bool,
    ) -> DbResult<()> {
        let key = normalize_location(location);
        if key.is_empty() {
            return Ok(());
        }
        let json = serde_json::to_string(image).map_err(|source| DbError::Json {
            column: "anchor_json",
            source,
        })?;
        let now = now_iso();
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let exists: Option<i64> = tx
            .query_row(
                "SELECT hops FROM scenes WHERE chat_id = ? AND location = ?",
                params![chat_id, key],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() || anchor {
            tx.execute(
                "INSERT INTO scenes (chat_id, location, anchor_json, last_json, hops, updated_at)
                 VALUES (?, ?, ?, ?, 0, ?)
                 ON CONFLICT(chat_id, location) DO UPDATE SET
                   anchor_json = excluded.anchor_json,
                   last_json = excluded.last_json,
                   hops = 0,
                   updated_at = excluded.updated_at",
                params![chat_id, key, json, json, now],
            )?;
        } else {
            tx.execute(
                "UPDATE scenes SET last_json = ?, hops = hops + 1, updated_at = ?
                  WHERE chat_id = ? AND location = ?",
                params![json, now, chat_id, key],
            )?;
        }
        tx.execute(
            "UPDATE chats SET current_scene = ?, updated_at = ? WHERE id = ?",
            params![key, now, chat_id],
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
