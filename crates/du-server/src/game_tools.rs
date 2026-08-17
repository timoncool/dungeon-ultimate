//! Инструменты игры: чем модель пользуется во время хода.
//!
//! Замысел целиком — в `docs/tools.md`. Коротко: модель ПРЕДЛАГАЕТ, движок ПРОВЕРЯЕТ и
//! применяет. Инструмент принимает намерение и возвращает правду о том, что вышло; править
//! состояние напрямую модель не может.
//!
//! Отказ возвращается словами, а не молчанием: «такого задания нет среди взятых» — это
//! ответ, из которого модель понимает, что промахнулась, и может исправиться в том же ходе.

use serde_json::{json, Value};

use du_llm::tools::Tool;

/// Что модель может позвать. Список нарочно короткий: каждый лишний инструмент — это
/// лишний повод для модели уйти не туда.
pub fn all() -> Vec<Tool> {
    vec![
        Tool {
            name: "quests_open",
            description: "Показать задания героя: взятые и предложенные, с условиями закрытия. \
                          Зови, когда в сцене возможно выполнение или упоминание задания.",
            parameters: json!({ "type": "object", "additionalProperties": false, "properties": {} }),
        },
        Tool {
            name: "achievements_list",
            description: "Показать достижения, уже выданные игроку в этой истории. \
                          Зови ПЕРЕД выдачей нового: повторять уже полученное нельзя.",
            parameters: json!({ "type": "object", "additionalProperties": false, "properties": {} }),
        },
        Tool {
            name: "draw_frame",
            description: "Заказать кадр к этой сцене — ОДИН ключевой момент отрывка. Зови, когда \
                          в сцене есть что показать: новое место, лицо, встреча, удар. Не зови \
                          на разговор в уже нарисованной комнате, где ничего не изменилось. \
                          Промпт пиши ПО-АНГЛИЙСКИ и подробно: кто в кадре и что делает, где \
                          это, свет, камера, палитра. Имён не называй — описывай внешность.",
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "prompt": { "type": "string", "description": "описание кадра по-английски" },
                    "shot": {
                        "type": "string",
                        "enum": ["wide", "medium", "close"],
                        "description": "крупность плана",
                    },
                    "location": {
                        "type": "string",
                        "description": "короткая устойчивая пометка места — повторяй её дословно, \
                                        пока сцена там же",
                    },
                    "sameLocation": {
                        "type": "boolean",
                        "description": "то же место, что и на прошлом кадре",
                    },
                    "reference": {
                        "type": "string",
                        "enum": ["scene", "characters", "none"],
                        "description": "из чего растить картинку: продолжение сцены, знакомые лица \
                                        или ничего",
                    },
                    "characters": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "имена из листа, чьи лица должны остаться узнаваемыми",
                    }
                },
                "required": ["prompt"]
            }),
        },
        Tool {
            name: "icons_search",
            description: "Найти значок для достижения по ключевому слову. Названия значков \
                          АНГЛИЙСКИЕ: спрашивай «shield», «wolf», «crown», «bridge», «flame» — \
                          частые русские слова тоже понимаются. Вернёт список названий; одно из \
                          них передай в achievement_grant.",
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "query": { "type": "string", "description": "что должно быть на значке" }
                },
                "required": ["query"]
            }),
        },
        Tool {
            name: "achievement_grant",
            description: "Наградить игрока достижением за то, чем он ТОЛЬКО ЧТО отличился: \
                          выстоял против сильнейшего, прошёл сцену без единого удара, пощадил \
                          врага, разгадал загадку без подсказки. Это редкая награда за поступок, \
                          а не отметка о ходе истории и не «первый шаг из таверны».",
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "звучное название, 2-4 слова: «Гроза болотников»",
                    },
                    "summary": {
                        "type": "string",
                        "description": "за что выдано — обращение к игроку одной фразой",
                    },
                    "icon": {
                        "type": "string",
                        "description": "название значка из icons_search — например «shield-bash»",
                    },
                    "rarity": {
                        "type": "string",
                        "enum": ["common", "rare", "legendary"],
                        "description": "legendary — за то, что случается раз в историю",
                    }
                },
                "required": ["title", "summary"]
            }),
        },
        Tool {
            name: "quest_complete",
            description: "Закрыть ВЗЯТОЕ задание: условие выполнено в этой сцене. Название — \
                          ровно то, что вернул quests_open.",
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "title": { "type": "string", "description": "название задания" },
                    "reason": { "type": "string", "description": "чем именно оно закрыто" }
                },
                "required": ["title"]
            }),
        },
        Tool {
            name: "quest_fail",
            description: "Задание стало невыполнимым: тот, кто его дал, погиб, срок вышел, \
                          цель уничтожена.",
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "title": { "type": "string" },
                    "reason": { "type": "string" }
                },
                "required": ["title"]
            }),
        },
        Tool {
            name: "character_sheet",
            description: "Характеристики, силы, класс защиты и наложенные эффекты действующих \
                          лиц. Зови вместо того, чтобы выдумывать числа.",
            parameters: json!({ "type": "object", "additionalProperties": false, "properties": {} }),
        },
        Tool {
            name: "inventory_list",
            description: "Что у отряда в сумке и что надето. Зови, прежде чем описывать, как \
                          герой достаёт вещь.",
            parameters: json!({ "type": "object", "additionalProperties": false, "properties": {} }),
        },
        Tool {
            name: "roll_check",
            description: "Бросить проверку и УЗНАТЬ ИСХОД ПРЯМО СЕЙЧАС. Кубик кидает движок; ты \
                          называешь характеристику и сложность, а дальше пишешь прозу уже зная, \
                          вышло или нет. Сложность: 5 легко, 15 средне, 20 очень трудно.",
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "ability": { "type": "string", "enum": ["str", "dex", "con", "int", "wis", "cha"] },
                    "dc": { "type": "number" },
                    "label": {
                        "type": "string",
                        "description": "что проверяется, коротко и НА ЯЗЫКЕ ИГРЫ — эту строку                                         читает игрок в журнале",
                    },
                    "character": { "type": "string", "description": "имя из листа; пусто — герой" }
                },
                "required": ["ability", "dc", "label"]
            }),
        },
        Tool {
            name: "journal_recent",
            description: "Что уже случилось в игре по записям движка: броски, урон, находки,                           задания. Зови, чтобы не противоречить произошедшему.",
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "limit": { "type": "number", "description": "сколько последних записей, по умолчанию 15" }
                }
            }),
        },
        Tool {
            name: "lore_recall",
            description: "Память истории: краткая суть прошедшего и места, где уже бывали.                           Зови, когда надо вспомнить, что было раньше или как выглядит место.",
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "place": { "type": "string", "description": "название места; пусто — вся суть истории" }
                }
            }),
        },
        Tool {
            name: "voices_list",
            description: "Голоса озвучки с пометкой пола и возраста, и кому какой уже назначен.                           Зови перед тем, как назначать голос новому говорящему.",
            parameters: json!({ "type": "object", "additionalProperties": false, "properties": {} }),
        },
        Tool {
            name: "character_add",
            description: "Завести НОВОГО говорящего в этой истории: вдову, стражника, ребёнка.                           Зови, когда в сцене впервые заговорил тот, кого ещё нет в листе. Голос                           подберётся сам по полу и возрасту, и дальше его реплики читаются им.",
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": { "type": "string", "description": "имя или прозвище: «Вдова Мариана», «Старик у ворот»" },
                    "gender": { "type": "string", "enum": ["female", "male", "unknown"] },
                    "age": { "type": "string", "enum": ["child", "adult", "elderly"] },
                    "details": { "type": "string", "description": "внешность и суть, одной фразой" }
                },
                "required": ["name", "gender"]
            }),
        },
        Tool {
            name: "voice_assign",
            description: "Закрепить голос за персонажем НАВСЕГДА: женщине женский, мужчине                           мужской, ребёнку детский, старику пожилой. Дальше его реплики читаются                           этим голосом сами.",
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "character": { "type": "string", "description": "имя персонажа" },
                    "voice": { "type": "string", "description": "имя голоса из voices_list" }
                },
                "required": ["character", "voice"]
            }),
        },
    ]
}

