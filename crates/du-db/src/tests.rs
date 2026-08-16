use super::*;
use du_rpg::{EventKind, ItemSlot};

fn store() -> Store {
    Store::open_in_memory().expect("база в памяти обязана открыться")
}

fn chat(store: &Store) -> String {
    store.create_chat(StorySettings::default(), "Тест").expect("чат создаётся")
}

fn message(id: &str, role: StoryRole, content: &str, created_at: &str) -> StoryMessage {
    StoryMessage {
        id: id.into(),
        role,
        content: content.into(),
        created_at: created_at.into(),
        attachments: vec![],
        image_request: None,
        generated_image: None,
        rpg_snapshot: None,
    }
}

fn attachment(url: &str) -> Attachment {
    Attachment {
        id: url.into(),
        name: url.into(),
        kind: "image/png".into(),
        url: url.into(),
        data_url: None,
    }
}

fn item(id: &str, slot: ItemSlot, owner: Option<&str>) -> Item {
    Item {
        id: id.into(),
        owner_id: owner.map(str::to_string),
        name: id.into(),
        slot,
        rarity: Default::default(),
        description: None,
        damage: None,
        modifiers: Default::default(),
        equipped: false,
        qty: 1,
        image_url: None,
        image_prompt_en: None,
        created_at: "2026-01-01T00:00:00.000Z".into(),
    }
}

#[test]
fn schema_creation_is_idempotent() {
    let store = store();
    // Повторный прогон не должен падать на уже существующих колонках.
    schema::ensure(&store.lock()).expect("повторное создание схемы безопасно");
}

#[test]
fn a_chat_round_trips_with_its_settings() {
    let store = store();
    let mut settings = StorySettings::default();
    settings.world = "Подземелье".into();
    settings.rpg_enabled = true;
    let id = store.create_chat(settings, "Первая история").unwrap();

    let chat = store.get_chat(&id).unwrap().expect("чат читается");
    assert_eq!(chat.summary.title, "Первая история");
    assert_eq!(chat.summary.settings.world, "Подземелье");
    assert!(chat.messages.is_empty());

    assert!(store.get_chat("нет-такого").unwrap().is_none());
}

#[test]
fn messages_come_back_in_chronological_order() {
    let store = store();
    let chat_id = chat(&store);
    store.add_message(&chat_id, &message("m2", StoryRole::Assistant, "второе", "2026-01-02T00:00:00.000Z")).unwrap();
    store.add_message(&chat_id, &message("m1", StoryRole::User, "первое", "2026-01-01T00:00:00.000Z")).unwrap();

    let messages = store.list_messages(&chat_id).unwrap();
    assert_eq!(messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["m1", "m2"]);
}

#[test]
fn deleting_a_message_also_drops_everything_after_it() {
    let store = store();
    let chat_id = chat(&store);
    for (index, id) in ["m1", "m2", "m3"].iter().enumerate() {
        let created = format!("2026-01-0{}T00:00:00.000Z", index + 1);
        store.add_message(&chat_id, &message(id, StoryRole::User, id, &created)).unwrap();
    }

    assert!(store.delete_message_and_after("m2", true).unwrap());
    let left: Vec<String> = store.list_messages(&chat_id).unwrap().into_iter().map(|m| m.id).collect();
    assert_eq!(left, ["m1"], "ход обязан сноситься целиком, иначе Повтор задвоит последствия");

    assert!(!store.delete_message_and_after("нет-такого", true).unwrap());
}

#[test]
fn deleting_a_single_message_keeps_the_rest() {
    let store = store();
    let chat_id = chat(&store);
    for (index, id) in ["m1", "m2", "m3"].iter().enumerate() {
        let created = format!("2026-01-0{}T00:00:00.000Z", index + 1);
        store.add_message(&chat_id, &message(id, StoryRole::User, id, &created)).unwrap();
    }
    store.delete_message_and_after("m2", false).unwrap();
    let left: Vec<String> = store.list_messages(&chat_id).unwrap().into_iter().map(|m| m.id).collect();
    assert_eq!(left, ["m1", "m3"]);
}

