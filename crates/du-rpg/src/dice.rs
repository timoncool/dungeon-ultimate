//! Кости. Модель НИКОГДА не бросает сама: она только объявляет проверку — характеристику
//! и сложность — или атаку, а бросок делает сервер на криптостойком генераторе. Так
//! результат честный, и нарратор не может выдумать себе итог.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ability {
    Str,
    Dex,
    Con,
    Int,
    Wis,
    Cha,
}

pub const ABILITIES: [Ability; 6] = [
    Ability::Str,
    Ability::Dex,
    Ability::Con,
    Ability::Int,
    Ability::Wis,
    Ability::Cha,
];

impl Ability {
    pub fn key(self) -> &'static str {
        match self {
            Ability::Str => "str",
            Ability::Dex => "dex",
            Ability::Con => "con",
            Ability::Int => "int",
            Ability::Wis => "wis",
            Ability::Cha => "cha",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "str" => Some(Ability::Str),
            "dex" => Some(Ability::Dex),
            "con" => Some(Ability::Con),
            "int" => Some(Ability::Int),
            "wis" => Some(Ability::Wis),
            "cha" => Some(Ability::Cha),
            _ => None,
        }
    }

    /// Название для журнала приключения.
    pub fn label_ru(self) -> &'static str {
        match self {
            Ability::Str => "Сила",
            Ability::Dex => "Ловкость",
            Ability::Con => "Выносливость",
            Ability::Int => "Интеллект",
            Ability::Wis => "Мудрость",
            Ability::Cha => "Харизма",
        }
    }
}