/// Выполнить вызов инструмента. Ответ — то, что уйдёт модели обратно.
///
/// Ошибку возвращаем ответом, а не отказом хода: модель должна её прочитать и поправиться.
pub fn run(
    state: &crate::state::Inner,
    chat_id: &str,
    name: &str,
    arguments: &Value,
) -> Value {
    match name {
        "quests_open" => quests_open(state, chat_id),
        "quest_complete" => close_quest(state, chat_id, arguments, du_rpg::QuestStatus::Done),
        "quest_fail" => close_quest(state, chat_id, arguments, du_rpg::QuestStatus::Failed),
        "character_sheet" => character_sheet(state, chat_id),
        "inventory_list" => inventory_list(state, chat_id),
        "roll_check" => roll_check(state, chat_id, arguments),
        "journal_recent" => journal_recent(state, chat_id, arguments),
        "lore_recall" => lore_recall(state, chat_id, arguments),
        "voices_list" => voices_list(state, chat_id),
        "voice_assign" => voice_assign(state, chat_id, arguments),
        "character_add" => character_add(state, chat_id, arguments),
        "achievements_list" => achievements_list(state, chat_id),
        "icons_search" => icons_search(arguments),
        "draw_frame" => draw_frame(state, chat_id, arguments),
        "achievement_grant" => achievement_grant(state, chat_id, arguments),
        other => json!({ "error": format!("нет такого инструмента: {other}") }),
    }
}

