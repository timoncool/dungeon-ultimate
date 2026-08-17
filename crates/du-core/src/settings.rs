//! Настройки истории и перечисления, которые видит фронт.

use serde::{Deserialize, Serialize};

/// Пропорции кадра, которые нарратор выбирает для сцены.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AspectPreset {
    #[default]
    Square,
    Portrait,
    Landscape,
}


/// Быстрый кадр (1024 по длинной стороне) против качественного (2048).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ImageMode {
    #[default]
    Fast,
    Slow,
}


/// Крупность плана. Жёсткая смена плана считается новым кадром, поэтому сцена
/// перерисовывается заново, а не редактируется поверх предыдущей.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageShot {
    Wide,
    Medium,
    Close,
}

/// Роль модели в пайплайне картинок. `Scene` рисует кадр с нуля, `Edit` эволюционирует
/// предыдущий по референсу, `Poster` — вариант под текст в кадре.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum ImageBackend {
    /// Krea-2 Turbo — художник сцены.
    #[default]
    Krea2Turbo,
    /// FLUX.2-klein 9B — редактор по референсу, держит консистентность.
    Klein9bEdit,
    /// Ideogram-4 — когда в кадре нужен читаемый текст.
    Ideogram4,
}


/// Размер шрифта прозы.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ProseSize {
    Tiny,
    Xsmall,
    Small,
    #[default]
    Medium,
    Large,
    Xlarge,
    Xxlarge,
    Huge,
    Giant,
}


/// Длина ответа нарратора.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ResponseLength {
    Short,
    #[default]
    Medium,
    Long,
    Epic,
}


/// Язык игры: на нём говорят нарратор, чипсы действий, подсказки и озвучка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Language {
    #[default]
    Ru,
    En,
    Es,
    Fr,
    De,
    Zh,
    Ja,
}


impl Language {
    pub const ALL: [Language; 7] = [
        Language::Ru,
        Language::En,
        Language::Es,
        Language::Fr,
        Language::De,
        Language::Zh,
        Language::Ja,
    ];

    /// Родное название для переключателя языка.
    pub fn label(self) -> &'static str {
        match self {
            Language::Ru => "Русский",
            Language::En => "English",
            Language::Es => "Español",
            Language::Fr => "Français",
            Language::De => "Deutsch",
            Language::Zh => "中文",
            Language::Ja => "日本語",
        }
    }

    /// Как язык называется МОДЕЛИ — с родным примером, чтобы она на нём и осталась.
    pub fn prompt_name(self) -> &'static str {
        match self {
            Language::Ru => "Russian (русский)",
            Language::En => "English",
            Language::Es => "Spanish (español)",
            Language::Fr => "French (français)",
            Language::De => "German (Deutsch)",
            Language::Zh => "Simplified Chinese (简体中文)",
            Language::Ja => "Japanese (日本語)",
        }
    }

    /// Код языка для движка озвучки.
    pub fn tts_code(self) -> &'static str {
        match self {
            Language::Ru => "ru",
            Language::En => "en",
            Language::Es => "es",
            Language::Fr => "fr",
            Language::De => "de",
            Language::Zh => "zh",
            Language::Ja => "ja",
        }
    }
}

/// Откуда берётся текстовая модель.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TextProvider {
    /// Встроенный сайдкар llama-server.
    #[default]
    Local,
    /// Любой OpenAI-совместимый бэкенд, заданный в приложении.
    Custom,
}


