//! Ход истории.
//!
//! Ход многопроходный, и это принципиально: нарратор пишет ТОЛЬКО чистую прозу и отдаёт её
//! потоком, а механика и кадр добываются ОТДЕЛЬНЫМИ вызовами, ограниченными JSON-схемой.
//! Когда всё это просили у нарратора разом, локальная модель протаскивала служебные блоки
//! прямо в текст истории.
//!
//! Числа модель не выдумывает: состояние игры мирроится в системный промпт как есть, а
//! броски делает движок.

use du_core::{Language, StoryCharacter, StoryMessage, StoryRole, StorySettings};
use du_llm::{Message, Sampling};
use du_prompts::{build_anti_repetition, pack_story_history, prompts_for, PromptSet};
use du_rpg::{ability_mod, Actor, ActorMap, CharacterRpg, Enemy, Item, ABILITIES};
use serde_json::{json, Value};

/// Запас символов истории, который влезает в контекст модели. Считается от контекста
/// сайдкара с поправкой на системный промпт и место под ответ.
pub const HISTORY_CHAR_BUDGET: usize = 48_000;

/// Блок состояния игры, который каждый ход зеркалится в системный промпт. Нарратор читает
/// отсюда статы, HP и идентификаторы — и не придумывает их сам.
pub fn build_rpg_section(
    actors: &ActorMap,
    items: &[Item],
    enemies: &[Enemy],
    quests: &[du_rpg::Quest],
    prompts: &PromptSet,
) -> String {
    let rules = prompts.rpg.rules.clone();
    if actors.is_empty() && enemies.is_empty() && quests.is_empty() {
        return rules;
    }

    let mut lines = Vec::new();
    for (id, actor) in actors.iter() {
        let stats = ABILITIES
            .iter()
            .map(|ability| {
                let value = actor.rpg.stats.get(*ability);
                let modifier = ability_mod(value);
                let sign = if modifier >= 0 { "+" } else { "" };
                format!("{} {value} ({sign}{modifier})", ability.label_ru())
            })
            .collect::<Vec<_>>()
            .join(", ");
        let dead = if actor.rpg.dead { format!(" — {}", prompts.rpg.dead) } else { String::new() };
        let mut block = format!(
            "• {} [ID: {id}] — {} {}/{}, {} {}, {}{}{dead}\n  {stats}",
            actor.name,
            prompts.rpg.hp,
            actor.rpg.hp.current.max(0),
            actor.rpg.hp.max,
            prompts.rpg.ac,
            actor.rpg.ac,
            prompts.rpg.level,
            actor.rpg.level,
        );
        if !actor.rpg.conditions.is_empty() {
            block.push_str(&format!(
                "\n  {}: {}",
                prompts.rpg.conditions,
                actor.rpg.conditions.join(", ")
            ));
        }
        if !actor.rpg.effects.is_empty() {
            let effects = actor
                .rpg
                .effects
                .iter()
                .map(|effect| format!("{} ({})", effect.name, effect.turns))
                .collect::<Vec<_>>()
                .join(", ");
            block.push_str(&format!("\n  {}: {effects}", prompts.rpg.effects));
        }
        lines.push(block);
    }

    // В инвентарь идёт только то, что принадлежит отряду. Вещь с чужим владельцем (лут
    // врага) не должна подаваться как снаряжение героя; бесхозная остаётся — она от сцены.
    let owned: Vec<&Item> = items
        .iter()
        .filter(|item| match item.owner_id.as_deref() {
            Some(owner) => actors.contains_key(owner),
            None => true,
        })
        .collect();
    // Владельца подписываем только если отряд не из одного героя, иначе подпись — шум.
    let multi_owner = actors.len() > 1;
    let inventory = if owned.is_empty() {
        String::new()
    } else {
        let rows = owned
            .iter()
            .map(|item| {
                let qty = if item.qty > 1 { format!(" ×{}", item.qty) } else { String::new() };
                let damage = item
                    .damage
                    .as_deref()
                    .map(|d| format!(", {} {d}", prompts.rpg.damage))
                    .unwrap_or_default();
                let equipped =
                    if item.equipped { format!(" [{}]", prompts.rpg.equipped) } else { String::new() };
                let owner = match (multi_owner, item.owner_id.as_deref()) {
                    (true, Some(owner)) => actors
                        .get(owner)
                        .map(|actor| format!(" — {}", actor.name))
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                format!("• {}{qty} ({:?}{damage}){equipped}{owner}", item.name, item.slot)
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n\n{}:\n{rows}", prompts.rpg.inventory)
    };

    let foes = if enemies.is_empty() {
        String::new()
    } else {
        let rows = enemies
            .iter()
            .map(|enemy| {
                let dead =
                    if enemy.rpg.dead { format!(" — {}", prompts.rpg.dead) } else { String::new() };
                format!(
                    "• {} [ID: {}] — {} {}/{}, {} {}{dead}",
                    enemy.name,
                    enemy.id,
                    prompts.rpg.hp,
                    enemy.rpg.hp.current.max(0),
                    enemy.rpg.hp.max,
                    prompts.rpg.ac,
                    enemy.rpg.ac
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n\n{}:\n{rows}", prompts.rpg.foes)
    };

    let head = if lines.is_empty() { "• (нет игровых персонажей)".to_string() } else { lines.join("\n") };
    format!("СОСТОЯНИЕ ИГРЫ (authoritative — опирайся на него, НЕ выдумывай числа):\n{head}{inventory}{foes}\n\n{rules}")
}

/// Указание языка отдельным системным сообщением ПОСЛЕ промпта нарратора: так оно
/// перебивает язык, зашитый в пользовательский промпт. Промпт кадра остаётся английским.
pub fn language_directive(language: Language) -> String {
    format!(
        "ЯЗЫК / LANGUAGE: Write the ENTIRE response — all narration and character dialogue — \
         in {}. Never switch languages, regardless of the language of these instructions. \
         The only exception is the image-generation prompt, which must stay in English.",
        language.prompt_name()
    )
}

/// Собрать сообщения для прохода нарратора.
pub fn build_story_messages(
    history: &[StoryMessage],
    input: &str,
    settings: &StorySettings,
    characters: &[StoryCharacter],
    story_summary: &str,
    rpg_section: &str,
) -> Vec<Message> {
    let prompts = prompts_for(settings.language);
    let packed = pack_story_history(history, HISTORY_CHAR_BUDGET);

    let roster = if characters.is_empty() {
        prompts.labels.no_characters.clone()
    } else {
        characters
            .iter()
            .map(|character| {
                let mut block = format!(
                    "{}: {}\n{}: {}",
                    prompts.labels.char_id, character.id, prompts.labels.char_name, character.name
                );
                for (label, value) in [
                    (&prompts.labels.char_details, &character.details),
                    (&prompts.labels.char_inventory, &character.inventory),
                    (&prompts.labels.char_skills, &character.skills),
                    (&prompts.labels.char_spells, &character.spells),
                ] {
                    if !value.trim().is_empty() {
                        block.push_str(&format!("\n{label}: {value}"));
                    }
                }
                block.push('\n');
                block.push_str(if character.portrait.is_some() {
                    &prompts.labels.portrait_available
                } else {
                    &prompts.labels.portrait_unavailable
                });
                block
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let anti_repetition = if settings.anti_repetition {
        build_anti_repetition(history)
            .map(|hint| {
                let mut lines =
                    vec![prompts.anti_repetition.header.clone(), prompts.anti_repetition.recent_openings.clone()];
                lines.extend(hint.beats.iter().map(|beat| format!("  • {beat}")));
                if hint.motifs.is_empty() {
                    lines.push(prompts.anti_repetition.vary_opening.clone());
                } else {
                    lines.push(format!("{}{}", prompts.anti_repetition.motifs_prefix, hint.motifs.join(", ")));
                }
                lines.join("\n")
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Кадр создаёт ОТДЕЛЬНЫЙ проход, поэтому нарратору прямо запрещено писать промпт
    // изображения: локальная модель иначе вставляет его в видимый текст.
    let image_directive = if settings.image_generation_enabled {
        "ИЛЛЮСТРАЦИИ к сцене создаёт ОТДЕЛЬНАЯ система, не ты. НИКОГДА не пиши промпт \
         изображения, маркеры вида [IMAGE_GEN_PROMPT], вызовы generate_image, английские \
         описания кадра или блоки ```json в видимом тексте. Пиши ТОЛЬКО живую прозу истории."
            .to_string()
    } else {
        prompts.image_disabled.clone()
    };

    let narrator = if settings.narrator_prompt.trim().is_empty() {
        prompts.narrator.clone()
    } else {
        settings.narrator_prompt.clone()
    };

    let world = if settings.world.trim().is_empty() {
        prompts.labels.world_fallback.clone()
    } else {
        settings.world.clone()
    };
    let style = if settings.style.trim().is_empty() {
        prompts.labels.style_fallback.clone()
    } else {
        settings.style.clone()
    };

    let mut system = vec![
        narrator,
        prompts.response_length.get(settings.response_length).to_string(),
    ];
    if settings.cause_aware_ending {
        system.push(prompts.ending.clone());
    }
    if settings.companion {
        system.push(prompts.companion.clone());
    }
    if !anti_repetition.is_empty() {
        system.push(anti_repetition);
    }
    system.push(image_directive);
    system.push(format!("{}:\n{world}", prompts.labels.world));
    system.push(format!("{}:\n{style}", prompts.labels.style));
    if !story_summary.trim().is_empty() {
        system.push(format!("{}:\n{story_summary}", prompts.labels.story_so_far));
    }
    system.push(format!("{}:\n{roster}", prompts.labels.saved_characters));
    if !rpg_section.trim().is_empty() {
        system.push(rpg_section.to_string());
    }

    let mut messages = vec![
        Message::system(system.join("\n\n")),
        Message::system(language_directive(settings.language)),
    ];
    for message in packed.recent {
        let attachments = if message.attachments.is_empty() {
            String::new()
        } else {
            let names = message
                .attachments
                .iter()
                .map(|attachment| attachment.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("\n[{}: {names}]", prompts.labels.attachments)
        };
        let content = format!("{}{attachments}", message.content);
        messages.push(match message.role {
            StoryRole::Assistant => Message::assistant(content),
            StoryRole::User => Message::user(content),
        });
    }
    messages.push(Message::user(input.to_string()));
    messages
}

/// Схема механики хода. Модель ОБЪЯВЛЯЕТ, что происходит; все броски и урон считает движок,
/// поэтому в схеме нет ни одного поля под готовый результат.
pub fn engine_schema() -> Value {
    let ability = json!({ "type": "string", "enum": ["str", "dex", "con", "int", "wis", "cha"] });
    let modifiers = json!({ "type": "object", "additionalProperties": { "type": "number" } });
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "rolls": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "ability": ability,
                "dc": { "type": "number" },
                "label": { "type": "string" },
                "actor": { "type": "string" },
                "kind": { "type": "string", "enum": ["skill", "attack", "save"] }
            }, "required": ["ability", "dc"] } },
            // Проверку ОБЪЯВЛЯЮТ здесь, а бросает и решает движок: писать исход словами
            // нельзя, иначе кубик не катится и правила подменяются пересказом.
            "spawnEnemies": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "name": { "type": "string" },
                "hp": { "type": "number" },
                "ac": { "type": "number" },
                "level": { "type": "number" }
            }, "required": ["name"] } },
            "attacks": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "attacker": { "type": "string" },
                "target": { "type": "string" },
                "ability": ability,
                "damage": { "type": "string" },
                "label": { "type": "string" }
            }, "required": ["target"] } },
            "hpDelta": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "character": { "type": "string" },
                "amount": { "type": "number" },
                "reason": { "type": "string" }
            }, "required": ["amount"] } },
            "grantItems": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "name": { "type": "string" },
                "slot": { "type": "string", "enum": ["weapon", "armor", "shield", "trinket", "consumable", "misc"] },
                "rarity": { "type": "string", "enum": ["common", "uncommon", "rare", "epic", "legendary"] },
                "description": { "type": "string" },
                "damage": { "type": "string" },
                "modifiers": modifiers,
                "qty": { "type": "number" },
                "withImage": { "type": "boolean" },
                "imagePromptEn": { "type": "string" }
            }, "required": ["name"] } },
            "applyEffects": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "character": { "type": "string" },
                "name": { "type": "string" },
                "kind": { "type": "string", "enum": ["buff", "debuff"] },
                "modifiers": modifiers,
                "turns": { "type": "number" },
                "note": { "type": "string" }
            }, "required": ["name"] } },
            "clearEffects": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "character": { "type": "string" },
                "name": { "type": "string" }
            }, "required": ["name"] } },
            "offerQuests": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "title": { "type": "string", "description": "короткое название задания" },
                "giver": { "type": "string", "description": "кто его дал — имя персонажа" },
                "summary": { "type": "string", "description": "что нужно сделать, одним предложением" },
                "conditions": { "type": "array", "items": { "type": "string" },
                    "description": "по каким условиям видно, что задание выполнено" },
                "reward": { "type": "string", "description": "что обещано за выполнение" },
                "xp": { "type": "number", "description": "опыт за выполнение: 50 за простое, 150 за среднее, 400 за трудное" }
            }, "required": ["title"] } },
            "completeQuests": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "title": { "type": "string", "description": "название задания из раздела ВЗЯТЫЕ ЗАДАНИЯ" },
                "reason": { "type": "string" }
            }, "required": ["title"] } },
            "failQuests": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "title": { "type": "string" },
                "reason": { "type": "string" }
            }, "required": ["title"] } },
            "awardXp": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": {
                "character": { "type": "string" },
                "amount": { "type": "number", "description": "опыт за победу или находку: 10-50 за бой по силам" },
                "reason": { "type": "string" }
            }, "required": ["amount"] } },
            "note": {
                "type": "string",
                "description": "короткая заметка ИГРОКУ о последствии хода. НЕ рассуждение,                     не план, не разбор правил и НИКОГДА не исход проверки — исход решает движок",
                "maxLength": 160
            }
        }
    })
}

