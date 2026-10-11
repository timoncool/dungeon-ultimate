//! Озвучка напрямую через Gemini API Google: запрос и разбор ответа — как в Dub Studio.
//!
//! Ключ берётся из переменной `GEMINI_API_KEY` или из файла в `secrets` рядом с игрой
//! (`credentials`), в ответы и журналы не попадает: в тексте ошибки он вымарывается.

use serde_json::{json, Value};
use std::path::Path;

const BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Модели озвучки Gemini, которые умеет игра; первая — по умолчанию.
pub const MODELS: [&str; 2] = ["gemini-3.8-flash-tts", "gemini-3.8-flash-lite-tts"];

/// Частота PCM, которую отдаёт Gemini, если MIME её не назвал.
const DEFAULT_RATE: u32 = 24_000;

pub fn validate_model(model: &str) -> Result<(), String> {
    if MODELS.contains(&model) {
        Ok(())
    } else {
        Err(format!("неизвестная модель озвучки Google «{model}»: доступны {}", MODELS.join(", ")))
    }
}

pub fn request(text: &str, voice: &str, style: &str) -> Result<Value, String> {
    if text.trim().is_empty() || voice.trim().is_empty() {
        return Err("озвучке Google нужны текст и голос".into());
    }
    let mut part = json!({"text":text});
    if !style.trim().is_empty() {
        part["speech_metadata"] = json!({"style":style});
    }
    Ok(json!({
        "contents":[{"role":"user","parts":[part]}],
        "generationConfig":{"responseModalities":["AUDIO"],"maxOutputTokens":8192,
            "speechConfig":{"voiceConfig":{"prebuiltVoiceConfig":{"voiceName":voice}}}}
    }))
}

pub fn audio(response: &Value) -> Result<(String, Vec<u8>), String> {
    let candidate = response["candidates"].as_array().and_then(|a| a.first()).ok_or_else(|| {
        format!("Google не вернул ответа (причина блокировки: {})", response["promptFeedback"]["blockReason"])
    })?;
    if candidate["finishReason"].as_str().is_some_and(|r| r != "STOP") {
        return Err(format!("озвучка Google не закончена: {}", candidate["finishReason"]));
    }
    let parts = candidate["content"]["parts"].as_array().ok_or("Google не вернул звука")?;
    let audio: Vec<_> = parts.iter().filter_map(|p| p.get("inlineData")).collect();
    if audio.len() != 1 {
        return Err(format!("Google вернул {} звуковых частей вместо одной", audio.len()));
    }
    let mime = audio[0]["mimeType"].as_str().ok_or("у звука Google нет MIME")?;
    if !mime.starts_with("audio/") {
        return Err("данные Google — не звук".into());
    }
    let bytes = crate::bytes::base64_decode(audio[0]["data"].as_str().ok_or("у звука Google нет данных")?)?;
    if bytes.len() < 200 {
        return Err("звук Google пустой или слишком короткий".into());
    }
    Ok((mime.into(), bytes))
}

/// PCM Gemini — WAV для плеера. Частота берётся из MIME (`audio/L16;rate=24000`).
fn to_wav(mime: &str, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if bytes.starts_with(b"RIFF") {
        return Ok(bytes);
    }
    if mime.starts_with("audio/L16") || mime.starts_with("audio/pcm") {
        let rate = mime
            .split(';')
            .find_map(|p| p.trim().strip_prefix("rate="))
            .and_then(|n| n.parse().ok())
            .unwrap_or(DEFAULT_RATE);
        return Ok(crate::bytes::wrap_pcm16_wav(&bytes, rate));
    }
    Err(format!("Google вернул звук в формате {mime}, который игра не читает"))
}

/// Озвучить фразу и записать WAV.
pub fn speak(key: &str, model: &str, voice: &str, text: &str, out: &Path) -> Result<(), String> {
    speak_at(BASE, key, model, voice, text, out)
}

fn speak_at(base: &str, key: &str, model: &str, voice: &str, text: &str, out: &Path) -> Result<(), String> {
    validate_model(model)?;
    let body = request(text, voice, "")?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(180)))
        .http_status_as_error(false)
        .build()
        .into();
    let response = agent
        .post(&format!("{base}/models/{model}:generateContent"))
        .header("x-goog-api-key", key)
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .map_err(|error| format!("озвучка Google: {}", error.to_string().replace(key, "[redacted]")))?;
    let status = response.status().as_u16();
    let raw = response
        .into_body()
        .read_to_string()
        .map_err(|error| format!("озвучка Google: обрыв ответа: {error}"))?;
    if !(200..300).contains(&status) {
        let safe = raw.replace(key, "[redacted]");
        return Err(format!("озвучка Google {status}: {}", safe.chars().take(600).collect::<String>()));
    }
    let parsed: Value = serde_json::from_str(&raw).map_err(|error| format!("ответ Google не разобрать: {error}"))?;
    let (mime, bytes) = audio(&parsed)?;
    std::fs::write(out, to_wav(&mime, bytes)?).map_err(|error| format!("не записать озвучку Google: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_are_metadata_not_spoken_text_and_unknown_models_are_refused() {
        let req = request("Привет.", "Orus", "Quietly, from a distance").unwrap();
        assert_eq!(req["contents"][0]["parts"][0]["text"], "Привет.");
        assert_eq!(req["contents"][0]["parts"][0]["speech_metadata"]["style"], "Quietly, from a distance");
        for model in ["", "models/x", "../other", "https://evil.invalid", "x?key=k"] {
            assert!(validate_model(model).is_err());
        }
        assert!(request("", "Orus", "").is_err());
    }

    #[test]
    fn an_unfinished_or_split_answer_is_an_error() {
        assert!(audio(&json!({"candidates":[{"finishReason":"MAX_TOKENS"}]})).is_err());
        let two = json!({"candidates":[{"finishReason":"STOP","content":{"parts":[
            {"inlineData":{"mimeType":"audio/L16;rate=24000","data":"AAAA"}},
            {"inlineData":{"mimeType":"audio/L16;rate=24000","data":"AAAA"}}]}}]});
        assert!(audio(&two).is_err());
    }

    #[test]
    fn pcm_becomes_a_wav_at_the_rate_the_mime_names() {
        let wav = to_wav("audio/L16;codec=pcm;rate=16000", vec![0u8; 400]).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        assert!(to_wav("audio/ogg", vec![1u8; 400]).is_err());
    }

    #[test]
    fn a_refusal_names_the_status_and_never_the_key() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut seen = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let read = stream.read(&mut buf).unwrap();
                seen.extend_from_slice(&buf[..read]);
                let text = String::from_utf8_lossy(&seen).to_string();
                if let Some(head) = text.find("\r\n\r\n") {
                    let length = text[..head]
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                        .unwrap_or(0);
                    if seen.len() >= head + 4 + length {
                        break;
                    }
                }
                if read == 0 {
                    break;
                }
            }
            let body = r#"{"error":{"message":"API key private-key not valid"}}"#;
            let _ = write!(stream, "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        });
        let dir = tempfile::tempdir().unwrap();
        let error = speak_at(&base, "private-key", "gemini-3.8-flash-tts", "Orus", "Привет.", &dir.path().join("a.wav")).unwrap_err();
        assert!(error.contains("400"), "{error}");
        assert!(!error.contains("private-key"), "{error}");
    }
}
