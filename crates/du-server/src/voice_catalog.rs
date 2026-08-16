//! Справочник облачных голосов: пол, возраст и то, тянет ли голос русскую речь.
//!
//! Список голосов провайдер по API не отдаёт, поэтому он собран перебором живыми запросами
//! и вшит в приложение — интерфейсу набор нужен до первого обращения к сети. Русский тут не
//! формальность: голос, который его не тянет, звучит ломано, и предлагать его для русской
//! игры нельзя.
//!
//! Сам перечень МОДЕЛЕЙ вшивать нельзя, он протухает (см. `cloud_models`): справочник
//! отвечает только на вопрос «какие голоса у этой модели», а не «какие модели бывают».

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceMeta {
    pub name: String,
    /// male | female | neutral | unknown
    pub gender: String,
    /// child | teen | adult | elderly | unknown
    pub age: String,
    /// Годится ли голос для русской речи.
    #[serde(default)]
    pub ru: bool,
}

#[derive(Debug, Deserialize)]
pub struct ModelVoices {
    #[serde(default)]
    pub supports_russian: bool,
    pub voices: Vec<VoiceMeta>,
}

fn catalog() -> &'static HashMap<String, ModelVoices> {
    static DB: OnceLock<HashMap<String, ModelVoices>> = OnceLock::new();
    DB.get_or_init(|| serde_json::from_str(include_str!("voice_db.json")).unwrap_or_default())
}

/// Все модели озвучки, о которых мы знаем.
pub fn models() -> Vec<String> {
    let mut names: Vec<String> = catalog().keys().cloned().collect();
    names.sort();
    names
}

/// Голоса модели. None — модели нет в справочнике, тогда выбор голоса остаётся ручным.
pub fn voices(model: &str) -> Option<&'static Vec<VoiceMeta>> {
    catalog().get(model).map(|entry| &entry.voices)
}

/// Пол голоса по его имени, из любой модели справочника: имена у провайдера не
/// повторяются между семействами, а звать голос надо уметь и без знания модели.
pub fn gender_of(name: &str) -> Option<&'static str> {
    catalog().values().find_map(|entry| {
        entry
            .voices
            .iter()
            .find(|voice| voice.name.eq_ignore_ascii_case(name))
            .map(|voice| voice.gender.as_str())
    })
}

/// Возраст голоса по имени — там, где он в справочнике записан.
pub fn age_of(name: &str) -> Option<&'static str> {
    catalog().values().find_map(|entry| {
        entry
            .voices
            .iter()
            .find(|voice| voice.name.eq_ignore_ascii_case(name))
            .map(|voice| voice.age.as_str())
    })
}

/// Тянет ли модель русский — интерфейс должен предупредить до того, как игрок услышит кашу.
pub fn supports_russian(model: &str) -> Option<bool> {
    catalog().get(model).map(|entry| entry.supports_russian)
}

/// Голоса, подходящие для языка игры и, если задан, пола персонажа.
pub fn suitable(model: &str, russian: bool, gender: Option<&str>) -> Vec<&'static VoiceMeta> {
    let Some(all) = voices(model) else { return Vec::new() };
    let mut pool: Vec<&VoiceMeta> = all
        .iter()
        .filter(|voice| !russian || voice.ru)
        .filter(|voice| gender.is_none_or(|want| voice.gender == want))
        .collect();
    // Детские и подростковые голоса не должны доставаться взрослому персонажу по умолчанию.
    let adultish: Vec<&VoiceMeta> = pool
        .iter()
        .copied()
        .filter(|voice| matches!(voice.age.as_str(), "adult" | "unknown" | "elderly"))
        .collect();
    if !adultish.is_empty() {
        pool = adultish;
    }
    pool
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Модель, реально доступная у провайдера на момент пересборки справочника.
    const LIVE: &str = "openai/gpt-audio";
    /// Настоящая модель озвучки — их у провайдера 18, и раньше справочник их не видел.
    const SPEECH: &str = "google/gemini-3.1-flash-tts-preview";

    #[test]
    fn the_catalog_loads_and_knows_the_models() {
        let models = models();
        assert!(!models.is_empty(), "справочник пуст");
        assert!(models.iter().any(|model| model == LIVE));
        assert!(
            models.iter().any(|model| model == SPEECH),
            "модели озвучки ищутся по модальности speech, а не audio — иначе их в списке нет"
        );
        assert!(models.len() >= 12, "справочник неполон: {} моделей", models.len());
    }

    #[test]
    fn a_voice_can_be_asked_about_its_gender_without_knowing_the_model() {
        assert_eq!(gender_of("onyx"), Some("male"));
        assert_eq!(gender_of("Nova"), Some("female"));
        assert_eq!(gender_of("нет-такого-голоса"), None);
    }

    #[test]
    fn gender_is_known_for_named_voices() {
        let voices = voices(LIVE).expect("модель есть в справочнике");
        let onyx = voices.iter().find(|voice| voice.name.eq_ignore_ascii_case("onyx")).unwrap();
        assert_eq!(onyx.gender, "male");
        let nova = voices.iter().find(|voice| voice.name.eq_ignore_ascii_case("nova")).unwrap();
        assert_eq!(nova.gender, "female");
    }

    #[test]
    fn russian_support_is_recorded_and_unknown_models_stay_unknown() {
        assert_eq!(supports_russian(LIVE), Some(true));
        assert_eq!(supports_russian("нет-такой-модели"), None);
    }

    #[test]
    fn a_russian_game_only_gets_voices_that_actually_speak_russian() {
        let voices = suitable(LIVE, true, None);
        assert!(!voices.is_empty());
        assert!(voices.iter().all(|voice| voice.ru));
    }

    #[test]
    fn filtering_by_gender_keeps_only_that_gender() {
        let male = suitable(LIVE, false, Some("male"));
        assert!(!male.is_empty());
        assert!(male.iter().all(|voice| voice.gender == "male"));
    }

    #[test]
    fn an_unknown_model_yields_an_empty_list_rather_than_a_panic() {
        assert!(voices("нет-такой-модели").is_none());
        assert!(suitable("нет-такой-модели", true, None).is_empty());
    }
}
