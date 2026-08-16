// Клиентская часть костей. Броски делает сервер на криптостойком генераторе, поэтому сюда
// перенесено только то, что нужно интерфейсу: названия характеристик, модификатор и форма
// результата. Генератора здесь нет намеренно — иначе в браузер уехал бы node:crypto.

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

/** Модификатор характеристики по правилам D&D 5e: (значение − 10) / 2 с округлением вниз. */
export function abilityMod(score: number): number {
  return Math.floor((score - 10) / 2);
}

export type CheckResult = {
  d20: number;
  modifier: number;
  total: number;
  dc: number;
  success: boolean;
  crit: "success" | "fail" | null;
};

export function clampStat(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, Math.round(value)));
}
