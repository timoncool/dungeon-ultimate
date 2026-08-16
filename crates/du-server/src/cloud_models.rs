//! Живой список облачных моделей.
//!
//! Вшивать перечень нельзя: он протухает. Проверено на этом же проекте — перенесённый из
//! Dub Studio справочник называл модели озвучки, которых у провайдера уже нет вовсе.
//! Поэтому список тянется из API и раскладывается по назначению по модальностям входа и
//! выхода, а не по угаданным именам.
//!
//! Ответ кэшируется на время работы приложения: список меняется днями, а дёргать сеть на
//! каждое открытие настроек незачем.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

/// Насколько живёт кэш списка.
const CACHE_TTL: Duration = Duration::from_secs(60 * 30);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudModel {
    pub id: String,
    pub name: String,
    /// Цена за миллион входных токенов, как её отдаёт провайдер.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_price: Option<String>,
}

/// Для чего модель годится в нашем пайплайне.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Рассказчик и служебные проходы — обычный текст на выходе.
    Text,
    /// Кадр — картинка на выходе.
    Image,
    /// Озвучка — звук на выходе.
    Tts,
    /// Распознавание речи — звук на входе.
    Stt,
}

impl Kind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "text" | "narrator" => Some(Kind::Text),
            "image" => Some(Kind::Image),
            "tts" => Some(Kind::Tts),
            "stt" | "asr" => Some(Kind::Stt),
            _ => None,
        }
    }
}

static CACHE: Mutex<Option<(Instant, String, Value)>> = Mutex::new(None);

/// Параметр запроса под назначение: провайдер сам отфильтрует список по модальностям.
fn query_for(kind: Kind) -> &'static str {
    match kind {
        Kind::Text => "",
        Kind::Image => "?output_modalities=image",
        // Модальности называются РОВНО так: озвучка — speech, распознавание — transcription.
        // На `audio` провайдер отвечает горсткой мультимодальных чат-моделей, а весь каталог
        // TTS и ASR — 18 и 19 моделей — остаётся невидимым.
        Kind::Tts => "?output_modalities=speech",
        Kind::Stt => "?output_modalities=transcription",
    }
}

/// Для озвучки каталог собирается из ДВУХ запросов: настоящие модели речи помечены
/// `speech`, а чат-модели, умеющие говорить (`openai/gpt-audio`), — `audio`. Оба варианта
/// игрок вправе выбрать, поэтому показываем и те, и другие.
fn extra_query_for(kind: Kind) -> Option<&'static str> {
    matches!(kind, Kind::Tts).then_some("?output_modalities=audio")
}

fn fetch(key: &str, kind: Kind) -> Result<Value, String> {
    fetch_query(key, query_for(kind))
}

fn fetch_query(key: &str, query: &str) -> Result<Value, String> {
    if let Ok(cache) = CACHE.lock() {
        if let Some((at, cached_query, value)) = cache.as_ref() {
            if cached_query == query && at.elapsed() < CACHE_TTL {
                return Ok(value.clone());
            }
        }
    }
    let response = ureq::get(&format!("https://openrouter.ai/api/v1/models{query}"))
        .header("Authorization", &format!("Bearer {key}"))
        .call()
        .map_err(|error| format!("не удалось получить список моделей: {error}"))?;
    let text = response
        .into_body()
        .read_to_string()
        .map_err(|error| format!("не прочитать ответ со списком моделей: {error}"))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| format!("непонятный ответ со списком моделей: {error}"))?;
    if let Ok(mut cache) = CACHE.lock() {
        *cache = Some((Instant::now(), query.to_string(), value.clone()));
    }
    Ok(value)
}

