//! Оконная нарезка длинного вокала для полнометражного пайплайна (задача #79).
//!
//! Проблема: `Asr::transcribe` / диаризация / сепарация гоняются «весь файл разом» — на часовом
//! фильме это OOM/таймаут. Решение (порт whisperX `Binarize.__call__` min-cut + `Vad.merge_chunks`):
//! построить огибающую активности речи по vocals16 (RMS-энергия по кадрам), детектировать активные
//! участки простым порогом-по-огибающей (VAD), а переросший участок (> `max_active`) резать НЕ по
//! таймеру, а в точке МИНИМУМА активации во 2-й половине — граница не рвёт слово. Затем жадно паковать
//! получившиеся речевые атомы в окна ≤ `chunk_size` (~90с). Окно = единица работы = ключ чекпоинта.
//!
//! Всё — чистая арифметика над массивом сэмплов (движки не трогаем). Публичная точка входа —
//! [`plan_windows`] (список [`Window`] с абсолютным `offset`), и низкоуровневые
//! [`speech_envelope`]/[`detect_active_spans`]/[`merge_windows`] для тестов и повторного использования.

/// Одно окно работы: полу-открытый интервал [start, end) секунд в исходном таймлайне.
/// `offset == start` — на это значение сдвигаются ВСЕ таймкоды, полученные при обработке окна
/// (сепарация/диаризация/ASR гонятся по обрезку [start,end), выдают времена от 0, мы их ре-базим).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub start: f64,
    pub end: f64,
}

impl Window {
    #[inline]
    pub fn offset(&self) -> f64 {
        self.start
    }
    #[inline]
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// Параметры оконной нарезки. Дефолты — из DESIGN.md (окна 60-120с, режем ~90с).
#[derive(Debug, Clone, Copy)]
pub struct WindowConfig {
    /// Размер кадра огибающей в секундах (RMS считается по неперекрывающимся кадрам этой длины).
    pub frame_sec: f64,
    /// Порог активности как доля от адаптивного уровня (см. [`speech_envelope`] — порог = floor + rel*(peak-floor)).
    pub rel_threshold: f64,
    /// Заполнять паузы короче этого (сек) — «склеить» дыхание/микропаузы внутри одной фразы.
    pub min_silence: f64,
    /// Отбрасывать активные атомы короче этого (сек) — щелчки/транзиенты, не речь.
    pub min_speech: f64,
    /// Активный участок длиннее этого (сек) — резать min-cut'ом (argmin активации во 2-й половине).
    pub max_active: f64,
    /// Жадная упаковка атомов: цель размера окна (сек). Мягкий кап — атом не рвём, но новое окно
    /// открываем, как только текущее превысит это.
    pub chunk_size: f64,
    /// Небольшой захлёст соседних окон (сек) — чтобы фраза на стыке попала целиком хотя бы в одно
    /// окно; де-дуп на overlap убирает дубли слов при склейке ASR.
    pub overlap: f64,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            frame_sec: 0.032, // ~32мс кадр (близко к 512 сэмплам @16к) — дёшево и достаточно для VAD
            rel_threshold: 0.15,
            min_silence: 0.30,
            min_speech: 0.20,
            max_active: 30.0,
            chunk_size: 90.0,
            overlap: 0.5,
        }
    }
}

/// Построить огибающую активности речи: RMS-энергия по неперекрывающимся кадрам `frame_sec`.
/// Возвращает (`envelope`, `frame_sec_effective`) — значение на кадр в линейной шкале RMS (>=0).
/// Пустой вход -> пустая огибающая. Порог активности вычисляется отдельно (адаптивно) в
/// [`detect_active_spans`], т.к. зависит от статистики всей огибающей.
pub fn speech_envelope(samples: &[f32], sr: u32, frame_sec: f64) -> (Vec<f32>, f64) {
    if samples.is_empty() || sr == 0 {
        return (Vec::new(), frame_sec.max(1e-3));
    }
    let frame = ((frame_sec * sr as f64).round() as usize).max(1);
    let n = samples.len().div_ceil(frame);
    let mut env = Vec::with_capacity(n);
    let mut i = 0;
    while i < samples.len() {
        let end = (i + frame).min(samples.len());
        let mut acc = 0.0f64;
        for &s in &samples[i..end] {
            acc += (s as f64) * (s as f64);
        }
        let len = (end - i) as f64;
        env.push((acc / len).sqrt() as f32);
        i += frame;
    }
    let frame_sec_eff = frame as f64 / sr as f64;
    (env, frame_sec_eff)
}

