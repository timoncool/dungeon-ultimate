//! Чат, сообщения, персонажи, сцены — зеркало `StoryChat`/`StoryMessage`/`StoryCharacter`.

use serde::{Deserialize, Serialize};

use crate::settings::{AspectPreset, ImageBackend, ImageMode, ImageShot, StorySettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoryRole {
    User,
    Assistant,
}

/// Файл, приложенный игроком к реплике. `data_url` заполняется, когда байты нужно
/// отдать движку картинок инлайном.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data_url: Option<String>,
}

/// Заявка на кадр, которую отдаёт отдельный проход-«оператор».
///
/// `location` — короткий СТАБИЛЬНЫЙ ярлык места, он повторяется дословно, пока сцена
/// стоит там же; `same_location` говорит, что этот кадр — то же физическое место, что и
/// прошлый иллюстрированный ход, поэтому картинку надо эволюционировать, а не рисовать
/// заново; `shot` — крупность плана. Решение «редактировать или рисовать с нуля»
/// принимается на сервере против состояния сцен чата.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRequest {
    pub needed: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mode: Option<ImageMode>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backend: Option<ImageBackend>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub aspect: Option<AspectPreset>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub character_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub same_location: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub shot: Option<ImageShot>,
}

/// Готовый кадр. `scene_location` — нормализованное место, которому кадр принадлежит;
/// `edited_from` — URL предыдущего кадра, из которого этот вырос (None на свежем
/// установочном плане), чтобы можно было проследить цепочку правок.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImage {
    pub id: String,
    pub url: String,
    pub prompt: String,
    pub mode: ImageMode,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backend: Option<ImageBackend>,
    pub aspect: AspectPreset,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub elapsed_seconds: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scene_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub edited_from: Option<String>,
    /// Кто НА САМОМ ДЕЛЕ нарисовал кадр.
    ///
    /// `backend` — это перечисление своих движков, и облачную модель им не назвать: под
    /// кадром от Gemini стояла подпись «krea2-turbo», выбранная в настройках карты.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rendered_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryMessage {
    pub id: String,
    pub role: StoryRole,
    pub content: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub image_request: Option<ImageRequest>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub generated_image: Option<GeneratedImage>,
    /// Снимок RPG-состояния ДО хода — по нему Retry/Erase откатывает последствия.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rpg_snapshot: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryCharacter {
    pub id: String,
    pub chat_id: String,
    pub name: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub inventory: String,
    #[serde(default)]
    pub skills: String,
    #[serde(default)]
    pub spells: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub portrait: Option<Attachment>,
    /// Голос персонажа для мультиголосой озвучки. Пусто = голос нарратора.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub voice: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryChatSummary {
    pub id: String,
    pub title: String,
    pub settings: StorySettings,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryChat {
    #[serde(flatten)]
    pub summary: StoryChatSummary,
    pub messages: Vec<StoryMessage>,
    pub characters: Vec<StoryCharacter>,
}

/// Состояние одной локации для непрерывности картинки. `anchor` — чистый установочный
/// кадр места, `last` — последний кадр в нём, `hops` — сколько правок подряд уже сделано
/// (по достижении лимита сцена переустанавливается свежим кадром, чтобы не накапливать дрейф).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub location: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub anchor: Option<Attachment>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last: Option<Attachment>,
    pub hops: u32,
    pub updated_at: String,
}

/// Артикли, которые снимаются с начала ярлыка места, — по одному на каждый язык игры.
const LOCATION_ARTICLES: [&str; 9] = ["the", "a", "an", "le", "la", "les", "el", "los", "der"];

/// Привести ярлык места от нарратора к устойчивому ключу: нижний регистр, снятая
/// пунктуация, схлопнутые пробелы, отброшенный ведущий артикль, обрезка до 80 символов —
/// чтобы «The Green Meadow.» и «green meadow» попали в одну локацию.
///
/// Пустая строка означает «места нет»: такой кадр не привязывается к сцене.
pub fn normalize_location(label: &str) -> String {
    let cleaned: String = label
        .to_lowercase()
        .chars()
        .map(|c| if "«»\"'`.,;:!?()[]".contains(c) { ' ' } else { c })
        .collect();
    let mut key = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");

    // Артикль снимается только как отдельное первое слово, иначе «lame cave» превратилась
    // бы в «me cave».
    for article in LOCATION_ARTICLES {
        if let Some(rest) = key.strip_prefix(article) {
            if rest.starts_with(' ') {
                key = rest.trim_start().to_string();
                break;
            }
        }
    }
    // «das» отдельно: в исходном списке он есть, но выше цикл уже мог сработать.
    if let Some(rest) = key.strip_prefix("das ") {
        key = rest.trim_start().to_string();
    }
    if let Some(rest) = key.strip_prefix("die ") {
        key = rest.trim_start().to_string();
    }

    key.chars().take(80).collect::<String>().trim_end().to_string()
}

/// Тот же ключ, но `None` вместо пустой строки — удобно для необязательного ярлыка.
pub fn normalize_location_opt(raw: Option<&str>) -> Option<String> {
    let key = normalize_location(raw?);
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_normalization_collapses_case_and_spacing() {
        assert_eq!(normalize_location("  Green   Meadow "), "green meadow");
        assert_eq!(normalize_location_opt(Some("   ")), None);
        assert_eq!(normalize_location_opt(None), None);
    }

    #[test]
    fn punctuation_and_a_leading_article_are_stripped() {
        // Именно этот случай и должен схлопываться в один ключ сцены.
        assert_eq!(normalize_location("The Green Meadow."), "green meadow");
        assert_eq!(normalize_location("«Крипта пепла»"), "крипта пепла");
        assert_eq!(normalize_location("La Taverne"), "taverne");
        assert_eq!(normalize_location("Das Tor"), "tor");
    }

    #[test]
    fn an_article_is_only_stripped_as_a_whole_word() {
        assert_eq!(normalize_location("Lame Cave"), "lame cave");
        assert_eq!(normalize_location("Anthill"), "anthill");
        assert_eq!(normalize_location("Theatre"), "theatre");
    }

    #[test]
    fn very_long_labels_are_capped() {
        let key = normalize_location(&"зал ".repeat(60));
        assert!(key.chars().count() <= 80);
        assert!(!key.ends_with(' '));
    }

    #[test]
    fn message_json_keeps_the_camel_case_contract() {
        let message = StoryMessage {
            id: "m1".into(),
            role: StoryRole::Assistant,
            content: "текст".into(),
            created_at: "2026-08-15T00:00:00.000Z".into(),
            attachments: vec![],
            image_request: None,
            generated_image: None,
            rpg_snapshot: None,
        };
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"createdAt\""));
        // Пустые поля не должны засорять ответ фронту.
        assert!(!json.contains("attachments"));
        assert!(!json.contains("imageRequest"));
    }
}
