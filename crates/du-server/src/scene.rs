//! Непрерывность сцены: решение «рисовать заново или править прошлый кадр» и сбор
//! референсов.
//!
//! Нарратор помечает каждый кадр устойчивым ярлыком места и подсказкой «то же место».
//! Если кадр продолжает активную сцену и правок ещё не слишком много, движок ЭВОЛЮЦИОНИРУЕТ
//! предыдущую картинку; возвращение в знакомое место берёт за основу его якорь; новое место
//! рисуется с нуля. Иначе локация и герой плывут от хода к ходу.

use du_core::{normalize_location, Attachment, ImageRequest};
use du_db::{DbResult, Store, MAX_EDIT_HOPS};
use du_rpg::Item;

/// Сколько всего референсов уезжает в движок: сначала персонаж, затем сцена, затем предмет.
/// Больше трёх модель начинает путать источники.
pub const MAX_REFERENCES: usize = 3;

#[derive(Debug, Default)]
pub struct SceneDecision {
    /// Ключ локации, за которой закрепится кадр. Пусто — кадр вне сцен.
    pub location: String,
    /// Кадр, который надо эволюционировать. None — рисуем с нуля.
    pub edit_from: Option<Attachment>,
    /// Сколько правок уже накоплено до этой.
    pub prior_hops: u32,
    pub references: Vec<Attachment>,
}

impl SceneDecision {
    pub fn is_edit(&self) -> bool {
        self.edit_from.is_some()
    }
}

/// Решить, править или рисовать заново, и собрать референсы.
pub fn decide(
    store: &Store,
    chat_id: &str,
    request: Option<&ImageRequest>,
    prompt: &str,
    client_references: &[Attachment],
) -> DbResult<SceneDecision> {
    let requested = request
        .and_then(|request| request.location.as_deref())
        .map(normalize_location)
        .filter(|key| !key.is_empty());
    let same_location_hint = request.and_then(|request| request.same_location) == Some(true);

    let mut decision = SceneDecision::default();
    let active = store.active_scene(chat_id)?;

    // Кадр продолжает активную сцену, если ярлык совпал ИЛИ нарратор прямо сказал «то же
    // место» и у сцены уже есть с чего расти.
    let continues_active = match (&active, &requested) {
        (Some(scene), Some(key)) => &scene.location == key,
        (Some(scene), None) => same_location_hint && (scene.last.is_some() || scene.anchor.is_some()),
        _ => false,
    };

    if let (true, Some(scene)) = (continues_active, active.as_ref()) {
        decision.location = scene.location.clone();
        decision.prior_hops = scene.hops;
        let prior = scene.last.clone().or_else(|| scene.anchor.clone());
        // Дойдя до предела правок, переустанавливаем место свежим кадром: иначе на
        // картинке копится дрейф от повторного редактирования.
        if scene.hops < MAX_EDIT_HOPS as u32 {
            decision.edit_from = prior;
        }
    } else if let Some(key) = requested {
        decision.location = key.clone();
        // Возвращение в знакомое место растёт из его якоря, а не из последней правки:
        // якорь — это чистый установочный кадр.
        if let Some(scene) = store.scene(chat_id, &key)? {
            decision.edit_from = scene.anchor;
        }
    }

    // Что класть в основу, решает ОПЕРАТОР: он видел отрывок целиком. `none` — новое место
    // и новые лица, рисуем с чистого листа; `characters` — важна внешность, а не место;
    // `scene` — действие продолжается там же. Не сказал ничего — работаем как раньше, по
    // совпадению места.
    let asked = request.and_then(|request| request.reference.as_deref());
    let want_characters = !matches!(asked, Some("none") | Some("scene"));
    let want_scene = !matches!(asked, Some("none") | Some("characters"));
    if !want_scene {
        decision.edit_from = None;
    }

    // (1) Персонажи идут первыми: именно внешность плывёт сильнее всего.
    let hero = store.hero(chat_id)?;
    let mut character_ids: Vec<String> =
        request.map(|request| request.character_ids.clone()).unwrap_or_default();
    if character_ids.is_empty() {
        if let Some(hero) = hero.as_ref() {
            character_ids.push(hero.id.clone());
        }
    }
    for id in &character_ids {
        if !want_characters || decision.references.len() >= MAX_REFERENCES {
            break;
        }
        if let Some(reference) = store.character_reference(chat_id, id)? {
            push_unique(&mut decision.references, reference);
        }
    }

    // (2) Предыдущий кадр места.
    if let Some(previous) = decision.edit_from.clone() {
        push_unique(&mut decision.references, previous);
    }

    // (3) Портреты названных в промпте предметов — чтобы знакомая вещь выглядела так же.
    for item in items_mentioned_in(store, chat_id, prompt)? {
        if decision.references.len() >= MAX_REFERENCES {
            break;
        }
        if let Some(url) = item.image_url.clone() {
            push_unique(
                &mut decision.references,
                Attachment {
                    id: format!("item-{}", item.id),
                    name: item.name.clone(),
                    kind: "image/png".into(),
                    url,
                    data_url: None,
                },
            );
        }
    }

    // (4) Остаток слотов — под то, что приложил игрок в этом ходу.
    for reference in client_references {
        if decision.references.len() >= MAX_REFERENCES {
            break;
        }
        push_unique(&mut decision.references, reference.clone());
    }

    Ok(decision)
}

