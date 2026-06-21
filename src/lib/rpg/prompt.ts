import type { Language } from "@/lib/types";
import { promptsFor } from "@/lib/prompts";
import { ABILITIES, ABILITY_LABELS_RU, abilityMod } from "./dice";
import type { CharacterRpg, Enemy, Item } from "./types";

// The authoritative game-state block mirrored into the system prompt each turn:
// player characters, inventory, and any active enemies, followed by the rules.
// The narrator reads stats/HP/ids from here and never invents them.
export function buildRpgSection(
  actors: Map<string, { name: string; rpg: CharacterRpg }>,
  items: Item[] = [],
  enemies: Enemy[] = [],
  language: Language = "ru",
): string {
  const p = promptsFor(language);
  const rules = p.rpg.rules;
  if (!actors.size && !enemies.length) {
    return rules;
  }
  const lines: string[] = [];
  for (const [id, { name, rpg }] of actors) {
    const stats = ABILITIES.map((ability) => {
      const mod = abilityMod(rpg.stats[ability]);
      return `${ABILITY_LABELS_RU[ability]} ${rpg.stats[ability]} (${mod >= 0 ? "+" : ""}${mod})`;
    }).join(", ");
    const head = `• ${name} [ID: ${id}] — ${p.rpg.hp} ${Math.max(0, rpg.hp.current)}/${rpg.hp.max}, ${p.rpg.ac} ${rpg.ac}, ${p.rpg.level}${rpg.level}${rpg.dead ? ` — ${p.rpg.dead}` : ""}`;
    const conditions = rpg.conditions.length ? `\n  ${p.rpg.conditions}: ${rpg.conditions.join(", ")}` : "";
    const effects = rpg.effects.length
      ? `\n  ${p.rpg.effects}: ${rpg.effects.map((e) => `${e.name} (${e.turns})`).join(", ")}`
      : "";
    lines.push(`${head}\n  ${stats}${conditions}${effects}`);
  }
  // Only party-owned items belong in ИНВЕНТАРЬ. Items dropped with a non-party
  // ownerId (e.g. an enemy's loot) must NOT be presented as the hero's gear.
  // Keep unowned/legacy loot (falsy ownerId) — it belongs to the scene; only
  // filter OUT items explicitly owned by a non-party actor (enemy/companion).
  const ownedItems = items.filter((item) => !item.ownerId || actors.has(item.ownerId));
  // Attribute each item to its owner only when the party has more than one member,
  // so a companion's gear isn't read as the hero's; solo hero stays unattributed.
  const multiOwner = actors.size > 1;
  const inventory = ownedItems.length
    ? `\n\n${p.rpg.inventory}:\n${ownedItems
        .map((item) => {
          const ownerName = item.ownerId ? actors.get(item.ownerId)?.name : undefined;
          const owner = multiOwner && ownerName ? ` — ${ownerName}` : "";
          return `• ${item.name}${item.qty > 1 ? ` ×${item.qty}` : ""} (${item.slot}${item.damage ? `, ${p.rpg.damage} ${item.damage}` : ""})${item.equipped ? ` [${p.rpg.equipped}]` : ""}${owner}`;
        })
        .join("\n")}`
    : "";
  const foes = enemies.length
    ? `\n\n${p.rpg.foes}:\n${enemies
        .map(
          (enemy) =>
            `• ${enemy.name} [ID: ${enemy.id}] — ${p.rpg.hp} ${Math.max(0, enemy.rpg.hp.current)}/${enemy.rpg.hp.max}, ${p.rpg.ac} ${enemy.rpg.ac}${enemy.rpg.dead ? ` — ${p.rpg.dead}` : ""}`,
        )
        .join("\n")}`
    : "";
  const head = actors.size
    ? lines.join("\n")
    : "• (нет игровых персонажей)";
  return `СОСТОЯНИЕ ИГРЫ (authoritative — опирайся на него, НЕ выдумывай числа):\n${head}${inventory}${foes}\n\n${rules}`;
}
