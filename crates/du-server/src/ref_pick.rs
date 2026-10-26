//! Подбор куска рефа для клона (Speech Studio `voices/ref_pick.rs`): окна по паузам между словами распознавания и
//! концам предложений (у Parakeet слова идут встык, и паузы в таймингах почти не видны), не длиннее лимита движка.
//! У каждого — длина, самая длинная пауза внутри, отношение сигнал/шум по кадрам 20 мс и доля клиппинга; лучший по
//! оценке первым, а причины, по которым кусок хуже, — коды для окна.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Word {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Candidate {
    pub start: f64,
    pub end: f64,
    pub duration: f64,
    pub max_pause: f64,
    pub snr_db: f64,
    pub clipping: f64,
    pub text: String,
    pub score: f64,
    pub issues: Vec<&'static str>,
}

const BOUNDARY_GAP: f64 = 0.15;
const LONG_PAUSE: f64 = 0.8;
const NOISY_SNR_DB: f64 = 20.0;
const CLIP_LEVEL: f32 = 0.995;
const CLIPPED_SHARE: f64 = 0.001;
const FRAME_SECS: f64 = 0.020;

fn frame_rms(samples: &[f32], sr: u32, start: f64, end: f64) -> Vec<f64> {
    let frame = ((sr as f64) * FRAME_SECS).max(1.0) as usize;
    let a = ((start.max(0.0) * sr as f64) as usize).min(samples.len());
    let b = ((end.max(0.0) * sr as f64) as usize).min(samples.len());
    samples[a..b].chunks(frame).filter(|c| c.len() == frame).map(|c| (c.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / c.len() as f64).sqrt()).collect()
}

