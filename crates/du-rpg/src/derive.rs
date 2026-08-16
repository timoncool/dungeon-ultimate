//! Производные статы = база + модификаторы надетого и активных эффектов.
//!
//! Функция ЧИСТАЯ: клонирует вход, ничего не мутирует и никуда не сохраняет. Хранимый
//! `CharacterRpg` остаётся канонической БАЗОЙ, и каждый раз выводится заново — поэтому
//! бонусы не накапливаются. Текущее HP переносится как есть, но зажимается новым
//! максимумом, чтобы снятие предмета на +макс.HP не оставило текущее выше потолка.
//!
//! В каждом слоте учитывается не больше ОДНОГО надетого предмета (побеждает более
//! новый по `created_at`) — правило «один предмет на слот» держится в точке истины, а
//! не только в обработчике надевания, иначе старые строки могли бы сложить два кольца.

use std::collections::HashMap;

use crate::dice::{clamp_stat, ABILITIES};
use crate::types::{CharacterRpg, Item, Modifiers};

#[derive(Debug, Clone)]
pub struct DerivedRpg {
    /// База со сложенными модификаторами. Ими пользуются И боевой резолвер, И лист
    /// персонажа, поэтому цифры на экране не могут разойтись с тем, против чего бросают.
    pub rpg: CharacterRpg,
    /// Сумма применённых модификаторов по ключам — для подсветки бонусов в интерфейсе.
    pub bonus: HashMap<String, i32>,
}

fn add_modifiers(bonus: &mut HashMap<String, i32>, modifiers: &Modifiers) {
    for (key, value) in modifiers.entries() {
        *bonus.entry(key.to_string()).or_insert(0) += value;
    }
}

pub fn derive_rpg(base: &CharacterRpg, items: &[Item]) -> DerivedRpg {
    // Один предмет на слот: побеждает самый свежий надетый.
    let mut by_slot: HashMap<String, &Item> = HashMap::new();
    for item in items.iter().filter(|item| item.equipped) {
        let slot = format!("{:?}", item.slot);
        match by_slot.get(&slot) {
            Some(previous) if previous.created_at >= item.created_at => {}
            _ => {
                by_slot.insert(slot, item);
            }
        }
    }

    let mut bonus: HashMap<String, i32> = HashMap::new();
    for item in by_slot.values() {
        add_modifiers(&mut bonus, &item.modifiers);
    }
    // Активные эффекты складываются точно так же, как экипировка.
    for effect in base.effects.iter().filter(|effect| effect.turns > 0) {
        add_modifiers(&mut bonus, &effect.modifiers);
    }

    let mut rpg = base.clone();
    for ability in ABILITIES {
        if let Some(delta) = bonus.get(ability.key()) {
            rpg.stats.set(ability, clamp_stat(rpg.stats.get(ability) + delta, 1, 30));
        }
    }
    // КЗ не опускаем ниже нуля: иначе крупный дебаф давал бы отрицательный класс защиты,
    // при котором любая атака попадает автоматически, а в интерфейсе светилось бы «КЗ −40».
    if let Some(delta) = bonus.get("ac") {
        rpg.ac = (rpg.ac + delta).max(0);
    }
    if let Some(delta) = bonus.get("maxHp") {
        rpg.hp.max = (rpg.hp.max + delta).max(1);
    }
    rpg.hp.current = rpg.hp.current.min(rpg.hp.max);

    DerivedRpg { rpg, bonus }
}

