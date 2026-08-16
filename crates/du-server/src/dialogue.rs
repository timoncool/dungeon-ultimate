//! Многоголосие: кто говорит в реплике и каким голосом её читать.
//!
//! Порт `src/lib/tts.ts`. В прежней версии эта логика жила только в браузере: страница
//! получала весь ход целиком, резала его на реплики и озвучивала по частям. Теперь озвучка
//! начинается ещё во время письма — значит, делить на голоса должен сервер, иначе потоковая
//! речь читалась бы одним голосом рассказчика и многоголосие пропало бы.
//!
//! Правила отбора голоса те же, что и были: явный голос персонажа → закреплённый за ним
//! голос из набора → голос рассказчика.

use du_core::{StoryCharacter, StorySettings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentKind {
    Narration,
    Quote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceSegment {
    pub text: String,
    pub voice: String,
    /// Персонаж, которому приписана реплика; у повествования его нет.
    pub character_id: Option<String>,
    pub kind: SegmentKind,
}

/// Устойчивый 32-битный хеш (FNV-1a). Одинаков между запусками, поэтому персонаж всегда
/// получает один и тот же голос.
fn hash_string(value: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    // Считаем по кодовым единицам UTF-16, как это делал прежний код: иначе у одних и тех же
    // персонажей после переезда сменились бы голоса.
    for unit in value.encode_utf16() {
        hash ^= u32::from(unit);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// Голоса, которые можно раздать персонажам: весь набор минус голос рассказчика, чтобы
/// персонаж без своего голоса не совпал с повествованием.
fn speaker_pool<'a>(pool: &'a [String], narrator: &str) -> Vec<&'a str> {
    let mut seen = std::collections::HashSet::new();
    pool.iter()
        .map(|voice| voice.trim())
        .filter(|voice| !voice.is_empty() && *voice != narrator && seen.insert(*voice))
        .collect()
}

/// Место персонажа среди персонажей чата по возрастанию идентификатора — так распределение
/// голосов не зависит от порядка загрузки. Незнакомому персонажу даём хеш: индекс всё равно
/// нужен, и он должен быть устойчивым.
pub fn sorted_character_index(character_id: &str, characters: &[StoryCharacter]) -> usize {
    let mut ids: Vec<&str> = characters.iter().map(|character| character.id.as_str()).collect();
    ids.sort_unstable();
    ids.iter()
        .position(|id| *id == character_id)
        .unwrap_or_else(|| hash_string(character_id) as usize)
}

/// Голос персонажу, у которого своего голоса нет: берём из набора по его месту, поэтому
/// первые персонажи получают РАЗНЫЕ голоса и начинают повторяться только когда набор кончился.
pub fn auto_voice_for_character(
    index: usize,
    pool: &[String],
    narrator: &str,
) -> Option<String> {
    let pool = speaker_pool(pool, narrator);
    if pool.is_empty() {
        return None;
    }
    Some(pool[index % pool.len()].to_string())
}

/// Каким голосом читать конкретного персонажа. Многоголосие учитывается только когда оно
/// включено; вызывать можно всегда.
pub fn voice_for_character(
    settings: &StorySettings,
    character: Option<&StoryCharacter>,
    pool: &[String],
    characters: &[StoryCharacter],
) -> String {
    if settings.multi_voice {
        if let Some(character) = character {
            if let Some(voice) = character.voice.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                return voice.to_string();
            }
            let index = sorted_character_index(&character.id, characters);
            if let Some(voice) = auto_voice_for_character(index, pool, &settings.voice) {
                return voice;
            }
        }
    }
    settings.voice.clone()
}

/// Встречается ли имя в тексте целым словом. Латиницу ограничиваем границами слова, чтобы
/// короткое имя не поймалось внутри другого; для кириллицы и прочего берём вхождение —
/// границы слова там ненадёжны.
fn mentions_name(haystack_lower: &str, name_lower: &str) -> bool {
    if name_lower.chars().count() < 2 {
        return false;
    }
    if !name_lower.is_ascii() {
        return haystack_lower.contains(name_lower);
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0;
    while let Some(at) = haystack_lower[from..].find(name_lower) {
        let start = from + at;
        let end = start + name_lower.len();
        let before_ok = haystack_lower[..start].chars().next_back().is_none_or(|c| !is_word(c));
        let after_ok = haystack_lower[end..].chars().next().is_none_or(|c| !is_word(c));
        if before_ok && after_ok {
            return true;
        }
        from = start + name_lower.len().max(1);
        if from >= haystack_lower.len() {
            break;
        }
    }
    false
}

/// Кто, скорее всего, говорит: ищем упомянутое имя в подводке к реплике. Длинные имена
/// проверяем первыми, чтобы «Капитан Мара» выиграл у «Мары». None — имени нет, читает рассказчик.
pub fn detect_speaker<'a>(
    context: &str,
    characters: &'a [StoryCharacter],
) -> Option<&'a StoryCharacter> {
    if context.trim().is_empty() {
        return None;
    }
    let haystack = context.to_lowercase();
    let mut by_length: Vec<&StoryCharacter> = characters
        .iter()
        .filter(|character| !character.name.trim().is_empty())
        .collect();
    by_length.sort_by_key(|character| std::cmp::Reverse(character.name.trim().chars().count()));
    by_length
        .into_iter()
        .find(|character| mentions_name(&haystack, &character.name.trim().to_lowercase()))
}