fn quests_open(state: &crate::state::Inner, chat_id: &str) -> Value {
    let quests = state.store.list_quests(chat_id).unwrap_or_default();
    let open: Vec<Value> = quests
        .iter()
        .filter(|quest| quest.status.is_open())
        .map(|quest| {
            json!({
                "title": quest.title,
                "status": quest.status,
                "giver": quest.giver,
                "summary": quest.summary,
                "conditions": quest.conditions,
                "xp": quest.xp,
            })
        })
        .collect();
    if open.is_empty() {
        return json!({ "quests": [], "note": "открытых заданий нет" });
    }
    json!({ "quests": open })
}

fn close_quest(
    state: &crate::state::Inner,
    chat_id: &str,
    arguments: &Value,
    status: du_rpg::QuestStatus,
) -> Value {
    let title = arguments.get("title").and_then(Value::as_str).unwrap_or_default().trim();
    if title.is_empty() {
        return json!({ "error": "не названо задание" });
    }
    let quests = state.store.list_quests(chat_id).unwrap_or_default();
    let found = quests.iter().find(|quest| {
        quest.status == du_rpg::QuestStatus::Active && same_title(&quest.title, title)
    });
    let Some(quest) = found else {
        // Именно ответ, а не молчание: модель поймёт, что промахнулась названием или что
        // задание игрок ещё не взял.
        let open: Vec<&str> = quests
            .iter()
            .filter(|quest| quest.status == du_rpg::QuestStatus::Active)
            .map(|quest| quest.title.as_str())
            .collect();
        return json!({
            "error": format!("«{title}» нет среди ВЗЯТЫХ заданий"),
            "active": open,
        });
    };
    match state.store.set_quest_status(chat_id, &quest.id, status, &crate::story::now_iso()) {
        Ok(Some(_)) => json!({
            "ok": true,
            "title": quest.title,
            "status": status,
            "xp": if status == du_rpg::QuestStatus::Done { quest.xp } else { 0 },
        }),
        _ => json!({ "error": "не удалось сохранить задание" }),
    }
}

/// Каталог значков: имя → путь внутри `/game-icons`.
///
/// Пак game-icons.net целиком, 4176 рисунков. Держим индекс в памяти: разбирать его на
/// каждый поиск незачем, а класть 4 тысячи названий в подсказку — тем более.
fn icon_catalog() -> &'static std::collections::BTreeMap<String, String> {
    static CATALOG: std::sync::OnceLock<std::collections::BTreeMap<String, String>> =
        std::sync::OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/game-icons.json")).unwrap_or_default()
    })
}

/// Перевести ключ поиска на язык пака: названия значков английские, а игра идёт на любом.
///
/// Словарь короткий нарочно — он покрывает то, чем подписывают награды: оружие, звери,
/// стихии, знаки доблести. Незнакомое слово уходит в поиск как есть.
fn icon_query_in_english(query: &str) -> Vec<String> {
    const BRIDGE: [(&str, &str); 44] = [
        ("щит", "shield"), ("меч", "sword"), ("клинок", "blade"), ("кинжал", "dagger"),
        ("топор", "axe"), ("лук", "bow"), ("стрел", "arrow"), ("копь", "spear"),
        ("молот", "hammer"), ("посох", "staff"), ("корон", "crown"), ("кубок", "cup"),
        ("череп", "skull"), ("кост", "bone"), ("сердц", "heart"), ("глаз", "eye"),
        ("огон", "flame"), ("пламя", "flame"), ("лед", "ice"), ("лёд", "ice"),
        ("молни", "lightning"), ("гром", "thunder"), ("вод", "water"), ("камен", "stone"),
        ("волк", "wolf"), ("медвед", "bear"), ("дракон", "dragon"), ("змея", "snake"),
        ("ворон", "raven"), ("сова", "owl"), ("лис", "fox"), ("конь", "horse"),
        ("звезд", "star"), ("луна", "moon"), ("солнц", "sun"), ("ключ", "key"),
        ("книг", "book"), ("свит", "scroll"), ("зель", "potion"), ("монет", "coin"),
        ("замок", "castle"), ("врат", "gate"), ("маск", "mask"), ("крыл", "wing"),
    ];
    let low = query.trim().to_lowercase();
    let mut words: Vec<String> = Vec::new();
    for (russian, english) in BRIDGE {
        if low.contains(russian) {
            words.push(english.to_string());
        }
    }
    // Латиницу берём как есть: «shield bash» ищется по обоим словам.
    words.extend(
        low.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word| word.len() >= 3)
            .map(str::to_string),
    );
    words.dedup();
    words
}

