import Database from "better-sqlite3";
import { mkdirSync } from "node:fs";
import path from "node:path";
import { DEFAULT_CHAT_TITLE, DEFAULT_STORY_SETTINGS, titleFromInput } from "@/lib/defaults";
import { isLocalTextModelId, isTextProvider } from "@/lib/text-models";
import type {
  Attachment,
  GeneratedImage,
  ImageRequest,
  StoryChat,
  StoryCharacter,
  StoryChatSummary,
  StoryMessage,
  StorySettings,
} from "@/lib/types";

const dbPath =
  process.env.SQLITE_DB_PATH || path.join(process.cwd(), "data", "local-roleplay.sqlite");

type ChatRow = {
  id: string;
  title: string;
  settings_json: string;
  created_at: string;
  updated_at: string;
  message_count?: number;
  last_message_preview?: string | null;
};

type MessageRow = {
  id: string;
  chat_id: string;
  role: "user" | "assistant";
  content: string;
  attachments_json: string;
  image_request_json: string | null;
  generated_image_json: string | null;
  created_at: string;
};

type CharacterRow = {
  id: string;
  chat_id: string;
  name: string;
  details: string;
  portrait_json: string | null;
  created_at: string;
  updated_at: string;
};

declare global {
  var __localRoleplayDb: Database.Database | undefined;
}

function ensureSchema(db: Database.Database) {
  db.exec(`
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

    CREATE INDEX IF NOT EXISTS idx_messages_chat_created
      ON messages(chat_id, created_at);

    CREATE TABLE IF NOT EXISTS characters (
      id TEXT PRIMARY KEY,
      chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
      name TEXT NOT NULL,
      details TEXT NOT NULL DEFAULT '',
      portrait_json TEXT,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_characters_chat_updated
      ON characters(chat_id, updated_at);
  `);

  // Compaction memory: a rolling "story so far" summary plus a watermark of
  // how many of the chat's oldest messages it already covers.
  const chatColumns = db.prepare(`PRAGMA table_info(chats)`).all() as Array<{ name: string }>;
  if (!chatColumns.some((column) => column.name === "story_summary")) {
    db.exec(`ALTER TABLE chats ADD COLUMN story_summary TEXT NOT NULL DEFAULT ''`);
  }
  if (!chatColumns.some((column) => column.name === "story_summary_count")) {
    db.exec(`ALTER TABLE chats ADD COLUMN story_summary_count INTEGER NOT NULL DEFAULT 0`);
  }
}

function getDatabase() {
  if (globalThis.__localRoleplayDb) {
    ensureSchema(globalThis.__localRoleplayDb);
    return globalThis.__localRoleplayDb;
  }

  mkdirSync(path.dirname(dbPath), { recursive: true });
  const db = new Database(dbPath);
  db.pragma("journal_mode = WAL");
  db.pragma("foreign_keys = ON");
  ensureSchema(db);

  globalThis.__localRoleplayDb = db;
  return db;
}

