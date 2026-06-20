import { ABILITIES, ABILITY_LABELS_RU, abilityMod } from "./dice";
import type { CharacterRpg, Enemy, Item } from "./types";

const RPG_RULES = `ПРАВИЛА МЕХАНИКИ (D&D-режим — соблюдай СТРОГО):

— Кубик кидает ДВИЖОК, не ты. НИКОГДА не пиши в тексте число кубика, итог броска, слова «успех/провал», величину урона или новое значение HP. Опиши только САМО действие и обстановку — движок вернёт исход следующим ходом.

— КОГДА НУЖЕН БРОСОК: каждый раз, когда исход действия игрока НЕ предрешён (атака, уклонение, взлом, убеждение, скрытность, прыжок, поиск ловушек, спасбросок и т.п.), ты ОБЯЗАН в САМОМ КОНЦЕ ответа добавить служебный блок [[GAME:{...}]] — игрок его не видит. Если действие тривиально (идёт, говорит, осматривается без риска) — блок НЕ добавляй.

ФОРМАТ — строгий JSON в одну строку в самом конце ответа:
[[GAME:{"rolls":[{"ability":"dex","dc":14,"label":"Прыжок через пропасть","actorId":"ID_ИГРОКА"}]}]]

Поля блока (все опциональны, добавляй только нужные):
— rolls: проверка. ability=str|dex|con|int|wis|cha; dc=5 (легко) … 15 (средне) … 20 (очень трудно); label кратко по-русски; actorId — ТОЧНЫЙ ID из «СОСТОЯНИЕ ИГРЫ».
— hpDelta: [{"characterId":"ID","amount":-6,"reason":"падение"}] — урон (минус) или лечение (плюс) ВНЕ боя.
— grantItems: [{"name":"Меч","slot":"weapon|armor|shield|trinket|consumable|misc","rarity":"common|uncommon|rare|epic|legendary","damage":"1d8","description":"...","withImage":true,"imagePromptEn":"glowing ice-blue longsword wreathed in mist, leather-wrapped hilt"}] — лут по правилам мира. imagePromptEn — короткое ВИЗУАЛЬНОЕ описание предмета по-АНГЛИЙСКИ (только если withImage:true), для отрисовки иконки; без имён собственных и текста на предмете.

БОЁВКА:
— spawnEnemies: в начале боя ОБЪЯВИ врагов — [{"name":"Гоблин","hp":12,"ac":13,"level":1,"stats":{"str":12,"dex":14}}]. Движок выдаст им ID и покажет в разделе «ПРОТИВНИКИ».
— attacks: атака по цели — [{"attackerId":"ID_атакующего","targetId":"ID_цели","ability":"str","damage":"1d8+2","label":"Удар мечом"}]. Движок кинет d20+модификатор против КЗ цели и при попадании посчитает урон. Действуй И за игрока (по врагу), И за врагов: в каждый ход боя живые враги атакуют через attacks, где attackerId — их ID, а targetId — ID игрока.
— Ссылайся на бойцов ТОЛЬКО по точным ID из блоков состояния. Смерть при HP ≤ 0 движок объявит сам — не убивай словами заранее.

ЭФФЕКТЫ (баффы/дебаффы, благословения, проклятья, яды):
— applyEffects: повесь временный эффект — [{"characterId":"ID","name":"Благословение силы","kind":"buff","modifiers":{"str":2},"turns":3,"note":"..."}]. kind=buff|debuff; modifiers меняют статы (str/dex/con/int/wis/cha/ac/maxHp); turns — на сколько ХОДОВ. Движок сам уменьшает срок и снимает эффект. Вешай при заклинаниях, ловушках, зельях, алтарях, ядах, ранениях. НЕ пиши числа статов в тексте — только опиши ощущение.
— clearEffects: снять эффект — [{"characterId":"ID","name":"Проклятье слабости"}] (или name:"*" — снять все). Используй при лечении и снятии проклятий.

ПРИМЕР. Текст: «Ты разбегаешься и в прыжке тянешься к дальнему краю расщелины…»
В самом конце: [[GAME:{"rolls":[{"ability":"dex","dc":13,"label":"Прыжок","actorId":"ID_ИГРОКА"}]}]]`;

// The authoritative game-state block mirrored into the system prompt each turn:
// player characters, inventory, and any active enemies, followed by the rules.
// The narrator reads stats/HP/ids from here and never invents them.
export function buildRpgSection(
  actors: Map<string, { name: string; rpg: CharacterRpg }>,
  items: Item[] = [],
  enemies: Enemy[] = [],
): string {
  if (!actors.size && !enemies.length) {
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
    const effects = rpg.effects.length
      ? `\n  Эффекты: ${rpg.effects.map((e) => `${e.name} (${e.turns})`).join(", ")}`
      : "";
    lines.push(`${head}\n  ${stats}${conditions}${effects}`);
  }
  const inventory = items.length
    ? `\n\nИНВЕНТАРЬ:\n${items
        .map(
          (item) =>
            `• ${item.name}${item.qty > 1 ? ` ×${item.qty}` : ""} (${item.slot}${item.damage ? `, урон ${item.damage}` : ""})${item.equipped ? " [надет]" : ""}`,
        )
        .join("\n")}`
    : "";
  const foes = enemies.length
    ? `\n\nПРОТИВНИКИ (атакуй их через attacks, цель — их ID):\n${enemies
        .map(
          (enemy) =>
            `• ${enemy.name} [ID: ${enemy.id}] — HP ${Math.max(0, enemy.rpg.hp.current)}/${enemy.rpg.hp.max}, КЗ ${enemy.rpg.ac}${enemy.rpg.dead ? " — МЁРТВ" : ""}`,
        )
        .join("\n")}`
    : "";
  const head = actors.size
    ? lines.join("\n")
    : "• (нет игровых персонажей)";
  return `СОСТОЯНИЕ ИГРЫ (authoritative — опирайся на него, НЕ выдумывай числа):\n${head}${inventory}${foes}\n\n${RPG_RULES}`;
}