fn modalities(model: &Value) -> (Vec<String>, Vec<String>) {
    let architecture = model.get("architecture");
    let list = |field: &str| -> Vec<String> {
        architecture
            .and_then(|architecture| architecture.get(field))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    (list("input_modalities"), list("output_modalities"))
}

/// Подходит ли модель под назначение. Смотрим на модальности, а не на имя: имена меняются,
/// а «что на входе и что на выходе» — это контракт.
fn matches(model: &Value, kind: Kind) -> bool {
    let (_input, output) = modalities(model);
    match kind {
        Kind::Text => output.iter().any(|value| value == "text"),
        Kind::Image => output.iter().any(|value| value == "image"),
        Kind::Tts => output.iter().any(|value| value == "speech" || value == "audio"),
        Kind::Stt => output.iter().any(|value| value == "transcription"),
    }
}

/// Модели провайдера, годные для назначения.
pub fn list(key: &str, kind: Kind) -> Result<Vec<CloudModel>, String> {
    let value = fetch(key, kind)?;
    let mut models = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or("в ответе нет списка моделей")?
        .clone();
    // Второй список (для озвучки — чат-модели с аудио) подмешиваем, если он есть.
    if let Some(query) = extra_query_for(kind) {
        if let Ok(extra) = fetch_query(key, query) {
            if let Some(items) = extra.get("data").and_then(Value::as_array) {
                models.extend(items.iter().cloned());
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<CloudModel> = models
        .iter()
        .filter(|model| matches(model, kind))
        .filter_map(|model| {
            let id = model.get("id")?.as_str()?.to_string();
            // Служебные псевдо-модели маршрутизатора в выбор не показываем: под конкретную
            // стадию нужна предсказуемая модель, а не «что-нибудь подходящее».
            if id.starts_with("openrouter/auto") || !seen.insert(id.clone()) {
                return None;
            }
            let pricing = model.get("pricing");
            Some(CloudModel {
                name: model
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&id)
                    .to_string(),
                id,
                prompt_price: pricing
                    .and_then(|pricing| pricing.get("prompt"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                completion_price: pricing
                    .and_then(|pricing| pricing.get("completion"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Голоса модели озвучки. У провайдера их в списке моделей нет, поэтому для известных
/// семейств отдаём их набор, а для незнакомой модели — пусто, и голос вводится вручную.
pub fn voices_for(model: &str) -> Vec<&'static str> {
    // Набор голосов моделей OpenAI, единственного семейства с озвучкой в текущем списке.
    const OPENAI: [&str; 11] = [
        "alloy", "ash", "ballad", "coral", "echo", "fable", "nova", "onyx", "sage", "shimmer",
        "verse",
    ];
    if model.starts_with("openai/") {
        OPENAI.to_vec()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model(id: &str, input: &[&str], output: &[&str]) -> Value {
        json!({
            "id": id,
            "name": id,
            "architecture": { "input_modalities": input, "output_modalities": output }
        })
    }

    #[test]
    fn purpose_is_decided_by_modalities_not_by_name() {
        let speech = model("некий/провайдер-икс", &["text"], &["speech"]);
        assert!(matches(&speech, Kind::Tts));
        assert!(!matches(&speech, Kind::Image));

        let vision = model("некий/картинки", &["text"], &["image"]);
        assert!(matches(&vision, Kind::Image));
        assert!(!matches(&vision, Kind::Tts));

        // Распознавание тоже определяется ВЫХОДОМ: провайдер помечает такие модели
        // `transcription`, а вход `audio` есть и у обычных мультимодальных чат-моделей.
        let listener = model("некий/слушатель", &["text", "audio"], &["transcription"]);
        assert!(matches(&listener, Kind::Stt));
        assert!(!matches(&listener, Kind::Text));
    }

    #[test]
    fn a_model_without_modalities_matches_nothing() {
        let bare = json!({ "id": "x/y", "name": "x" });
        for kind in [Kind::Text, Kind::Image, Kind::Tts, Kind::Stt] {
            assert!(!matches(&bare, kind));
        }
    }

    #[test]
    fn purpose_names_from_the_interface_are_understood() {
        assert_eq!(Kind::parse("tts"), Some(Kind::Tts));
        assert_eq!(Kind::parse("asr"), Some(Kind::Stt));
        assert_eq!(Kind::parse("narrator"), Some(Kind::Text));
        assert_eq!(Kind::parse("чепуха"), None);
    }

    #[test]
    fn known_families_offer_their_voices_and_unknown_ones_stay_manual() {
        assert!(voices_for("openai/gpt-audio").contains(&"onyx"));
        assert!(voices_for("некий/новый-голос").is_empty());
    }
}
