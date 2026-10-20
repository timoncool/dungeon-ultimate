//! Эталонный клип голоса так, как его берёт движок озвучки: WAV 16 кГц моно. Пак голосов лежит в mp3, поэтому клип
//! раскодируется здесь (symphonia) и пересэмплируется с ограничением полосы (rubato), как в студиях: ffmpeg на
//! компьютере игрока для этого не нужен.

use std::path::Path;

use audioadapter::Adapter;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Fft, FixedSync, Resampler};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// Частота эталона, которую ждёт движок голоса.
pub const RATE: u32 = 16_000;

/// Первые `seconds` секунд клипа `source` в WAV 16 кГц моно 16 бит по пути `out`.
pub fn to_wav(source: &Path, seconds: u32, out: &Path) -> Result<(), String> {
    let (mut mono, rate) = decode_mono(source)?;
    mono.truncate(rate as usize * seconds as usize);
    let samples = resample(&mono, rate, RATE)?;
    if samples.is_empty() {
        return Err(format!("в {} нет звука", source.display()));
    }
    let spec = hound::WavSpec { channels: 1, sample_rate: RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let partial = out.with_extension("part");
    let written = (|| -> Result<(), hound::Error> {
        let mut writer = hound::WavWriter::create(&partial, spec)?;
        for sample in &samples {
            writer.write_sample((sample.clamp(-1.0, 1.0) * 32_767.0).round() as i16)?;
        }
        writer.finalize()
    })();
    written.map_err(|error| format!("записать эталон {}: {error}", out.display()))?;
    std::fs::rename(&partial, out).map_err(|error| format!("записать эталон {}: {error}", out.display()))
}

/// Клип в моно (каналы усредняются) на его собственной частоте.
fn decode_mono(source: &Path) -> Result<(Vec<f32>, u32), String> {
    let failed = |what: &str, error: &dyn std::fmt::Display| format!("{what} {}: {error}", source.display());
    let file = std::fs::File::open(source).map_err(|error| failed("открыть", &error))?;
    let mut hint = Hint::new();
    if let Some(extension) = source.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, MediaSourceStream::new(Box::new(file), Default::default()), FormatOptions::default(), MetadataOptions::default())
        .map_err(|error| failed("распознать формат", &error))?;
    let track = format.default_track(TrackType::Audio).ok_or_else(|| format!("в {} нет звуковой дорожки", source.display()))?;
    let track_id = track.id;
    let Some(CodecParameters::Audio(params)) = &track.codec_params else {
        return Err(format!("в {} нет звуковой дорожки", source.display()));
    };
    let mut rate = params.sample_rate.unwrap_or(44_100);
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|error| failed("нет декодера для", &error))?;
    let mut mono = Vec::new();
    let mut planes: Vec<Vec<f32>> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(symphonia::core::errors::Error::IoError(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(failed("прочитать", &error)),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(error) => return Err(failed("раскодировать", &error)),
        };
        rate = decoded.spec().rate();
        decoded.copy_to_vecs_planar(&mut planes);
        let channels = planes.len().max(1) as f32;
        let frames = planes.first().map_or(0, Vec::len);
        mono.extend((0..frames).map(|frame| planes.iter().map(|plane| plane[frame]).sum::<f32>() / channels));
    }
    Ok((mono, rate))
}

/// Один канал на новую частоту; всё выше её Найквиста отфильтровывается, а не заворачивается в полосу.
fn resample(samples: &[f32], from: u32, to: u32) -> Result<Vec<f32>, String> {
    if from == to || samples.len() < 2 {
        return Ok(samples.to_vec());
    }
    let failed = |error: &dyn std::fmt::Display| format!("пересэмплировать эталон {from}->{to} Гц: {error}");
    let input = vec![samples.iter().map(|&sample| f64::from(sample)).collect::<Vec<_>>()];
    let adapter = SequentialSliceOfVecs::new(&input, 1, samples.len()).map_err(|error| failed(&error))?;
    let mut resampler = Fft::<f64>::new(from as usize, to as usize, 1024, 1, FixedSync::Both).map_err(|error| failed(&error))?;
    let output = resampler.process_all(&adapter, samples.len(), None).map_err(|error| failed(&error))?;
    (0..output.frames())
        .map(|frame| output.read_sample(0, frame).map(|value| value as f32).ok_or_else(|| failed(&format!("кадр {frame} вне буфера"))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stereo_clip_becomes_mono_16k_cut_to_its_seconds() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("Голос.wav");
        let spec = hound::WavSpec { channels: 2, sample_rate: 44_100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut writer = hound::WavWriter::create(&source, spec).unwrap();
        for i in 0..44_100 * 4 {
            let tone = ((i as f32 * 440.0 * std::f32::consts::TAU / 44_100.0).sin() * 12_000.0) as i16;
            writer.write_sample(tone).unwrap();
            writer.write_sample(tone).unwrap();
        }
        writer.finalize().unwrap();
        let out = dir.path().join("Голос.ref3s.wav");
        to_wav(&source, 3, &out).unwrap();
        let reader = hound::WavReader::open(&out).unwrap();
        assert_eq!((reader.spec().channels, reader.spec().sample_rate), (1, RATE));
        let frames = reader.duration() as i64;
        assert!((frames - 3 * RATE as i64).abs() < 1_100, "{frames}");
        let loudest = hound::WavReader::open(&out).unwrap().into_samples::<i16>().map(|sample| sample.unwrap().unsigned_abs()).max().unwrap();
        assert!((10_000..=13_000).contains(&loudest), "the tone keeps its level: {loudest}");
    }

    /// Настоящий клип пака голосов: mp3 раскодируется без ffmpeg.
    #[test]
    fn a_voice_pack_mp3_is_read_without_ffmpeg() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/voices/RU_Male_kuravlev_leonid.mp3");
        if !pack.is_file() {
            eprintln!("no voice pack on this machine: {}", pack.display());
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("ref.wav");
        to_wav(&pack, 5, &out).unwrap();
        let reader = hound::WavReader::open(&out).unwrap();
        assert_eq!(reader.spec().sample_rate, RATE);
        assert!(reader.duration() > RATE * 4, "{}", reader.duration());
    }
}
