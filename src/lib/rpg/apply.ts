import { randomUUID } from "node:crypto";
import { ABILITIES, ABILITY_LABELS_RU, clampStat, rollCheck, rollDie, rollNotation } from "./dice";
import { coerceEffect, DEFAULT_RPG } from "./types";
import type {
  CharacterRpg,
  Effect,
  EffectModifiers,
  Enemy,
  GameEvent,
  GameUpdate,
  Item,
  ItemRarity,
  ItemSlot,
} from "./types";

const RARITY_RU: Record<string, string> = {
  common: "обычный",
  uncommon: "необычный",
  rare: "редкий",
  epic: "эпический",
  legendary: "легендарный",
};

export type ActorMap = Map<string, { name: string; rpg: CharacterRpg }>;

export type ApplyResult = {
  events: GameEvent[];
  changed: Set<string>; // actor ids (characters or enemies) whose rpg state changed
  items: Item[]; // new items to persist into the inventory
  spawnedEnemies: Enemy[]; // foes brought onto the field this turn
};

function makeEvent(kind: GameEvent["kind"], text: string, data?: unknown): GameEvent {
  return { id: randomUUID(), kind, text, data, createdAt: new Date().toISOString() };
}

function firstActorId(actors: ActorMap): string | undefined {
  for (const id of actors.keys()) return id;
  return undefined;
}

function fmtMod(mod: number): string {
  return `${mod >= 0 ? "+" : ""}${mod}`;
}

// ── Effects (buffs / debuffs / blessings / curses) ──────────────────────────
const EFFECT_CAP = 8; // keep the active-effect list from growing without bound
const RANDOM_EVENT_CHANCE = 15; // percent chance of a random event per resolved turn

const EFFECT_STAT_RU: Record<string, string> = {
  str: "СИЛ", dex: "ЛОВ", con: "ВЫН", int: "ИНТ", wis: "МУД", cha: "ХАР", ac: "КЗ", maxHp: "макс.HP",
};

function pluralTurns(n: number): string {
  const a = Math.abs(n) % 100;
  if (a >= 11 && a <= 14) return "ходов";
  const b = a % 10;
  if (b === 1) return "ход";
  if (b >= 2 && b <= 4) return "хода";
  return "ходов";
}

function effectSummary(name: string, modifiers: EffectModifiers, turns: number): string {
  const mods = Object.entries(modifiers)
    .filter(([, v]) => typeof v === "number" && v !== 0)
    .map(([k, v]) => `${EFFECT_STAT_RU[k] ?? k} ${(v as number) >= 0 ? "+" : ""}${v}`)
    .join(", ");
  return `${name}${mods ? ` (${mods})` : ""} — ${turns} ${pluralTurns(turns)}`;
}

// Add or refresh an effect (same name replaces, resetting duration); cap the list.
function mergeEffect(rpg: CharacterRpg, effect: Effect) {
  rpg.effects = (rpg.effects ?? []).filter(
    (e) => e.name.trim().toLowerCase() !== effect.name.trim().toLowerCase(),
  );
  rpg.effects.push(effect);
  if (rpg.effects.length > EFFECT_CAP) rpg.effects = rpg.effects.slice(-EFFECT_CAP);
}

// Decrement every effect by one turn and drop the expired; returns worn-off names.
function tickEffects(rpg: CharacterRpg): string[] {
  const expired: string[] = [];
  const kept: Effect[] = [];
  for (const e of rpg.effects ?? []) {
    const turns = e.turns - 1;
    if (turns <= 0) expired.push(e.name);
    else kept.push({ ...e, turns });
  }
  rpg.effects = kept;
  return expired;
}

