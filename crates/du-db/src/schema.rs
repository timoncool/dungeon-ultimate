//! Схема базы. Совпадает с той, что создавала версия на better-sqlite3, поэтому
//! существующий `local-roleplay.sqlite` открывается портом без миграции данных.
//!
//! Добавленные позже колонки заводятся так же, как в исходнике, — проверкой
//! `PRAGMA table_info` перед `ALTER TABLE`, чтобы повторный запуск был безопасен.

use rusqlite::Connection;

use crate::DbResult;

const BASE: &str = r#"
CREATE TABLE IF NOT EXISTS chats (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  settings_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
  content TEXT NOT NULL,
  attachments_json TEXT NOT NULL DEFAULT '[]',
  image_request_json TEXT,
  generated_image_json TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_chat_created ON messages(chat_id, created_at);

CREATE TABLE IF NOT EXISTS characters (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  details TEXT NOT NULL DEFAULT '',
  inventory TEXT NOT NULL DEFAULT '',
  skills TEXT NOT NULL DEFAULT '',
  spells TEXT NOT NULL DEFAULT '',
  portrait_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_characters_chat_updated ON characters(chat_id, updated_at);

CREATE TABLE IF NOT EXISTS scenes (
  chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  location TEXT NOT NULL,
  anchor_json TEXT,
  last_json TEXT,
  hops INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (chat_id, location)
);

CREATE TABLE IF NOT EXISTS events (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  text TEXT NOT NULL,
  data_json TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_chat_created ON events(chat_id, created_at);

CREATE TABLE IF NOT EXISTS items (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  data_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_items_chat_created ON items(chat_id, created_at);

CREATE TABLE IF NOT EXISTS quests (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  data_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_quests_chat_created ON quests(chat_id, created_at);

-- Достижения принадлежат ИГРОКУ, а не истории: заслуженное однажды остаётся в профиле,
-- даже если саму историю потом удалили. Поэтому ни ссылки на чат, ни каскада тут нет —
-- чат помнится строкой, просто чтобы знать, где это случилось.
CREATE TABLE IF NOT EXISTS profile_achievements (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL UNIQUE,
  data_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_profile_achievements_created ON profile_achievements(created_at);
"#;

/// Колонки, доехавшие до схемы позже: (таблица, колонка, определение).
const ADDED_COLUMNS: &[(&str, &str, &str)] = &[
    // Сворачиваемая память истории плюс отметка, сколько старых сообщений она покрывает.
    ("chats", "story_summary", "TEXT NOT NULL DEFAULT ''"),
    ("chats", "story_summary_count", "INTEGER NOT NULL DEFAULT 0"),
    ("chats", "rpg_state_json", "TEXT NOT NULL DEFAULT ''"),
    ("chats", "combatants_json", "TEXT NOT NULL DEFAULT ''"),
    // Текущая локация чата: движок картинок знает, из какой сцены растит следующий кадр.
    ("chats", "current_scene", "TEXT NOT NULL DEFAULT ''"),
    // Снимок состояния ДО хода, чтобы Retry/Erase откатывал последствия.
    ("messages", "rpg_snapshot_json", "TEXT"),
    ("characters", "inventory", "TEXT NOT NULL DEFAULT ''"),
    ("characters", "skills", "TEXT NOT NULL DEFAULT ''"),
    ("characters", "spells", "TEXT NOT NULL DEFAULT ''"),
    ("characters", "voice", "TEXT"),
    ("characters", "rpg_json", "TEXT NOT NULL DEFAULT ''"),
    // Эволюционирующий референс внешности: отдельно от статичного портрета, заданного игроком.
    ("characters", "reference_json", "TEXT"),
];

pub fn ensure(conn: &Connection) -> DbResult<()> {
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(BASE)?;
    for (table, column, definition) in ADDED_COLUMNS {
        if !has_column(conn, table, column)? {
            conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"))?;
        }
    }
    carry_over_achievements(conn)?;
    Ok(())
}

/// Перенести награды из первой, привязанной к истории таблицы в профиль.
///
/// Достижения недолго прожили внутри чата, и заработанное тогда терять нельзя: переносим
/// и оставляем старую таблицу нетронутой — удалять чужие данные не наше дело.
fn carry_over_achievements(conn: &Connection) -> DbResult<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'achievements'",
        [],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Ok(());
    }
    conn.execute_batch(
        "INSERT OR IGNORE INTO profile_achievements (id, title, data_json, created_at)
         SELECT id, json_extract(data_json, '$.title'), data_json, created_at FROM achievements
          WHERE json_extract(data_json, '$.title') IS NOT NULL",
    )?;
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> DbResult<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}
