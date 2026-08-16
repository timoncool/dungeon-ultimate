//! Глобальная согласованность спикеров через окна/эпизоды (задача #83) — ЗАГОТОВКА.
//!
//! Цель: «персонаж = один голос во всём фильме/сериале». Sortformer диаризует ≤4 спикеров ПО ФАЙЛУ
//! без сквозной согласованности — при оконной нарезке (window.rs) спикер `1` в окне A и спикер `1` в
//! окне B могут быть разными людьми. Решение: извлечь фиксированный аудио-эмбеддинг (CAM++/ECAPA/
//! x-vector через ONNX, engine-neutral) на доминирующего спикера окна и кластеризовать эмбеддинги в
//! глобальные ID (cosine + агломеративно), затем локальные метки мапить в глобальные.
//!
//! Статус (по DESIGN.md §2.6): движок эмбеддингов — ГЕЙТ. Пока ONNX-модель эмбеддера не подключена,
//! [`SpeakerEmbedder`] реализован заглушкой [`NullEmbedder`] (возвращает `None`), а оркестратор при
//! `None` откатывается на per-window Sortformer-метки (текущее поведение, ничего не ломается). Сама
//! КЛАСТЕРИЗАЦИЯ ([`cluster_embeddings`], [`cosine`]) реализована полностью и покрыта тестами — как
//! только эмбеддер появится, глобальные ID заработают без переписывания.

/// Аудио-эмбеддинг спикера (L2-нормализованный вектор фикс. размерности).
pub type Embedding = Vec<f32>;

/// Извлекатель эмбеддинга спикера из среза сэмплов (16k/mono). Реализуется поверх ONNX-модели
/// (CAM++/ECAPA) — engine-neutral, оффлайн. Трейт-объект: без дженериков, чтобы analyze мог держать
/// `Box<dyn SpeakerEmbedder>` и не знать деталей движка.
pub trait SpeakerEmbedder: Send + Sync {
    /// Эмбеддинг для среза `samples` (16 кГц, mono). `None` — движок недоступен/срез негоден
    /// (слишком короткий/тихий): вызывающий откатывается на локальные метки.
    fn embed(&self, samples: &[f32], sr: u32) -> Option<Embedding>;

    /// Доступен ли движок (веса загружены). Заглушка возвращает `false`.
    fn is_available(&self) -> bool {
        false
    }
}

/// Заглушка-эмбеддер: движок не подключён, всегда `None`. Дефолт до появления ONNX-модели —
/// оркестратор при этом остаётся на per-window Sortformer-метках (см. модуль-док).
#[derive(Debug, Default, Clone, Copy)]
pub struct NullEmbedder;

impl SpeakerEmbedder for NullEmbedder {
    fn embed(&self, _samples: &[f32], _sr: u32) -> Option<Embedding> {
        None
    }
    fn is_available(&self) -> bool {
        false
    }
}

/// Косинусная близость двух векторов в [-1,1]. Для не-нормализованных векторов делит на нормы;
/// нулевой вектор -> 0.0 (не пересекается ни с чем).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Локальный (per-window) спикер с эмбеддингом — вход глобальной кластеризации.
#[derive(Debug, Clone)]
pub struct LocalSpeaker {
    /// Индекс окна, из которого извлечён эмбеддинг.
    pub window: usize,
    /// Локальная метка спикера в этом окне (0..k-1 внутри окна).
    pub local_id: i32,
    pub embedding: Embedding,
}

/// Агломеративная кластеризация эмбеддингов по cosine-близости (single-linkage, порог `threshold`).
/// Возвращает вектор глобальных ID той же длины, что `items` (i-й элемент -> глобальный кластер i-го
/// эмбеддинга). Пустой вход -> пустой выход. Порог по умолчанию ~0.5-0.7 (cosine) в зависимости от
/// эмбеддера — задаёт вызывающий. Реализация O(n^2) достаточна: спикеров/окон немного.
pub fn cluster_embeddings(items: &[Embedding], threshold: f32) -> Vec<usize> {
    let n = items.len();
    if n == 0 {
        return Vec::new();
    }
    // union-find
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]]; // сжатие пути
            x = parent[x];
        }
        x
    }
    for i in 0..n {
        for j in (i + 1)..n {
            if cosine(&items[i], &items[j]) >= threshold {
                let ri = find(&mut parent, i);
                let rj = find(&mut parent, j);
                if ri != rj {
                    parent[ri] = rj;
                }
            }
        }
    }
    // компактная нумерация корней в 0..k-1 в порядке первого появления (детерминизм)
    use std::collections::HashMap;
    let mut remap: HashMap<usize, usize> = HashMap::new();
    let mut next = 0usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let r = find(&mut parent, i);
        let g = *remap.entry(r).or_insert_with(|| {
            let v = next;
            next += 1;
            v
        });
        out.push(g);
    }
    out
}

/// Свести локальные (окно, local_id) метки в глобальные ID. Возвращает карту
/// (window, local_id) -> global_id. Оркестратор применяет её к сегментам: `speaker` реплики из окна w
/// с локальной меткой l становится `global_map[(w,l)]`. Если эмбеддингов нет (эмбеддер-заглушка),
/// вызывающий эту функцию НЕ зовёт и оставляет локальные метки — поведение не меняется.
pub fn map_local_to_global(
    locals: &[LocalSpeaker],
    threshold: f32,
) -> std::collections::HashMap<(usize, i32), usize> {
    let embs: Vec<Embedding> = locals.iter().map(|l| l.embedding.clone()).collect();
    let clusters = cluster_embeddings(&embs, threshold);
    locals
        .iter()
        .zip(clusters)
        .map(|(l, g)| ((l.window, l.local_id), g))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_embedder_unavailable() {
        let e = NullEmbedder;
        assert!(!e.is_available());
        assert!(e.embed(&[0.1f32; 16000], 16000).is_none());
    }

    #[test]
    fn cosine_identical_is_one() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn cosine_mismatched_or_zero_is_zero() {
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    }

    #[test]
    fn clusters_two_tight_groups() {
        // две плотные группы близких векторов -> два кластера
        let items = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.98, 0.02, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.01, 0.99, 0.0],
        ];
        let g = cluster_embeddings(&items, 0.9);
        assert_eq!(g[0], g[1], "первые два — один кластер");
        assert_eq!(g[2], g[3], "вторые два — один кластер");
        assert_ne!(g[0], g[2], "группы различны");
    }

    #[test]
    fn empty_input_empty_output() {
        assert!(cluster_embeddings(&[], 0.5).is_empty());
    }

    #[test]
    fn map_local_to_global_merges_same_voice_across_windows() {
        // спикер local_id=1 в окне 0 и local_id=0 в окне 1 — один голос (близкие эмбеддинги) -> один глобальный
        let locals = vec![
            LocalSpeaker { window: 0, local_id: 0, embedding: vec![1.0, 0.0] },
            LocalSpeaker { window: 0, local_id: 1, embedding: vec![0.0, 1.0] },
            LocalSpeaker { window: 1, local_id: 0, embedding: vec![0.02, 0.99] }, // = окно0 local1
        ];
        let m = map_local_to_global(&locals, 0.9);
        assert_eq!(m[&(0, 1)], m[&(1, 0)], "один голос в двух окнах -> один глобальный id");
        assert_ne!(m[&(0, 0)], m[&(0, 1)], "разные голоса -> разные id");
    }
}