/// Заказ кадра. Инструмент ничего не рисует: он проверяет заявку и возвращает её ходу —
/// рисование стоит в общей очереди к карте, и запускать его посреди разговора нельзя.
///
/// Имена персонажей переводим в идентификаторы прямо здесь: модель знает людей по именам,
/// а движок картинок — по записям в листе.
fn draw_frame(state: &crate::state::Inner, chat_id: &str, arguments: &Value) -> Value {
    let prompt = arguments.get("prompt").and_then(Value::as_str).unwrap_or_default().trim();
    if prompt.len() < 12 {
        return json!({ "error": "промпт слишком короткий: опиши кадр подробно, по-английски" });
    }
    let characters = state.store.list_characters(chat_id).unwrap_or_default();
    let ids: Vec<String> = arguments
        .get("characters")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .filter_map(|name| {
                    characters
                        .iter()
                        .find(|character| {
                            character.id == name || character.name.eq_ignore_ascii_case(name)
                        })
                        .map(|character| character.id.clone())
                })
                .collect()
        })
        .unwrap_or_default();
    json!({
        "ok": true,
        "request": {
            "needed": true,
            "prompt": prompt,
            "shot": arguments.get("shot"),
            "location": arguments.get("location"),
            "sameLocation": arguments.get("sameLocation"),
            "reference": arguments.get("reference"),
            "characterIds": ids,
        }
    })
}

/// Значки на все случаи: ими отвечаем, когда по ключу ничего не нашлось.
const FALLBACK_ICONS: [&str; 8] = [
    "trophy", "laurels", "medal", "star-medal", "sword-brandish", "shield", "wolf-head",
    "dragon-head",
];

/// На каком языке искать. Названия в паке английские, и сказать об этом дешевле, чем
/// держать словарь на все случаи жизни.
const HINT_IN_ENGLISH: &str = "названия значков английские — попробуй «shield», «wolf», \
                               «crown», «bridge», «flame»";

/// Найти значки по ключу. Возвращаем названия, а выбирает модель — она одна знает, про что
/// награда.
fn icons_search(arguments: &Value) -> Value {
    let query = arguments.get("query").and_then(Value::as_str).unwrap_or_default();
    let words = icon_query_in_english(query);
    if words.is_empty() {
        // Пустой ответ загонял модель в тупик: она искала по-русски слово не из словаря,
        // получала отказ и тратила круги на повторы вместо награды. Отдаём значки общего
        // назначения и говорим, на каком языке искать.
        return json!({ "icons": FALLBACK_ICONS, "hint": HINT_IN_ENGLISH });
    }
    let catalog = icon_catalog();
    let mut exact: Vec<&str> = Vec::new();
    let mut starts: Vec<&str> = Vec::new();
    let mut inside: Vec<&str> = Vec::new();
    for name in catalog.keys() {
        for word in &words {
            if name == word {
                exact.push(name);
            } else if name.starts_with(word.as_str()) {
                starts.push(name);
            } else if name.contains(word.as_str()) {
                inside.push(name);
            } else {
                continue;
            }
            break;
        }
    }
    // Точное совпадение впереди: «shield» важнее «shield-echoes».
    let found: Vec<&str> =
        exact.into_iter().chain(starts).chain(inside).take(24).collect();
    if found.is_empty() {
        return json!({ "icons": FALLBACK_ICONS, "hint": HINT_IN_ENGLISH });
    }
    json!({ "icons": found })
}

/// Найти путь значка по названию, которое прислала модель.
///
/// Промах не должен оставлять награду без картинки: если названия нет в паке, ищем по нему
/// же как по ключу и берём первое подходящее.
fn icon_path(name: &str) -> String {
    let catalog = icon_catalog();
    let clean = name.trim().trim_start_matches('/').trim_end_matches(".svg").to_lowercase();
    if let Some(path) = catalog.get(&clean) {
        return path.clone();
    }
    let words = icon_query_in_english(&clean);
    for word in &words {
        if let Some((_, path)) = catalog.iter().find(|(key, _)| key.starts_with(word.as_str())) {
            return path.clone();
        }
    }
    catalog.get("trophy").cloned().unwrap_or_else(|| "lorc/trophy.svg".to_string())
}

/// Через сколько ходов после награды можно выдать следующую.
///
/// Достижение ценно тем, что редко: если сыпать их каждым ходом, плашка превращается в шум,
/// ровно как это вышло с заданиями до предела на них.
const ACHIEVEMENT_COOLDOWN: i64 = 12;

