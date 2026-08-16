//! Перевод объявлений модели в идентификаторы движка.
//!
//! Модель не должна печатать UUID — она пишет осмысленные имена («герой», «Гоблин»), а
//! сопоставление с идентификаторами делает сервер. Требовать от двенадцатимиллиардной
//! модели точного повторения UUID бессмысленно: она их путает и ход разваливается.

use du_rpg::{
    ActorMap, ApplyEffectDecl, AttackDecl, ClearEffectDecl, GameUpdate, GrantItemDecl, HpDeltaDecl,
    RollDecl, SpawnDecl,
};
use serde_json::Value;

/// Слова, которыми модель называет протагониста.
const HERO_WORDS: [&str; 10] = [
    "hero", "герой", "player", "игрок", "protagonist", "protagonista", "me", "я", "self", "самого",
];

/// Найти участника по имени, идентификатору или слову «герой».
fn resolve_actor(actors: &ActorMap, raw: Option<&str>, hero_id: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    if actors.contains_key(raw) {
        return Some(raw.to_string());
    }
    let lower = raw.to_lowercase();
    if HERO_WORDS.iter().any(|word| lower == *word) {
        return hero_id.map(str::to_string).or_else(|| actors.keys().next().cloned());
    }
    // Точное совпадение имени сильнее частичного, иначе «Гоблин» выберет «Гоблин-вожак».
    if let Some((id, _)) = actors.iter().find(|(_, actor)| actor.name.to_lowercase() == lower) {
        return Some(id.clone());
    }
    actors
        .iter()
        .find(|(_, actor)| {
            let name = actor.name.to_lowercase();
            name.contains(&lower) || lower.contains(&name)
        })
        .map(|(id, _)| id.clone())
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_string)
}

fn number(value: &Value, key: &str) -> Option<i32> {
    value.get(key)?.as_f64().map(|number| number.round() as i32)
}

fn parse_enum<T: serde::de::DeserializeOwned>(value: &Value, key: &str) -> Option<T> {
    serde_json::from_value(value.get(key)?.clone()).ok()
}

/// Превратить ответ структурного прохода в объявление хода с настоящими идентификаторами.
pub fn resolve_engine_update(raw: &Value, actors: &ActorMap, hero_id: Option<&str>) -> GameUpdate {
    let mut update = GameUpdate::default();
    let array = |key: &str| -> Vec<Value> {
        raw.get(key).and_then(Value::as_array).cloned().unwrap_or_default()
    };

    for item in array("rolls") {
        let Some(dc) = number(&item, "dc") else { continue };
        update.rolls.push(RollDecl {
            ability: parse_enum(&item, "ability"),
            dc,
            label: text(&item, "label"),
            actor_id: resolve_actor(actors, text(&item, "actor").as_deref(), hero_id),
            kind: text(&item, "kind"),
            target_id: None,
        });
    }

    for item in array("spawnEnemies") {
        let Some(name) = text(&item, "name") else { continue };
        update.spawn_enemies.push(SpawnDecl {
            name,
            hp: number(&item, "hp"),
            ac: number(&item, "ac"),
            level: number(&item, "level"),
            stats: None,
        });
    }

    for item in array("attacks") {
        // Цель обязана разрешиться: удар в никого пропускаем, а не переводим на героя.
        let Some(target_id) = resolve_actor(actors, text(&item, "target").as_deref(), None) else {
            continue;
        };
        update.attacks.push(AttackDecl {
            attacker_id: resolve_actor(actors, text(&item, "attacker").as_deref(), hero_id),
            target_id,
            ability: parse_enum(&item, "ability"),
            damage: text(&item, "damage"),
            label: text(&item, "label"),
        });
    }

    for item in array("hpDelta") {
        let Some(amount) = number(&item, "amount") else { continue };
        update.hp_delta.push(HpDeltaDecl {
            character_id: resolve_actor(actors, text(&item, "character").as_deref(), hero_id),
            amount,
            reason: text(&item, "reason"),
        });
    }

    for item in array("grantItems") {
        let Some(name) = text(&item, "name") else { continue };
        update.grant_items.push(GrantItemDecl {
            owner_id: resolve_actor(actors, text(&item, "owner").as_deref(), hero_id),
            name,
            slot: parse_enum(&item, "slot"),
            rarity: parse_enum(&item, "rarity"),
            description: text(&item, "description"),
            damage: text(&item, "damage"),
            modifiers: parse_enum(&item, "modifiers"),
            qty: number(&item, "qty"),
            with_image: item.get("withImage").and_then(Value::as_bool),
            image_prompt_en: text(&item, "imagePromptEn"),
        });
    }

    for item in array("applyEffects") {
        let Some(name) = text(&item, "name") else { continue };
        update.apply_effects.push(ApplyEffectDecl {
            character_id: resolve_actor(actors, text(&item, "character").as_deref(), hero_id),
            name,
            kind: parse_enum(&item, "kind"),
            modifiers: parse_enum(&item, "modifiers"),
            turns: number(&item, "turns"),
            note: text(&item, "note"),
        });
    }

    for item in array("clearEffects") {
        let Some(name) = text(&item, "name") else { continue };
        update.clear_effects.push(ClearEffectDecl {
            character_id: resolve_actor(actors, text(&item, "character").as_deref(), hero_id),
            name,
        });
    }

    for item in array("offerQuests") {
        let Some(title) = text(&item, "title") else { continue };
        update.offer_quests.push(du_rpg::OfferQuestDecl {
            title,
            giver: text(&item, "giver"),
            summary: text(&item, "summary"),
            conditions: item
                .get("conditions")
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            reward: text(&item, "reward"),
            xp: number(&item, "xp"),
        });
    }

    for (key, bucket) in [
        ("completeQuests", &mut update.complete_quests),
        ("failQuests", &mut update.fail_quests),
    ] {
        for item in raw.get(key).and_then(Value::as_array).cloned().unwrap_or_default() {
            let Some(title) = text(&item, "title") else { continue };
            bucket.push(du_rpg::QuestOutcomeDecl { title, reason: text(&item, "reason") });
        }
    }

    for item in array("awardXp") {
        let Some(amount) = number(&item, "amount") else { continue };
        update.award_xp.push(du_rpg::XpAwardDecl {
            character_id: resolve_actor(actors, text(&item, "character").as_deref(), hero_id),
            amount,
            reason: text(&item, "reason"),
        });
    }

    update.note = text(raw, "note");
    update
}

