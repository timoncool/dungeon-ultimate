//! Упаковка истории под контекст модели.

use du_core::{StoryMessage, StoryRole};

/// Выкидывать историю по одному сообщению нельзя: начало промпта менялось бы каждый ход и
/// обнуляло префикс-кэш сервера, заставляя пересчитывать всю историю заново. Поэтому
/// выкидываем блоками — префикс остаётся стабильным надолго, и большинство ходов платят
/// только за новые токены.
const EVICTION_BLOCK: usize = 16;

pub struct PackedHistory<'a> {
    /// Что уходит в промпт как есть.
    pub recent: &'a [StoryMessage],
    /// Что выпало из окна и должно свернуться в «историю до сих пор».
    pub evicted: &'a [StoryMessage],
}

/// Оставить в окне столько последних сообщений, сколько влезает в бюджет символов, округлив
/// выброшенное вверх до целого блока. Последнее сообщение остаётся всегда, даже если оно
/// одно длиннее бюджета: без него ход теряет то, на что игрок только что ответил.
pub fn pack_story_history(messages: &[StoryMessage], char_budget: usize) -> PackedHistory<'_> {
    let mut used = 0usize;
    let mut keep = 0usize;

    for message in messages.iter().rev() {
        // Надбавка на разметку роли и переводы строк вокруг реплики.
        let cost = message.content.chars().count() + 80;
        if keep > 0 && used + cost > char_budget {
            break;
        }
        used += cost;
        keep += 1;
    }

    let mut dropped = messages.len().saturating_sub(keep);
    if dropped > 0 {
        let blocks = dropped.div_ceil(EVICTION_BLOCK) * EVICTION_BLOCK;
        dropped = blocks.min(messages.len().saturating_sub(1));
    }

    PackedHistory { recent: &messages[dropped..], evicted: &messages[..dropped] }
}

/// Сколько последних реплик нарратора смотрим, выискивая повторы.
const BEATS: usize = 4;

/// Слова, которые встречаются в любой прозе и мотивом не являются.
const STOPWORDS: [&str; 24] = [
    "этот", "эта", "это", "эти", "тебе", "тебя", "твой", "твоя", "твоё", "твои", "перед", "после",
    "когда", "затем", "потом", "снова", "очень", "будто", "словно", "здесь", "сейчас", "также",
    "если", "чтобы",
];

/// Первая осмысленная фраза реплики — по ней узнаётся повторяющийся зачин, без вываливания
/// всего абзаца обратно в промпт.
fn beat_from_passage(content: &str) -> String {
    let flat: String = content
        .chars()
        .map(|c| if "*_`#>".contains(c) { ' ' } else { c })
        .collect();
    let flat = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return String::new();
    }
    let sentence = match flat.char_indices().find(|(_, c)| ".!?…".contains(*c)) {
        // Слишком ранняя точка (сокращение, инициал) — не конец мысли.
        Some((index, c)) if flat[..index].chars().count() > 24 => &flat[..index + c.len_utf8()],
        _ => flat.as_str(),
    };
    if sentence.chars().count() > 160 {
        let cut: String = sentence.chars().take(157).collect();
        format!("{}…", cut.trim_end())
    } else {
        sentence.to_string()
    }
}

/// Заметные слова, повторяющиеся в нескольких последних сценах: ими и называем мотивы,
/// за которые модель цепляется ход за ходом.
fn recurring_motifs(passages: &[&str]) -> Vec<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for passage in passages {
        let normalized = passage.to_lowercase().replace('ё', "е");
        let mut seen: Vec<String> = Vec::new();
        for word in normalized.split(|c: char| !c.is_alphabetic() && c != '-') {
            let word = word.trim_matches('-');
            if word.chars().count() < 5 || STOPWORDS.contains(&word) || seen.iter().any(|s| s == word) {
                continue;
            }
            seen.push(word.to_string());
            match counts.iter_mut().find(|(known, _)| known == word) {
                Some((_, count)) => *count += 1,
                None => counts.push((word.to_string(), 1)),
            }
        }
    }
    counts.retain(|(_, count)| *count >= 2);
    // Сортировка устойчивая, поэтому при равной частоте порядок остаётся по первому появлению.
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    counts.into_iter().take(6).map(|(word, _)| word).collect()
}