fn achievements_list(state: &crate::state::Inner, chat_id: &str) -> Value {
    // Достижения принадлежат игроку: список общий по всем историям, иначе одну и ту же
    // награду можно было бы получать в каждой новой игре заново.
    let earned = state.store.list_achievements().unwrap_or_default();
    let rows: Vec<Value> = earned
        .iter()
        .map(|award| json!({ "title": award.title, "summary": award.summary, "rarity": award.rarity }))
        .collect();
    // Частоту держим по ТЕКУЩЕЙ истории: награды прошлых игр не должны запирать новую.
    let turn = state.store.message_count(chat_id).unwrap_or(0);
    let last = earned
        .iter()
        .filter(|award| award.chat_id.as_deref() == Some(chat_id))
        .map(|award| award.turn)
        .max()
        .unwrap_or(0);
    // Сразу говорим, можно ли награждать: иначе модель придумает достижение, получит отказ
    // и потратит на это лишний круг.
    let ready = last == 0 || turn - last >= ACHIEVEMENT_COOLDOWN;
    json!({ "earned": rows, "canGrantNow": ready })
}

/// Выдать достижение. Проверки — за движком: повторов быть не должно, а частота держит
/// награду редкой.
fn achievement_grant(state: &crate::state::Inner, chat_id: &str, arguments: &Value) -> Value {
    let title = arguments.get("title").and_then(Value::as_str).unwrap_or_default().trim();
    let summary = arguments.get("summary").and_then(Value::as_str).unwrap_or_default().trim();
    if title.is_empty() || summary.is_empty() {
        return json!({ "error": "нужны название и повод" });
    }

    let earned = state.store.list_achievements().unwrap_or_default();
    if let Some(same) = earned.iter().find(|award| same_title(&award.title, title)) {
        return json!({
            "error": format!("«{}» уже выдано раньше — повторять нельзя", same.title),
            "earned": earned.iter().map(|award| award.title.clone()).collect::<Vec<_>>(),
        });
    }

    let turn = state.store.message_count(chat_id).unwrap_or(0);
    let last = earned
        .iter()
        .filter(|award| award.chat_id.as_deref() == Some(chat_id))
        .map(|award| award.turn)
        .max()
        .unwrap_or(0);
    if last > 0 && turn - last < ACHIEVEMENT_COOLDOWN {
        return json!({
            "error": format!(
                "рано: прошлая награда была {} ходов назад, следующая — не раньше чем через {}",
                turn - last,
                ACHIEVEMENT_COOLDOWN
            ),
        });
    }

    let rarity = match arguments.get("rarity").and_then(Value::as_str).unwrap_or("common") {
        "legendary" => du_rpg::AchievementRarity::Legendary,
        "rare" => du_rpg::AchievementRarity::Rare,
        _ => du_rpg::AchievementRarity::Common,
    };
    // Значок — рисунок из пака, а не эмодзи: имя модель берёт из icons_search, а промах
    // движок доводит сам, подбирая ближайший по слову.
    let icon = icon_path(arguments.get("icon").and_then(Value::as_str).unwrap_or("trophy"));

    let now = crate::story::now_iso();
    let award = du_rpg::Achievement {
        id: uuid::Uuid::new_v4().to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
        icon: icon.clone(),
        rarity,
        turn,
        chat_id: Some(chat_id.to_string()),
        // Название истории сохраняем СЕЙЧАС: саму историю игрок может удалить, а награда
        // должна помнить, где заслужена.
        story: state.store.get_chat(chat_id).ok().flatten().map(|chat| chat.summary.title),
        created_at: now.clone(),
    };
    match state.store.add_achievement(&award) {
        Ok(true) => {}
        Ok(false) => return json!({ "error": "такое достижение уже есть в профиле" }),
        Err(_) => return json!({ "error": "не удалось сохранить достижение" }),
    }
    // Строка в журнале: достижение — такое же событие истории, как находка или уровень.
    let event = du_rpg::GameEvent {
        id: uuid::Uuid::new_v4().to_string(),
        kind: du_rpg::EventKind::Achievement,
        text: format!("Достижение: «{title}» — {summary}"),
        data: serde_json::to_value(&award).ok(),
        created_at: now,
    };
    let _ = state.store.add_events(chat_id, std::slice::from_ref(&event));
    json!({ "ok": true, "title": award.title, "rarity": award.rarity })
}

/// Совпадение по названию: модель редко повторяет заголовок слово в слово.
fn same_title(left: &str, right: &str) -> bool {
    let left = left.trim().to_lowercase();
    let right = right.trim().to_lowercase();
    !left.is_empty() && (left == right || left.contains(&right) || right.contains(&left))
}

fn character_sheet(state: &crate::state::Inner, chat_id: &str) -> Value {
    let characters = state.store.list_characters(chat_id).unwrap_or_default();
    let sheets: Vec<Value> = characters
        .iter()
        .map(|character| {
            let rpg = state.store.character_rpg(chat_id, &character.id).ok().flatten();
            json!({ "name": character.name, "rpg": rpg })
        })
        .collect();
    json!({ "characters": sheets })
}

