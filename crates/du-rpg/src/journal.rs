//! Подписи журнала: то, что игрок читает про броски, урон, добычу и эффекты.
//!
//! Прежняя версия писала журнал по-русски при любом языке игры: английская партия видела
//! «крит. попадание» и «СИЛ 14». Здесь подписи вынесены наружу и приходят из набора промптов
//! на языке игры. По умолчанию — прежний русский текст до символа, поэтому старые сохранения
//! и вызовы без языка выглядят ровно как раньше.
//!
//! Шаблоны подставляют значения по имени: `{name}`, `{total}` и так далее. Порядок слов в
//! разных языках свой, поэтому позиционной подстановки тут нет намеренно.

use serde::Deserialize;

/// Подставить значения в шаблон. Неизвестные ключи остаются как есть — это заметно в журнале
/// и чинится правкой набора, а не тихой потерей текста.
pub fn render(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in values {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilityLabels {
    pub str: String,
    pub dex: String,
    pub con: String,
    pub int: String,
    pub wis: String,
    pub cha: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdicts {
    pub crit_hit: String,
    pub hit: String,
    pub miss: String,
    pub crit_success: String,
    pub crit_fail: String,
    pub success: String,
    pub failure: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RarityLabels {
    pub common: String,
    pub uncommon: String,
    pub rare: String,
    pub epic: String,
    pub legendary: String,
}

/// Формы слова «ход» после числа. В языках без склонения все три совпадают.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnWord {
    pub one: String,
    pub few: String,
    pub many: String,
}

impl TurnWord {
    pub fn pick(&self, n: i32) -> &str {
        let tail = n.abs() % 100;
        if (11..=14).contains(&tail) {
            return &self.many;
        }
        match tail % 10 {
            1 => &self.one,
            2..=4 => &self.few,
            _ => &self.many,
        }
    }
}

/// Название и присказка случайного благословения или проклятия.
#[derive(Debug, Clone, Deserialize)]
pub struct RandomEventText {
    pub name: String,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalLabels {
    pub abilities: AbilityLabels,
    /// Короткая подпись класса защиты в сводке эффекта.
    pub ac: String,
    pub max_hp: String,
    pub turns: TurnWord,
    /// Имя для броска без персонажа и для безымянного врага.
    pub player: String,
    pub enemy: String,
    pub verdict: Verdicts,
    pub rarity: RarityLabels,
    pub effect_faded: String,
    pub enemy_enters: String,
    pub attack: String,
    pub damage: String,
    pub crit_doubled: String,
    pub defeated: String,
    pub check: String,
    pub hp_change: String,
    pub revived: String,
    pub died: String,
    pub item_gained: String,
    pub effect_summary: String,
    pub random_event: String,
    /// По порядку соответствуют встроенному перечню случайных событий.
    #[serde(default)]
    pub random_events: Vec<RandomEventText>,
}

impl JournalLabels {
    /// Подпись редкости предмета.
    pub fn rarity(&self, rarity: crate::types::ItemRarity) -> &str {
        match rarity {
            crate::types::ItemRarity::Common => &self.rarity.common,
            crate::types::ItemRarity::Uncommon => &self.rarity.uncommon,
            crate::types::ItemRarity::Rare => &self.rarity.rare,
            crate::types::ItemRarity::Epic => &self.rarity.epic,
            crate::types::ItemRarity::Legendary => &self.rarity.legendary,
        }
    }

    /// Подпись характеристики по ключу движка. Незнакомый ключ отдаём как есть.
    pub fn stat<'a>(&'a self, key: &'a str) -> &'a str {
        match key {
            "str" => &self.abilities.str,
            "dex" => &self.abilities.dex,
            "con" => &self.abilities.con,
            "int" => &self.abilities.int,
            "wis" => &self.abilities.wis,
            "cha" => &self.abilities.cha,
            "ac" => &self.ac,
            "maxHp" => &self.max_hp,
            other => other,
        }
    }
}

impl Default for JournalLabels {
    fn default() -> Self {
        serde_json::from_str(include_str!("journal_ru.json"))
            .expect("встроенные русские подписи журнала должны разбираться")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builtin_russian_labels_parse() {
        let labels = JournalLabels::default();
        assert_eq!(labels.abilities.str, "СИЛ");
        assert_eq!(labels.stat("ac"), "КЗ");
        assert_eq!(labels.stat("неизвестно"), "неизвестно");
        assert_eq!(labels.random_events.len(), 11, "у всех случайных событий есть текст");
    }

    #[test]
    fn the_turn_word_declines_the_way_russian_does() {
        let turns = JournalLabels::default().turns;
        assert_eq!(turns.pick(1), "ход");
        assert_eq!(turns.pick(3), "хода");
        assert_eq!(turns.pick(5), "ходов");
        assert_eq!(turns.pick(11), "ходов");
        assert_eq!(turns.pick(21), "ход");
    }

    #[test]
    fn templates_substitute_by_name() {
        assert_eq!(render("{a} и {b}", &[("a", "раз"), ("b", "два")]), "раз и два");
    }

    #[test]
    fn an_unknown_placeholder_stays_visible_instead_of_vanishing() {
        assert_eq!(render("{a} {z}", &[("a", "раз")]), "раз {z}");
    }
}