/// Схема кадра: один ключевой момент этой сцены.
pub fn image_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "needed": {
                "type": "boolean",
                "description": "нужен ли кадр этому отрывку. true — сцена сменила место,                     появился новый персонаж или существо, случилось что-то зримое: удар,                     находка, обряд, гроза. false — разговор на том же месте без новых лиц и                     без событий, которые стоит увидеть"
            },
            "prompt": {
                "type": "string",
                "description": "ПОДРОБНЫЙ английский промпт кадра: кто и что делает, во что                     одет, окружение и что в нём видно, источник и характер света, план и                     ракурс камеры, палитра, настроение. Живые детали вместо общих слов;                     никаких имён собственных и никакого текста в кадре",
                "minLength": 120
            },
            "location": { "type": "string" },
            "sameLocation": { "type": "boolean" },
            "shot": { "type": "string", "enum": ["wide", "medium", "close"] },
            "characters": { "type": "array", "items": { "type": "string" } },
            "reference": {
                "type": "string",
                "enum": ["scene", "characters", "none"],
                "description": "что положить в основу кадра: scene — прошлый кадр этого же                     места, когда действие продолжается там же; characters — портреты                     названных персонажей, когда важно не потерять их внешность; none —                     новое место и новые лица, рисуем с чистого листа"
            }
        },
        // prompt и location обязательны вместе с needed: без промпта заявка на кадр
        // бесполезна, а без ярлыка места ломается непрерывность сцены.
        "required": ["needed", "prompt", "location"]
    })
}

