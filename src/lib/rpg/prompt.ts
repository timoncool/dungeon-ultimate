import { ABILITIES, ABILITY_LABELS_RU, abilityMod } from "./dice";
import type { CharacterRpg, Item } from "./types";

const RPG_RULES = `ПРАВИЛА МЕХАНИКИ (D&D-режим — соблюдай строго):
— Кубик кидает ДВИЖОК, не ты. Ты НЕ называешь в тексте результат броска, итог, успех/провал или новое число HP — опиши само действие, движок вернёт исход.
— Когда срабатывает механика, добавь В САМОМ КОНЦЕ ответа служебный блок (игроку он не показывается):
[[GAME:{"rolls":[{"ability":"dex","dc":14,"label":"Прыжок через пропасть","actorId":"ID_героя"}],"hpDelta":[{"characterId":"ID_героя","amount":-6,"reason":"падение"}]}]]
— rolls: ability (str/dex/con/int/wis/cha), dc (5 легко … 15 средне … 20 очень трудно), label, actorId. hpDelta: characterId, amount (минус — урон, плюс — лечение), reason. Смерть при HP ≤ 0 движок объявит сам.
— Лут/находка → grantItems: { name, slot (weapon/armor/shield/trinket/consumable/misc), rarity (common/uncommon/rare/epic/legendary), damage (напр. «1d8»), description, modifiers, withImage:true для иллюстрации }. Создавай предметы по правилам мира.
— Ссылайся на персонажей ТОЛЬКО по точному ID из «СОСТОЯНИЕ ИГРЫ» выше. Все поля блока опциональны.
— В чисто повествовательных ходах блок [[GAME]] НЕ добавляй.`;

// The authoritative game-state block mirrored into the system prompt each turn,
// plus the rules. The narrator reads stats/HP from here and never invents them.
export function buildRpgSection(
  actors: Map<string, { name: string; rpg: CharacterRpg }>,
  items: Item[] = [],
): string {
  if (!actors.size) {
    return RPG_RULES;
  }
  const lines: string[] = [];
  for (const [id, { name, rpg }] of actors) {
    const stats = ABILITIES.map((ability) => {
      const mod = abilityMod(rpg.stats[ability]);
      return `${ABILITY_LABELS_RU[ability]} ${rpg.stats[ability]} (${mod >= 0 ? "+" : ""}${mod})`;
    }).join(", ");
    const head = `• ${name} [ID: ${id}] — HP ${Math.max(0, rpg.hp.current)}/${rpg.hp.max}, КЗ ${rpg.ac}, ур.${rpg.level}${rpg.dead ? " — МЁРТВ" : ""}`;
    const conditions = rpg.conditions.length ? `\n  Состояния: ${rpg.conditions.join(", ")}` : "";
    lines.push(`${head}\n  ${stats}${conditions}`);
  }
  const inventory = items.length
    ? `\n\nИНВЕНТАРЬ:\n${items
        .map(
          (item) =>
            `• ${item.name}${item.qty > 1 ? ` ×${item.qty}` : ""} (${item.slot}${item.damage ? `, урон ${item.damage}` : ""})${item.equipped ? " [надет]" : ""}`,
        )
        .join("\n")}`
    : "";
  return `СОСТОЯНИЕ ИГРЫ (authoritative — опирайся на него, НЕ выдумывай числа):\n${lines.join("\n")}${inventory}\n\n${RPG_RULES}`;
}