/// Кавычки, которыми нарратор оформляет прямую речь. Каждая пара — открывающая и закрывающая.
const QUOTE_SPANS: [(&str, &str); 3] = [("«", "»"), ("\u{201c}", "\u{201d}"), ("\"", "\"")];

/// Разложить отрывок на повествование и реплики, каждую — со своим голосом.
///
/// `context` — то, что читалось перед этим отрывком: реплика часто идёт следующей фразой
/// после того, как назвали говорящего, и без этой подсказки поток терял бы имя на стыке фраз.
pub fn split_dialogue_segments(
    passage: &str,
    settings: &StorySettings,
    characters: &[StoryCharacter],
    pool: &[String],
    context: &str,
) -> Vec<VoiceSegment> {
    let text = passage.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let narrator = settings.voice.clone();
    let whole = |voice: &str| {
        vec![VoiceSegment {
            text: text.to_string(),
            voice: voice.to_string(),
            character_id: None,
            kind: SegmentKind::Narration,
        }]
    };
    // Быстрый путь: один голос на весь отрывок — прежний договор.
    if !settings.multi_voice || characters.is_empty() {
        return whole(&narrator);
    }

    let mut segments: Vec<VoiceSegment> = Vec::new();
    let mut cursor = 0usize;
    let push_narration = |segments: &mut Vec<VoiceSegment>, slice: &str| {
        let trimmed = slice.trim();
        if !trimmed.is_empty() {
            segments.push(VoiceSegment {
                text: trimmed.to_string(),
                voice: narrator.clone(),
                character_id: None,
                kind: SegmentKind::Narration,
            });
        }
    };

    while cursor < text.len() {
        let mut next_open: Option<(usize, (&str, &str))> = None;
        for span in QUOTE_SPANS {
            if let Some(at) = text[cursor..].find(span.0) {
                let at = cursor + at;
                if next_open.is_none_or(|(best, _)| at < best) {
                    next_open = Some((at, span));
                }
            }
        }
        let Some((open_at, span)) = next_open else {
            push_narration(&mut segments, &text[cursor..]);
            break;
        };
        let after_open = open_at + span.0.len();
        let Some(close_at) = text[after_open..].find(span.1).map(|at| after_open + at) else {
            // Кавычка не закрыта — остаток читает рассказчик.
            push_narration(&mut segments, &text[cursor..]);
            break;
        };

        let leading = &text[cursor..open_at];
        push_narration(&mut segments, leading);

        let quoted = &text[open_at..close_at + span.1.len()];
        // Имя ищем сначала в подводке этой же фразы, потом в том, что читалось до неё.
        let speaker = detect_speaker(leading, characters)
            .or_else(|| detect_speaker(context, characters));
        let voice = voice_for_character(settings, speaker, pool, characters);
        segments.push(VoiceSegment {
            text: quoted.trim().to_string(),
            voice,
            character_id: speaker.map(|character| character.id.clone()),
            kind: SegmentKind::Quote,
        });
        cursor = close_at + span.1.len();
    }

    // Крошечный кусок ПОВЕСТВОВАНИЯ («— сказал он», двоеточие, точка) приклеиваем к предыдущему
    // куску, но только если тот читается тем же голосом: иначе повествование зазвучало бы
    // голосом персонажа, а реплика — утонула бы в повествовании.
    let is_tiny = |segment: &VoiceSegment| {
        segment.kind == SegmentKind::Narration
            && (segment.text.chars().count() < 14
                || segment.text.chars().all(|c| c.is_whitespace() || !c.is_alphanumeric()))
    };
    let mut merged: Vec<VoiceSegment> = Vec::new();
    for segment in segments {
        match merged.last_mut() {
            Some(last) if is_tiny(&segment) && last.voice == segment.voice => {
                last.text = format!("{} {}", last.text, segment.text).trim().to_string();
            }
            _ => merged.push(segment),
        }
    }

    // Приклеить кусок из одной пунктуации некуда, когда сосед читается другим голосом —
    // произносить его нечем, а отдельным клипом он звучит как заикание. Выбрасываем.
    merged.retain(|segment| segment.text.chars().any(char::is_alphanumeric));

    // Если ни один кусок так и не получил отдельного голоса — возвращаем один кусок, как
    // обещано быстрым путём.
    if merged.is_empty() || merged.iter().all(|segment| segment.voice == narrator) {
        return whole(&narrator);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(id: &str, name: &str, voice: Option<&str>) -> StoryCharacter {
        StoryCharacter {
            id: id.to_string(),
            chat_id: "chat".to_string(),
            name: name.to_string(),
            details: String::new(),
            inventory: String::new(),
            skills: String::new(),
            spells: String::new(),
            portrait: None,
            voice: voice.map(str::to_string),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn settings(multi: bool) -> StorySettings {
        StorySettings { voice: "narrator".to_string(), multi_voice: multi, ..Default::default() }
    }

    #[test]
    fn with_multi_voice_off_the_whole_passage_stays_one_clip() {
        let cast = vec![character("1", "Каэл", Some("kael"))];
        let segments = split_dialogue_segments(
            "Каэл поднял факел. «Идём», — сказал он.",
            &settings(false),
            &cast,
            &[],
            "",
        );
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].voice, "narrator");
    }

    #[test]
    fn a_named_speaker_gets_their_own_voice_and_narration_keeps_the_narrator() {
        let cast = vec![character("1", "Каэл", Some("kael"))];
        let segments = split_dialogue_segments(
            "Каэл поднял факел и произнёс: «Идём дальше, тут пусто».",
            &settings(true),
            &cast,
            &[],
            "",
        );
        // Ровно два куска: точка после реплики отдельным клипом не становится.
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].kind, SegmentKind::Narration);
        assert_eq!(segments[0].voice, "narrator");
        assert_eq!(segments[1].kind, SegmentKind::Quote);
        assert_eq!(segments[1].voice, "kael");
        assert_eq!(segments[1].character_id.as_deref(), Some("1"));
    }

    #[test]
    fn a_punctuation_only_tail_never_becomes_a_clip_of_its_own() {
        let cast = vec![character("1", "Каэл", Some("kael"))];
        let segments =
            split_dialogue_segments("Каэл кивнул: «Да».", &settings(true), &cast, &[], "");
        assert!(segments.iter().all(|segment| segment.text.chars().any(char::is_alphanumeric)));
    }

    #[test]
    fn a_speaker_named_in_the_previous_sentence_is_still_recognised() {
        // Ради этого сервер и помнит контекст: в потоке фраза с именем уже ушла в синтез.
        let cast = vec![character("1", "Каэл", Some("kael"))];
        let segments = split_dialogue_segments(
            "«Стой», — прозвучало из темноты.",
            &settings(true),
            &cast,
            &[],
            "Каэл замер у поворота.",
        );
        assert_eq!(segments[0].voice, "kael");
    }

    #[test]
    fn a_character_without_a_voice_borrows_a_distinct_one_from_the_pool() {
        let cast = vec![character("a", "Мара", None), character("b", "Тор", None)];
        let pool = vec!["narrator".to_string(), "v1".to_string(), "v2".to_string()];
        let mara = voice_for_character(&settings(true), Some(&cast[0]), &pool, &cast);
        let tor = voice_for_character(&settings(true), Some(&cast[1]), &pool, &cast);
        assert_ne!(mara, tor, "разным персонажам — разные голоса");
        assert_ne!(mara, "narrator", "голос рассказчика персонажам не достаётся");
        assert_ne!(tor, "narrator");
    }

    #[test]
    fn without_a_pool_an_unvoiced_character_falls_back_to_the_narrator() {
        let cast = vec![character("a", "Мара", None)];
        assert_eq!(voice_for_character(&settings(true), Some(&cast[0]), &[], &cast), "narrator");
    }

    #[test]
    fn the_longest_matching_name_wins() {
        let cast = vec![
            character("a", "Мара", Some("v1")),
            character("b", "Капитан Мара", Some("v2")),
        ];
        let found = detect_speaker("Капитан Мара обернулась", &cast).unwrap();
        assert_eq!(found.id, "b");
    }

    #[test]
    fn a_latin_name_does_not_match_inside_another_word() {
        let cast = vec![character("a", "Ash", Some("v1"))];
        assert!(detect_speaker("the cashier nodded", &cast).is_none());
        assert!(detect_speaker("Ash nodded", &cast).is_some());
    }

    #[test]
    fn an_unterminated_quote_does_not_swallow_the_passage() {
        let cast = vec![character("1", "Каэл", Some("kael"))];
        let segments =
            split_dialogue_segments("Каэл сказал: «Идём", &settings(true), &cast, &[], "");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].voice, "narrator");
    }

    #[test]
    fn character_index_is_stable_regardless_of_input_order() {
        let cast = vec![character("b", "Тор", None), character("a", "Мара", None)];
        assert_eq!(sorted_character_index("a", &cast), 0);
        assert_eq!(sorted_character_index("b", &cast), 1);
    }
}
