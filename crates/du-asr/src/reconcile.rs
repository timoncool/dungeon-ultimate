//! Развязка ASR и диаризации + сверка по перекрытию (задачи #79/#83, порт whisperX
//! `assign_word_speakers`).
//!
//! Идея: транскрибируем и диаризуем НЕЗАВИСИМО, затем каждой реплике ASR назначаем спикера по
//! МАКСИМАЛЬНОМУ суммарному перекрытию по времени с turn'ами диаризации (argmax по спикеру). Это
//! убирает потерю качества Parakeet на коротких turn'ах (diarize-first режет аудио по границам
//! turn'а до ASR) и делает диаризацию сменяемой. Для дыр (реплика без перекрытия) — `fill_nearest`:
//! ближайший по центру turn.
//!
//! Ускорение: turn'ы диаризации держим как отсортированный по `start` `Vec` + бинарный поиск первого
//! потенциально пересекающегося turn'а (аналог IntervalTree-запроса, O(log n + k) на реплику вместо
//! O(n)). Это чистая арифметика над временами — движки не трогаем.

use crate::Turn;

/// Отсортированный по `start` набор turn'ов диаризации с запросом «все turn'ы, пересекающие [a,b)».
/// Тонкая обёртка над `Vec<Turn>`: реализует «дерево интервалов» как sorted-Vec + бинарный поиск.
/// Для корректной работы бинарного поиска дополнительно храним префиксный максимум `end` — turn'ы
/// сортированы по `start`, но `end` не монотонен, поэтому «первый кандидат слева» находим по
/// max_end_prefix (ни один turn левее не может дотянуться до запроса).
#[derive(Debug, Clone)]
pub struct DiarIndex {
    turns: Vec<Turn>,
    /// max_end_prefix[i] = max(turns[0..=i].end) — для отсечения левого хвоста при запросе.
    max_end_prefix: Vec<f64>,
}

impl DiarIndex {
    /// Построить индекс. Копирует turn'ы, сортирует по `start` (стабильно), считает префиксный max end.
    pub fn new(turns: &[Turn]) -> Self {
        let mut t = turns.to_vec();
        t.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
        let mut max_end_prefix = Vec::with_capacity(t.len());
        let mut run = f64::NEG_INFINITY;
        for turn in &t {
            run = run.max(turn.end);
            max_end_prefix.push(run);
        }
        Self { turns: t, max_end_prefix }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    #[inline]
    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    /// Индекс первого turn'а, который МОЖЕТ пересечь [a, ..) — т.е. первый i с max_end_prefix[i] > a.
    /// Всё левее гарантированно кончается до a. Бинарный поиск по монотонному max_end_prefix.
    fn first_candidate(&self, a: f64) -> usize {
        // ищем наименьший i: max_end_prefix[i] > a
        let mut lo = 0usize;
        let mut hi = self.turns.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.max_end_prefix[mid] > a {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        lo
    }

    /// Спикер для реплики [start,end): argmax суммарного перекрытия по спикеру. Возвращает
    /// `Some(speaker)` при наличии перекрытия, иначе `None` (тогда вызвать [`Self::nearest`]).
    pub fn speaker_by_overlap(&self, start: f64, end: f64) -> Option<i32> {
        if self.turns.is_empty() || end <= start {
            return None;
        }
        use std::collections::HashMap;
        let mut acc: HashMap<i32, f64> = HashMap::new();
        // с первого кандидата идём вправо, пока turn.start < end (дальше пересечений быть не может,
        // т.к. отсортировано по start)
        let from = self.first_candidate(start);
        for t in &self.turns[from..] {
            if t.start >= end {
                break;
            }
            let ov = end.min(t.end) - start.max(t.start);
            if ov > 0.0 {
                *acc.entry(t.speaker).or_insert(0.0) += ov;
            }
        }
        // argmax по перекрытию; тай-брейк — меньший id спикера (детерминизм).
        acc.into_iter()
            .max_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.0.cmp(&a.0)) // при равном перекрытии предпочесть МЕНЬШИЙ id
            })
            .map(|(spk, _)| spk)
    }

    /// Ближайший спикер по центру реплики — фолбэк для реплик без перекрытия (fill_nearest).
    pub fn nearest(&self, start: f64, end: f64) -> Option<i32> {
        if self.turns.is_empty() {
            return None;
        }
        let mid = (start + end) / 2.0;
        self.turns
            .iter()
            .min_by(|a, b| {
                let da = (mid - (a.start + a.end) / 2.0).abs();
                let db = (mid - (b.start + b.end) / 2.0).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|t| t.speaker)
    }

    /// Назначить спикера реплике: max-overlap, при отсутствии перекрытия — ближайший, при пустой
    /// диаризации — 0 (тот же single-speaker контракт, что у ASR/analyze). Замена/дополнение линейного
    /// `speaker_for` в analyze.rs — та же семантика, но O(log n + k) и с fill_nearest для дыр.
    pub fn assign(&self, start: f64, end: f64) -> i32 {
        self.speaker_by_overlap(start, end)
            .or_else(|| self.nearest(start, end))
            .unwrap_or(0)
    }
}