/// Параметры сэмплирования служебных проходов: они должны быть скучными и точными, а не
/// изобретательными, поэтому температура низкая.
pub fn structured_sampling() -> Sampling {
    // Лимит с запасом: рассуждающие модели тратят часть его на скрытые размышления, и при
    // тесном лимите на сам ответ уже ничего не остаётся — проход возвращает пустоту.
    Sampling::new(0.2, 3072)
}

/// Собрать состав хода: герой первым — к нему падают объявления без явного адресата.
pub fn build_actors(
    characters: &[StoryCharacter],
    rpg: &[(String, CharacterRpg)],
    enemies: &[Enemy],
) -> ActorMap {
    let mut actors = ActorMap::new();
    for character in characters {
        let state = rpg
            .iter()
            .find(|(id, _)| id == &character.id)
            .map(|(_, state)| state.clone())
            .unwrap_or_default();
        actors.insert(character.id.clone(), Actor { name: character.name.clone(), rpg: state });
    }
    for enemy in enemies.iter().filter(|enemy| !enemy.rpg.dead) {
        actors.insert(enemy.id.clone(), Actor { name: enemy.name.clone(), rpg: enemy.rpg.clone() });
    }
    actors
}

#[cfg(test)]
mod tests {
    use super::*;
    use du_rpg::{Hp, ItemSlot};

