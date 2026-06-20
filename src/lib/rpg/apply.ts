import { randomUUID } from "node:crypto";
import { ABILITIES, ABILITY_LABELS_RU, clampStat, rollCheck, rollNotation } from "./dice";
import { DEFAULT_RPG } from "./types";
import type { CharacterRpg, Enemy, GameEvent, GameUpdate, Item, ItemRarity, ItemSlot } from "./types";

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

// Resolve the narrator's declared mechanics deterministically. The LLM proposes
// (a check + DC, an HP delta); the engine rolls + clamps + decides death here so
// the model can never invent a total or a survival.
export function applyGameUpdate(update: GameUpdate, actors: ActorMap): ApplyResult {
  const events: GameEvent[] = [];
  const changed = new Set<string>();
  const items: Item[] = [];
  const spawnedEnemies: Enemy[] = [];

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
    const base = rollNotation(notation).total;
    const damage = Math.max(1, result.crit === "success" ? base * 2 : base);
    target.rpg.hp.current = clampStat(target.rpg.hp.current - damage, -999, target.rpg.hp.max);
    changed.add(attack.targetId);
    events.push(
      makeEvent(
        "hp",
        `💥 ${target.name}: -${damage} HP (${notation}${result.crit === "success" ? ", крит ×2" : ""}) → ${Math.max(0, target.rpg.hp.current)}/${target.rpg.hp.max}`,
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
    if (actor.rpg.hp.current <= 0 && !actor.rpg.dead) {
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

  if (update.note && update.note.trim()) {
    events.push(makeEvent("note", update.note.trim()));
  }

  return { events, changed, items, spawnedEnemies };
}