/// Удобный свободный вызов: спикер для одной реплики по срезу turn'ов (строит временный индекс).
/// Для многих реплик предпочитайте [`DiarIndex::new`] один раз + [`DiarIndex::assign`] в цикле.
pub fn speaker_for_overlap(start: f64, end: f64, turns: &[Turn]) -> i32 {
    DiarIndex::new(turns).assign(start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(start: f64, end: f64, speaker: i32) -> Turn {
        Turn { start, end, speaker }
    }

    #[test]
    fn picks_max_overlap_speaker() {
        // реплика [1,4]; спикер 0 покрывает [0,2] (перекрытие 1с), спикер 1 покрывает [2,5] (2с) -> 1
        let turns = vec![t(0.0, 2.0, 0), t(2.0, 5.0, 1)];
        let idx = DiarIndex::new(&turns);
        assert_eq!(idx.assign(1.0, 4.0), 1);
    }

    #[test]
    fn tie_break_prefers_smaller_id() {
        // равное перекрытие 1с каждому -> детерминированно меньший id (0)
        let turns = vec![t(0.0, 2.0, 0), t(2.0, 4.0, 1)];
        let idx = DiarIndex::new(&turns);
        assert_eq!(idx.assign(1.0, 3.0), 0);
    }

    #[test]
    fn fill_nearest_when_no_overlap() {
        // реплика [10,11] не пересекает ни один turn -> ближайший по центру: [8,9](центр8.5) ближе,
        // чем [0,1](центр0.5) -> спикер 1
        let turns = vec![t(0.0, 1.0, 0), t(8.0, 9.0, 1)];
        let idx = DiarIndex::new(&turns);
        assert_eq!(idx.speaker_by_overlap(10.0, 11.0), None);
        assert_eq!(idx.assign(10.0, 11.0), 1);
    }

    #[test]
    fn empty_diar_is_single_speaker_zero() {
        let idx = DiarIndex::new(&[]);
        assert!(idx.is_empty());
        assert_eq!(idx.assign(1.0, 2.0), 0);
    }

    #[test]
    fn binary_search_matches_linear_scan() {
        // случайно-подобный набор turn'ов; сверяем sorted-Vec+binsearch с наивным линейным argmax
        let turns = vec![
            t(0.0, 3.0, 0),
            t(2.5, 4.0, 1),
            t(4.0, 10.0, 2),
            t(9.5, 12.0, 0),
            t(12.0, 20.0, 1),
            t(6.0, 6.2, 3), // короткий turn внутри чужого — end не монотонен
        ];
        let idx = DiarIndex::new(&turns);
        // наивный argmax
        let linear = |s: f64, e: f64| -> i32 {
            use std::collections::HashMap;
            let mut acc: HashMap<i32, f64> = HashMap::new();
            for tt in &turns {
                let ov = e.min(tt.end) - s.max(tt.start);
                if ov > 0.0 {
                    *acc.entry(tt.speaker).or_insert(0.0) += ov;
                }
            }
            acc.into_iter()
                .max_by(|a, b| {
                    a.1.partial_cmp(&b.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(b.0.cmp(&a.0))
                })
                .map(|(spk, _)| spk)
                .unwrap_or(0)
        };
        for &(s, e) in &[(1.0, 3.5), (5.0, 11.0), (6.0, 6.1), (13.0, 19.0), (3.0, 4.5)] {
            assert_eq!(idx.assign(s, e), linear(s, e), "разошлось на [{s},{e}]");
        }
    }

    #[test]
    fn first_candidate_skips_left_tail() {
        // turn'ы, кончающиеся до запроса, не должны попадать в перекрытие
        let turns = vec![t(0.0, 1.0, 0), t(1.0, 2.0, 0), t(50.0, 60.0, 1)];
        let idx = DiarIndex::new(&turns);
        assert_eq!(idx.assign(52.0, 58.0), 1);
    }
}
