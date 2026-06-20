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
//
// At most ONE equipped item per slot is counted (newest by createdAt wins), which
// makes the "one item per slot" rule hold at the point of truth — not only inside
// setItemEquipped — so legacy/imported rows can never stack two rings' bonuses.
export function deriveRpg(base: CharacterRpg, items: Item[]): DerivedRpg {
  const bySlot = new Map<string, Item>();
  for (const item of items) {
    if (!item.equipped) continue;
    const prev = bySlot.get(item.slot);
    if (!prev || (item.createdAt ?? "") > (prev.createdAt ?? "")) bySlot.set(item.slot, item);
  }

  const bonus: Record<string, number> = {};
  for (const item of bySlot.values()) {
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

// Derive a character's stats using ONLY the items they own. The combat resolver
// and the character sheet MUST both go through this with the same ownerId so the
// displayed numbers always match what the engine rolls against.
export function deriveForOwner(
  base: CharacterRpg,
  items: Item[],
  ownerId: string | undefined,
): DerivedRpg {
  return deriveRpg(base, items.filter((item) => item.ownerId === ownerId));
}
