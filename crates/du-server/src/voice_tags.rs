//! Пометки голосов, которые рассказчик ставит прямо в тексте.
//!
//! Раздавать голоса по одним только заведённым персонажам бессмысленно: в истории говорят
//! вдовы, стражники и дети, которых игрок никогда не заводил. Зато сам рассказчик знает,
//! КТО произносит каждую реплику. Поэтому набор голосов с пометками пола и возраста уходит
//! ему на вход, а он ставит перед каждой прямой речью `[[V:имя]]`.
//!
//! Пометки не должны попасть игроку на глаза ни в ленте, ни в книге, ни в базе — поэтому
//! текст чистится и в потоке, и в итоговом отрывке.

/// Кого можно дать реплике: имя голоса и то, что о нём известно.
pub struct VoiceHint {
    pub name: String,
    pub gender: String,
    pub age: String,
}

/// Указание рассказчику: чем помечать прямую речь.
///
/// Пустой набор голосов — пустое указание: просить помечать нечем, а лишний абзац в
/// подсказке только сбивает модель.
pub fn instruction(voices: &[VoiceHint], language_is_ru: bool) -> String {
    if voices.is_empty() {
        return String::new();
    }
    let list = voices
        .iter()
        .map(|voice| format!("{} ({}, {})", voice.name, voice.gender, voice.age))
        .collect::<Vec<_>>()
        .join(", ");
    if language_is_ru {
        format!(
            "ГОЛОСА. Доступные голоса озвучки: {list}.\n\
             Перед КАЖДОЙ прямой речью ставь пометку [[V:имя_голоса]] — ровно перед открывающей \
             кавычкой, на той же строке. Выбирай голос по говорящему: женщине — женский, \
             мужчине — мужской, ребёнку — детский, старику — пожилой. Одному и тому же \
             персонажу давай ОДИН И ТОТ ЖЕ голос на протяжении всей истории. Повествование \
             пометками не помечай — его читает рассказчик. Игрок пометок не видит.\n\
             Пример: Вдова ставит горшок на стол: [[V:{first}]]«Просыпайся, гость».",
            first = voices[0].name
        )
    } else {
        format!(
            "VOICES. Available narration voices: {list}.\n\
             Before EVERY line of direct speech put a marker [[V:voice_name]] — right before \
             the opening quotation mark, on the same line. Pick the voice by who is speaking: \
             a female voice for a woman, a male one for a man, a child voice for a child, an \
             elderly one for an old person. Keep the SAME voice for the same character \
             throughout the story. Do not mark narration — the narrator reads it. The player \
             never sees the markers.\n\
             Example: The widow sets the pot down: [[V:{first}]]\"Wake up, guest.\"",
            first = voices[0].name
        )
    }
}