/// Строки подсказки против повторов. Пусто — когда сцен ещё слишком мало, чтобы судить.
pub struct AntiRepetition {
    pub beats: Vec<String>,
    pub motifs: Vec<String>,
}

pub fn build_anti_repetition(messages: &[StoryMessage]) -> Option<AntiRepetition> {
    let narration: Vec<&StoryMessage> = messages
        .iter()
        .filter(|message| matches!(message.role, StoryRole::Assistant))
        .rev()
        .take(BEATS)
        .collect();
    if narration.len() < 2 {
        return None;
    }
    let narration: Vec<&StoryMessage> = narration.into_iter().rev().collect();

    let beats: Vec<String> = narration
        .iter()
        .map(|message| beat_from_passage(&message.content))
        .filter(|beat| !beat.is_empty())
        .collect();
    if beats.is_empty() {
        return None;
    }
    let passages: Vec<&str> = narration.iter().map(|message| message.content.as_str()).collect();
    Some(AntiRepetition { beats, motifs: recurring_motifs(&passages) })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn history(count: usize) -> Vec<StoryMessage> {
        (0..count).map(|index| message(StoryRole::User, &format!("реплика {index}"))).collect()
    }

    #[test]
    fn a_short_history_fits_whole() {
        let messages = history(5);
        let packed = pack_story_history(&messages, 100_000);
        assert_eq!(packed.recent.len(), 5);
        assert!(packed.evicted.is_empty());
    }

    #[test]
    fn eviction_happens_in_whole_blocks_to_keep_the_prompt_prefix_stable() {
        let messages = history(50);
        // Бюджет на пару реплик: выброшенное обязано округлиться вверх до кратного блоку.
        let packed = pack_story_history(&messages, 300);
        assert_eq!(packed.evicted.len() % 16, 0, "выброс не по границе блока обнулит префикс-кэш");
        assert_eq!(packed.recent.len() + packed.evicted.len(), 50);
    }

    #[test]
    fn the_last_message_survives_even_a_tiny_budget() {
        let messages = history(3);
        let packed = pack_story_history(&messages, 1);
        assert_eq!(packed.recent.len(), 1);
        assert_eq!(packed.recent[0].content, "реплика 2");
    }

    #[test]
    fn eviction_never_swallows_the_entire_history() {
        let messages = history(10);
        let packed = pack_story_history(&messages, 1);
        assert!(!packed.recent.is_empty(), "хотя бы одна реплика обязана остаться");
        assert_eq!(packed.evicted.len(), 9);
    }

    #[test]
    fn anti_repetition_needs_at_least_two_narrated_turns() {
        assert!(build_anti_repetition(&[]).is_none());
        let one = vec![message(StoryRole::Assistant, "Дверь скрипнула.")];
        assert!(build_anti_repetition(&one).is_none());
    }

    #[test]
    fn repeated_openings_and_motifs_are_surfaced() {
        let messages = vec![
            message(StoryRole::Assistant, "Дверь скрипнула, и свеча дрогнула. Дальше был коридор."),
            message(StoryRole::User, "иду вперёд"),
            message(StoryRole::Assistant, "Дверь скрипнула снова, свеча почти погасла."),
        ];
        let hint = build_anti_repetition(&messages).expect("две сцены уже дают подсказку");
        assert_eq!(hint.beats.len(), 2);
        assert!(hint.beats[0].starts_with("Дверь скрипнула"));
        assert!(hint.motifs.iter().any(|motif| motif == "скрипнула"));
        assert!(!hint.motifs.iter().any(|motif| motif == "снова"), "стоп-слова мотивом не считаются");
    }

    #[test]
    fn a_long_opening_sentence_is_trimmed_with_an_ellipsis() {
        let long = "а".repeat(400);
        let messages = vec![
            message(StoryRole::Assistant, &long),
            message(StoryRole::Assistant, &long),
        ];
        let hint = build_anti_repetition(&messages).unwrap();
        assert!(hint.beats[0].chars().count() <= 158);
        assert!(hint.beats[0].ends_with('…'));
    }
}