function parseJson<T>(raw: string | null | undefined, fallback: T): T {
  if (!raw) {
    return fallback;
  }

  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function normalizeSettings(settings?: Partial<StorySettings>): StorySettings {
  const merged = {
    ...DEFAULT_STORY_SETTINGS,
    ...settings,
  };

  if (
    merged.aspect !== "square" &&
    merged.aspect !== "portrait" &&
    merged.aspect !== "landscape"
  ) {
    merged.aspect = DEFAULT_STORY_SETTINGS.aspect;
  }

  if (
    merged.imageBackend !== "mflux-hs" &&
    merged.imageBackend !== "sdnq-hs"
  ) {
    merged.imageBackend = DEFAULT_STORY_SETTINGS.imageBackend;
  }

  if (!isTextProvider(merged.textProvider)) {
    merged.textProvider = DEFAULT_STORY_SETTINGS.textProvider;
  }

  if (!isLocalTextModelId(merged.localTextModel)) {
    merged.localTextModel = DEFAULT_STORY_SETTINGS.localTextModel;
  }

  merged.customBaseUrl =
    typeof merged.customBaseUrl === "string" ? merged.customBaseUrl.trim().slice(0, 500) : "";
  merged.customModel =
    typeof merged.customModel === "string" ? merged.customModel.trim().slice(0, 200) : "";
  merged.customApiKey =
    typeof merged.customApiKey === "string" ? merged.customApiKey.trim().slice(0, 400) : "";

  return merged;
}

function mapChatSummary(row: ChatRow): StoryChatSummary {
  return {
    id: row.id,
    title: row.title,
    settings: normalizeSettings(parseJson<Partial<StorySettings>>(row.settings_json, {})),
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    messageCount: Number(row.message_count || 0),
    lastMessagePreview: row.last_message_preview || undefined,
  };
}

function mapMessage(row: MessageRow): StoryMessage {
  return {
    id: row.id,
    role: row.role,
    content: row.content,
    createdAt: row.created_at,
    attachments: parseJson<Attachment[]>(row.attachments_json, []),
    imageRequest: parseJson<ImageRequest | undefined>(row.image_request_json, undefined),
    generatedImage: parseJson<GeneratedImage | undefined>(row.generated_image_json, undefined),
  };
}

function mapCharacter(row: CharacterRow): StoryCharacter {
  return {
    id: row.id,
    chatId: row.chat_id,
    name: row.name,
    details: row.details,
    portrait: parseJson<Attachment | undefined>(row.portrait_json, undefined),
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

export function listChats(): StoryChatSummary[] {
  const db = getDatabase();
  const rows = db
    .prepare(
      `
        SELECT
          c.id,
          c.title,
          c.settings_json,
          c.created_at,
          c.updated_at,
          COUNT(m.id) AS message_count,
          (
            SELECT content
            FROM messages
            WHERE chat_id = c.id
            ORDER BY created_at DESC, rowid DESC
            LIMIT 1
          ) AS last_message_preview
        FROM chats c
        LEFT JOIN messages m ON m.chat_id = c.id
        GROUP BY c.id
        ORDER BY c.updated_at DESC
      `,
    )
    .all() as ChatRow[];

  return rows.map(mapChatSummary);
}

export function createChat(settings?: Partial<StorySettings>, title = DEFAULT_CHAT_TITLE): StoryChat {
  const db = getDatabase();
  const now = new Date().toISOString();
  const chatId = crypto.randomUUID();

  db.prepare(
    `
      INSERT INTO chats (id, title, settings_json, created_at, updated_at)
      VALUES (@id, @title, @settingsJson, @createdAt, @updatedAt)
    `,
  ).run({
    id: chatId,
    title,
    settingsJson: JSON.stringify(normalizeSettings(settings)),
    createdAt: now,
    updatedAt: now,
  });

  const chat = getChat(chatId);
  if (!chat) {
    throw new Error("Failed to create chat.");
  }

  return chat;
}

export function getStorySummary(chatId: string): { summary: string; coveredCount: number } {
  const db = getDatabase();
  const row = db
    .prepare(`SELECT story_summary, story_summary_count FROM chats WHERE id = ?`)
    .get(chatId) as { story_summary?: string; story_summary_count?: number } | undefined;

  return {
    summary: row?.story_summary || "",
    coveredCount: Number(row?.story_summary_count || 0),
  };
}

export function setStorySummary(chatId: string, summary: string, coveredCount: number) {
  const db = getDatabase();
  db.prepare(
    `UPDATE chats SET story_summary = @summary, story_summary_count = @coveredCount WHERE id = @chatId`,
  ).run({ chatId, summary, coveredCount });
}

export function getChat(chatId: string): StoryChat | null {
  const db = getDatabase();
  const chatRow = db
    .prepare(
      `
        SELECT
          c.id,
          c.title,
          c.settings_json,
          c.created_at,
          c.updated_at,
          COUNT(m.id) AS message_count,
          (
            SELECT content
            FROM messages
            WHERE chat_id = c.id
            ORDER BY created_at DESC, rowid DESC
            LIMIT 1
          ) AS last_message_preview
        FROM chats c
        LEFT JOIN messages m ON m.chat_id = c.id
        WHERE c.id = ?
        GROUP BY c.id
      `,
    )
    .get(chatId) as ChatRow | undefined;

  if (!chatRow) {
    return null;
  }

  const messageRows = db
    .prepare(
      `
        SELECT id, chat_id, role, content, attachments_json, image_request_json, generated_image_json, created_at
        FROM messages
        WHERE chat_id = ?
        ORDER BY created_at ASC, rowid ASC
      `,
    )
    .all(chatId) as MessageRow[];

  return {
    ...mapChatSummary(chatRow),
    messages: messageRows.map(mapMessage),
    characters: listCharacters(chatId),
  };
}

export function updateChat(
  chatId: string,
  updates: { title?: string; settings?: Partial<StorySettings> },
): StoryChat | null {
  const current = getChat(chatId);
  if (!current) {
    return null;
  }

  const nextTitle = updates.title?.trim() || current.title;
  const nextSettings = updates.settings
    ? normalizeSettings({ ...current.settings, ...updates.settings })
    : current.settings;
  const now = new Date().toISOString();

  getDatabase()
    .prepare(
      `
        UPDATE chats
        SET title = @title, settings_json = @settingsJson, updated_at = @updatedAt
        WHERE id = @id
      `,
    )
    .run({
      id: chatId,
      title: nextTitle,
      settingsJson: JSON.stringify(nextSettings),
      updatedAt: now,
    });

  return getChat(chatId);
}

export function updateChatTitleFromInput(chatId: string, input: string) {
  const chat = getChat(chatId);
  if (!chat || chat.title !== DEFAULT_CHAT_TITLE) {
    return;
  }

  updateChat(chatId, { title: titleFromInput(input) });
}

export function deleteChat(chatId: string) {
  const result = getDatabase().prepare("DELETE FROM chats WHERE id = ?").run(chatId);
  return result.changes > 0;
}

export function deleteAllLocalStoryData() {
  const db = getDatabase();
  const remove = db.transaction(() => {
    db.prepare("DELETE FROM characters").run();
    db.prepare("DELETE FROM messages").run();
    db.prepare("DELETE FROM chats").run();
  });

  remove();
  db.pragma("wal_checkpoint(TRUNCATE)");
  db.exec("VACUUM");
}

export function addMessage(chatId: string, message: StoryMessage) {
  const db = getDatabase();
  const now = new Date().toISOString();

  const insert = db.transaction(() => {
    db.prepare(
      `
        INSERT OR IGNORE INTO messages (
          id,
          chat_id,
          role,
          content,
          attachments_json,
          image_request_json,
          generated_image_json,
          created_at
        )
        VALUES (@id, @chatId, @role, @content, @attachmentsJson, @imageRequestJson, @generatedImageJson, @createdAt)
      `,
    ).run({
      id: message.id,
      chatId,
      role: message.role,
      content: message.content,
      attachmentsJson: JSON.stringify(message.attachments || []),
      imageRequestJson: message.imageRequest ? JSON.stringify(message.imageRequest) : null,
      generatedImageJson: message.generatedImage ? JSON.stringify(message.generatedImage) : null,
      createdAt: message.createdAt,
    });

    db.prepare("UPDATE chats SET updated_at = ? WHERE id = ?").run(now, chatId);
  });

  insert();
}

export function updateMessageGeneratedImage(messageId: string, generatedImage: GeneratedImage) {
  const db = getDatabase();
  const row = db
    .prepare("SELECT chat_id FROM messages WHERE id = ?")
    .get(messageId) as { chat_id: string } | undefined;

  if (!row) {
    return false;
  }

  const update = db.transaction(() => {
    db.prepare("UPDATE messages SET generated_image_json = ? WHERE id = ?").run(
      JSON.stringify(generatedImage),
      messageId,
    );
    db.prepare("UPDATE chats SET updated_at = ? WHERE id = ?").run(
      new Date().toISOString(),
      row.chat_id,
    );
  });

  update();
  return true;
}

export function updateMessageContent(messageId: string, content: string): boolean {
  const db = getDatabase();
  const row = db
    .prepare("SELECT chat_id FROM messages WHERE id = ?")
    .get(messageId) as { chat_id: string } | undefined;

  if (!row) {
    return false;
  }

  db.transaction(() => {
    db.prepare("UPDATE messages SET content = ? WHERE id = ?").run(content, messageId);
    db.prepare("UPDATE chats SET updated_at = ? WHERE id = ?").run(
      new Date().toISOString(),
      row.chat_id,
    );
  })();

  return true;
}

// Delete a message and, optionally, every message after it in the same chat
// (used by retry/erase, which discard the tail of the story).
export function deleteMessageAndAfter(messageId: string, includeAfter = true): boolean {
  const db = getDatabase();
  const row = db
    .prepare("SELECT chat_id, created_at FROM messages WHERE id = ?")
    .get(messageId) as { chat_id: string; created_at: string } | undefined;

  if (!row) {
    return false;
  }

  db.transaction(() => {
    if (includeAfter) {
      db.prepare(
        "DELETE FROM messages WHERE chat_id = ? AND (created_at > ? OR (created_at = ? AND id = ?))",
      ).run(row.chat_id, row.created_at, row.created_at, messageId);
    } else {
      db.prepare("DELETE FROM messages WHERE id = ?").run(messageId);
    }
    db.prepare("UPDATE chats SET updated_at = ? WHERE id = ?").run(
      new Date().toISOString(),
      row.chat_id,
    );
  })();

  return true;
}

export function listCharacters(chatId: string): StoryCharacter[] {
  const rows = getDatabase()
    .prepare(
      `
        SELECT id, chat_id, name, details, portrait_json, created_at, updated_at
        FROM characters
        WHERE chat_id = ?
        ORDER BY updated_at DESC, created_at DESC
      `,
    )
    .all(chatId) as CharacterRow[];

  return rows.map(mapCharacter);
}

export function getCharactersByIds(chatId: string, characterIds: string[]) {
  if (!characterIds.length) {
    return [];
  }

  const placeholders = characterIds.map(() => "?").join(",");
  const rows = getDatabase()
    .prepare(
      `
        SELECT id, chat_id, name, details, portrait_json, created_at, updated_at
        FROM characters
        WHERE chat_id = ? AND id IN (${placeholders})
      `,
    )
    .all(chatId, ...characterIds) as CharacterRow[];
  const byId = new Map(rows.map((row) => [row.id, mapCharacter(row)]));

  return characterIds.flatMap((id) => {
    const character = byId.get(id);
    return character ? [character] : [];
  });
}

export function createCharacter(
  chatId: string,
  input: { name: string; details?: string; portrait?: Attachment },
): StoryCharacter | null {
  const db = getDatabase();
  const now = new Date().toISOString();
  const id = crypto.randomUUID();
  const name = input.name.trim();

  if (!name) {
    throw new Error("Character name is required.");
  }

  const insert = db.transaction(() => {
    db.prepare(
      `
        INSERT INTO characters (id, chat_id, name, details, portrait_json, created_at, updated_at)
        VALUES (@id, @chatId, @name, @details, @portraitJson, @createdAt, @updatedAt)
      `,
    ).run({
      id,
      chatId,
      name,
      details: input.details?.trim() || "",
      portraitJson: input.portrait ? JSON.stringify(input.portrait) : null,
      createdAt: now,
      updatedAt: now,
    });
    db.prepare("UPDATE chats SET updated_at = ? WHERE id = ?").run(now, chatId);
  });

  insert();

  return getCharactersByIds(chatId, [id])[0] || null;
}

export function updateCharacter(
  chatId: string,
  characterId: string,
  updates: { name?: string; details?: string; portrait?: Attachment | null },
): StoryCharacter | null {
  const existing = getCharactersByIds(chatId, [characterId])[0];
  if (!existing) {
    return null;
  }

  const nextName = updates.name !== undefined ? updates.name.trim() : existing.name;
  if (!nextName) {
    throw new Error("Character name is required.");
  }

  const nextDetails = updates.details !== undefined ? updates.details.trim() : existing.details;
  const nextPortrait =
    updates.portrait !== undefined ? updates.portrait || undefined : existing.portrait;
  const now = new Date().toISOString();
  const db = getDatabase();

  const update = db.transaction(() => {
    db
      .prepare(
        `
          UPDATE characters
          SET name = @name, details = @details, portrait_json = @portraitJson, updated_at = @updatedAt
          WHERE id = @id AND chat_id = @chatId
        `,
      )
      .run({
        id: characterId,
        chatId,
        name: nextName,
        details: nextDetails,
        portraitJson: nextPortrait ? JSON.stringify(nextPortrait) : null,
        updatedAt: now,
      });
    db.prepare("UPDATE chats SET updated_at = ? WHERE id = ?").run(now, chatId);
  });

  update();

  return getCharactersByIds(chatId, [characterId])[0] || null;
}

export function deleteCharacter(chatId: string, characterId: string) {
  const db = getDatabase();
  const now = new Date().toISOString();
  const remove = db.transaction(() => {
    const result = db
      .prepare("DELETE FROM characters WHERE id = ? AND chat_id = ?")
      .run(characterId, chatId);
    if (result.changes > 0) {
      db.prepare("UPDATE chats SET updated_at = ? WHERE id = ?").run(now, chatId);
    }
    return result.changes > 0;
  });

  return remove();
}
