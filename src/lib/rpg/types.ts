import type { Ability } from "./dice";

export type CharacterStats = Record<Ability, number>;

// Per-character RPG state, stored as JSON on the character row (additive — the
// existing free-text inventory/skills/spells stay as prose flavour).
export type CharacterRpg = {
  stats: CharacterStats;
  hp: { current: number; max: number };
  ac: number;
  level: number;
  xp: number;
  conditions: string[];
  dead: boolean;
};

export const DEFAULT_RPG: CharacterRpg = {
  stats: { str: 10, dex: 10, con: 10, int: 10, wis: 10, cha: 10 },
  hp: { current: 20, max: 20 },
  ac: 10,
  level: 1,
  xp: 0,
  conditions: [],
  dead: false,
};

// One adventure-log entry. The journal is both player recap and the audit trail
// of what the engine resolved from the narrator's declarations.
export type GameEvent = {
  id: string;
  kind: "roll" | "hp" | "death" | "item" | "combat" | "note";
  text: string; // Russian, player-facing
  data?: unknown; // raw resolution detail
  createdAt: string;
};

export type ItemSlot = "weapon" | "armor" | "shield" | "trinket" | "consumable" | "misc";
export type ItemRarity = "common" | "uncommon" | "rare" | "epic" | "legendary";

// An inventory item. `image` (a generated portrait of the item) is filled in by
// the image pipeline when the drop is illustrated, then reused via image2image.
export type Item = {
  id: string;
  ownerId?: string;
  name: string;
  slot: ItemSlot;
  rarity: ItemRarity;
  description?: string;
  damage?: string; // dice notation, e.g. "1d8+1"
  modifiers?: Partial<Record<Ability | "ac" | "maxHp", number>>;
  equipped: boolean;
  qty: number;
  imageUrl?: string;
  imagePromptEn?: string; // optional English visual prompt for the item icon
  createdAt: string;
};

// The narrator's mechanical declaration for a turn (the `[[GAME]]` block). v1
// resolves rolls + hpDelta + note + item drops; later phases add combat/xp.
// A combat opponent — mechanically identical to a character's RPG block, but
// stored per-chat as transient encounter state rather than on a character row.
export type Enemy = {
  id: string;
  name: string;
  rpg: CharacterRpg;
};

export type GameUpdate = {
  rolls?: Array<{
    ability: Ability;
    dc: number;
    label?: string;
    actorId?: string;
    kind?: "skill" | "attack" | "save";
    targetId?: string;
  }>;
  // Bring foes onto the field. The engine assigns ids; later attacks target them.
  spawnEnemies?: Array<{
    name: string;
    hp?: number;
    ac?: number;
    level?: number;
    stats?: Partial<CharacterStats>;
  }>;
  // An attack: roll d20 + attacker's ability mod vs target AC; on hit roll damage.
  attacks?: Array<{
    attackerId?: string;
    targetId: string;
    ability?: Ability;
    damage?: string; // dice notation, e.g. "1d8+2"
    label?: string;
  }>;
  hpDelta?: Array<{ characterId: string; amount: number; reason?: string }>;
  grantItems?: Array<{
    ownerId?: string;
    name: string;
    slot?: ItemSlot;
    rarity?: ItemRarity;
    description?: string;
    damage?: string;
    modifiers?: Partial<Record<Ability | "ac" | "maxHp", number>>;
    qty?: number;
    withImage?: boolean;
    imagePromptEn?: string;
  }>;
  note?: string;
};

// Chat-level RPG state, stored as JSON on the chat row.
export type RpgState = {
  enabled: boolean;
};

export const DEFAULT_RPG_STATE: RpgState = { enabled: false };

export function coerceCharacterRpg(value: unknown): CharacterRpg {
  if (!value || typeof value !== "object") return structuredClone(DEFAULT_RPG);
  const raw = value as Partial<CharacterRpg>;
  const base = structuredClone(DEFAULT_RPG);
  if (raw.stats && typeof raw.stats === "object") {
    for (const key of Object.keys(base.stats) as Ability[]) {
      const v = (raw.stats as Record<string, unknown>)[key];
      if (typeof v === "number" && Number.isFinite(v)) base.stats[key] = Math.round(v);
    }
  }
  if (raw.hp && typeof raw.hp === "object") {
    const hp = raw.hp as { current?: number; max?: number };
    if (typeof hp.max === "number") base.hp.max = Math.max(1, Math.round(hp.max));
    if (typeof hp.current === "number") base.hp.current = Math.round(hp.current);
  }
  if (typeof raw.ac === "number") base.ac = Math.round(raw.ac);
  if (typeof raw.level === "number") base.level = Math.max(1, Math.round(raw.level));
  if (typeof raw.xp === "number") base.xp = Math.max(0, Math.round(raw.xp));
  if (Array.isArray(raw.conditions)) base.conditions = raw.conditions.filter((c) => typeof c === "string");
  base.dead = Boolean(raw.dead) || base.hp.current <= 0;
  return base;
}
