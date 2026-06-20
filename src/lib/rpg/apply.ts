import { randomUUID } from "node:crypto";
import { ABILITY_LABELS_RU, clampStat, rollCheck } from "./dice";
import type { CharacterRpg, GameEvent, GameUpdate, Item, ItemRarity, ItemSlot } from "./types";

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
  changed: Set<string>; // character ids whose rpg state was mutated
  items: Item[]; // new items to persist into the inventory
};

function makeEvent(kind: GameEvent["kind"], text: string, data?: unknown): GameEvent {
  return { id: randomUUID(), kind, text, data, createdAt: new Date().toISOString() };
}

function firstActorId(actors: ActorMap): string | undefined {
  for (const id of actors.keys()) return id;
  return undefined;
}

// Resolve the narrator's declared mechanics deterministically. The LLM proposes
// (a check + DC, an HP delta); the engine rolls + clamps + decides death here so
// the model can never invent a total or a survival.
export function applyGameUpdate(update: GameUpdate, actors: ActorMap): ApplyResult {
  const events: GameEvent[] = [];
  const changed = new Set<string>();
  const items: Item[] = [];

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
        { item },
      ),
    );
  }

  if (update.note && update.note.trim()) {
    events.push(makeEvent("note", update.note.trim()));
  }

  return { events, changed, items };
}
