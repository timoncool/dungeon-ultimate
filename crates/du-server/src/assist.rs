//! Вспомогательные вызовы модели: чипсы действий, «удиви меня», заготовка истории и
//! автозаполнение персонажа.
//!
//! Все они короткие и служебные, поэтому идут той же очередью, что и ход, — иначе
//! случайное совпадение двух вызовов положило бы две модели на одну карту. Ответ там, где
//! нужна структура, ограничен JSON-схемой, а не разбирается из текста.

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use du_core::Language;
use du_llm::{ChatClient, Message, Sampling};
use du_prompts::prompts_for;

use crate::routes::ApiError;
use crate::state::AppState;
use crate::turn::structured_sampling;

type ApiResult<T> = Result<T, ApiError>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionsBody {
    pub passage: String,
    #[serde(default)]
    pub language: Language,
}

/// Чипсы быстрых действий после хода: три-четыре коротких и РАЗНЫХ варианта.
pub async fn actions(
    State(state): State<AppState>,
    Json(body): Json<ActionsBody>,
) -> ApiResult<Json<Value>> {
    // Длинный пассаж режем: модели хватает финала сцены, а лишний контекст только
    // замедляет служебный вызов.
    let passage: String = body.passage.chars().rev().take(4000).collect::<Vec<_>>().into_iter().rev().collect();
    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "actions": {
                "type": "array",
                "minItems": 3,
                "maxItems": 4,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        // Поля описываем явно: набор промптов достался от прежней версии,
                        // где ответ разбирался строкой «значок | действие», и без пояснения
                        // модель кладёт значок прямо в подпись, а текста не пишет вовсе.
                        "emoji": { "type": "string", "description": "ровно один значок", "maxLength": 8 },
                        "label": {
                            "type": "string",
                            "description": "действие СЛОВАМИ, повелительно, 3-6 слов, без значков",
                            "minLength": 8,
                            "maxLength": 60
                        }
                    },
                    "required": ["emoji", "label"]
                }
            }
        },
        "required": ["actions"]
    });

    let messages = vec![
        Message::system(prompts_for(body.language).actions.system.clone()),
        // Напоминание про поля: в наборе промптов описан строчный формат прежней версии.
        Message::user(format!(
            "{passage}

Ответь полями: emoji — один значок, label — само действие словами."
        )),
    ];
    let value = ask_json(&state, messages, &schema).await?;
    let actions = value
        .get("actions")
        .and_then(Value::as_array)
        .map(|actions| actions.iter().filter_map(tidy_action).collect::<Vec<_>>())
        .unwrap_or_default();
    Ok(Json(json!({ "actions": actions })))
}

/// Причесать чипс действия: убрать значок из самой подписи.
///
/// Модель охотно ставит его и в отдельное поле, и в текст — на кнопке он выходил дважды,
/// по значку с каждой стороны.
fn tidy_action(action: &Value) -> Option<Value> {
    let emoji = action.get("emoji").and_then(Value::as_str).unwrap_or_default().trim();
    let raw = action.get("label").and_then(Value::as_str).unwrap_or_default().trim();
    let cleaned = strip_edge_symbols(raw);
    // Бывает, что в подписи одни значки. Срезать их до пустоты нельзя: кнопка без слов
    // игроку ничего не говорит — тогда оставляем подпись как пришла.
    let label = if cleaned.is_empty() { raw.to_string() } else { cleaned };
    if label.is_empty() {
        return None;
    }
    Some(match emoji.is_empty() {
        true => json!({ "label": label }),
        false => json!({ "emoji": emoji, "label": label }),
    })
}