#[test]
fn the_first_scene_image_anchors_and_later_ones_count_as_edits() {
    let store = store();
    let chat_id = chat(&store);

    store.record_scene_image(&chat_id, "The Green Meadow.", &attachment("/1.png"), true).unwrap();
    let scene = store.active_scene(&chat_id).unwrap().expect("сцена стала активной");
    assert_eq!(scene.location, "green meadow", "ярлык обязан нормализоваться в ключ");
    assert_eq!(scene.hops, 0);
    assert_eq!(scene.anchor.as_ref().unwrap().url, "/1.png");

    store.record_scene_image(&chat_id, "green meadow", &attachment("/2.png"), false).unwrap();
    let scene = store.active_scene(&chat_id).unwrap().unwrap();
    assert_eq!(scene.hops, 1);
    assert_eq!(scene.anchor.as_ref().unwrap().url, "/1.png", "якорь не двигается правкой");
    assert_eq!(scene.last.as_ref().unwrap().url, "/2.png");
}

#[test]
fn re_anchoring_resets_the_edit_counter() {
    let store = store();
    let chat_id = chat(&store);
    store.record_scene_image(&chat_id, "крипта", &attachment("/1.png"), true).unwrap();
    for index in 2..=4 {
        store.record_scene_image(&chat_id, "крипта", &attachment(&format!("/{index}.png")), false).unwrap();
    }
    assert_eq!(store.scene(&chat_id, "крипта").unwrap().unwrap().hops, 3);

    store.record_scene_image(&chat_id, "крипта", &attachment("/свежий.png"), true).unwrap();
    let scene = store.scene(&chat_id, "крипта").unwrap().unwrap();
    assert_eq!(scene.hops, 0);
    assert_eq!(scene.anchor.as_ref().unwrap().url, "/свежий.png");
}

#[test]
fn moving_to_another_location_keeps_the_previous_one_for_a_return() {
    let store = store();
    let chat_id = chat(&store);
    store.record_scene_image(&chat_id, "таверна", &attachment("/таверна.png"), true).unwrap();
    store.record_scene_image(&chat_id, "лес", &attachment("/лес.png"), true).unwrap();

    assert_eq!(store.active_scene(&chat_id).unwrap().unwrap().location, "лес");
    let tavern = store.scene(&chat_id, "таверна").unwrap().expect("прошлое место обязано сохраниться");
    assert_eq!(tavern.anchor.as_ref().unwrap().url, "/таверна.png");
}

#[test]
fn an_empty_location_label_is_not_tracked() {
    let store = store();
    let chat_id = chat(&store);
    store.record_scene_image(&chat_id, "   ", &attachment("/1.png"), true).unwrap();
    assert!(store.active_scene(&chat_id).unwrap().is_none());
}

#[test]
fn a_character_reference_falls_back_to_the_player_set_portrait() {
    let store = store();
    let chat_id = chat(&store);
    let mut hero = StoryCharacter {
        id: "hero".into(),
        chat_id: chat_id.clone(),
        name: "Герой".into(),
        details: String::new(),
        inventory: String::new(),
        skills: String::new(),
        spells: String::new(),
        portrait: Some(attachment("/портрет.png")),
        voice: None,
        created_at: "2026-01-01T00:00:00.000Z".into(),
        updated_at: "2026-01-01T00:00:00.000Z".into(),
    };
    store.create_character(&hero).unwrap();

    let reference = store.character_reference(&chat_id, "hero").unwrap().unwrap();
    assert_eq!(reference.url, "/портрет.png", "до первой сцены референс — это портрет");

    store.set_character_reference(&chat_id, "hero", &attachment("/сцена.png")).unwrap();
    let reference = store.character_reference(&chat_id, "hero").unwrap().unwrap();
    assert_eq!(reference.url, "/сцена.png", "нарисованная сцена обновляет облик");

    hero.name = "Герой-2".into();
    store.update_character(&hero).unwrap();
    assert_eq!(store.hero(&chat_id).unwrap().unwrap().name, "Герой-2");
}

