import { z } from "zod";
import type { GameUpdate } from "./types";

const ability = z.enum(["str", "dex", "con", "int", "wis", "cha"]);

const gameUpdateSchema = z
  .object({
    rolls: z
      .array(
        z.object({
          ability,
          dc: z.number(),
          label: z.string().optional(),
          actorId: z.string().optional(),
          kind: z.enum(["skill", "attack", "save"]).optional(),
          targetId: z.string().optional(),
        }),
      )
      .optional(),
    hpDelta: z
      .array(
        z.object({ characterId: z.string(), amount: z.number(), reason: z.string().optional() }),
      )
      .optional(),
    grantItems: z
      .array(
        z.object({
          ownerId: z.string().optional(),
          name: z.string(),
          slot: z
            .enum(["weapon", "armor", "shield", "trinket", "consumable", "misc"])
            .optional(),
          rarity: z.enum(["common", "uncommon", "rare", "epic", "legendary"]).optional(),
          description: z.string().optional(),
          damage: z.string().optional(),
          modifiers: z.record(z.string(), z.number()).optional(),
          qty: z.number().optional(),
          withImage: z.boolean().optional(),
          imagePromptEn: z.string().optional(),
        }),
      )
      .optional(),
    // Combat: foes entering the field. The engine assigns ids; attacks target them.
    // stats uses a LOOSE string-key record (like modifiers above) — an enum-keyed
    // record would make all six abilities required in Zod and reject partial stats,
    // failing the whole parse. applyGameUpdate iterates ABILITIES and type-checks.
    spawnEnemies: z
      .array(
        z.object({
          name: z.string(),
          hp: z.number().optional(),
          ac: z.number().optional(),
          level: z.number().optional(),
          stats: z.record(z.string(), z.number()).optional(),
        }),
      )
      .optional(),
    attacks: z
      .array(
        z.object({
          attackerId: z.string().optional(),
          targetId: z.string(),
          ability: ability.optional(),
          damage: z.string().optional(),
          label: z.string().optional(),
        }),
      )
      .optional(),
    // Temporary buffs/debuffs (blessing, curse, poison …). Loose modifiers record
    // for the same reason as grantItems above.
    applyEffects: z
      .array(
        z.object({
          characterId: z.string().optional(),
          name: z.string(),
          kind: z.enum(["buff", "debuff"]).optional(),
          modifiers: z.record(z.string(), z.number()).optional(),
          turns: z.number().optional(),
          note: z.string().optional(),
        }),
      )
      .optional(),
    clearEffects: z
      .array(z.object({ characterId: z.string().optional(), name: z.string() }))
      .optional(),
    note: z.string().optional(),
  })
  .strip();

// The narrator appends a [[GAME:{...}]] block when mechanics fire. Pull it out of
// the passage, validate it, and return the cleaned prose + the parsed update.
const GAME_BLOCK = /\[\[GAME:\s*(\{[\s\S]*?\})\s*\]\]/i;

export function extractGameUpdate(text: string): { clean: string; update: GameUpdate | null } {
  const match = GAME_BLOCK.exec(text);
  if (!match) {
    return { clean: text, update: null };
  }
  const clean = text.replace(GAME_BLOCK, "").trim();
  let parsedJson: unknown;
  try {
    parsedJson = JSON.parse(match[1]);
  } catch {
    return { clean, update: null };
  }
  const result = gameUpdateSchema.safeParse(parsedJson);
  return { clean, update: result.success ? (result.data as GameUpdate) : null };
}
