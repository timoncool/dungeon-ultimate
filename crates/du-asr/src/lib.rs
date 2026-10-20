//! Распознавание речи для голосового ввода: Whisper (faster-whisper сабпроцессом) за общим [`AsrEngine`];
//! Parakeet считает transcribe.cpp через `dub-tcpp` прямо в du-server. Входной WAV приводится к 16 кГц моно здесь.

mod segment;
mod whisper;
pub use whisper::WhisperAsr;

pub use segment::{segment_words, Segment, Word, SEG_MAX_GAP, SEG_MAX_DUR};

use std::path::Path;
use thiserror::Error;

/// Частота, на которой распознают оба движка.
pub const TARGET_SR: u32 = 16_000;

#[derive(Error, Debug)]
pub enum AsrError {
    #[error("whisper: {0}")]
    Whisper(String),
    #[error("не удалось прочитать wav {0}: {1}")]
    WavRead(String, String),
    #[error("io: {0}")]
    Io(String),
}

/// Одна реплика диаризации: [start, end] в секундах, speaker — контиг. id (0..k-1).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Turn {
    pub start: f64,
    pub end: f64,
    pub speaker: i32,
}

/// Сегмент с привязкой к спикеру (результат transcribe_turns).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SpeakerSegment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub speaker: i32,
}

/// Единый интерфейс ASR-движка: транскрипция whole-clip и per-speaker (реплики). Методы берут `&Path`
/// (объект-трейт: без дженериков).
pub trait AsrEngine {
    fn transcribe(&mut self, wav: &Path, lang: &str) -> Result<Vec<Segment>, AsrError>;
    fn transcribe_turns(&mut self, wav: &Path, turns: &[Turn], lang: &str) -> Result<Vec<SpeakerSegment>, AsrError>;
    /// Пакетная транскрипция МНОГИХ коротких файлов → полный текст каждого (None = не распознан/сбой).
    /// Дефолт — цикл transcribe (Parakeet in-process и так быстр); Whisper переопределяет ОДНИМ
    /// сабпроцессом на весь список (старт процесса дорогой, 333 файла по-одному — минуты впустую).
    /// Используется QC-верификацией синтеза в рендере (сверка сказанного с ожидаемым переводом).
    fn transcribe_many(&mut self, files: &[std::path::PathBuf], lang: &str) -> Vec<Option<String>> {
        files
            .iter()
            .map(|f| {
                self.transcribe(f, lang)
                    .ok()
                    .map(|segs| segs.into_iter().map(|s| s.text).collect::<Vec<_>>().join(" "))
            })
            .collect()
    }
}

// ─── загрузка/подготовка аудио ──────────────────────────────────────────────

/// Прочитать WAV, свести в моно и ресемплировать в 16 кГц.
pub fn load_wav_16k_mono(path: &Path) -> Result<(Vec<f32>, u32), AsrError> {
    let disp = path.display().to_string();
    let wav_err = |e: hound::Error| AsrError::WavRead(disp.clone(), e.to_string());
    let mut reader = hound::WavReader::open(path).map_err(wav_err)?;
    let spec = reader.spec();
    let ch = spec.channels.max(1) as usize;

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(wav_err)?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<Vec<_>, _>>()
                .map_err(wav_err)?
        }
    };

    // Даунмикс в моно.
    let mono: Vec<f32> = if ch <= 1 {
        interleaved
    } else {
        interleaved
            .chunks(ch)
            .map(|frame| frame.iter().sum::<f32>() / ch as f32)
            .collect()
    };

    let out = if spec.sample_rate == TARGET_SR {
        mono
    } else {
        resample_linear(&mono, spec.sample_rate, TARGET_SR)
    };
    Ok((out, TARGET_SR))
}

/// Линейный ресемпл. Для извлечения мел-фич ASR этого достаточно; тяжёлый sinc не нужен.
fn resample_linear(input: &[f32], src_sr: u32, dst_sr: u32) -> Vec<f32> {
    if input.is_empty() || src_sr == dst_sr {
        return input.to_vec();
    }
    let ratio = dst_sr as f64 / src_sr as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}