#[test]
fn equipping_an_item_unequips_the_slot_for_the_same_owner_only() {
    let store = store();
    let chat_id = chat(&store);
    let mut first = item("меч", ItemSlot::Weapon, Some("hero"));
    first.equipped = true;
    let second = item("топор", ItemSlot::Weapon, Some("hero"));
    let companion = {
        let mut companion = item("посох", ItemSlot::Weapon, Some("спутник"));
        companion.equipped = true;
        companion
    };
    store.add_items(&chat_id, &[first, second, companion]).unwrap();

    store.set_item_equipped(&chat_id, "топор", true).unwrap();
    let items = store.list_items(&chat_id).unwrap();
    let by = |id: &str| items.iter().find(|item| item.id == id).unwrap().equipped;
    assert!(by("топор"));
    assert!(!by("меч"), "второй предмет в том же слоте обязан сняться");
    assert!(by("посох"), "чужая экипировка трогаться не должна");
}

#[test]
fn an_item_portrait_can_be_attached_by_name() {
    let store = store();
    let chat_id = chat(&store);
    store.add_items(&chat_id, &[item("Ржавый меч", ItemSlot::Weapon, None)]).unwrap();

    let updated = store.set_item_image(&chat_id, "ржавый меч", "/предмет.png").unwrap();
    assert_eq!(updated.unwrap().image_url.as_deref(), Some("/предмет.png"));
    assert!(store.set_item_image(&chat_id, "щит", "/x.png").unwrap().is_none());
}

#[test]
fn the_journal_reads_oldest_first_even_though_the_newest_are_selected() {
    let store = store();
    let chat_id = chat(&store);
    let events: Vec<GameEvent> = (1..=3)
        .map(|index| GameEvent {
            id: format!("e{index}"),
            kind: EventKind::Roll,
            text: format!("бросок {index}"),
            data: None,
            created_at: format!("2026-01-0{index}T00:00:00.000Z"),
        })
        .collect();
    store.add_events(&chat_id, &events).unwrap();

    let read = store.list_events(&chat_id, 200).unwrap();
    assert_eq!(read.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["e1", "e2", "e3"]);

    // Лимит отсекает СТАРЫЕ записи, оставляя свежий хвост.
    let tail = store.list_events(&chat_id, 2).unwrap();
    assert_eq!(tail.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["e2", "e3"]);

    store.delete_events(&["e2".into()]).unwrap();
    assert_eq!(store.list_events(&chat_id, 200).unwrap().len(), 2);
}

#[test]
fn rpg_state_and_combatants_survive_a_round_trip() {
    let store = store();
    let chat_id = chat(&store);
    assert!(!store.rpg_state(&chat_id).unwrap().enabled, "по умолчанию режим выключен");

    store.set_rpg_state(&chat_id, &RpgState { enabled: true }).unwrap();
    assert!(store.rpg_state(&chat_id).unwrap().enabled);

    let enemies = vec![Enemy { id: "e1".into(), name: "Гоблин".into(), rpg: CharacterRpg::default() }];
    store.set_combatants(&chat_id, &enemies).unwrap();
    assert_eq!(store.combatants(&chat_id).unwrap()[0].name, "Гоблин");
}

#[test]
fn deleting_a_chat_takes_its_messages_characters_and_items_with_it() {
    let store = store();
    let chat_id = chat(&store);
    store.add_message(&chat_id, &message("m1", StoryRole::User, "текст", "2026-01-01T00:00:00.000Z")).unwrap();
    store.add_items(&chat_id, &[item("меч", ItemSlot::Weapon, None)]).unwrap();
    store.record_scene_image(&chat_id, "лес", &attachment("/1.png"), true).unwrap();

    store.delete_chat(&chat_id).unwrap();
    assert!(store.get_chat(&chat_id).unwrap().is_none());
    assert!(store.list_messages(&chat_id).unwrap().is_empty());
    assert!(store.list_items(&chat_id).unwrap().is_empty());
    assert!(store.scene(&chat_id, "лес").unwrap().is_none());
}

#[test]
fn a_corrupt_json_column_degrades_instead_of_failing_the_chat() {
    let store = store();
    let chat_id = chat(&store);
    store
        .lock()
        .execute("UPDATE chats SET settings_json = 'не json' WHERE id = ?", params![chat_id])
        .unwrap();

    let chat = store.get_chat(&chat_id).unwrap().expect("чат обязан открыться");
    assert!(chat.summary.settings.image_generation_enabled, "настройки падают на значения по умолчанию");
}
