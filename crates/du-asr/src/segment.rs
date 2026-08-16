//! Сегментация словного потока в реплики дубляжа. Порт _segment из dubengine/asr.py:
//! разрыв на паузах > max_gap, конце предложения (.!?…) или превышении max_dur.

use serde::Serialize;

/// Дефолтные параметры сегментации (порт из dubengine/asr.py): разрыв на паузе > SEG_MAX_GAP сек и
/// жёсткий кап длины реплики SEG_MAX_DUR сек. Единый источник для всех ASR-движков (Parakeet/Whisper).
pub const SEG_MAX_GAP: f64 = 0.6;
pub const SEG_MAX_DUR: f64 = 8.0;

/// Слово со временем (секунды).
#[derive(Debug, Clone, Serialize)]
pub struct Word {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

/// Сегмент дубляжа: [start,end] + текст + список слов. Тот же контракт, что в Python-движке.
#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<Word>,
}

fn ends_sentence(word: &str) -> bool {
    word.ends_with(['.', '!', '?', '…'])
}

/// Склейка слов сегмента через пробел (эквивалент `join(" ").trim()`, без промежуточного Vec).
fn join_words(ws: &[Word]) -> String {
    let mut s = String::new();
    for x in ws {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&x.word);
    }
    s.trim().to_string()
}

/// Разбить поток слов на сегменты. max_gap=0.6с, max_dur=8.0с (как в _segment).
pub fn segment_words(words: &[Word], max_gap: f64, max_dur: f64) -> Vec<Segment> {
    let mut segs: Vec<Vec<Word>> = Vec::new();
    let mut cur: Vec<Word> = Vec::new();

    for w in words {
        if let (Some(last), Some(first)) = (cur.last(), cur.first()) {
            let gap = w.start - last.end;
            let dur = last.end - first.start;
            if gap > max_gap || dur > max_dur {
                segs.push(std::mem::take(&mut cur));
            }
        }
        cur.push(w.clone());
        if ends_sentence(&w.word) {
            segs.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        segs.push(cur);
    }

    segs.into_iter()
        .map(|ws| Segment {
            start: ws.first().unwrap().start,
            end: ws.last().unwrap().end,
            text: join_words(&ws),
            words: ws,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(word: &str, start: f64, end: f64) -> Word {
        Word { word: word.into(), start, end }
    }

    #[test]
    fn splits_on_sentence_end() {
        let words = vec![w("Hello", 0.0, 0.4), w("world.", 0.4, 0.8), w("Next", 0.9, 1.2)];
        let segs = segment_words(&words, 0.6, 8.0);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, "Hello world.");
        assert_eq!(segs[1].text, "Next");
    }

    #[test]
    fn splits_on_pause() {
        let words = vec![w("a", 0.0, 0.2), w("b", 2.0, 2.2)]; // пауза 1.8с > 0.6
        let segs = segment_words(&words, 0.6, 8.0);
        assert_eq!(segs.len(), 2);
    }

    #[test]
    fn splits_on_max_dur() {
        let words = vec![w("a", 0.0, 0.2), w("b", 5.0, 5.2), w("c", 9.0, 9.2)];
        // при добавлении c dur (5.0-0.0)=5.0 <8, но добавление b: dur=0 ok; c: dur=5.2-0=... проверяем разрыв
        let segs = segment_words(&words, 100.0, 4.0);
        assert!(segs.len() >= 2);
    }
}
