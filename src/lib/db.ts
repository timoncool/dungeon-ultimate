import Database from "better-sqlite3";
import { mkdirSync } from "node:fs";
import path from "node:path";
import { DEFAULT_CHAT_TITLE, titleFromInput } from "@/lib/defaults";
import { configuredDefaultStorySettings } from "@/lib/runtime-defaults";
import { isLocalTextModelId, isTextProvider } from "@/lib/text-models";
import { isLanguage, isProseSize, isResponseLength } from "@/lib/types";
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
import { coerceCharacterRpg, DEFAULT_RPG_STATE } from "@/lib/rpg/types";
import type { CharacterRpg, Enemy, GameEvent, Item, RpgSnapshot, RpgState } from "@/lib/rpg/types";
import { deriveForOwner } from "@/lib/rpg/derive";

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
  inventory: string;
  skills: string;
  spells: string;
  portrait_json: string | null;
  voice: string | null;
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
      inventory TEXT NOT NULL DEFAULT '',
      skills TEXT NOT NULL DEFAULT '',
      spells TEXT NOT NULL DEFAULT '',
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

  const messageColumns = db.prepare(`PRAGMA table_info(messages)`).all() as Array<{ name: string }>;
  // Pre-turn RPG snapshot, so deleting an assistant message (Retry/Erase) can roll
  // its HP/effects/combatants/loot back instead of leaving them double-applied.
  if (!messageColumns.some((column) => column.name === "rpg_snapshot_json")) {
    db.exec(`ALTER TABLE messages ADD COLUMN rpg_snapshot_json TEXT`);
  }

  const characterColumns = db.prepare(`PRAGMA table_info(characters)`).all() as Array<{ name: string }>;
  if (!characterColumns.some((column) => column.name === "inventory")) {
    db.exec(`ALTER TABLE characters ADD COLUMN inventory TEXT NOT NULL DEFAULT ''`);
  }
  if (!characterColumns.some((column) => column.name === "skills")) {
    db.exec(`ALTER TABLE characters ADD COLUMN skills TEXT NOT NULL DEFAULT ''`);
  }
  if (!characterColumns.some((column) => column.name === "spells")) {
    db.exec(`ALTER TABLE characters ADD COLUMN spells TEXT NOT NULL DEFAULT ''`);
  }

  // Optional per-character TTS voice id (multi-voice narration). Nullable: blank
  // means "use the chat's single narrator voice", so existing rows need no value.
  if (!characterColumns.some((column) => column.name === "voice")) {
    db.exec(`ALTER TABLE characters ADD COLUMN voice TEXT`);
  }

  // RPG layer (additive): per-character stats/HP and chat-level game state as
  // JSON, plus an adventure-log events table for the journal + dice/HP audit.
  if (!characterColumns.some((column) => column.name === "rpg_json")) {
    db.exec(`ALTER TABLE characters ADD COLUMN rpg_json TEXT NOT NULL DEFAULT ''`);
  }

  // img2img entity reuse (additive): an evolving reference image for the
  // character, refreshed by the latest illustrated scene and re-attached as an
  // init/reference on later scene generations so the protagonist stays visually
  // consistent. Distinct from `portrait_json` (the user-set, static portrait):
  // the portrait seeds the look; this tracks how the character currently looks.
  if (!characterColumns.some((column) => column.name === "reference_json")) {
    db.exec(`ALTER TABLE characters ADD COLUMN reference_json TEXT`);
  }
  if (!chatColumns.some((column) => column.name === "rpg_state_json")) {
    db.exec(`ALTER TABLE chats ADD COLUMN rpg_state_json TEXT NOT NULL DEFAULT ''`);
  }
  if (!chatColumns.some((column) => column.name === "combatants_json")) {
    db.exec(`ALTER TABLE chats ADD COLUMN combatants_json TEXT NOT NULL DEFAULT ''`);
  }
  // Scene continuity (img2img): the chat's currently-active location label, so the
  // image engine knows which scene the next shot evolves from. Per-location anchor
  // / last image live in the scenes table below.
  if (!chatColumns.some((column) => column.name === "current_scene")) {
    db.exec(`ALTER TABLE chats ADD COLUMN current_scene TEXT NOT NULL DEFAULT ''`);
  }
  db.exec(`
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
  `);
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
  const defaultSettings = configuredDefaultStorySettings();
  const merged = {
    ...defaultSettings,
    ...settings,
  };

  // Migrate legacy OpenRouter chats into the unified custom provider. Runs
  // before provider validation, since "openrouter" is no longer a valid value.
  const legacy = (settings ?? {}) as Record<string, unknown>;
  if (legacy.textProvider === "openrouter") {
    merged.textProvider = "custom";
    if (!merged.customBaseUrl) merged.customBaseUrl = "https://openrouter.ai/api/v1";
    if (!merged.customModel && typeof legacy.openrouterModel === "string") {
      merged.customModel = legacy.openrouterModel;
    }
    if (!merged.customApiKey && typeof legacy.openrouterApiKey === "string") {
      merged.customApiKey = legacy.openrouterApiKey;
    }
  }

  if (
    merged.aspect !== "square" &&
    merged.aspect !== "portrait" &&
    merged.aspect !== "landscape"
  ) {
    merged.aspect = defaultSettings.aspect;
  }

  if (
    merged.imageBackend !== "mflux-hs" &&
    merged.imageBackend !== "sdnq-hs"
  ) {
    merged.imageBackend = defaultSettings.imageBackend;
  }

  if (merged.imageMode !== "fast" && merged.imageMode !== "slow") {
    merged.imageMode = defaultSettings.imageMode;
  }

  if (typeof merged.imageGenerationEnabled !== "boolean") {
    merged.imageGenerationEnabled = defaultSettings.imageGenerationEnabled;
  }

  if (typeof merged.autoImages !== "boolean") {
    merged.autoImages = defaultSettings.autoImages;
  }

  if (!isProseSize(merged.proseSize)) {
    merged.proseSize = defaultSettings.proseSize;
  }

  if (!isResponseLength(merged.responseLength)) {
    merged.responseLength = defaultSettings.responseLength;
  }

  if (!isLanguage(merged.language)) {
    merged.language = defaultSettings.language;
  }

  if (typeof merged.voice !== "string") {
    merged.voice = defaultSettings.voice;
  }

  merged.narratorPrompt =
    typeof merged.narratorPrompt === "string"
      ? merged.narratorPrompt.slice(0, 20_000)
      : defaultSettings.narratorPrompt;
  merged.imagePrompt =
    typeof merged.imagePrompt === "string"
      ? merged.imagePrompt.slice(0, 20_000)
      : defaultSettings.imagePrompt;
  merged.imageStylePrefix =
    typeof merged.imageStylePrefix === "string"
      ? merged.imageStylePrefix.slice(0, 2_000)
      : defaultSettings.imageStylePrefix;

  if (typeof merged.antiRepetition !== "boolean") {
    merged.antiRepetition = defaultSettings.antiRepetition;
  }

  if (typeof merged.causeAwareEnding !== "boolean") {
    merged.causeAwareEnding = defaultSettings.causeAwareEnding;
  }

  if (typeof merged.multiVoice !== "boolean") {
    merged.multiVoice = defaultSettings.multiVoice;
  }

  if (typeof merged.autoplay !== "boolean") {
    merged.autoplay = defaultSettings.autoplay;
  }

  if (typeof merged.ttsVolume !== "number") {
    merged.ttsVolume = defaultSettings.ttsVolume;
  }

  if (typeof merged.ttsSpeed !== "number") {
    merged.ttsSpeed = defaultSettings.ttsSpeed;
  }

  if (!isTextProvider(merged.textProvider)) {
    merged.textProvider = defaultSettings.textProvider;
  }

  if (!isLocalTextModelId(merged.localTextModel)) {
    merged.localTextModel = defaultSettings.localTextModel;
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
    inventory: row.inventory,
    skills: row.skills,
    spells: row.spells,
    portrait: parseJson<Attachment | undefined>(row.portrait_json, undefined),
    voice: row.voice || undefined,
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
  const db = getDatabase();
  // Only title/settings are needed for the merge — avoid re-hydrating every
  // message/character on the settings-autosave hot path.
  const current = db
    .prepare("SELECT title, settings_json FROM chats WHERE id = ?")
    .get(chatId) as { title: string; settings_json: string } | undefined;
  if (!current) {
    return null;
  }

  const currentSettings = normalizeSettings(
    parseJson<Partial<StorySettings>>(current.settings_json, {}),
  );
  const nextTitle = updates.title?.trim() || current.title;
  const nextSettings = updates.settings
    ? normalizeSettings({ ...currentSettings, ...updates.settings })
    : currentSettings;
  const now = new Date().toISOString();

  db
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
          rpg_snapshot_json,
          created_at
        )
        VALUES (@id, @chatId, @role, @content, @attachmentsJson, @imageRequestJson, @generatedImageJson, @rpgSnapshotJson, @createdAt)
      `,
    ).run({
      id: message.id,
      chatId,
      role: message.role,
      content: message.content,
      attachmentsJson: JSON.stringify(message.attachments || []),
      imageRequestJson: message.imageRequest ? JSON.stringify(message.imageRequest) : null,
      generatedImageJson: message.generatedImage ? JSON.stringify(message.generatedImage) : null,
      rpgSnapshotJson: message.rpgSnapshot ? JSON.stringify(message.rpgSnapshot) : null,
      createdAt: message.createdAt,
    });

    db.prepare("UPDATE chats SET updated_at = ? WHERE id = ?").run(now, chatId);
  });

  insert();
}

// Chat id + the persisted image request for a message, so the images route can
// recover img2img context (which characters/items the scene is about) from just
// the messageId it was handed.
export function getMessageContext(
  messageId: string,
): { chatId: string; imageRequest: ImageRequest | undefined } | null {
  const row = getDatabase()
    .prepare("SELECT chat_id, image_request_json FROM messages WHERE id = ?")
    .get(messageId) as { chat_id: string; image_request_json: string | null } | undefined;
  if (!row) {
    return null;
  }
  return {
    chatId: row.chat_id,
    imageRequest: parseJson<ImageRequest | undefined>(row.image_request_json, undefined),
  };
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

  // The messages about to be removed (the target + everything after, or just it),
  // oldest first — so we can roll back the RPG side effects of those turns.
  const doomed = (
    includeAfter
      ? db
          .prepare(
            "SELECT id, rpg_snapshot_json FROM messages WHERE chat_id = ? AND (created_at > ? OR (created_at = ? AND id = ?)) ORDER BY created_at ASC, rowid ASC",
          )
          .all(row.chat_id, row.created_at, row.created_at, messageId)
      : db.prepare("SELECT id, rpg_snapshot_json FROM messages WHERE id = ?").all(messageId)
  ) as Array<{ id: string; rpg_snapshot_json?: string | null }>;

  const snapshots = doomed
    .map((m) =>
      m.rpg_snapshot_json
        ? (parseJson<unknown>(m.rpg_snapshot_json, undefined) as RpgSnapshot)
        : null,
    )
    .filter((s): s is RpgSnapshot => !!s);

  db.transaction(() => {
    // Roll back from the EARLIEST removed turn: restore each character's BASE rpg and
    // the combatant roster to their pre-turn state, and delete every item / journal
    // event those turns created — otherwise a Retry would double-apply them.
    if (snapshots.length) {
      const now = new Date().toISOString();
      const base = snapshots[0];
      const restoreChar = db.prepare(
        "UPDATE characters SET rpg_json = ?, updated_at = ? WHERE id = ? AND chat_id = ?",
      );
      for (const [id, rpg] of Object.entries(base.chars ?? {})) {
        restoreChar.run(JSON.stringify(rpg), now, id, row.chat_id);
      }
      db.prepare("UPDATE chats SET combatants_json = ? WHERE id = ?").run(
        JSON.stringify(base.combatants ?? []),
        row.chat_id,
      );
      const delItem = db.prepare("DELETE FROM items WHERE id = ? AND chat_id = ?");
      for (const id of snapshots.flatMap((s) => s.itemIds ?? [])) delItem.run(id, row.chat_id);
      const delEvent = db.prepare("DELETE FROM events WHERE id = ? AND chat_id = ?");
      for (const id of snapshots.flatMap((s) => s.eventIds ?? [])) delEvent.run(id, row.chat_id);
    }

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
        SELECT id, chat_id, name, details, inventory, skills, spells, portrait_json, voice, created_at, updated_at
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
        SELECT id, chat_id, name, details, inventory, skills, spells, portrait_json, voice, created_at, updated_at
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
  input: {
    name: string;
    details?: string;
    inventory?: string;
    skills?: string;
    spells?: string;
    portrait?: Attachment;
    voice?: string;
  },
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
        INSERT INTO characters (
          id,
          chat_id,
          name,
          details,
          inventory,
          skills,
          spells,
          portrait_json,
          voice,
          created_at,
          updated_at
        )
        VALUES (
          @id,
          @chatId,
          @name,
          @details,
          @inventory,
          @skills,
          @spells,
          @portraitJson,
          @voice,
          @createdAt,
          @updatedAt
        )
      `,
    ).run({
      id,
      chatId,
      name,
      details: input.details?.trim() || "",
      inventory: input.inventory?.trim() || "",
      skills: input.skills?.trim() || "",
      spells: input.spells?.trim() || "",
      portraitJson: input.portrait ? JSON.stringify(input.portrait) : null,
      voice: input.voice?.trim() || null,
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
  updates: {
    name?: string;
    details?: string;
    inventory?: string;
    skills?: string;
    spells?: string;
    portrait?: Attachment | null;
    voice?: string | null;
  },
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
  const nextInventory =
    updates.inventory !== undefined ? updates.inventory.trim() : existing.inventory;
  const nextSkills = updates.skills !== undefined ? updates.skills.trim() : existing.skills;
  const nextSpells = updates.spells !== undefined ? updates.spells.trim() : existing.spells;
  const nextPortrait =
    updates.portrait !== undefined ? updates.portrait || undefined : existing.portrait;
  // undefined = leave unchanged; "" or null = clear back to the single voice.
  const nextVoice =
    updates.voice !== undefined ? updates.voice?.trim() || undefined : existing.voice;
  const now = new Date().toISOString();
  const db = getDatabase();

  const update = db.transaction(() => {
    db
      .prepare(
        `
          UPDATE characters
          SET
            name = @name,
            details = @details,
            inventory = @inventory,
            skills = @skills,
            spells = @spells,
            portrait_json = @portraitJson,
            voice = @voice,
            updated_at = @updatedAt
          WHERE id = @id AND chat_id = @chatId
        `,
      )
      .run({
        id: characterId,
        chatId,
        name: nextName,
        details: nextDetails,
        inventory: nextInventory,
        skills: nextSkills,
        spells: nextSpells,
        portraitJson: nextPortrait ? JSON.stringify(nextPortrait) : null,
        voice: nextVoice ?? null,
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

// ---------------------------------------------------------------------------
// img2img entity reuse: an evolving per-character reference image.
// ---------------------------------------------------------------------------

// The protagonist of a chat: the first character created (oldest row). The
// roleplay seeds the player character before any NPC, so this is a stable
// "who is the hero" heuristic without coupling to the RPG layer.
export function getHeroCharacter(chatId: string): StoryCharacter | null {
  const row = getDatabase()
    .prepare(
      `
        SELECT id, chat_id, name, details, inventory, skills, spells, portrait_json, voice, created_at, updated_at
        FROM characters
        WHERE chat_id = ?
        ORDER BY created_at ASC, rowid ASC
        LIMIT 1
      `,
    )
    .get(chatId) as CharacterRow | undefined;

  return row ? mapCharacter(row) : null;
}

// The character's current evolving reference image, if one has been captured.
// Falls back to the static portrait so a brand-new character still has a seed.
export function getCharacterReference(
  chatId: string,
  characterId: string,
): Attachment | null {
  const row = getDatabase()
    .prepare("SELECT reference_json, portrait_json FROM characters WHERE id = ? AND chat_id = ?")
    .get(characterId, chatId) as
    | { reference_json?: string | null; portrait_json?: string | null }
    | undefined;
  if (!row) {
    return null;
  }
  return (
    parseJson<Attachment | null>(row.reference_json, null) ||
    parseJson<Attachment | null>(row.portrait_json, null)
  );
}

// Refresh the evolving reference to the latest illustrated scene of this
// character. Stored separately from the portrait so clearing the portrait
// (or never setting one) doesn't wipe the learned look, and vice-versa.
export function setCharacterReference(
  chatId: string,
  characterId: string,
  reference: Attachment,
): boolean {
  const db = getDatabase();
  const result = db
    .prepare(
      "UPDATE characters SET reference_json = ?, updated_at = ? WHERE id = ? AND chat_id = ?",
    )
    .run(JSON.stringify(reference), new Date().toISOString(), characterId, chatId);
  return result.changes > 0;
}

// ---------------------------------------------------------------------------
// RPG layer: per-character stats/HP, chat game state, and the adventure log.
// ---------------------------------------------------------------------------

export function getRpgState(chatId: string): RpgState {
  const db = getDatabase();
  const row = db.prepare("SELECT rpg_state_json FROM chats WHERE id = ?").get(chatId) as
    | { rpg_state_json?: string }
    | undefined;
  if (row?.rpg_state_json) {
    try {
      return { ...DEFAULT_RPG_STATE, ...(JSON.parse(row.rpg_state_json) as Partial<RpgState>) };
    } catch {
      // fall through to default
    }
  }
  return { ...DEFAULT_RPG_STATE };
}

export function setRpgState(chatId: string, state: RpgState) {
  const db = getDatabase();
  db.prepare("UPDATE chats SET rpg_state_json = ?, updated_at = ? WHERE id = ?").run(
    JSON.stringify(state),
    new Date().toISOString(),
    chatId,
  );
}

// Current encounter foes, stored as a JSON blob on the chat row. Defeated enemies
// are dropped by the resolver before persisting, so this stays small.
export function getCombatants(chatId: string): Enemy[] {
  const db = getDatabase();
  const row = db.prepare("SELECT combatants_json FROM chats WHERE id = ?").get(chatId) as
    | { combatants_json?: string }
    | undefined;
  if (!row?.combatants_json) return [];
  const parsed = parseJson<unknown>(row.combatants_json, undefined);
  if (!Array.isArray(parsed)) return [];
  return parsed
    .map((value) => {
      const enemy = value as Partial<Enemy>;
      if (!enemy || typeof enemy.id !== "string" || typeof enemy.name !== "string") return null;
      return { id: enemy.id, name: enemy.name, rpg: coerceCharacterRpg(enemy.rpg) } as Enemy;
    })
    .filter((enemy): enemy is Enemy => enemy !== null);
}

export function setCombatants(chatId: string, enemies: Enemy[]) {
  const db = getDatabase();
  db.prepare("UPDATE chats SET combatants_json = ?, updated_at = ? WHERE id = ?").run(
    JSON.stringify(enemies),
    new Date().toISOString(),
    chatId,
  );
}

// Map of characterId -> { name, rpg } for the resolver. The rpg is DERIVED:
// base stats (from rpg_json, defaulted via coerceCharacterRpg) with each
// character's equipped-item modifiers folded in, so rolls / AC / damage use the
// same effective numbers the character sheet shows. The stored base is never
// mutated here — resolveRpgTurn persists only the HP/dead delta back onto it.
export function getCharacterRpgMap(
  chatId: string,
): Map<string, { name: string; rpg: CharacterRpg }> {
  const db = getDatabase();
  // created_at ASC keeps the FIRST map entry the oldest character (the protagonist),
  // matching getHeroCharacter and the firstActorId fallback in the resolver.
  const rows = db
    .prepare("SELECT id, name, rpg_json FROM characters WHERE chat_id = ? ORDER BY created_at ASC, rowid ASC")
    .all(chatId) as Array<{ id: string; name: string; rpg_json?: string }>;
  const items = listItems(chatId);
  const map = new Map<string, { name: string; rpg: CharacterRpg }>();
  rows.forEach((row, index) => {
    let parsed: unknown;
    if (row.rpg_json) {
      try {
        parsed = JSON.parse(row.rpg_json);
      } catch {
        parsed = undefined;
      }
    }
    // The first row is the protagonist (created_at ASC); only they absorb legacy
    // items that predate per-owner tracking (ownerId undefined).
    map.set(row.id, {
      name: row.name,
      rpg: deriveForOwner(coerceCharacterRpg(parsed), items, row.id, index === 0).rpg,
    });
  });
  return map;
}

export function saveCharacterRpg(characterId: string, rpg: CharacterRpg) {
  const db = getDatabase();
  db.prepare("UPDATE characters SET rpg_json = ?, updated_at = ? WHERE id = ?").run(
    JSON.stringify(rpg),
    new Date().toISOString(),
    characterId,
  );
}

export function addEvents(chatId: string, events: GameEvent[]) {
  if (!events.length) return;
  const db = getDatabase();
  const insert = db.prepare(
    "INSERT INTO events (id, chat_id, kind, text, data_json, created_at) VALUES (?, ?, ?, ?, ?, ?)",
  );
  const run = db.transaction(() => {
    for (const event of events) {
      insert.run(
        event.id,
        chatId,
        event.kind,
        event.text,
        event.data === undefined ? null : JSON.stringify(event.data),
        event.createdAt,
      );
    }
  });
  run();
}

export function listEvents(chatId: string, limit = 200): GameEvent[] {
  const db = getDatabase();
  const rows = db
    .prepare(
      // rowid tiebreaker: a combat burst stamps roll/hp/death with the same ms, so
      // without it the reloaded journal can show a death before the hit that caused it.
      "SELECT id, kind, text, data_json, created_at FROM events WHERE chat_id = ? ORDER BY created_at ASC, rowid ASC LIMIT ?",
    )
    .all(chatId, limit) as Array<{
    id: string;
    kind: string;
    text: string;
    data_json?: string;
    created_at: string;
  }>;
  return rows.map((row) => ({
    id: row.id,
    kind: row.kind as GameEvent["kind"],
    text: row.text,
    data: row.data_json ? parseJson<unknown>(row.data_json, undefined) : undefined,
    createdAt: row.created_at,
  }));
}

export function getCharacterRpg(chatId: string, characterId: string): CharacterRpg | null {
  const db = getDatabase();
  const row = db
    .prepare("SELECT rpg_json FROM characters WHERE id = ? AND chat_id = ?")
    .get(characterId, chatId) as { rpg_json?: string } | undefined;
  if (!row) return null;
  return coerceCharacterRpg(row.rpg_json ? parseJson<unknown>(row.rpg_json, undefined) : undefined);
}

export function setItemEquipped(chatId: string, itemId: string, equipped: boolean): Item | null {
  const db = getDatabase();
  const targetRow = db
    .prepare("SELECT data_json FROM items WHERE id = ? AND chat_id = ?")
    .get(itemId, chatId) as { data_json: string } | undefined;
  if (!targetRow) return null;
  const item = parseJson<unknown>(targetRow.data_json, undefined) as Item | undefined;
  if (!item) return null;
  item.equipped = equipped;
  db.transaction(() => {
    // One item per slot: equipping a piece unequips any other equipped item that
    // occupies the same slot for the same owner (it stays in inventory), so a
    // player can't stack three rings or two blades and multiply the bonuses.
    // slot/ownerId live inside data_json (not SQL-queryable), so the sibling scan
    // only loads the rest of the inventory on the equip path that needs it.
    if (equipped) {
      const rows = db
        .prepare("SELECT id, data_json FROM items WHERE chat_id = ?")
        .all(chatId) as Array<{ id: string; data_json: string }>;
      const update = db.prepare("UPDATE items SET data_json = ? WHERE id = ? AND chat_id = ?");
      for (const row of rows) {
        if (row.id === itemId) continue;
        const other = parseJson<unknown>(row.data_json, undefined) as Item | undefined;
        if (!other || !other.equipped || other.slot !== item.slot) continue;
        if (other.ownerId !== item.ownerId) continue;
        other.equipped = false;
        update.run(JSON.stringify(other), row.id, chatId);
      }
    }
    db.prepare("UPDATE items SET data_json = ? WHERE id = ? AND chat_id = ?").run(
      JSON.stringify(item),
      itemId,
      chatId,
    );
    db.prepare("UPDATE chats SET updated_at = ? WHERE id = ?").run(new Date().toISOString(), chatId);
  })();
  return item;
}

// Persist a generated portrait onto a named item so recurring mentions of it
// can be illustrated consistently via image2image. Matches by id when known,
// otherwise by the most recent item with that name (case-insensitive) that has
// no image yet — the drop the narrator just illustrated.
export function setItemImage(
  chatId: string,
  match: { id?: string; name?: string },
  imageUrl: string,
  options: { overwrite?: boolean } = {},
): Item | null {
  const db = getDatabase();
  const rows = db
    .prepare("SELECT id, data_json FROM items WHERE chat_id = ? ORDER BY created_at DESC, rowid DESC")
    .all(chatId) as Array<{ id: string; data_json: string }>;

  let targetId: string | null = null;
  if (match.id) {
    targetId = rows.some((row) => row.id === match.id) ? match.id : null;
  } else if (match.name) {
    const wanted = match.name.trim().toLowerCase();
    // Target the most recent same-named item that has no portrait yet — the drop
    // just illustrated. With overwrite, fall back to any match (re-illustrate).
    let fallbackId: string | null = null;
    for (const row of rows) {
      const item = parseJson<unknown>(row.data_json, undefined) as Item | undefined;
      if (!item || item.name.trim().toLowerCase() !== wanted) continue;
      if (fallbackId === null) fallbackId = row.id;
      if (!item.imageUrl) {
        targetId = row.id;
        break;
      }
    }
    // Once an item has a canonical portrait we keep it (it becomes the reuse
    // reference); only an explicit overwrite replaces it.
    if (targetId === null && options.overwrite) targetId = fallbackId;
  }

  if (!targetId) return null;

  const row = db
    .prepare("SELECT data_json FROM items WHERE id = ? AND chat_id = ?")
    .get(targetId, chatId) as { data_json: string } | undefined;
  if (!row) return null;
  const item = parseJson<unknown>(row.data_json, undefined) as Item | undefined;
  if (!item) return null;

  item.imageUrl = imageUrl;
  db.transaction(() => {
    db.prepare("UPDATE items SET data_json = ? WHERE id = ? AND chat_id = ?").run(
      JSON.stringify(item),
      targetId,
      chatId,
    );
    db.prepare("UPDATE chats SET updated_at = ? WHERE id = ?").run(new Date().toISOString(), chatId);
  })();
  return item;
}

export function addItems(chatId: string, items: Item[]) {
  if (!items.length) return;
  const db = getDatabase();
  const insert = db.prepare(
    "INSERT INTO items (id, chat_id, data_json, created_at) VALUES (?, ?, ?, ?)",
  );
  const run = db.transaction(() => {
    for (const item of items) {
      insert.run(item.id, chatId, JSON.stringify(item), item.createdAt);
    }
  });
  run();
}

export function listItems(chatId: string): Item[] {
  const db = getDatabase();
  const rows = db
    .prepare("SELECT data_json FROM items WHERE chat_id = ? ORDER BY created_at ASC")
    .all(chatId) as Array<{ data_json: string }>;
  return rows
    .map((row) => parseJson<unknown>(row.data_json, undefined) as Item | undefined)
    .filter((item): item is Item => Boolean(item));
}

export function getItem(chatId: string, itemId: string): Item | null {
  const db = getDatabase();
  const row = db
    .prepare("SELECT data_json FROM items WHERE chat_id = ? AND id = ?")
    .get(chatId, itemId) as { data_json: string } | undefined;
  if (!row) {
    return null;
  }
  return (parseJson<unknown>(row.data_json, undefined) as Item | undefined) ?? null;
}

// ---------------------------------------------------------------------------
// Scene continuity (img2img): per-location anchor / last image, so a scene can
// EVOLVE across turns (a meadow, then the same meadow with goblins) instead of
// being redrawn from scratch. One active location per chat (chats.current_scene);
// each location keeps its establishing "anchor" image, the most recent "last"
// image, and an edit-hop counter so the engine edits from the prior image and
// re-anchors after a few hops to bound iterative drift.
// ---------------------------------------------------------------------------

export type SceneState = {
  location: string;
  anchor: Attachment | null;
  last: Attachment | null;
  hops: number;
};

type SceneRow = {
  location: string;
  anchor_json: string | null;
  last_json: string | null;
  hops: number;
};

function mapSceneRow(row: SceneRow): SceneState {
  return {
    location: row.location,
    anchor: parseJson<Attachment | null>(row.anchor_json, null),
    last: parseJson<Attachment | null>(row.last_json, null),
    hops: Number(row.hops || 0),
  };
}

// Normalize a narrator-supplied location label into a stable key: lowercased,
// trimmed, whitespace-collapsed, a leading article and surrounding punctuation
// stripped, capped — so "The Green Meadow." and "green meadow" map together.
export function normalizeLocation(label: string | undefined | null): string {
  if (!label) {
    return "";
  }
  return label
    .toLowerCase()
    .replace(/[«»"'`.,;:!?()[\]]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^(the|a|an|le|la|les|el|los|der|die|das)\s+/u, "")
    .trim()
    .slice(0, 80);
}

export function getScene(chatId: string, location: string): SceneState | null {
  const key = normalizeLocation(location);
  if (!key) {
    return null;
  }
  const row = getDatabase()
    .prepare(
      "SELECT location, anchor_json, last_json, hops FROM scenes WHERE chat_id = ? AND location = ?",
    )
    .get(chatId, key) as SceneRow | undefined;
  return row ? mapSceneRow(row) : null;
}

export function getActiveScene(chatId: string): SceneState | null {
  const db = getDatabase();
  const chat = db.prepare("SELECT current_scene FROM chats WHERE id = ?").get(chatId) as
    | { current_scene?: string }
    | undefined;
  const key = chat?.current_scene;
  if (!key) {
    return null;
  }
  const row = db
    .prepare(
      "SELECT location, anchor_json, last_json, hops FROM scenes WHERE chat_id = ? AND location = ?",
    )
    .get(chatId, key) as SceneRow | undefined;
  return row ? mapSceneRow(row) : null;
}

// Persist the image just rendered for a location and make it the chat's active
// scene. `anchor: true` (re)establishes the location — the image becomes both
// anchor and last and the hop counter resets; `anchor: false` records an edit,
// advancing only `last` and the hop counter while keeping the original
// establishing anchor for re-anchoring later.
export function recordSceneImage(
  chatId: string,
  location: string,
  image: Attachment,
  options: { anchor: boolean },
): void {
  const key = normalizeLocation(location);
  if (!key) {
    return;
  }
  const db = getDatabase();
  const now = new Date().toISOString();
  const imageJson = JSON.stringify(image);
  const run = db.transaction(() => {
    const existing = db
      .prepare("SELECT hops FROM scenes WHERE chat_id = ? AND location = ?")
      .get(chatId, key) as { hops?: number } | undefined;
    if (!existing || options.anchor) {
      db.prepare(
        "INSERT INTO scenes (chat_id, location, anchor_json, last_json, hops, updated_at) VALUES (?, ?, ?, ?, 0, ?) " +
          "ON CONFLICT(chat_id, location) DO UPDATE SET anchor_json = excluded.anchor_json, last_json = excluded.last_json, hops = 0, updated_at = excluded.updated_at",
      ).run(chatId, key, imageJson, imageJson, now);
    } else {
      db.prepare(
        "UPDATE scenes SET last_json = ?, hops = hops + 1, updated_at = ? WHERE chat_id = ? AND location = ?",
      ).run(imageJson, now, chatId, key);
    }
    db.prepare("UPDATE chats SET current_scene = ?, updated_at = ? WHERE id = ?").run(
      key,
      now,
      chatId,
    );
  });
  run();
}