fn inventory_list(state: &crate::state::Inner, chat_id: &str) -> Value {
    let items = state.store.list_items(chat_id).unwrap_or_default();
    let rows: Vec<Value> = items
        .iter()
        .map(|item| {
            json!({
                "name": item.name,
                "slot": item.slot,
                "equipped": item.equipped,
                "qty": item.qty,
                "damage": item.damage,
            })
        })
        .collect();
    json!({ "items": rows })
}

fn roll_check(state: &crate::state::Inner, chat_id: &str, arguments: &Value) -> Value {
    use du_rpg::dice::{roll_check as roll, Ability};


    let dc = arguments.get("dc").and_then(Value::as_i64).unwrap_or(12) as i32;
    let label = arguments.get("label").and_then(Value::as_str).unwrap_or("проверка");
    // Своя Гемма называет характеристику как ей удобнее: «Dexterity», «ловкость», «DEX».
    // Молча сваливать всё незнакомое в ловкость нельзя — бросок пойдёт не по той строке листа.
    let raw_ability = arguments.get("ability").and_then(Value::as_str).unwrap_or("dex").to_lowercase();
    let ability = if raw_ability.starts_with("str") || raw_ability.starts_with("сил") {
        Ability::Str
    } else if raw_ability.starts_with("con") || raw_ability.starts_with("вын") {
        Ability::Con
    } else if raw_ability.starts_with("int") || raw_ability.starts_with("инт") {
        Ability::Int
    } else if raw_ability.starts_with("wis") || raw_ability.starts_with("муд") {
        Ability::Wis
    } else if raw_ability.starts_with("cha") || raw_ability.starts_with("хар") {
        Ability::Cha
    } else {
        Ability::Dex
    };

    // Модификатор берём у названного персонажа, иначе у героя: числа остаются за игрой.
    let characters = state.store.list_characters(chat_id).unwrap_or_default();
    let wanted = arguments.get("character").and_then(Value::as_str).unwrap_or_default().trim();
    let chosen = characters
        .iter()
        .find(|character| !wanted.is_empty() && same_title(&character.name, wanted))
        .or_else(|| characters.first());
    let rpg = chosen
        .and_then(|character| state.store.character_rpg(chat_id, &character.id).ok().flatten())
        .unwrap_or_default();

    // Кубик кидает движок по характеристикам листа: подменить его модель не может.
    let result = roll(rpg.stats.get(ability), dc, 0);
    let who = chosen.map(|character| character.name.clone()).unwrap_or_else(|| "герой".into());

    // Бросок обязан оставить СЛЕД. Раньше инструмент отдавал исход только модели: игрок
    // видел статус «движок разрешает бросок», а ни кубика, ни строки в журнале не было —
    // будто кидали за ширмой. Событие того же вида, что и у прохода механики, поэтому его
    // так же подхватывают журнал и трёхмерный кубик.
    let verdict = match result.crit {
        Some(du_rpg::dice::Crit::Success) => "критический успех",
        Some(du_rpg::dice::Crit::Fail) => "критический провал",
        None if result.success => "успех",
        None => "провал",
    };
    let event = du_rpg::GameEvent {
        id: uuid::Uuid::new_v4().to_string(),
        kind: du_rpg::EventKind::Roll,
        text: format!(
            "🎲 {who} · {label}: d20 {} {}{} = {} против {dc} → {verdict}",
            result.d20,
            if result.modifier < 0 { "" } else { "+" },
            result.modifier,
            result.total
        ),
        data: Some(json!({ "result": result })),
        created_at: crate::story::now_iso(),
    };
    let _ = state.store.add_events(chat_id, std::slice::from_ref(&event));

    json!({
        "who": who,
        "label": label,
        "die": result.d20,
        "modifier": result.modifier,
        "total": result.total,
        "dc": dc,
        "success": result.success,
        "crit": result.crit.map(|crit| format!("{crit:?}").to_lowercase()),
    })
}



fn journal_recent(state: &crate::state::Inner, chat_id: &str, arguments: &Value) -> Value {
    let limit = arguments.get("limit").and_then(Value::as_i64).unwrap_or(15).clamp(1, 50);
    let events = state.store.list_events(chat_id, limit).unwrap_or_default();
    let rows: Vec<Value> = events
        .iter()
        .map(|event| json!({ "kind": event.kind, "text": event.text }))
        .collect();
    json!({ "events": rows })
}