/// Вырезать пометки из текста.
pub fn strip(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[[V:") {
        out.push_str(&rest[..start]);
        match rest[start..].find("]]") {
            Some(end) => rest = &rest[start + end + 2..],
            None => {
                // Незакрытая пометка — дальше текста нет смысла искать, но и показывать её
                // нельзя: обрываем.
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Голоса из пометок в порядке появления и текст без них.
///
/// Пометка всегда стоит перед своей репликой, поэтому k-я пометка относится к k-й реплике.
pub fn take(text: &str) -> (Vec<String>, String) {
    let mut voices = Vec::new();
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[[V:") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 4..];
        match after.find("]]") {
            Some(end) => {
                // Модель иногда пишет `[[V:eve|eve]]` или `[[V: eve (женский)]]` — имя это
                // первое слово, остальное пояснение для себя.
                let name = after[..end]
                    .split(['|', ',', '(', ' '])
                    .map(str::trim)
                    .find(|part| !part.is_empty())
                    .unwrap_or_default();
                voices.push(name.to_string());
                rest = &after[end + 2..];
            }
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    (voices, out)
}

/// Разбить текст на куски по пометкам: у первого куска голоса нет (это повествование до
/// первой реплики), у остальных — голос из пометки перед ними.
///
/// Так пометка работает и с кавычками, и с тире: русская проза оформляет речь и так, и так,
/// а искать только кавычки — значит терять половину реплик.
pub fn runs(text: &str) -> Vec<(Option<String>, String)> {
    let mut out: Vec<(Option<String>, String)> = Vec::new();
    let mut rest = text;
    let mut voice: Option<String> = None;
    loop {
        match rest.find("[[V:") {
            Some(start) => {
                if !rest[..start].trim().is_empty() {
                    out.push((voice.clone(), rest[..start].to_string()));
                }
                let after = &rest[start + 4..];
                match after.find("]]") {
                    Some(end) => {
                        let name = after[..end]
                            .split(['|', ',', '(', ' '])
                            .map(str::trim)
                            .find(|part| !part.is_empty())
                            .unwrap_or_default();
                        voice = Some(name.to_string());
                        rest = &after[end + 2..];
                    }
                    // Пометка не закрылась — остаток показывать нельзя.
                    None => return out,
                }
            }
            None => {
                if !rest.trim().is_empty() {
                    out.push((voice, rest.to_string()));
                }
                return out;
            }
        }
    }
}

/// Отделить саму реплику от слов автора внутри куска, помеченного голосом.
///
/// `— Поздно же ты, — говорит она.` → реплика `— Поздно же ты,` и слова автора `говорит она.`
/// Слова автора читает рассказчик: голосом персонажа они звучали бы как его же речь.
pub fn speech_and_tail(text: &str) -> (String, String) {
    let trimmed = text.trim();
    // Кавычки надёжнее всего: реплика ровно между ними.
    for (open, close) in [("«", "»"), ("\u{201c}", "\u{201d}")] {
        if let Some(start) = trimmed.find(open) {
            if let Some(end) = trimmed[start + open.len()..].find(close) {
                let end = start + open.len() + end + close.len();
                return (trimmed[start..end].to_string(), trimmed[end..].trim().to_string());
            }
        }
    }
    // Речь через тире: слова автора начинаются со ВТОРОГО тире.
    if let Some(first) = trimmed.chars().next() {
        if matches!(first, '—' | '–') {
            let after_first = &trimmed[first.len_utf8()..];
            if let Some(at) = ['—', '–'].iter().filter_map(|dash| after_first.find(*dash)).min() {
                let speech = &trimmed[..first.len_utf8() + at];
                let tail = after_first[at..].trim_start_matches(['—', '–']).trim();
                return (speech.trim().to_string(), tail.to_string());
            }
        }
    }
    (trimmed.to_string(), String::new())
}

/// Сколько символов с конца могут оказаться НАЧАЛОМ пометки.
///
/// В потоке пометка приходит по кускам, и «[[V» на границе куска нельзя ни показать, ни
/// выбросить — его надо придержать до следующего куска.
pub fn dangling(text: &str) -> usize {
    // Незакрытая пометка целиком: показывать нельзя, пока не пришло «]]».
    if let Some(start) = text.rfind("[[V:") {
        if !text[start..].contains("]]") {
            return text.len() - start;
        }
    }
    // Обрывок самой скобки в конце: «[», «[[», «[[V», «[[V:».
    for candidate in ["[[V:", "[[V", "[[", "["] {
        if text.ends_with(candidate) {
            return candidate.len();
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_never_reaches_the_reader() {
        assert_eq!(strip("Вдова говорит: [[V:eve]]«Привет»."), "Вдова говорит: «Привет».");
        assert_eq!(strip("без пометок"), "без пометок");
        // Оборванная пометка тоже не должна показаться.
        assert_eq!(strip("Хвост [[V:ev"), "Хвост ");
    }

    #[test]
    fn markers_are_read_in_order_of_their_lines() {
        let (voices, text) =
            take("[[V:eve]]«Стой». Он качает головой: [[V:leo]]«Не пойду».");
        assert_eq!(voices, vec!["eve", "leo"]);
        assert_eq!(text, "«Стой». Он качает головой: «Не пойду».");
    }

    #[test]
    fn a_marker_split_across_chunks_is_held_back() {
        assert_eq!(dangling("Вдова: [[V"), 3);
        assert_eq!(dangling("Вдова: [[V:eve"), 7);
        assert_eq!(dangling("Вдова: [[V:eve]]«Ох"), 0);
        assert_eq!(dangling("обычный текст"), 0);
    }

    #[test]
    fn a_sloppy_marker_still_names_the_voice() {
        let (voices, text) = take("[[V:eve|eve]]«Раз». [[V: leo (мужской)]]«Два».");
        assert_eq!(voices, vec!["eve", "leo"]);
        assert_eq!(text, "«Раз». «Два».");
    }

    #[test]
    fn text_is_cut_into_runs_by_the_markers() {
        let cut = runs("Она обернулась. [[V:eve]]— Поздно же ты. [[V:sal]]«Пусти».");
        assert_eq!(cut[0].0, None);
        assert_eq!(cut[0].1.trim(), "Она обернулась.");
        assert_eq!(cut[1].0.as_deref(), Some("eve"));
        assert_eq!(cut[2].0.as_deref(), Some("sal"));
    }

    #[test]
    fn the_authors_words_are_not_read_by_the_character() {
        let (speech, tail) = speech_and_tail("— Поздно же ты, — говорит она.");
        assert_eq!(speech, "— Поздно же ты,");
        assert_eq!(tail, "говорит она.");

        let (speech, tail) = speech_and_tail("«Пусти», — буркнул старик.");
        assert_eq!(speech, "«Пусти»");
        assert_eq!(tail, ", — буркнул старик.");

        // Реплика без слов автора остаётся целой.
        let (speech, tail) = speech_and_tail("— Уходи.");
        assert_eq!(speech, "— Уходи.");
        assert!(tail.is_empty());
    }

    #[test]
    fn without_voices_there_is_nothing_to_ask_for() {
        assert!(instruction(&[], true).is_empty());
        let hint = VoiceHint {
            name: "eve".into(),
            gender: "female".into(),
            age: "adult".into(),
        };
        let asked = instruction(std::slice::from_ref(&hint), true);
        assert!(asked.contains("eve (female, adult)"));
        assert!(asked.contains("[[V:"));
    }
}
