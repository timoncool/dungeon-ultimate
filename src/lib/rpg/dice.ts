// Deterministic dice engine for the RPG layer. The LLM never rolls — it only
// declares a check (ability + DC) or an attack; the server rolls here with a
// real CSPRNG and reports the outcome. This keeps results fair and prevents the
// narrator from inventing totals.

import { randomInt } from "node:crypto";

export type Ability = "str" | "dex" | "con" | "int" | "wis" | "cha";
export const ABILITIES: Ability[] = ["str", "dex", "con", "int", "wis", "cha"];
export const ABILITY_LABELS_RU: Record<Ability, string> = {
  str: "Сила",
  dex: "Ловкость",
  con: "Выносливость",
  int: "Интеллект",
  wis: "Мудрость",
  cha: "Харизма",
};

// D&D 5e ability modifier: (score - 10) / 2, rounded down.
export function abilityMod(score: number): number {
  return Math.floor((score - 10) / 2);
}

// One fair die in [1, sides].
export function rollDie(sides: number): number {
  const s = Math.max(2, Math.floor(sides));
  return randomInt(1, s + 1);
}

// Parse + roll standard dice notation like "1d8", "2d6+3", "1d20-1".
export function rollNotation(notation: string): { total: number; rolls: number[] } {
  const match = /^\s*(\d*)d(\d+)\s*([+-]\s*\d+)?\s*$/i.exec(notation || "");
  if (!match) {
    return { total: 0, rolls: [] };
  }
  const count = Math.min(20, Math.max(1, Number.parseInt(match[1] || "1", 10)));
  const sides = Number.parseInt(match[2], 10);
  const modifier = match[3] ? Number.parseInt(match[3].replace(/\s+/g, ""), 10) : 0;
  const rolls: number[] = [];
  for (let i = 0; i < count; i += 1) rolls.push(rollDie(sides));
  const total = rolls.reduce((sum, value) => sum + value, 0) + modifier;
  return { total, rolls };
}

export type CheckResult = {
  d20: number;
  modifier: number;
  total: number;
  dc: number;
  success: boolean;
  crit: "success" | "fail" | null;
};

// A d20 ability check vs a difficulty class. Natural 20 always succeeds (crit),
// natural 1 always fails.
export function rollCheck(score: number, dc: number, extra = 0): CheckResult {
  const d20 = rollDie(20);
  const modifier = abilityMod(score) + extra;
  const total = d20 + modifier;
  const crit = d20 === 20 ? "success" : d20 === 1 ? "fail" : null;
  const success = crit === "success" ? true : crit === "fail" ? false : total >= dc;
  return { d20, modifier, total, dc: Math.max(1, Math.round(dc)), success, crit };
}

export function clampStat(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, Math.round(value)));
}