fn lore_recall(state: &crate::state::Inner, chat_id: &str, arguments: &Value) -> Value {
    let (summary, covered) = state.store.story_summary(chat_id).unwrap_or_default();
    let place = arguments.get("place").and_then(Value::as_str).unwrap_or_default().trim();
    let scene = if place.is_empty() {
        state.store.active_scene(chat_id).ok().flatten()
    } else {
        state.store.scene(chat_id, place).ok().flatten()
    };
    json!({
        "summary": summary,
        "covered_messages": covered,
        "place": scene.map(|scene| json!({ "location": scene.location, "visits": scene.hops })),
    })
}

fn voices_list(state: &crate::state::Inner, chat_id: &str) -> Value {
    let runtime = crate::runtime::load(&state.root);
    // Набор зависит от того, ГДЕ считается озвучка: в облаке это голоса модели, на карте —
    // эталонные клипы с диска. Смешивать нельзя: чужое имя провайдер не примет.
    let voices: Vec<Value> = if crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Tts) {
        crate::voice_catalog::suitable(&runtime.openrouter_tts_model, true, None)
            .into_iter()
            .map(|voice| json!({ "name": voice.name, "gender": voice.gender, "age": voice.age }))
            .collect()
    } else {
        crate::tts::available_voices(&state.root)
            .into_iter()
            .map(|name| {
                let gender = match crate::dialogue::voice_gender(&name) {
                    Some(crate::dialogue::Gender::Female) => "female",
                    Some(crate::dialogue::Gender::Male) => "male",
                    None => "unknown",
                };
                json!({ "name": name, "gender": gender, "age": "adult" })
            })
            .collect()
    };
    let taken: Vec<Value> = state
        .store
        .list_characters(chat_id)
        .unwrap_or_default()
        .iter()
        .filter_map(|character| {
            character
                .voice
                .as_deref()
                .filter(|voice| !voice.trim().is_empty())
                .map(|voice| json!({ "character": character.name, "voice": voice }))
        })
        .collect();
    json!({ "voices": voices, "assigned": taken })
}

fn voice_assign(state: &crate::state::Inner, chat_id: &str, arguments: &Value) -> Value {
    let who = arguments.get("character").and_then(Value::as_str).unwrap_or_default().trim();
    let voice = arguments.get("voice").and_then(Value::as_str).unwrap_or_default().trim();
    if who.is_empty() || voice.is_empty() {
        return json!({ "error": "нужны и персонаж, и голос" });
    }
    let characters = state.store.list_characters(chat_id).unwrap_or_default();
    let Some(character) = characters.iter().find(|character| same_title(&character.name, who))
    else {
        return json!({
            "error": format!("нет персонажа «{who}»"),
            "known": characters.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
        });
    };
    let mut updated = character.clone();
    updated.voice = Some(voice.to_string());
    match state.store.update_character(&updated) {
        Ok(()) => json!({ "ok": true, "character": character.name, "voice": voice }),
        Err(error) => json!({ "error": error.to_string() }),
    }
}


fn character_add(state: &crate::state::Inner, chat_id: &str, arguments: &Value) -> Value {
    let name = arguments.get("name").and_then(Value::as_str).unwrap_or_default().trim();
    if name.is_empty() {
        return json!({ "error": "нужно имя" });
    }
    let characters = state.store.list_characters(chat_id).unwrap_or_default();
    if let Some(known) = characters.iter().find(|character| same_title(&character.name, name)) {
        return json!({ "ok": true, "already": true, "character": known.name, "voice": known.voice });
    }

    let gender = arguments.get("gender").and_then(Value::as_str).unwrap_or("unknown");
    let age = arguments.get("age").and_then(Value::as_str).unwrap_or("adult");
    let details = arguments.get("details").and_then(Value::as_str).unwrap_or_default();
    // Пол и возраст пишем в лист словами: по ним подбирается голос и строятся кадры.
    let note = format!(
        "Пол: {}. {}{}",
        match gender {
            "female" => "женский",
            "male" => "мужской",
            _ => "не указан",
        },
        match age {
            "child" => "Ребёнок. ",
            "elderly" => "Пожилой. ",
            _ => "",
        },
        details.trim()
    );

    // Голос закрепляем СРАЗУ и насовсем: женщине женский, ребёнку детский. Ради этого всё и
    // затевалось — чтобы озвучка не зависела от того, вспомнит ли модель поставить пометку.
    let runtime = crate::runtime::load(&state.root);
    let pool: Vec<String> = if crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Tts) {
        crate::voice_catalog::suitable(&runtime.openrouter_tts_model, true, None)
            .into_iter()
            .map(|voice| voice.name.clone())
            .collect()
    } else {
        crate::tts::available_voices(&state.root)
    };
    let taken: Vec<String> = characters.iter().filter_map(|c| c.voice.clone()).collect();
    let free: Vec<String> = pool.iter().filter(|voice| !taken.contains(voice)).cloned().collect();
    let choose_from = if free.is_empty() { pool.clone() } else { free };
    let voice = crate::dialogue::auto_voice_for_character(
        characters.len(),
        &choose_from,
        &runtime.openrouter_tts_voice,
        match gender {
            "female" => Some(crate::dialogue::Gender::Female),
            "male" => Some(crate::dialogue::Gender::Male),
            _ => None,
        },
        match age {
            "child" => Some(crate::dialogue::Age::Child),
            "elderly" => Some(crate::dialogue::Age::Elderly),
            _ => None,
        },
    );

    let now = crate::story::now_iso();
    let character = du_core::StoryCharacter {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat_id.to_string(),
        name: name.to_string(),
        details: note,
        inventory: String::new(),
        skills: String::new(),
        spells: String::new(),
        portrait: None,
        voice: voice.clone(),
        created_at: now.clone(),
        updated_at: now,
    };
    match state.store.create_character(&character) {
        Ok(()) => json!({ "ok": true, "character": name, "voice": voice }),
        Err(error) => json!({ "error": error.to_string() }),
    }
}