// Curated blessings/curses for the per-turn random event (deterministic crypto RNG).
const RANDOM_EVENTS: Array<{ name: string; kind: Effect["kind"]; modifiers: EffectModifiers; turns: number; note: string }> = [
  { name: "Благословение силы", kind: "buff", modifiers: { str: 2 }, turns: 3, note: "Тело наливается мощью." },
  { name: "Кошачья ловкость", kind: "buff", modifiers: { dex: 2 }, turns: 3, note: "Движения становятся текучими." },
  { name: "Каменная кожа", kind: "buff", modifiers: { ac: 2 }, turns: 2, note: "Кожа твердеет, словно камень." },
  { name: "Прилив жизни", kind: "buff", modifiers: { maxHp: 4 }, turns: 3, note: "Запас сил прибывает." },
  { name: "Ясность ума", kind: "buff", modifiers: { int: 2, wis: 1 }, turns: 3, note: "Мысли становятся острее." },
  { name: "Воодушевление", kind: "buff", modifiers: { cha: 2 }, turns: 3, note: "Слова звучат убедительнее." },
  { name: "Проклятье слабости", kind: "debuff", modifiers: { str: -2 }, turns: 3, note: "Мышцы наливаются свинцом." },
  { name: "Дрожь в руках", kind: "debuff", modifiers: { dex: -2 }, turns: 2, note: "Пальцы не слушаются." },
  { name: "Лихорадка", kind: "debuff", modifiers: { con: -1, str: -1 }, turns: 3, note: "Жар туманит тело." },
  { name: "Сглаз", kind: "debuff", modifiers: { ac: -2 }, turns: 2, note: "Удача отворачивается." },
  { name: "Смятение", kind: "debuff", modifiers: { int: -2 }, turns: 2, note: "Мысли путаются." },
];

function rollRandomEvent(rpg: CharacterRpg): Effect | null {
  if (rollDie(100) > RANDOM_EVENT_CHANCE) return null;
  const pick = RANDOM_EVENTS[rollDie(RANDOM_EVENTS.length) - 1];
  if (!pick) return null;
  const effect: Effect = { id: randomUUID(), ...pick };
  mergeEffect(rpg, effect);
  return effect;
}

export type ApplyOptions = { heroId?: string; randomEvents?: boolean };

