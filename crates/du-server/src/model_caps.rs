//! Схемы облачных моделей — как их описывает сам провайдер.
//!
//! Каталог `/api/v1/models` отдаёт по каждой модели всё, что нужно, чтобы не гадать:
//!
//! * `reasoning.mandatory` — можно ли вообще отключить «размышления». У Gemini и Grok
//!   отключать НЕЛЬЗЯ, они отвечают отказом; у DeepSeek можно.
//! * `reasoning.supported_efforts` — какие уровни усилия принимает модель. Просить `low`
//!   у модели, которая знает только `high` и `xhigh`, — гарантированный пустой ответ.
//! * `supported_parameters` — принимает ли она строгий разбор по схеме и прочее.
//! * `default_parameters` — её собственные значения температуры и `top_p`.
//! * `supported_voices` — голоса модели озвучки, прямо из схемы.
//! * `top_provider.max_completion_tokens` — потолок ответа.
//!
//! Каталог тянем ОДИН раз и держим в памяти: он общий на все модели и меняется редко.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

const CATALOG: &str = "https://openrouter.ai/api/v1/models";
const CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);

/// Что провайдер рассказал о модели.
#[derive(Debug, Clone, Default)]
pub struct Caps {
    /// Схема нашлась — значит, ответам ниже можно верить.
    pub known: bool,
    /// Модель принимает поле `reasoning`.
    pub reasoning: bool,
    /// Размышления отключить НЕЛЬЗЯ — можно только спрятать из ответа.
    pub reasoning_mandatory: bool,
    /// Уровни усилия, которые она принимает, от слабого к сильному.
    pub efforts: Vec<String>,
    /// Умеет строгий разбор по схеме.
    pub structured: bool,
    /// Её собственная температура по умолчанию.
    pub default_temperature: Option<f32>,
    /// Потолок длины ответа.
    pub max_completion_tokens: Option<u32>,
    /// Голоса модели озвучки, как их перечисляет схема.
    pub voices: Vec<String>,
}

impl Caps {
    /// Самый слабый уровень усилия из тех, что модель принимает.
    ///
    /// Слабее — быстрее и дешевле, а для служебных проходов глубокие размышления только
    /// сжигают лимит. Просить уровень «мимо списка» нельзя: ответ придёт пустым.
    pub fn cheapest_effort(&self) -> Option<&str> {
        for wanted in ["minimal", "low", "medium", "high", "xhigh"] {
            if let Some(found) = self.efforts.iter().find(|effort| effort == &wanted) {
                return Some(found);
            }
        }
        self.efforts.last().map(String::as_str)
    }
}

fn cache() -> &'static Mutex<Option<(Instant, HashMap<String, Caps>)>> {
    static CACHE: OnceLock<Mutex<Option<(Instant, HashMap<String, Caps>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn parse(model: &Value) -> Caps {
    let params: HashSet<&str> = model
        .get("supported_parameters")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let reasoning = model.get("reasoning");
    Caps {
        known: true,
        reasoning: params.contains("reasoning") || reasoning.is_some(),
        reasoning_mandatory: reasoning
            .and_then(|block| block.get("mandatory"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        efforts: reasoning
            .and_then(|block| block.get("supported_efforts"))
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default(),
        structured: params.contains("structured_outputs") || params.contains("response_format"),
        default_temperature: model
            .pointer("/default_parameters/temperature")
            .and_then(Value::as_f64)
            .map(|value| value as f32),
        max_completion_tokens: model
            .pointer("/top_provider/max_completion_tokens")
            .and_then(Value::as_u64)
            .map(|value| value as u32),
        voices: model
            .get("supported_voices")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|voice| {
                        voice.as_str().map(str::to_string).or_else(|| {
                            voice.get("name").and_then(Value::as_str).map(str::to_string)
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn fetch(key: &str) -> Result<HashMap<String, Caps>, String> {
    let response = ureq::get(CATALOG)
        .header("Authorization", &format!("Bearer {key}"))
        .call()
        .map_err(|error| error.to_string())?;
    let body = response.into_body().read_to_string().map_err(|error| error.to_string())?;
    let parsed: Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    let list = parsed.get("data").and_then(Value::as_array).ok_or("в ответе нет списка моделей")?;

    let mut catalog = HashMap::new();
    for model in list {
        if let Some(id) = model.get("id").and_then(Value::as_str) {
            catalog.insert(id.to_string(), parse(model));
        }
    }
    if catalog.is_empty() {
        return Err("каталог пуст".into());
    }
    Ok(catalog)
}

/// Схема конкретной модели. Спросить не вышло — «неизвестно», и вызывающий работает по
/// общему пути, а не отказывается от хода.
pub fn caps(model: &str, key: &str) -> Caps {
    let model = model.trim();
    if model.is_empty() || key.trim().is_empty() {
        return Caps::default();
    }
    if let Ok(guard) = cache().lock() {
        if let Some((at, catalog)) = guard.as_ref() {
            if at.elapsed() < CACHE_TTL {
                return catalog.get(model).cloned().unwrap_or_default();
            }
        }
    }
    match fetch(key) {
        Ok(catalog) => {
            let caps = catalog.get(model).cloned().unwrap_or_default();
            if let Ok(mut guard) = cache().lock() {
                *guard = Some((Instant::now(), catalog));
            }
            caps
        }
        Err(error) => {
            tracing::debug!("каталог моделей не получен: {error}");
            Caps::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_mandatory_reasoning_model_is_recognised() {
        let caps = parse(&json!({
            "supported_parameters": ["reasoning", "reasoning_effort", "structured_outputs"],
            "reasoning": { "mandatory": true, "supported_efforts": ["high", "medium", "low"] }
        }));
        assert!(caps.reasoning && caps.reasoning_mandatory && caps.structured);
        assert_eq!(caps.cheapest_effort(), Some("low"));
    }

    #[test]
    fn an_effort_outside_the_list_is_never_chosen() {
        // DeepSeek принимает только high и xhigh: просить low бессмысленно.
        let caps = parse(&json!({
            "reasoning": { "mandatory": false, "supported_efforts": ["xhigh", "high"] }
        }));
        assert_eq!(caps.cheapest_effort(), Some("high"));
        assert!(!caps.reasoning_mandatory);
    }

    #[test]
    fn voices_and_limits_come_from_the_schema() {
        let caps = parse(&json!({
            "supported_voices": ["Zephyr", { "name": "Puck" }],
            "default_parameters": { "temperature": 0.7 },
            "top_provider": { "max_completion_tokens": 4096 }
        }));
        assert_eq!(caps.voices, vec!["Zephyr", "Puck"]);
        assert_eq!(caps.default_temperature, Some(0.7));
        assert_eq!(caps.max_completion_tokens, Some(4096));
    }

    #[test]
    fn an_unknown_model_claims_nothing() {
        let caps = Caps::default();
        assert!(!caps.known && caps.efforts.is_empty() && caps.cheapest_effort().is_none());
    }
}