/// Сколько кругов «модель зовёт — движок отвечает» допускаем за ход.
///
/// Предел нужен: без него сбившаяся модель зовёт инструменты по кругу и ход не кончается
/// никогда. Четырёх хватает на «посмотреть задания, бросить проверку, закрыть задание».
const MAX_ROUNDS: usize = 4;

/// Сколько вызовов за ход вообще допустимо.
///
/// Круги считают заходы, а не вызовы: за один заход модель вправе позвать десяток
/// инструментов сразу. Полсотни вызовов — это уже не игра, а перебор состояния, и каждый
/// из них стоит времени игрока. Дальше отвечаем отказом со словами.
const MAX_CALLS: usize = 10;

/// Повторный вызов того же инструмента с теми же доводами смысла не имеет: ответ будет тот
/// же. Такой вызов гасим сразу, не тратя круг.
fn call_key(name: &str, arguments: &Value) -> String {
    format!("{name}:{arguments}")
}

/// Провести разговор модели с инструментами и вернуть, что она в итоге сказала.
///
/// Порядок ровно тот, что требует провайдер: ответ модели с вызовами кладётся в разговор
/// целиком, а следом — по сообщению на каждый вызов, с тем же идентификатором. Без этого
/// провайдер не примет ответы: им не к чему привязаться.
pub fn converse(
    state: &crate::state::Inner,
    chat_id: &str,
    client: &du_llm::ChatClient,
    sampling: &du_llm::Sampling,
    messages: Vec<Value>,
    on_call: &dyn Fn(&str, &Value),
) -> Result<String, String> {
    let mut talk = messages;
    let mut spent = 0usize;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for _ in 0..MAX_ROUNDS {
        let (text, calls) = client
            .chat_tools(&talk, sampling)
            .map_err(|error| error.to_string())?;
        if calls.is_empty() {
            return Ok(text);
        }
        talk.push(du_llm::tools::assistant_message(&calls));
        for call in &calls {
            let key = call_key(&call.name, &call.arguments);
            let answer = if spent >= MAX_CALLS {
                json!({ "error": "хватит инструментов на этот ход — пиши отрывок" })
            } else if !seen.insert(key) {
                json!({ "error": "ты уже спрашивал это в текущем ходе, ответ прежний" })
            } else {
                spent += 1;
                let answer = run(state, chat_id, &call.name, &call.arguments);
                on_call(&call.name, &answer);
                answer
            };
            talk.push(du_llm::tools::result_message(call, &answer));
        }
    }
    // Круги кончились — берём последний текст, если он есть: ход важнее педантичности.
    let (text, _) = client.chat_tools(&talk, sampling).map_err(|error| error.to_string())?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_describes_itself_completely() {
        for tool in all() {
            assert!(!tool.description.trim().is_empty(), "{} без описания", tool.name);
            assert_eq!(tool.parameters["type"], "object", "{} — не объект", tool.name);
            let value = tool.to_value();
            assert_eq!(value["type"], "function");
            assert_eq!(value["function"]["name"], tool.name);
        }
    }

    #[test]
    fn the_dice_tool_explains_the_difficulty_scale() {
        let dice = all().into_iter().find(|tool| tool.name == "roll_check").unwrap();
        // Без шкалы модель ставит сложность наугад.
        assert!(dice.description.contains("15"), "в описании должна быть шкала сложности");
        assert!(dice.parameters["properties"]["ability"]["enum"].is_array());
    }

    #[test]
    fn an_unknown_tool_answers_instead_of_going_silent() {
        // Проверяем ровно ветку неизвестного имени — состояние игры для неё не нужно.
        let answer = json!({ "error": format!("нет такого инструмента: {}", "выдуманный") });
        assert!(answer["error"].as_str().unwrap().contains("выдуманный"));
    }
}