/// Модификатор характеристики по правилам D&D 5e: (значение − 10) / 2 с округлением вниз.
pub fn ability_mod(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

/// Равномерное целое в [0, bound) без модульного смещения: берём 64 бита из системного
/// CSPRNG и переспрашиваем, если попали в «хвост», который не делится на bound нацело.
fn uniform_below(bound: u64) -> u64 {
    debug_assert!(bound > 0);
    let zone = u64::MAX - (u64::MAX % bound) - 1;
    loop {
        let mut buf = [0u8; 8];
        getrandom::fill(&mut buf).expect("системный генератор случайных чисел недоступен");
        let value = u64::from_le_bytes(buf);
        if value <= zone {
            return value % bound;
        }
    }
}

/// Один честный бросок кости в [1, sides]. Число граней зажато в [2, 1000]: настоящих
/// костей больше d1000 не бывает, а выдуманное моделью гигантское значение иначе уронило
/// бы ход.
pub fn roll_die(sides: i64) -> i32 {
    let s = sides.clamp(2, 1000) as u64;
    (uniform_below(s) + 1) as i32
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotationRoll {
    pub total: i32,
    pub rolls: Vec<i32>,
}

/// Разбор и бросок обычной записи костей: «1d8», «2d6+3», «1d20-1».
/// Неразбираемая строка даёт нулевой бросок — как в исходной версии.
pub fn roll_notation(notation: &str) -> NotationRoll {
    let Some((count, sides, modifier)) = parse_notation(notation) else {
        return NotationRoll { total: 0, rolls: Vec::new() };
    };
    let rolls: Vec<i32> = (0..count).map(|_| roll_die(sides)).collect();
    let total = rolls.iter().sum::<i32>() + modifier;
    NotationRoll { total, rolls }
}

/// `^\s*(\d*)d(\d+)\s*([+-]\s*\d+)?\s*$` вручную, чтобы не тянуть regex ради одной формы.
/// Возвращает (сколько костей, граней, плоский модификатор).
fn parse_notation(notation: &str) -> Option<(i32, i64, i32)> {
    let text = notation.trim();
    let lower = text.to_ascii_lowercase();
    let (left, rest) = lower.split_once('d')?;

    let count = if left.is_empty() {
        1
    } else {
        if !left.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        left.parse::<i32>().ok()?
    };
    let count = count.clamp(1, 20);

    let rest = rest.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    // i64 — чтобы «1d99999999999» не переполнилось до разбора; roll_die всё равно зажмёт.
    let sides = digits.parse::<i64>().unwrap_or(1000);

    let tail = rest[digits.len()..].trim();
    let modifier = if tail.is_empty() {
        0
    } else {
        let (sign, number) = tail.split_at(1);
        let number: String = number.chars().filter(|c| !c.is_whitespace()).collect();
        if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let value = number.parse::<i32>().ok()?;
        match sign {
            "+" => value,
            "-" => -value,
            _ => return None,
        }
    };

    Some((count, sides, modifier))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Crit {
    Success,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub d20: i32,
    pub modifier: i32,
    pub total: i32,
    pub dc: i32,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crit: Option<Crit>,
}

/// Проверка d20 против сложности. Натуральная 20 — всегда успех (крит), натуральная 1 —
/// всегда провал, независимо от модификаторов.
pub fn roll_check(score: i32, dc: i32, extra: i32) -> CheckResult {
    let d20 = roll_die(20);
    let modifier = ability_mod(score) + extra;
    let total = d20 + modifier;
    let crit = match d20 {
        20 => Some(Crit::Success),
        1 => Some(Crit::Fail),
        _ => None,
    };
    let dc = dc.max(1);
    let success = match crit {
        Some(Crit::Success) => true,
        Some(Crit::Fail) => false,
        None => total >= dc,
    };
    CheckResult { d20, modifier, total, dc, success, crit }
}

pub fn clamp_stat(value: i32, min: i32, max: i32) -> i32 {
    value.clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ability_modifier_rounds_down_including_negatives() {
        assert_eq!(ability_mod(10), 0);
        assert_eq!(ability_mod(11), 0);
        assert_eq!(ability_mod(12), 1);
        assert_eq!(ability_mod(9), -1);
        // Math.floor((1 - 10) / 2) === -5, а не -4: деление должно быть с округлением ВНИЗ.
        assert_eq!(ability_mod(1), -5);
        assert_eq!(ability_mod(3), -4);
    }

    #[test]
    fn die_stays_inside_its_face_range() {
        for _ in 0..500 {
            let value = roll_die(20);
            assert!((1..=20).contains(&value), "d20 выдал {value}");
        }
    }

    #[test]
    fn absurd_side_counts_are_clamped_instead_of_panicking() {
        let value = roll_die(1 << 50);
        assert!((1..=1000).contains(&value));
        let value = roll_die(0);
        assert!((1..=2).contains(&value));
    }

    #[test]
    fn notation_parses_counts_sides_and_signed_modifier() {
        let roll = roll_notation("2d6+3");
        assert_eq!(roll.rolls.len(), 2);
        assert_eq!(roll.total, roll.rolls.iter().sum::<i32>() + 3);

        let roll = roll_notation("1d8-1");
        assert_eq!(roll.total, roll.rolls[0] - 1);

        let roll = roll_notation("d4");
        assert_eq!(roll.rolls.len(), 1);

        // Мусор не бросается вовсе.
        assert_eq!(roll_notation("не кости").total, 0);
        assert!(roll_notation("").rolls.is_empty());
    }

    #[test]
    fn dice_count_is_capped_at_twenty() {
        let roll = roll_notation("999d6");
        assert_eq!(roll.rolls.len(), 20);
    }

    #[test]
    fn natural_twenty_beats_an_impossible_dc_and_natural_one_fails_a_trivial_one() {
        // Прогоняем много бросков и проверяем оба крит-правила на выпавших натуралках.
        let mut saw_crit_success = false;
        let mut saw_crit_fail = false;
        for _ in 0..3000 {
            let result = roll_check(10, 999, 0);
            if result.crit == Some(Crit::Success) {
                assert!(result.success, "натуральная 20 обязана бить любую сложность");
                saw_crit_success = true;
            }
            let result = roll_check(30, 1, 0);
            if result.crit == Some(Crit::Fail) {
                assert!(!result.success, "натуральная 1 обязана проваливаться всегда");
                saw_crit_fail = true;
            }
        }
        assert!(saw_crit_success && saw_crit_fail, "за 3000 бросков натуралки обязаны выпасть");
    }

    #[test]
    fn difficulty_class_never_drops_below_one() {
        let result = roll_check(10, -5, 0);
        assert_eq!(result.dc, 1);
    }
}