    fn character(id: &str, name: &str) -> StoryCharacter {
        StoryCharacter {
            id: id.into(),
            chat_id: "чат".into(),
            name: name.into(),
            details: String::new(),
            inventory: String::new(),
            skills: String::new(),
            spells: String::new(),
            portrait: None,
            voice: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn item(name: &str, owner: Option<&str>) -> Item {
        Item {
            id: name.into(),
            owner_id: owner.map(str::to_string),
            name: name.into(),
            slot: ItemSlot::Weapon,
            rarity: Default::default(),
            description: None,
            damage: Some("1d8".into()),
            modifiers: Default::default(),
            equipped: true,
            qty: 1,
            image_url: None,
            image_prompt_en: None,
            created_at: String::new(),
        }
    }

    fn message(role: StoryRole, content: &str) -> StoryMessage {
        StoryMessage {
            id: String::new(),
            role,
            content: content.into(),
            created_at: String::new(),
            attachments: vec![],
            image_request: None,
            generated_image: None,
            rpg_snapshot: None,
        }
    }

    #[test]
    fn the_game_state_block_carries_ids_stats_and_foes() {
        let mut actors = ActorMap::new();
        let mut rpg = CharacterRpg::default();
        rpg.hp = Hp { current: 12, max: 20 };
        actors.insert("hero".into(), Actor { name: "Герой".into(), rpg });
        let enemies = vec![Enemy { id: "foe".into(), name: "Гоблин".into(), rpg: CharacterRpg::default() }];

        let section = build_rpg_section(&actors, &[], &enemies, &[], prompts_for(Language::Ru));
        assert!(section.contains("[ID: hero]"));
        assert!(section.contains("12/20"));
        assert!(section.contains("Гоблин"));
        assert!(section.contains("НЕ выдумывай числа"));
    }

    #[test]
    fn an_enemys_loot_is_not_presented_as_the_heros_gear() {
        let mut actors = ActorMap::new();
        actors.insert("hero".into(), Actor { name: "Герой".into(), rpg: CharacterRpg::default() });
        let items = vec![item("меч героя", Some("hero")), item("ржавый тесак", Some("враг-которого-нет"))];

        let section = build_rpg_section(&actors, &items, &[], &[], prompts_for(Language::Ru));
        assert!(section.contains("меч героя"));
        assert!(!section.contains("ржавый тесак"), "чужой лут не должен попадать в инвентарь отряда");
    }

    #[test]
    fn an_empty_party_still_gets_the_rules() {
        let section = build_rpg_section(&ActorMap::new(), &[], &[], &[], prompts_for(Language::Ru));
        assert_eq!(section, prompts_for(Language::Ru).rpg.rules);
    }

    #[test]
    fn the_language_directive_comes_after_the_narrator_prompt() {
        let settings = StorySettings { language: Language::En, ..Default::default() };
        let messages = build_story_messages(&[], "иду вперёд", &settings, &[], "", "");
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "system");
        // Второе системное сообщение обязано перебивать язык, зашитый в первое.
        assert!(messages[1].role == "system");
        assert_eq!(messages.last().unwrap().role, "user");
    }

    #[test]
    fn the_narrator_is_forbidden_to_write_image_prompts_when_pictures_are_on() {
        let settings = StorySettings { image_generation_enabled: true, ..Default::default() };
        let messages = build_story_messages(&[], "жду", &settings, &[], "", "");
        let system = format!("{:?}", messages[0]);
        assert!(system.contains("IMAGE_GEN_PROMPT"), "запрет на утечку промпта обязателен");
    }

    #[test]
    fn a_custom_narrator_prompt_replaces_the_built_in_one() {
        let settings = StorySettings {
            narrator_prompt: "Ты — суровый летописец.".into(),
            ..Default::default()
        };
        let messages = build_story_messages(&[], "жду", &settings, &[], "", "");
        let system = format!("{:?}", messages[0]);
        assert!(system.contains("суровый летописец"));
    }

    #[test]
    fn history_and_the_players_input_end_up_in_order() {
        let history = vec![
            message(StoryRole::User, "осматриваюсь"),
            message(StoryRole::Assistant, "Зал пуст."),
        ];
        let messages = build_story_messages(&history, "иду к двери", &StorySettings::default(), &[], "", "");
        let roles: Vec<&str> = messages.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, ["system", "system", "user", "assistant", "user"]);
    }

    #[test]
    fn dead_foes_are_left_out_of_the_turns_cast() {
        let characters = vec![character("hero", "Герой")];
        let mut dead = CharacterRpg::default();
        dead.dead = true;
        let enemies = vec![
            Enemy { id: "живой".into(), name: "Гоблин".into(), rpg: CharacterRpg::default() },
            Enemy { id: "мёртвый".into(), name: "Скелет".into(), rpg: dead },
        ];
        let actors = build_actors(&characters, &[], &enemies);
        assert!(actors.contains_key("живой"));
        assert!(!actors.contains_key("мёртвый"));
        assert_eq!(actors.keys().next().unwrap(), "hero", "герой обязан быть первым в составе");
    }

    #[test]
    fn the_engine_schema_never_lets_the_model_report_a_result() {
        let schema = engine_schema().to_string();
        assert!(!schema.contains("\"total\""), "итог броска считает движок, не модель");
        assert!(!schema.contains("\"success\""));
        assert!(schema.contains("\"dc\""));
    }
}
