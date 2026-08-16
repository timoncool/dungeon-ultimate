//! Кто каким голосом говорит.
//!
//! Голос персонажу подбирает сама модель: ей отдают отрывок и перечень доступных голосов с
//! пометками, а она возвращает раскладку. Это точнее любых догадок по словам в листе —
//! модель видит, что «трактирщик — грузный мужик с руками-лопатами», и берёт низкий голос,
//! а не считает буквы в описании.
//!
//! Догадки остаются запасным путём: сеть моргнула или модель ответила чепухой — раскладку
//! доберёт разбор по полу и возрасту, чтобы озвучка не пропала совсем.

use serde_json::{json, Value};

use du_core::StoryCharacter;
use du_llm::{ChatClient, Message, Sampling};

/// Раскладка: имя персонажа → имя голоса.
pub type Casting = std::collections::HashMap<String, String>;

/// Описание голоса для модели: имя и то, что мы о нём знаем.
fn describe(voices: &[(String, String, String)]) -> String {
    voices
        .iter()
        .map(|(name, gender, age)| {
            let gender = match gender.as_str() {
                "female" => "женский",
                "male" => "мужской",
                _ => "нейтральный",
            };
            let age = match age.as_str() {
                "child" | "teen" => ", молодой",
                "elderly" => ", пожилой",
                _ => "",
            };
            format!("{name} ({gender}{age})")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Попросить модель разложить голоса по говорящим.
///
/// Возвращает пустую раскладку, если спросить не вышло: молчание здесь лучше выдумки, а
/// вызывающий подберёт голоса сам.
pub fn assign(
    client: &ChatClient,
    passage: &str,
    characters: &[StoryCharacter],
    voices: &[(String, String, String)],
) -> Casting {
    if voices.is_empty() || characters.is_empty() || passage.trim().is_empty() {
        return Casting::new();
    }
    let names: Vec<&str> = characters.iter().map(|character| character.name.as_str()).collect();
    let sheets = characters
        .iter()
        .map(|character| format!("{} — {}", character.name, character.details.trim()))
        .collect::<Vec<_>>()
        .join("\n");

    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "casting": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "character": { "type": "string", "description": "имя персонажа из списка" },
                        "voice": { "type": "string", "description": "имя голоса из списка" }
                    },
                    "required": ["character", "voice"]
                }
            }
        },
        "required": ["casting"]
    });

    let messages = vec![
        Message::system(
            "Ты подбираешь голоса для озвучки ролевой игры. Тебе дают персонажей и перечень \
             доступных голосов. Для КАЖДОГО персонажа выбери голос, который ему подходит по \
             полу, возрасту и характеру. Один голос можно дать только одному персонажу. \
             Отвечай ТОЛЬКО именами из перечня, ничего не выдумывай."
                .to_string(),
        ),
        Message::user(format!(
            "ПЕРСОНАЖИ:\n{sheets}\n\nДОСТУПНЫЕ ГОЛОСА: {}\n\nОТРЫВОК:\n{}",
            describe(voices),
            passage.chars().take(1200).collect::<String>()
        )),
    ];

    let value = match client.chat_json(&messages, &Sampling::new(0.3, 600), &schema) {
        Ok(value) => value,
        Err(error) => {
            tracing::debug!("кастинг голосов не удался: {error}");
            return Casting::new();
        }
    };

    let known: std::collections::HashSet<&str> =
        voices.iter().map(|(name, _, _)| name.as_str()).collect();
    let mut casting = Casting::new();
    let mut taken: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in value.get("casting").and_then(Value::as_array).unwrap_or(&Vec::new()) {
        let character = entry.get("character").and_then(Value::as_str).unwrap_or_default().trim();
        let voice = entry.get("voice").and_then(Value::as_str).unwrap_or_default().trim();
        // Берём только то, что существует: модель могла придумать имя, которого нет.
        if character.is_empty() || !known.contains(voice) {
            continue;
        }
        if !names.iter().any(|name| name.eq_ignore_ascii_case(character)) {
            continue;
        }
        // Один голос — одному персонажу: иначе двое зазвучат одинаково.
        if !taken.insert(voice.to_string()) {
            continue;
        }
        casting.insert(character.to_string(), voice.to_string());
    }
    casting
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voices() -> Vec<(String, String, String)> {
        vec![
            ("Kore".into(), "female".into(), "adult".into()),
            ("Charon".into(), "male".into(), "adult".into()),
            ("Leda".into(), "female".into(), "teen".into()),
        ]
    }

    #[test]
    fn the_voice_list_is_described_in_words_the_model_can_use() {
        let text = describe(&voices());
        assert!(text.contains("Kore (женский)"));
        assert!(text.contains("Leda (женский, молодой)"));
        assert!(text.contains("Charon (мужской)"));
    }

    #[test]
    fn nothing_to_cast_means_an_empty_layout() {
        let client = ChatClient::new("http://127.0.0.1:1", std::time::Duration::from_millis(50)).unwrap();
        assert!(assign(&client, "", &[], &voices()).is_empty());
    }
}