#[cfg(test)]
mod tests {
    use super::*;
    use du_rpg::{Actor, CharacterRpg};
    use serde_json::json;

    fn cast() -> ActorMap {
        let mut actors = ActorMap::new();
        actors.insert("uuid-hero".into(), Actor { name: "Кайра".into(), rpg: CharacterRpg::default() });
        actors.insert("uuid-foe".into(), Actor { name: "Гоблин".into(), rpg: CharacterRpg::default() });
        actors.insert("uuid-boss".into(), Actor { name: "Гоблин-вожак".into(), rpg: CharacterRpg::default() });
        actors
    }

    #[test]
    fn the_word_hero_maps_to_the_protagonist() {
        let actors = cast();
        let raw = json!({ "rolls": [{ "ability": "dex", "dc": 12, "actor": "герой" }] });
        let update = resolve_engine_update(&raw, &actors, Some("uuid-hero"));
        assert_eq!(update.rolls[0].actor_id.as_deref(), Some("uuid-hero"));
    }

    #[test]
    fn a_name_maps_to_its_actor_and_an_exact_match_wins() {
        let actors = cast();
        let raw = json!({ "attacks": [{ "attacker": "Кайра", "target": "Гоблин", "damage": "1d8" }] });
        let update = resolve_engine_update(&raw, &actors, Some("uuid-hero"));
        assert_eq!(update.attacks[0].attacker_id.as_deref(), Some("uuid-hero"));
        assert_eq!(
            update.attacks[0].target_id, "uuid-foe",
            "точное имя обязано выигрывать у частичного совпадения"
        );
    }

    #[test]
    fn an_attack_on_nobody_is_dropped_rather_than_redirected() {
        let actors = cast();
        let raw = json!({ "attacks": [{ "attacker": "Кайра", "target": "призрак", "damage": "1d8" }] });
        let update = resolve_engine_update(&raw, &actors, Some("uuid-hero"));
        assert!(update.attacks.is_empty(), "неразрешённая цель не должна превращаться в героя");
    }

    #[test]
    fn a_raw_identifier_still_works() {
        let actors = cast();
        let raw = json!({ "hpDelta": [{ "character": "uuid-foe", "amount": -3 }] });
        let update = resolve_engine_update(&raw, &actors, Some("uuid-hero"));
        assert_eq!(update.hp_delta[0].character_id.as_deref(), Some("uuid-foe"));
    }

    #[test]
    fn garbage_entries_are_skipped_without_breaking_the_turn() {
        let actors = cast();
        let raw = json!({
            "rolls": [{ "ability": "str" }, { "ability": "str", "dc": 10 }],
            "grantItems": [{ "description": "без имени" }, { "name": "Ключ" }],
            "note": "  "
        });
        let update = resolve_engine_update(&raw, &actors, Some("uuid-hero"));
        assert_eq!(update.rolls.len(), 1, "бросок без сложности неполон");
        assert_eq!(update.grant_items.len(), 1);
        assert_eq!(update.grant_items[0].name, "Ключ");
    }

    #[test]
    fn an_empty_answer_yields_an_empty_turn() {
        let update = resolve_engine_update(&json!({}), &cast(), None);
        assert!(update.rolls.is_empty() && update.attacks.is_empty() && update.note.is_none());
    }
}