// Resolve the narrator's declared mechanics deterministically. The LLM proposes
// (a check + DC, an HP delta); the engine rolls + clamps + decides death here so
// the model can never invent a total or a survival.
export function applyGameUpdate(
  update: GameUpdate,
  actors: ActorMap,
  opts: ApplyOptions = {},
): ApplyResult {
  const events: GameEvent[] = [];
  const changed = new Set<string>();
  const items: Item[] = [];
  const spawnedEnemies: Enemy[] = [];

  const resolveActorId = (preferred?: string): string | undefined => {
    if (preferred && actors.has(preferred)) return preferred;
    if (opts.heroId && actors.has(opts.heroId)) return opts.heroId;
    return firstActorId(actors);
  };

  // Tick active effects down a turn (one resolved turn = one tick) and drop the
  // expired — done first so an effect applied below keeps its full duration.
  for (const [id, actor] of actors) {
    for (const name of tickEffects(actor.rpg)) {
      changed.add(id);
      events.push(makeEvent("note", `⏳ Эффект развеялся: ${name}.`));
    }
  }

  // Foes arrive first so the same turn's attacks can target them by id.
  for (const spawn of update.spawnEnemies ?? []) {
    const rpg = structuredClone(DEFAULT_RPG);
    if (typeof spawn.hp === "number" && Number.isFinite(spawn.hp)) {
      rpg.hp.max = Math.max(1, Math.round(spawn.hp));
      rpg.hp.current = rpg.hp.max;
    }
    if (typeof spawn.ac === "number") rpg.ac = clampStat(spawn.ac, 1, 40);
    if (typeof spawn.level === "number") rpg.level = Math.max(1, Math.round(spawn.level));
    if (spawn.stats) {
      for (const ability of ABILITIES) {
        const value = spawn.stats[ability];
        if (typeof value === "number") rpg.stats[ability] = clampStat(value, 1, 30);
      }
    }
    const enemy: Enemy = { id: randomUUID(), name: spawn.name || "Враг", rpg };
    actors.set(enemy.id, { name: enemy.name, rpg: enemy.rpg });
    spawnedEnemies.push(enemy);
    changed.add(enemy.id);
    events.push(
      makeEvent("combat", `👹 В бой вступает ${enemy.name} — HP ${rpg.hp.max}, КЗ ${rpg.ac}`, { enemy }),
    );
  }

  // Attacks: d20 + the attacker's ability mod vs the target's AC; a hit rolls damage.
  for (const attack of update.attacks ?? []) {
    const attackerId =
      attack.attackerId && actors.has(attack.attackerId) ? attack.attackerId : firstActorId(actors);
    const attacker = attackerId ? actors.get(attackerId) : undefined;
    const target = actors.get(attack.targetId);
    if (!attacker || !target || target.rpg.dead) continue;
    const ability = attack.ability ?? "str";
    const score = attacker.rpg.stats[ability] ?? 10;
    const ac = target.rpg.ac ?? 10;
    const result = rollCheck(score, ac);
    const hit = result.success;
    const label = attack.label || `${attacker.name} → ${target.name}`;
    events.push(
      makeEvent(
        "roll",
        `⚔️ ${label}: d20 ${result.d20} ${fmtMod(result.modifier)} = ${result.total} против КЗ ${ac} → ${hit ? (result.crit === "success" ? "крит. попадание" : "попадание") : "промах"}`,
        { roll: { ...attack, kind: "attack", dc: ac }, result },
      ),
    );
    if (!hit) continue;
    const notation = attack.damage || "1d6";
    // 5e crits double the DICE, not the flat modifier: 1d8+3 → 2d8+3, not (1d8+3)×2.
    const rolled = rollNotation(notation);
    const diceSum = rolled.rolls.reduce((sum, value) => sum + value, 0);
    const flat = rolled.total - diceSum; // signed; correct for "1d8-1" too
    const damage = Math.max(
      1,
      result.crit === "success" ? diceSum * 2 + flat : rolled.total,
    );
    target.rpg.hp.current = clampStat(target.rpg.hp.current - damage, -999, target.rpg.hp.max);
    changed.add(attack.targetId);
    events.push(
      makeEvent(
        "hp",
        `💥 ${target.name}: -${damage} HP (${notation}${result.crit === "success" ? ", крит — удвоенные кости" : ""}) → ${Math.max(0, target.rpg.hp.current)}/${target.rpg.hp.max}`,
        { damage },
      ),
    );
    if (target.rpg.hp.current <= 0 && !target.rpg.dead) {
      target.rpg.dead = true;
      events.push(makeEvent("death", `☠️ ${target.name} повержен.`, { characterId: attack.targetId }));
    }
  }

  for (const roll of update.rolls ?? []) {
    const actorId = roll.actorId && actors.has(roll.actorId) ? roll.actorId : firstActorId(actors);
    const actor = actorId ? actors.get(actorId) : undefined;
    const score = actor ? actor.rpg.stats[roll.ability] ?? 10 : 10;
    const result = rollCheck(score, roll.dc);
    const label = roll.label || ABILITY_LABELS_RU[roll.ability];
    const name = actor?.name ?? "Игрок";
    const verdict =
      result.crit === "success"
        ? "крит. успех"
        : result.crit === "fail"
          ? "крит. провал"
          : result.success
            ? "успех"
            : "провал";
    const signed = `${result.modifier >= 0 ? "+" : ""}${result.modifier}`;
    events.push(
      makeEvent(
        "roll",
        `🎲 ${name} · ${label}: d20 ${result.d20} ${signed} = ${result.total} против ${result.dc} → ${verdict}`,
        { roll, result },
      ),
    );
  }

  for (const delta of update.hpDelta ?? []) {
    const actor = actors.get(delta.characterId);
    if (!actor || !Number.isFinite(delta.amount)) continue;
    actor.rpg.hp.current = clampStat(actor.rpg.hp.current + delta.amount, -999, actor.rpg.hp.max);
    changed.add(delta.characterId);
    const sign = delta.amount >= 0 ? "+" : "";
    const heart = delta.amount >= 0 ? "💚" : "💔";
    events.push(
      makeEvent(
        "hp",
        `${heart} ${actor.name}: ${sign}${delta.amount} HP${delta.reason ? ` (${delta.reason})` : ""} → ${Math.max(0, actor.rpg.hp.current)}/${actor.rpg.hp.max}`,
        { delta },
      ),
    );
    // A heal that lifts a downed actor back above 0 revives them — otherwise a
    // character could read "alive" on the HP bar yet stay flagged dead forever.
    if (actor.rpg.dead && actor.rpg.hp.current > 0) {
      actor.rpg.dead = false;
      events.push(makeEvent("hp", `✨ ${actor.name} приходит в себя.`, { characterId: delta.characterId }));
    } else if (actor.rpg.hp.current <= 0 && !actor.rpg.dead) {
      actor.rpg.dead = true;
      events.push(makeEvent("death", `☠️ ${actor.name} погибает.`, { characterId: delta.characterId }));
    }
  }

  for (const grant of update.grantItems ?? []) {
    const ownerId =
      grant.ownerId && actors.has(grant.ownerId) ? grant.ownerId : firstActorId(actors);
    const item: Item = {
      id: randomUUID(),
      ownerId,
      name: grant.name,
      slot: (grant.slot as ItemSlot | undefined) ?? "misc",
      rarity: (grant.rarity as ItemRarity | undefined) ?? "common",
      description: grant.description,
      damage: grant.damage,
      modifiers: grant.modifiers as Item["modifiers"],
      equipped: false,
      qty: grant.qty && grant.qty > 0 ? Math.round(grant.qty) : 1,
      imagePromptEn: grant.imagePromptEn?.trim() || undefined,
      createdAt: new Date().toISOString(),
    };
    items.push(item);
    const rarity = RARITY_RU[item.rarity] ?? item.rarity;
    events.push(
      makeEvent(
        "item",
        `📦 Получен предмет: ${item.name} (${rarity}${item.qty > 1 ? `, ×${item.qty}` : ""})`,
        // withImage tells the client to generate a dedicated portrait for this
        // drop; the picture then becomes the item's image2img reference.
        { item, withImage: grant.withImage === true },
      ),
    );
  }

  // Narrator-declared effects: clear first, then apply fresh buffs/debuffs.
  for (const clear of update.clearEffects ?? []) {
    const id = resolveActorId(clear.characterId);
    const actor = id ? actors.get(id) : undefined;
    if (!actor || !id) continue;
    const before = actor.rpg.effects?.length ?? 0;
    actor.rpg.effects =
      clear.name.trim() === "*"
        ? []
        : (actor.rpg.effects ?? []).filter(
            (e) => e.name.trim().toLowerCase() !== clear.name.trim().toLowerCase(),
          );
    if ((actor.rpg.effects?.length ?? 0) !== before) changed.add(id);
  }
  for (const decl of update.applyEffects ?? []) {
    const id = resolveActorId(decl.characterId);
    const actor = id ? actors.get(id) : undefined;
    if (!actor || !id) continue;
    const turns = decl.turns && decl.turns > 0 ? Math.round(decl.turns) : 3;
    const effect = coerceEffect({ ...decl, id: randomUUID(), turns });
    if (!effect) continue;
    mergeEffect(actor.rpg, effect);
    changed.add(id);
    const icon = effect.kind === "debuff" ? "🔻" : "✨";
    events.push(makeEvent("note", `${icon} ${effectSummary(effect.name, effect.modifiers, effect.turns)}`));
  }

  // A random blessing/curse may strike the hero each turn (toggleable).
  if (opts.randomEvents) {
    const id = opts.heroId && actors.has(opts.heroId) ? opts.heroId : firstActorId(actors);
    const actor = id ? actors.get(id) : undefined;
    if (actor && id) {
      const event = rollRandomEvent(actor.rpg);
      if (event) {
        changed.add(id);
        const icon = event.kind === "debuff" ? "🌑" : "🌟";
        events.push(
          makeEvent(
            "note",
            `${icon} Случайное событие: ${effectSummary(event.name, event.modifiers, event.turns)}. ${event.note}`,
          ),
        );
      }
    }
  }

  if (update.note && update.note.trim()) {
    events.push(makeEvent("note", update.note.trim()));
  }

  return { events, changed, items, spawnedEnemies };
}