fn percentile(mut v: Vec<f64>, p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// Сигнал/шум окна: громкие кадры речи (90-й перцентиль RMS) против тихих (10-й), в дБ.
fn snr_db(samples: &[f32], sr: u32, start: f64, end: f64) -> f64 {
    let rms = frame_rms(samples, sr, start, end);
    let loud = percentile(rms.clone(), 0.9);
    let quiet = percentile(rms, 0.1).max(1e-5);
    20.0 * (loud.max(1e-5) / quiet).log10()
}

fn clipping(samples: &[f32], sr: u32, start: f64, end: f64) -> f64 {
    let a = ((start.max(0.0) * sr as f64) as usize).min(samples.len());
    let b = ((end.max(0.0) * sr as f64) as usize).min(samples.len());
    if b <= a {
        return 0.0;
    }
    samples[a..b].iter().filter(|x| x.abs() >= CLIP_LEVEL).count() as f64 / (b - a) as f64
}

/// Куски рефа длиной от `min_len` до `max_len` секунд, лучший первым, не больше `limit`, без сильных перекрытий.
pub fn candidates(words: &[Word], samples: &[f32], sr: u32, min_len: f64, max_len: f64, limit: usize) -> Vec<Candidate> {
    let n = words.len();
    if n == 0 || sr == 0 {
        return Vec::new();
    }
    let total = samples.len() as f64 / sr as f64;
    let gap_before = |i: usize| if i == 0 { words[0].start } else { (words[i].start - words[i - 1].end).max(0.0) };
    let gap_after = |j: usize| if j + 1 == n { (total - words[j].end).max(0.0) } else { (words[j + 1].start - words[j].end).max(0.0) };
    let sentence_end = |j: usize| words[j].word.trim_end().ends_with(['.', '!', '?', '…', ';', ':']);
    let boundary_after = |j: usize| j + 1 == n || gap_after(j) >= BOUNDARY_GAP || sentence_end(j);
    let mut all = Vec::new();
    for i in (0..n).filter(|&i| i == 0 || boundary_after(i - 1)) {
        let best_end = (i..n)
            .filter(|&j| boundary_after(j))
            .take_while(|&j| words[j].end - words[i].start <= max_len)
            .filter(|&j| words[j].end - words[i].start >= min_len)
            .last();
        let Some(j) = best_end else { continue };
        let start = (words[i].start - (gap_before(i) / 2.0).min(0.15)).max(0.0);
        let end = (words[j].end + (gap_after(j) / 2.0).min(0.2)).min(total.max(words[j].end));
        let max_pause = (i + 1..=j).map(|k| (words[k].start - words[k - 1].end).max(0.0)).fold(0.0, f64::max);
        let snr = snr_db(samples, sr, start, end);
        let clip = clipping(samples, sr, start, end);
        let duration = end - start;
        let mut issues = Vec::new();
        if max_pause >= LONG_PAUSE {
            issues.push("long_pause");
        }
        if snr < NOISY_SNR_DB {
            issues.push("noisy");
        }
        if clip >= CLIPPED_SHARE {
            issues.push("clipped");
        }
        if duration < max_len * 0.6 {
            issues.push("short");
        }
        let score = duration / max_len * 40.0 + snr.min(40.0) - max_pause * 10.0 - clip * 2000.0;
        let text = words[i..=j].iter().map(|w| w.word.trim()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
        all.push(Candidate { start, end, duration, max_pause, snr_db: snr, clipping: clip, text, score, issues });
    }
    all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut picked: Vec<Candidate> = Vec::new();
    for c in all {
        let overlaps = picked.iter().any(|p| {
            let common = (c.end.min(p.end) - c.start.max(p.start)).max(0.0);
            common > 0.5 * c.duration.min(p.duration)
        });
        if !overlaps {
            picked.push(c);
        }
        if picked.len() == limit {
            break;
        }
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(word: &str, start: f64, end: f64) -> Word {
        Word { word: word.into(), start, end }
    }

    /// Речь (громкий синус) на словах и тихий шум между ними.
    fn audio(words: &[Word], total: f64, sr: u32, noise: f32) -> Vec<f32> {
        let mut x: Vec<f32> = (0..(total * sr as f64) as usize).map(|i| noise * ((i * 7919 % 13) as f32 / 13.0 - 0.5)).collect();
        for wd in words {
            for i in (wd.start * sr as f64) as usize..((wd.end * sr as f64) as usize).min(x.len()) {
                x[i] = 0.5 * (i as f32 * 0.07).sin();
            }
        }
        x
    }

    fn sentence(at: f64, n: usize) -> Vec<Word> {
        (0..n).map(|k| w(&format!("w{k}"), at + k as f64 * 0.5, at + k as f64 * 0.5 + 0.4)).collect()
    }

    #[test]
    fn windows_start_and_end_at_pauses_and_stay_within_the_limit() {
        let mut words = sentence(0.5, 10);
        words.extend(sentence(6.5, 10));
        words.extend(sentence(12.5, 10));
        let sr = 16_000;
        let x = audio(&words, 18.0, sr, 0.001);
        let c = candidates(&words, &x, sr, 4.0, 12.0, 5);
        assert!(!c.is_empty());
        for k in &c {
            assert!(k.duration <= 12.0 + 0.4, "{k:?}");
            assert!(k.issues.iter().all(|i| *i != "noisy"), "{k:?}");
        }
        assert!(c[0].duration > 9.0, "the best fills the limit: {:?}", c[0]);
        assert!(c[0].text.starts_with("w0"));
    }

    #[test]
    fn a_noisy_or_clipped_recording_says_so() {
        let words = sentence(0.5, 20);
        let sr = 16_000;
        let mut x = audio(&words, 11.0, sr, 0.3);
        for v in x.iter_mut().step_by(50) {
            *v = 1.0;
        }
        let c = candidates(&words, &x, sr, 4.0, 12.0, 3);
        assert!(c[0].issues.contains(&"noisy") && c[0].issues.contains(&"clipped"), "{:?}", c[0]);
    }

    #[test]
    fn a_long_pause_inside_is_marked_and_scored_lower() {
        let mut words = sentence(0.5, 6);
        words.extend(sentence(4.5, 6));
        let sr = 16_000;
        let x = audio(&words, 8.0, sr, 0.001);
        let c = candidates(&words, &x, sr, 3.0, 12.0, 3);
        let whole = c.iter().find(|k| k.duration > 6.0).expect("the window across the pause");
        assert!(whole.issues.contains(&"long_pause") && whole.max_pause > 1.0);
    }

    #[test]
    fn nothing_fits_when_the_speech_is_shorter_than_the_minimum() {
        let words = sentence(0.2, 3);
        let x = audio(&words, 2.0, 16_000, 0.001);
        assert!(candidates(&words, &x, 16_000, 4.0, 12.0, 3).is_empty());
        assert!(candidates(&[], &x, 16_000, 4.0, 12.0, 3).is_empty());
    }

    #[test]
    fn sentences_cut_the_windows_when_the_words_touch() {
        let mut words = Vec::new();
        let mut t = 0.16;
        for k in 0..40 {
            let text = if k % 8 == 7 { format!("w{k}.") } else { format!("w{k}") };
            words.push(w(&text, t, t + 0.32));
            t += 0.32;
        }
        let x = audio(&words, t + 0.3, 16_000, 0.001);
        let c = candidates(&words, &x, 16_000, 4.0, 12.0, 3);
        assert!(!c.is_empty());
        assert!(c[0].text.ends_with('.') && c[0].duration <= 12.4, "{:?}", c[0]);
    }
}