fn push_unique(references: &mut Vec<Attachment>, next: Attachment) {
    let duplicate = references
        .iter()
        .any(|existing| existing.url == next.url || existing.id == next.id);
    if !duplicate {
        references.push(next);
    }
}

/// Предметы чата, чьи имена упомянуты в промпте; длинное имя выигрывает у короткого,
/// потому что слотов референсов мало.
pub fn items_mentioned_in(store: &Store, chat_id: &str, prompt: &str) -> DbResult<Vec<Item>> {
    let haystack = prompt.to_lowercase();
    let mut items: Vec<Item> = store
        .list_items(chat_id)?
        .into_iter()
        .filter(|item| mentions_name(&haystack, &item.name))
        .collect();
    items.sort_by_key(|item| std::cmp::Reverse(item.name.chars().count()));
    Ok(items)
}

/// Упомянуто ли имя. Для латиницы требуем границу слова, чтобы короткий «axe» не срабатывал
/// внутри «axed»; для кириллицы берём подстроку с порогом длины — границы слов там ненадёжны.
fn mentions_name(haystack_lower: &str, name: &str) -> bool {
    let needle = name.trim().to_lowercase();
    if needle.chars().count() < 3 {
        return false;
    }
    if needle.is_ascii() {
        let mut from = 0usize;
        while let Some(found) = haystack_lower[from..].find(&needle) {
            let start = from + found;
            let end = start + needle.len();
            let before_ok = start == 0
                || !haystack_lower[..start]
                    .chars()
                    .next_back()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false);
            let after_ok = haystack_lower[end..]
                .chars()
                .next()
                .map(|c| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(true);
            if before_ok && after_ok {
                return true;
            }
            from = end;
        }
        false
    } else {
        haystack_lower.contains(&needle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use du_core::StorySettings;

    fn setup() -> (Store, String) {
        let store = Store::open_in_memory().unwrap();
        let chat = store.create_chat(StorySettings::default(), "тест").unwrap();
        (store, chat)
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

    fn request(location: &str, same: bool) -> ImageRequest {
        ImageRequest {
            needed: true,
            location: Some(location.into()),
            same_location: Some(same),
            ..Default::default()
        }
    }

    #[test]
    fn a_brand_new_place_is_drawn_from_scratch() {
        let (store, chat) = setup();
        let decision = decide(&store, &chat, Some(&request("green meadow", false)), "", &[]).unwrap();
        assert!(!decision.is_edit());
        assert_eq!(decision.location, "green meadow");
    }

    #[test]
    fn staying_in_the_same_place_evolves_the_previous_frame() {
        let (store, chat) = setup();
        store.record_scene_image(&chat, "green meadow", &attachment("/1.png"), true).unwrap();

        let decision = decide(&store, &chat, Some(&request("green meadow", true)), "", &[]).unwrap();
        assert!(decision.is_edit());
        assert_eq!(decision.edit_from.as_ref().unwrap().url, "/1.png");
        assert!(decision.references.iter().any(|r| r.url == "/1.png"));
    }

    #[test]
    fn the_same_location_hint_works_without_a_label() {
        let (store, chat) = setup();
        store.record_scene_image(&chat, "крипта", &attachment("/1.png"), true).unwrap();

        let hint = ImageRequest { needed: true, same_location: Some(true), ..Default::default() };
        let decision = decide(&store, &chat, Some(&hint), "", &[]).unwrap();
        assert!(decision.is_edit(), "подсказка «то же место» обязана продолжать сцену");
        assert_eq!(decision.location, "крипта");
    }

    #[test]
    fn drift_is_bounded_by_re_anchoring_after_the_hop_limit() {
        let (store, chat) = setup();
        store.record_scene_image(&chat, "крипта", &attachment("/0.png"), true).unwrap();
        for index in 1..=MAX_EDIT_HOPS {
            store
                .record_scene_image(&chat, "крипта", &attachment(&format!("/{index}.png")), false)
                .unwrap();
        }
        let decision = decide(&store, &chat, Some(&request("крипта", true)), "", &[]).unwrap();
        assert!(!decision.is_edit(), "после предела правок место переустанавливается свежим кадром");
        assert_eq!(decision.prior_hops, MAX_EDIT_HOPS as u32);
    }

    #[test]
    fn returning_to_a_known_place_grows_from_its_anchor_not_its_last_edit() {
        let (store, chat) = setup();
        store.record_scene_image(&chat, "таверна", &attachment("/якорь.png"), true).unwrap();
        store.record_scene_image(&chat, "таверна", &attachment("/правка.png"), false).unwrap();
        store.record_scene_image(&chat, "лес", &attachment("/лес.png"), true).unwrap();

        let decision = decide(&store, &chat, Some(&request("таверна", false)), "", &[]).unwrap();
        assert_eq!(decision.edit_from.as_ref().unwrap().url, "/якорь.png");
    }

    #[test]
    fn a_hard_cut_to_another_place_is_drawn_fresh() {
        let (store, chat) = setup();
        store.record_scene_image(&chat, "таверна", &attachment("/1.png"), true).unwrap();
        let decision = decide(&store, &chat, Some(&request("подвал", false)), "", &[]).unwrap();
        assert!(!decision.is_edit());
        assert_eq!(decision.location, "подвал");
    }

    #[test]
    fn references_never_exceed_the_limit_and_never_repeat() {
        let (store, chat) = setup();
        store.record_scene_image(&chat, "лес", &attachment("/сцена.png"), true).unwrap();
        let extras: Vec<Attachment> = (0..5).map(|i| attachment(&format!("/игрок{i}.png"))).collect();

        let decision = decide(&store, &chat, Some(&request("лес", true)), "", &extras).unwrap();
        assert!(decision.references.len() <= MAX_REFERENCES);
        let mut urls: Vec<&str> = decision.references.iter().map(|r| r.url.as_str()).collect();
        urls.sort();
        let count = urls.len();
        urls.dedup();
        assert_eq!(urls.len(), count, "референсы не должны повторяться");
    }

    #[test]
    fn a_latin_item_name_matches_only_as_a_whole_word() {
        assert!(mentions_name("he raised the axe high", "axe"));
        assert!(!mentions_name("the tree was axed down", "axe"));
        assert!(mentions_name("рядом лежал ржавый меч", "ржавый меч"));
        assert!(!mentions_name("любой текст", "ме"), "слишком короткое имя не ищется");
    }
}