/// Вывести статы, считая ТОЛЬКО предметы этого владельца. Боевой резолвер и лист
/// персонажа обязаны ходить сюда с одним и тем же `owner_id`, чтобы показанные цифры
/// всегда совпадали с теми, против которых бросает движок.
///
/// `include_unowned` включается только для протагониста: предметы, сохранённые до
/// появления владельцев, принадлежат герою, и его производные статы обязаны их учитывать
/// (так же, как их показывает панель инвентаря). Для любого другого персонажа флаг
/// остаётся выключенным, чтобы спутник не унаследовал чужое.
pub fn derive_for_owner(
    base: &CharacterRpg,
    items: &[Item],
    owner_id: Option<&str>,
    include_unowned: bool,
) -> DerivedRpg {
    let mine: Vec<Item> = items
        .iter()
        .filter(|item| {
            item.owner_id.as_deref() == owner_id || (include_unowned && item.owner_id.is_none())
        })
        .cloned()
        .collect();
    derive_rpg(base, &mine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Effect, EffectKind, Hp, ItemSlot};

    fn item(id: &str, slot: ItemSlot, created_at: &str, modifiers: Modifiers) -> Item {
        Item {
            id: id.into(),
            owner_id: None,
            name: id.into(),
            slot,
            rarity: Default::default(),
            description: None,
            damage: None,
            modifiers,
            equipped: true,
            qty: 1,
            image_url: None,
            image_prompt_en: None,
            created_at: created_at.into(),
        }
    }

    #[test]
    fn equipped_modifiers_fold_into_stats() {
        let base = CharacterRpg::default();
        let items = vec![item(
            "меч",
            ItemSlot::Weapon,
            "2026-01-01",
            Modifiers { str: Some(2), ac: Some(1), ..Default::default() },
        )];
        let derived = derive_rpg(&base, &items);
        assert_eq!(derived.rpg.stats.str, 12);
        assert_eq!(derived.rpg.ac, 11);
        assert_eq!(derived.bonus["str"], 2);
    }

    #[test]
    fn only_the_newest_item_in_a_slot_counts() {
        let base = CharacterRpg::default();
        let items = vec![
            item("кольцо-старое", ItemSlot::Trinket, "2026-01-01", Modifiers { str: Some(2), ..Default::default() }),
            item("кольцо-новое", ItemSlot::Trinket, "2026-02-01", Modifiers { str: Some(3), ..Default::default() }),
        ];
        let derived = derive_rpg(&base, &items);
        assert_eq!(derived.rpg.stats.str, 13, "два кольца не должны складываться");
    }

    #[test]
    fn unequipped_items_are_ignored() {
        let base = CharacterRpg::default();
        let mut off = item("щит", ItemSlot::Shield, "2026-01-01", Modifiers { ac: Some(5), ..Default::default() });
        off.equipped = false;
        let derived = derive_rpg(&base, &[off]);
        assert_eq!(derived.rpg.ac, 10);
    }

    #[test]
    fn expired_effects_do_not_apply_but_active_ones_do() {
        let mut base = CharacterRpg::default();
        base.effects = vec![
            Effect {
                id: "1".into(),
                name: "Каменная кожа".into(),
                kind: EffectKind::Buff,
                modifiers: Modifiers { ac: Some(2), ..Default::default() },
                turns: 2,
                note: None,
            },
            Effect {
                id: "2".into(),
                name: "Выдохшийся".into(),
                kind: EffectKind::Debuff,
                modifiers: Modifiers { ac: Some(-5), ..Default::default() },
                turns: 0,
                note: None,
            },
        ];
        let derived = derive_rpg(&base, &[]);
        assert_eq!(derived.rpg.ac, 12);
    }

    #[test]
    fn armor_class_floors_at_zero() {
        let mut base = CharacterRpg::default();
        base.effects = vec![Effect {
            id: "1".into(),
            name: "Сглаз".into(),
            kind: EffectKind::Debuff,
            modifiers: Modifiers { ac: Some(-40), ..Default::default() },
            turns: 3,
            note: None,
        }];
        assert_eq!(derive_rpg(&base, &[]).rpg.ac, 0);
    }

    #[test]
    fn current_hp_is_clamped_when_a_max_hp_bonus_goes_away() {
        let mut base = CharacterRpg::default();
        base.hp = Hp { current: 24, max: 20 };
        let derived = derive_rpg(&base, &[]);
        assert_eq!(derived.rpg.hp.current, 20);
    }

    #[test]
    fn deriving_is_pure_and_never_compounds() {
        let base = CharacterRpg::default();
        let items = vec![item("меч", ItemSlot::Weapon, "2026-01-01", Modifiers { str: Some(2), ..Default::default() })];
        let first = derive_rpg(&base, &items);
        let second = derive_rpg(&base, &items);
        assert_eq!(first.rpg.stats.str, second.rpg.stats.str);
        assert_eq!(base.stats.str, 10, "база обязана остаться нетронутой");
    }

    #[test]
    fn only_the_hero_inherits_ownerless_legacy_items() {
        let base = CharacterRpg::default();
        let mut legacy = item("наследие", ItemSlot::Weapon, "2026-01-01", Modifiers { str: Some(4), ..Default::default() });
        legacy.owner_id = None;
        let mut companions = item("посох", ItemSlot::Weapon, "2026-01-01", Modifiers { str: Some(1), ..Default::default() });
        companions.owner_id = Some("спутник".into());
        let items = vec![legacy, companions];

        let hero = derive_for_owner(&base, &items, Some("герой"), true);
        assert_eq!(hero.rpg.stats.str, 14);

        let companion = derive_for_owner(&base, &items, Some("спутник"), false);
        assert_eq!(companion.rpg.stats.str, 11);
    }
}
