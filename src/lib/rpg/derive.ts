import { ABILITIES, clampStat } from "./dice";
import type { CharacterRpg, Item } from "./types";

export type DerivedRpg = {
  // Base stats with every equipped item's modifiers folded in. Used by BOTH the
  // combat resolver and the character sheet so the numbers can never disagree.
  rpg: CharacterRpg;
  // Per-key sum of the applied modifiers (str/dex/.../ac/maxHp), for amber tints.
  bonus: Record<string, number>;
};

// Derived stats = base + equipped modifiers. PURE: it clones, never mutates the
// input, and never persists — callers keep the stored CharacterRpg as the
// canonical BASE and re-derive each time, so bonuses can't compound. Current HP
// carries over unchanged but is clamped to the (possibly buffed) derived max, so
// removing a +maxHP item can't leave current above the new cap.
export function deriveRpg(base: CharacterRpg, items: Item[]): DerivedRpg {
  const bonus: Record<string, number> = {};
  for (const item of items) {
    if (!item.equipped) continue;
    for (const [key, value] of Object.entries(item.modifiers ?? {})) {
      if (typeof value === "number" && Number.isFinite(value)) {
        bonus[key] = (bonus[key] ?? 0) + value;
      }
    }
  }

  const rpg = structuredClone(base);
  for (const ability of ABILITIES) {
    if (bonus[ability]) rpg.stats[ability] = clampStat(rpg.stats[ability] + bonus[ability], 1, 30);
  }
  if (bonus.ac) rpg.ac += bonus.ac;
  if (bonus.maxHp) rpg.hp.max = Math.max(1, rpg.hp.max + bonus.maxHp);
  rpg.hp.current = Math.min(rpg.hp.current, rpg.hp.max);

  return { rpg, bonus };
}