/// Настройки истории. Хранятся как JSON на строке чата, поэтому имена полей обязаны
/// совпадать с прежним контрактом `StorySettings` — иначе старые базы не прочитаются.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StorySettings {
    pub world: String,
    pub style: String,
    /// Пустая строка = взять встроенный промпт нарратора для выбранного языка.
    pub narrator_prompt: String,
    pub image_prompt: String,
    /// Дописывается в начало КАЖДОГО промпта кадра, чтобы арт держал один стиль.
    pub image_style_prefix: String,
    /// Подмешивать в системный промпт последние «биты» сцен и требовать разнообразия.
    pub anti_repetition: bool,
    /// Писать осмысленный эпилог, когда история действительно заканчивается.
    pub cause_aware_ending: bool,
    /// Читать реплики персонажей их собственными голосами.
    pub multi_voice: bool,
    /// Ехидный внутриигровой спутник, добавляющий короткую реплику к сцене.
    pub companion: bool,
    pub text_provider: TextProvider,
    pub local_text_model: String,
    pub custom_base_url: String,
    pub custom_model: String,
    pub custom_api_key: String,
    pub image_mode: ImageMode,
    pub image_backend: ImageBackend,
    pub aspect: AspectPreset,
    pub image_generation_enabled: bool,
    pub auto_images: bool,
    pub rpg_enabled: bool,
    pub random_events: bool,
    pub dice_enabled: bool,
    pub dice_sound: bool,
    pub dice_volume: f32,
    pub prose_size: ProseSize,
    pub response_length: ResponseLength,
    pub language: Language,
    pub voice: String,
    pub autoplay: bool,
    pub tts_volume: f32,
    pub tts_speed: f32,
}

impl Default for StorySettings {
    fn default() -> Self {
        Self {
            world: String::new(),
            style: String::new(),
            narrator_prompt: String::new(),
            image_prompt: String::new(),
            image_style_prefix: String::new(),
            anti_repetition: true,
            cause_aware_ending: true,
            // Реплики персонажей своими голосами — по умолчанию ВКЛЮЧЕНО: ради этого
            // и собран справочник голосов, а слушать всю игру одним голосом скучно.
            multi_voice: true,
            companion: false,
            text_provider: TextProvider::default(),
            local_text_model: String::new(),
            custom_base_url: String::new(),
            custom_model: String::new(),
            custom_api_key: String::new(),
            image_mode: ImageMode::default(),
            image_backend: ImageBackend::default(),
            aspect: AspectPreset::default(),
            image_generation_enabled: true,
            auto_images: true,
            rpg_enabled: true,
            random_events: true,
            dice_enabled: true,
            dice_sound: true,
            dice_volume: 0.6,
            prose_size: ProseSize::default(),
            response_length: ResponseLength::default(),
            language: Language::default(),
            // Голос по умолчанию — клон эталонного «Ведущего» из пака: игра начинается
            // озвученной, а не молчаливой.
            voice: "Vedushchiy".to_string(),
            autoplay: true,
            tts_volume: 1.0,
            tts_speed: 1.0,
        }
    }
}

/// Размер кадра по режиму и пропорциям — порт `dimensionsForImage`.
pub fn dimensions_for_image(mode: ImageMode, aspect: AspectPreset) -> (u32, u32) {
    let long_side: f32 = if matches!(mode, ImageMode::Slow) { 2048.0 } else { 1024.0 };
    match aspect {
        AspectPreset::Portrait => ((long_side * 0.75).round() as u32, long_side as u32),
        AspectPreset::Landscape => (long_side as u32, (long_side * 0.75).round() as u32),
        AspectPreset::Square => (long_side as u32, long_side as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_match_the_typescript_table() {
        assert_eq!(dimensions_for_image(ImageMode::Fast, AspectPreset::Square), (1024, 1024));
        assert_eq!(dimensions_for_image(ImageMode::Slow, AspectPreset::Square), (2048, 2048));
        assert_eq!(dimensions_for_image(ImageMode::Fast, AspectPreset::Portrait), (768, 1024));
        assert_eq!(dimensions_for_image(ImageMode::Slow, AspectPreset::Landscape), (2048, 1536));
    }

    #[test]
    fn settings_round_trip_through_camel_case_json() {
        let json = serde_json::to_string(&StorySettings::default()).unwrap();
        assert!(json.contains("\"narratorPrompt\""));
        assert!(json.contains("\"imageStylePrefix\""));
        let back: StorySettings = serde_json::from_str(&json).unwrap();
        assert!(back.rpg_enabled);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let settings: StorySettings = serde_json::from_str("{\"world\":\"лес\"}").unwrap();
        assert_eq!(settings.world, "лес");
        assert!(matches!(settings.language, Language::Ru));
        assert!(settings.image_generation_enabled);
    }
}