/// Адаптивный порог по огибающей: floor (робастный «тихий» уровень) + rel*(peak-floor). Floor берём
/// как малый перцентиль (устойчив к всплескам), peak — как высокий перцентиль (устойчив к клиппингу).
fn adaptive_threshold(env: &[f32], rel: f64) -> f32 {
    if env.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f32> = env.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pct = |p: f64| -> f32 {
        let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    };
    let floor = pct(0.10);
    let peak = pct(0.95);
    floor + (rel as f32) * (peak - floor).max(0.0)
}

/// Детектировать активные (речевые) участки по огибающей и порезать переросшие min-cut'ом.
/// Возвращает список [start,end) в СЕКУНДАХ (таймлайн исходного вокала). Алгоритм:
/// 1) бинаризовать огибающую порогом (адаптивный, см. [`adaptive_threshold`]);
/// 2) склеить активные кадры в спаны, заполнив паузы короче `min_silence`;
/// 3) отбросить спаны короче `min_speech`;
/// 4) спан длиннее `max_active` рекурсивно резать в argmin огибающей во 2-й половине (min-cut).
pub fn detect_active_spans(env: &[f32], frame_sec: f64, cfg: &WindowConfig) -> Vec<(f64, f64)> {
    if env.is_empty() {
        return Vec::new();
    }
    let thr = adaptive_threshold(env, cfg.rel_threshold);
    let active: Vec<bool> = env.iter().map(|&e| e > thr).collect();

    // Кадры -> сырые активные спаны (в индексах кадров), заполняя короткие паузы.
    let min_sil_frames = (cfg.min_silence / frame_sec).round().max(0.0) as usize;
    let mut spans: Vec<(usize, usize)> = Vec::new(); // [first_active, last_active] инклюзивно
    let mut i = 0;
    while i < active.len() {
        if !active[i] {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i; // последний активный кадр
        let mut j = i + 1;
        while j < active.len() {
            if active[j] {
                end = j;
                j += 1;
            } else {
                // сколько подряд неактивных?
                let mut k = j;
                while k < active.len() && !active[k] {
                    k += 1;
                }
                let gap = k - j;
                if k < active.len() && gap <= min_sil_frames {
                    // короткая пауза внутри фразы — перешагнуть
                    j = k;
                } else {
                    break;
                }
            }
        }
        spans.push((start, end));
        i = end + 1;
    }

    // Индексы кадров -> секунды; порог min_speech; min-cut переросших.
    let to_sec = |f: usize| f as f64 * frame_sec;
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (a, b) in spans {
        let start_s = to_sec(a);
        // конец спана = конец ПОСЛЕДНЕГО активного кадра (b инклюзивно) -> (b+1)*frame_sec
        let end_s = to_sec(b + 1);
        if (end_s - start_s) < cfg.min_speech {
            continue;
        }
        min_cut_recursive(env, frame_sec, a, b + 1, cfg.max_active, &mut out);
    }
    out
}

/// Рекурсивный min-cut: активный участок кадров [f0, f1) секунд длиннее `max_active` режем в кадре
/// минимума огибающей во ВТОРОЙ половине участка (граница проходит по «самому тихому» месту —
/// наименьший шанс порвать слово), затем обе половины проверяем повторно. Порт whisperX Binarize.
fn min_cut_recursive(
    env: &[f32],
    frame_sec: f64,
    f0: usize,
    f1: usize,
    max_active: f64,
    out: &mut Vec<(f64, f64)>,
) {
    let dur = (f1 - f0) as f64 * frame_sec;
    if dur <= max_active || (f1 - f0) < 4 {
        out.push((f0 as f64 * frame_sec, f1 as f64 * frame_sec));
        return;
    }
    // argmin во 2-й половине [mid, f1). Порт: search_after = len//2; idx = search_after + argmin(...).
    let mid = f0 + (f1 - f0) / 2;
    let mut cut = mid;
    let mut best = f32::INFINITY;
    for f in mid..f1 {
        if env[f] < best {
            best = env[f];
            cut = f;
        }
    }
    // защита от вырожденного реза (cut на самом краю) — гарантируем прогресс
    let cut = cut.clamp(f0 + 1, f1 - 1);
    min_cut_recursive(env, frame_sec, f0, cut, max_active, out);
    min_cut_recursive(env, frame_sec, cut, f1, max_active, out);
}

/// Жадно упаковать речевые атомы (список [start,end) сек, отсортированный по start) в окна ≤chunk_size.
/// Порт `Vad.merge_chunks`: копим атомы, пока окно влезает в `chunk_size`; атом длиннее окна сам
/// становится окном. Между окнами добавляем `overlap` захлёста (окно расширяется вправо/влево в
/// пределах [0, total]) — чтобы фраза на стыке гарантированно попала целиком хотя бы в одно окно.
pub fn merge_windows(atoms: &[(f64, f64)], total: f64, cfg: &WindowConfig) -> Vec<Window> {
    if atoms.is_empty() {
        // нет речи — одно окно на весь файл (короткий путь не ломаем)
        return vec![Window { start: 0.0, end: total.max(0.0) }];
    }
    let mut raw: Vec<Window> = Vec::new();
    let mut cur_start = atoms[0].0;
    let mut cur_end = atoms[0].1;
    for &(s, e) in &atoms[1..] {
        // если добавление атома выведет окно за chunk_size — закрыть текущее и начать новое
        if (e - cur_start) > cfg.chunk_size && (cur_end - cur_start) > 0.0 {
            raw.push(Window { start: cur_start, end: cur_end });
            cur_start = s;
            cur_end = e;
        } else {
            cur_end = cur_end.max(e);
        }
    }
    raw.push(Window { start: cur_start, end: cur_end });

    // Добавить захлёст и заклампить в [0, total]. Захлёст симметричный (по half с каждой стороны),
    // но не заходит за соседние границы дальше, чем нужно для перекрытия — простая версия: расширяем
    // конец на overlap (кроме последнего), начало оставляем (де-дуп ASR разберётся с дублями слов).
    let n = raw.len();
    let mut out = Vec::with_capacity(n);
    for (i, w) in raw.iter().enumerate() {
        let mut start = w.start;
        let mut end = w.end;
        if i + 1 < n {
            end = (end + cfg.overlap).min(total.max(end));
        }
        if i > 0 {
            start = (start - cfg.overlap).max(0.0);
        }
        out.push(Window { start, end });
    }
    out
}

/// Полный план окон: огибающая -> активные спаны (min-cut) -> упаковка в окна. Точка входа для
/// оркестратора. `total` — длительность вокала в секундах (для клампа последнего окна). При отсутствии
/// речи или коротком файле (≤ chunk_size) вернёт ОДНО окно [0,total) — короткий путь = текущее поведение.
pub fn plan_windows(samples: &[f32], sr: u32, total: f64, cfg: &WindowConfig) -> Vec<Window> {
    // Короткий файл — одно окно, без нарезки (питон-паритет: window=1 = как было).
    if total <= cfg.chunk_size || samples.is_empty() || sr == 0 {
        return vec![Window { start: 0.0, end: total.max(0.0) }];
    }
    let (env, frame_sec) = speech_envelope(samples, sr, cfg.frame_sec);
    let atoms = detect_active_spans(&env, frame_sec, cfg);
    merge_windows(&atoms, total, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Синтетический вокал: sr=16000, тон в активных интервалах, тишина иначе.
    fn synth(sr: u32, total_sec: f64, active: &[(f64, f64)]) -> Vec<f32> {
        let n = (total_sec * sr as f64) as usize;
        let mut s = vec![0.0f32; n];
        for &(a, b) in active {
            let ia = (a * sr as f64) as usize;
            let ib = ((b * sr as f64) as usize).min(n);
            for (k, x) in s.iter_mut().enumerate().take(ib).skip(ia) {
                let t = k as f32 / sr as f32;
                *x = (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.5;
            }
        }
        s
    }

    #[test]
    fn envelope_high_in_speech_low_in_silence() {
        let sr = 16000;
        let s = synth(sr, 2.0, &[(0.5, 1.0)]);
        let (env, fs) = speech_envelope(&s, sr, 0.032);
        assert!(!env.is_empty());
        // кадр в районе 0.7с должен быть заметно громче кадра в 1.5с
        let idx = |t: f64| (t / fs) as usize;
        assert!(env[idx(0.7)] > env[idx(1.5)] * 4.0);
    }

    #[test]
    fn detects_two_spans() {
        let sr = 16000;
        let s = synth(sr, 6.0, &[(0.5, 1.5), (4.0, 5.0)]);
        let (env, fs) = speech_envelope(&s, sr, 0.032);
        let spans = detect_active_spans(&env, fs, &WindowConfig::default());
        assert_eq!(spans.len(), 2, "ждём два речевых атома, получили {spans:?}");
        assert!((spans[0].0 - 0.5).abs() < 0.15, "старт 1: {:?}", spans[0]);
        assert!((spans[1].1 - 5.0).abs() < 0.15, "конец 2: {:?}", spans[1]);
    }

    #[test]
    fn min_cut_splits_overgrown_span_at_quiet_point() {
        // один длинный активный участок с явным «провалом» энергии в середине -> min-cut режет там.
        let sr = 16000;
        // 40с активности с тихим окном на 25-25.4с (во 2-й половине [20,40))
        let mut active: Vec<(f64, f64)> = Vec::new();
        active.push((0.0, 25.0));
        active.push((25.4, 40.0));
        let s = synth(sr, 40.0, &active);
        let cfg = WindowConfig { max_active: 30.0, min_silence: 0.20, ..Default::default() };
        let (env, fs) = speech_envelope(&s, sr, 0.032);
        let spans = detect_active_spans(&env, fs, &cfg);
        // должен быть разрез рядом с 25с (во 2-й половине), давая ≥2 куска
        assert!(spans.len() >= 2, "min-cut должен порезать переросший участок: {spans:?}");
        // хотя бы один рез рядом с тихим окном 25.0..25.4
        let has_cut_near = spans.iter().any(|&(_, e)| (e - 25.2).abs() < 1.5)
            || spans.iter().any(|&(st, _)| (st - 25.2).abs() < 1.5);
        assert!(has_cut_near, "рез должен попасть в тихое окно ~25с: {spans:?}");
    }

    #[test]
    fn merge_packs_atoms_into_windows() {
        let cfg = WindowConfig { chunk_size: 10.0, overlap: 0.0, ..Default::default() };
        // 5 атомов по ~3с -> при chunk_size=10 упакуются в 2 окна
        let atoms = vec![(0.0, 3.0), (3.5, 6.0), (7.0, 9.0), (11.0, 14.0), (15.0, 18.0)];
        let ws = merge_windows(&atoms, 18.0, &cfg);
        assert!(ws.len() >= 2, "ждём ≥2 окна: {ws:?}");
        // окна не выходят за total
        for w in &ws {
            assert!(w.start >= 0.0 && w.end <= 18.0 + 1e-6, "окно вне [0,total]: {w:?}");
            assert!(w.end > w.start, "пустое окно: {w:?}");
        }
    }

    #[test]
    fn short_file_single_window() {
        // файл короче chunk_size -> ровно одно окно [0,total] (короткий путь = текущее поведение)
        let cfg = WindowConfig::default();
        let ws = plan_windows(&[0.1f32; 16000], 16000, 30.0, &cfg);
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0].start, 0.0);
        assert!((ws[0].end - 30.0).abs() < 1e-9);
    }

    #[test]
    fn no_speech_single_window() {
        // тишина -> одно окно на весь файл (не падаем, не теряем аудио)
        let cfg = WindowConfig { chunk_size: 10.0, ..Default::default() };
        let ws = merge_windows(&[], 100.0, &cfg);
        assert_eq!(ws.len(), 1);
        assert!((ws[0].end - 100.0).abs() < 1e-9);
    }

    #[test]
    fn window_offset_equals_start() {
        let w = Window { start: 12.5, end: 40.0 };
        assert_eq!(w.offset(), 12.5);
        assert!((w.duration() - 27.5).abs() < 1e-9);
    }
}