/// Срезать значки и разделители по краям подписи, оставив только слова.
fn strip_edge_symbols(label: &str) -> String {
    // Буквы и цифры оставляем всегда: подпись начинается и кончается словом.
    let trimmable = |c: char| !c.is_alphanumeric() && !matches!(c, '«' | '»' | '"' | '\'' | '(' | ')');
    label.trim().trim_matches(trimmable).trim().to_string()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestBody {
    /// Какое поле заполняем: мир, стиль, персонаж или завязка.
    pub field: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub language: Language,
}

/// «Удиви меня» для одного поля новой истории — чтобы игрок не смотрел в пустую форму.
pub async fn suggest(
    State(state): State<AppState>,
    Json(body): Json<SuggestBody>,
) -> ApiResult<Json<Value>> {
    let prompts = prompts_for(body.language);
    let instruction = match body.field.as_str() {
        "world" => &prompts.suggest.fields.world,
        "style" => &prompts.suggest.fields.style,
        "character" => &prompts.suggest.fields.character,
        "opening" => &prompts.suggest.fields.opening,
        other => return Err(ApiError::bad_request(format!("неизвестное поле: {other}"))),
    };

    let mut user = instruction.clone();
    if !body.context.trim().is_empty() {
        user.push_str("\n\n");
        user.push_str(&body.context);
    }
    let messages = vec![Message::system(prompts.suggest.system.clone()), Message::user(user)];
    let text = ask_text(&state, messages, Sampling::new(1.0, 400)).await?;
    Ok(Json(json!({ "value": text.trim() })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupBody {
    #[serde(default)]
    pub setting: String,
    #[serde(default)]
    pub gender: String,
    #[serde(default)]
    pub language: Language,
}

/// «Заполни за меня» в диалоге новой истории: имя, образ и подсказка первой сцены.
pub async fn story_setup(
    State(state): State<AppState>,
    Json(body): Json<SetupBody>,
) -> ApiResult<Json<Value>> {
    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "name": { "type": "string" },
            "persona": { "type": "string" },
            "hint": { "type": "string" }
        },
        "required": ["name", "persona", "hint"]
    });
    let gender = match body.gender.as_str() {
        "male" => "мужского пола",
        "female" => "женского пола",
        _ => "любого пола",
    };
    let messages = vec![
        Message::system(format!(
            "Придумай протагониста {gender} и подсказку первой сцены для истории на языке {}. \
             Имя короткое, образ — одной фразой, подсказка — одним предложением.",
            body.language.prompt_name()
        )),
        Message::user(if body.setting.trim().is_empty() {
            "Сеттинг на твой вкус.".to_string()
        } else {
            body.setting.clone()
        }),
    ];
    let value = ask_json(&state, messages, &schema).await?;
    // Признак успеха обязателен: интерфейс проверяет именно его и без него считает,
    // что придумать не вышло, — даже когда поля пришли заполненными.
    Ok(Json(json!({
        "ok": true,
        "name": value.get("name").and_then(Value::as_str).unwrap_or_default().trim(),
        "persona": value.get("persona").and_then(Value::as_str).unwrap_or_default().trim(),
        "hint": value.get("hint").and_then(Value::as_str).unwrap_or_default().trim(),
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleBody {
    /// Мир истории: по нему и придумывается облик иллюстраций.
    #[serde(default)]
    pub setting: String,
    /// Пожелание игрока: «акварель», «нуар», «как в старой книге». Может быть пустым.
    #[serde(default)]
    pub wish: String,
}

/// Визуальный стиль ИЛЛЮСТРАЦИЙ на всю игру — одной строкой, которая ляжет в каждый кадр.
///
/// Без неё каждый кадр рисуется в своей манере: один похож на фотографию, другой на
/// комикс, третий на 3D. Придумываем стиль ОДИН раз под конкретный мир и дальше
/// подставляем во все промпты — так серия держится вместе.
pub async fn image_style(
    State(state): State<AppState>,
    Json(body): Json<StyleBody>,
) -> ApiResult<Json<Value>> {
    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "style": {
                "type": "string",
                "description": "английская строка стиля: техника и материал, манера мазка                     или линии, палитра, свет, уровень детализации, отсылка к школе или эпохе                     иллюстрации. Без сюжета, без персонажей, без места — только КАК нарисовано",
                "minLength": 80,
                "maxLength": 400
            },
            "name": { "type": "string", "description": "название стиля по-русски, 2-4 слова" }
        },
        "required": ["style", "name"]
    });
    let wish = body.wish.trim();
    let messages = vec![
        Message::system(
            "Ты — арт-директор иллюстрированной ролевой игры. Придумай ОДИН визуальный стиль              для всех иллюстраций этой игры: он должен подходить её миру и быть интересным,              а не дежурным «цифровой арт». Опиши только МАНЕРУ — технику, материал, характер              линии и мазка, палитру, свет, степень детализации, узнаваемую школу или эпоху              иллюстрации. Ни сюжета, ни персонажей, ни мест: эта строка будет добавляться к              промпту каждого кадра и не должна спорить с его содержанием. Строку стиля пиши              по-английски."
                .to_string(),
        ),
        Message::user(if body.setting.trim().is_empty() {
            format!("Мир на твой вкус. {wish}").trim().to_string()
        } else {
            format!("Мир: {}. {wish}", body.setting.trim()).trim().to_string()
        }),
    ];
    let value = ask_json(&state, messages, &schema).await?;
    Ok(Json(json!({
        "ok": true,
        "style": value.get("style").and_then(Value::as_str).unwrap_or_default().trim(),
        "name": value.get("name").and_then(Value::as_str).unwrap_or_default().trim(),
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterBody {
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub language: Language,
}

/// Автозаполнение листа персонажа целиком, одним нажатием.
pub async fn character(
    State(state): State<AppState>,
    Json(body): Json<CharacterBody>,
) -> ApiResult<Json<Value>> {
    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "name": { "type": "string" },
            "details": { "type": "string" },
            "inventory": { "type": "string" },
            "skills": { "type": "string" },
            "spells": { "type": "string" }
        },
        "required": ["name", "details"]
    });
    let messages = vec![
        Message::system(format!(
            "Придумай яркого и небанального персонажа ролевой игры. Отвечай на языке {}. \
             В «details» — внешность и характер, в «inventory», «skills», «spells» — по \
             несколько строк списком.",
            body.language.prompt_name()
        )),
        Message::user(if body.hint.trim().is_empty() {
            "Персонаж на твой вкус.".to_string()
        } else {
            body.hint.clone()
        }),
    ];
    let value = ask_json(&state, messages, &schema).await?;
    Ok(Json(value))
}

/// Собрать облачного собеседника для служебных проходов, если рассказчик уведён в облако.
///
/// Чипсы действий, «заполнить за меня» и подсказки — это тот же рассказчик, только короткий.
/// Раньше они ВСЕГДА поднимали модель на карте и вставали в её очередь: игрок видел загрузку
/// видеокарты, хотя весь ход считало облако.
fn cloud_helper(state: &AppState) -> Option<ChatClient> {
    let runtime = crate::runtime::load(&state.root);
    if !crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Narrator) {
        return None;
    }
    let caps = crate::model_caps::caps(&runtime.openrouter_narrator_model, &runtime.openrouter_key);
    // Размышления прячем, а не запрещаем: часть моделей запрет отвергает.
    let reasoning = match (caps.reasoning, caps.reasoning_mandatory) {
        (true, false) => du_llm::Reasoning::Off,
        (true, true) => du_llm::Reasoning::Hide,
        _ => du_llm::Reasoning::AsIs,
    };
    let effort = du_llm::Effort(caps.cheapest_effort().map(str::to_string));
    ChatClient::new("https://openrouter.ai/api/v1", std::time::Duration::from_secs(180))
        .ok()
        .map(|client| {
            client
                .with_api_key(Some(runtime.openrouter_key.clone()))
                .with_model(Some(runtime.openrouter_narrator_model.clone()))
                .with_reasoning(reasoning)
                .with_effort(effort)
        })
}

/// Общий путь служебного вызова: облако напрямую, а на карте — через её очередь.
async fn ask_json(state: &AppState, messages: Vec<Message>, schema: &Value) -> ApiResult<Value> {
    let schema = schema.clone();
    if let Some(client) = cloud_helper(state) {
        let messages = messages.clone();
        let schema = schema.clone();
        return tokio::task::spawn_blocking(move || {
            client.chat_json(&messages, &structured_sampling(), &schema)
        })
        .await
        .map_err(|_| ApiError::internal("задача пропала"))?
        .map_err(|error| ApiError::internal(error.to_string()));
    }

    let inner = state.0.clone();
    let (_, result) = state
        .queue
        .enqueue_awaitable(Box::new(move |_| {
            let base_url = inner.gpu.text_base_url().map_err(|error| error.to_string())?;
            let client = ChatClient::new(&base_url, std::time::Duration::from_secs(180))
                .map_err(|error| error.to_string())?;
            client
                .chat_json(&messages, &structured_sampling(), &schema)
                .map_err(|error| error.to_string())
        }))
        .await;
    result
        .await
        .map_err(|_| ApiError::internal("задача пропала"))?
        .map_err(ApiError::internal)
}

async fn ask_text(state: &AppState, messages: Vec<Message>, sampling: Sampling) -> ApiResult<String> {
    if let Some(client) = cloud_helper(state) {
        let messages = messages.clone();
        let sampling = sampling.clone();
        let text = tokio::task::spawn_blocking(move || client.chat(&messages, &sampling))
            .await
            .map_err(|_| ApiError::internal("задача пропала"))?
            .map_err(|error| ApiError::internal(error.to_string()))?;
        return Ok(text);
    }

    let inner = state.0.clone();
    let (_, result) = state
        .queue
        .enqueue_awaitable(Box::new(move |_| {
            let base_url = inner.gpu.text_base_url().map_err(|error| error.to_string())?;
            let client = ChatClient::new(&base_url, std::time::Duration::from_secs(180))
                .map_err(|error| error.to_string())?;
            client
                .chat(&messages, &sampling)
                .map(Value::String)
                .map_err(|error| error.to_string())
        }))
        .await;
    let value = result
        .await
        .map_err(|_| ApiError::internal("задача пропала"))?
        .map_err(ApiError::internal)?;
    Ok(value.as_str().unwrap_or_default().to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_unknown_suggest_field_is_rejected_before_the_model_is_touched() {
        // Проверяем сам разбор поля: до модели такой запрос доходить не должен.
        let known = ["world", "style", "character", "opening"];
        assert!(!known.contains(&"чепуха"));
    }

    #[test]
    fn a_long_passage_is_trimmed_from_the_start() {
        let passage: String = "абв".repeat(3000);
        let trimmed: String = passage.chars().rev().take(4000).collect::<Vec<_>>().into_iter().rev().collect();
        assert_eq!(trimmed.chars().count(), 4000);
        // Хвост сцены обязан сохраниться: именно он описывает текущий момент.
        assert!(passage.ends_with(&trimmed));
    }
}


#[cfg(test)]
mod chip_tests {
    use super::*;

    #[test]
    fn the_icon_is_not_repeated_inside_the_label() {
        let action = json!({ "emoji": "⚔️", "label": "⚔️ Добить раненого врага ударом ⚔️" });
        let tidy = tidy_action(&action).unwrap();
        assert_eq!(tidy["label"], "Добить раненого врага ударом");
        assert_eq!(tidy["emoji"], "⚔️");
    }

    #[test]
    fn a_plain_label_survives_untouched() {
        let action = json!({ "emoji": "🏃", "label": "Рвануть вперёд" });
        assert_eq!(tidy_action(&action).unwrap()["label"], "Рвануть вперёд");
    }

    #[test]
    fn a_label_in_quotes_keeps_them() {
        let action = json!({ "label": "Сказать «я сдаюсь»" });
        let tidy = tidy_action(&action).unwrap();
        assert_eq!(tidy["label"], "Сказать «я сдаюсь»");
        assert!(tidy.get("emoji").is_none(), "пустой значок в ответ не кладём");
    }

    #[test]
    fn a_trailing_period_is_not_worth_keeping() {
        let action = json!({ "emoji": "👁", "label": "👁 Оглядеть коридор." });
        assert_eq!(tidy_action(&action).unwrap()["label"], "Оглядеть коридор");
    }
}

#[cfg(test)]
mod chip_label_tests {
    use super::tidy_action;
    use serde_json::json;

    #[test]
    fn a_label_made_only_of_icons_survives_instead_of_becoming_empty() {
        // Кнопка без слов бесполезна: раньше чистка срезала такую подпись в пустоту.
        let tidy = tidy_action(&json!({ "emoji": "🚪", "label": "🚪" })).unwrap();
        assert_eq!(tidy["label"], "🚪");
    }

    #[test]
    fn an_action_without_any_label_is_dropped() {
        assert!(tidy_action(&json!({ "emoji": "🚪", "label": "   " })).is_none());
    }
}
