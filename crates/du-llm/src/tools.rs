//! Инструменты, которыми модель пользуется сама.
//!
//! Раньше состояние игры уезжало модели одним куском текста в подсказке, и оставалось
//! надеяться, что она его прочтёт: задания висели открытыми по сто ходов, потому что модель
//! просто не помнила, что они есть. С инструментами она спрашивает сама — «покажи открытые
//! задания» — и сама говорит, что закрыть.
//!
//! Порядок ролей неизменный и он же общепринятый: модель ПРЕДЛАГАЕТ, движок ПРОВЕРЯЕТ и
//! применяет. Инструмент не даёт модели править состояние напрямую — он принимает намерение,
//! а решение остаётся за игрой. Ответ инструмента для модели важнее её собственных
//! представлений: он и есть истина о состоянии.
//!
//! Договор провайдера (проверено по докам OpenRouter):
//! * в запросе — `tools: [{type:"function", function:{name, description, parameters}}]`;
//! * в ответе — `tool_calls: [{id, type, function:{name, arguments}}]`, где `arguments`
//!   это СТРОКА с JSON, её надо разбирать отдельно;
//! * результат возвращается сообщением `{role:"tool", tool_call_id, content}`.

use serde_json::{json, Value};

/// Описание инструмента для модели.
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    /// Схема аргументов в том же виде, что и у структурных ответов.
    pub parameters: Value,
}

impl Tool {
    pub fn to_value(&self) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters,
            }
        })
    }
}

/// Просьба модели вызвать инструмент.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    /// Идентификатор вызова: с ним же возвращается ответ.
    pub id: String,
    pub name: String,
    /// Разобранные аргументы. Провайдер отдаёт их строкой.
    pub arguments: Value,
}

/// Достать вызовы инструментов из ответа модели.
///
/// Пустой список — обычное дело: модель вправе ответить текстом и ничего не звать.
pub fn parse_calls(message: &Value) -> Vec<ToolCall> {
    let Some(list) = message.get("tool_calls").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|call| {
            let function = call.get("function")?;
            let name = function.get("name").and_then(Value::as_str)?.to_string();
            let raw = function.get("arguments").and_then(Value::as_str).unwrap_or("{}");
            // Аргументы приходят строкой. Кривой JSON не повод ронять ход: считаем, что
            // аргументов нет, — движок разберётся сам и вернёт понятную ошибку.
            let arguments = serde_json::from_str(raw).unwrap_or_else(|_| json!({}));
            Some(ToolCall {
                id: call.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
                name,
                arguments,
            })
        })
        .collect()
}

/// Сообщение с ответом инструмента — ровно в той форме, которую ждёт провайдер.
pub fn result_message(call: &ToolCall, content: &Value) -> Value {
    json!({
        "role": "tool",
        "tool_call_id": call.id,
        "content": content.to_string(),
    })
}

/// Сообщение самой модели с её вызовами: без него провайдер не примет ответы инструментов —
/// им не к чему будет привязаться.
pub fn assistant_message(calls: &[ToolCall]) -> Value {
    json!({
        "role": "assistant",
        "content": Value::Null,
        "tool_calls": calls
            .iter()
            .map(|call| json!({
                "id": call.id,
                "type": "function",
                "function": { "name": call.name, "arguments": call.arguments.to_string() },
            }))
            .collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_are_read_out_of_the_answer() {
        let message = json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": { "name": "quests_open", "arguments": "{}" }
            }, {
                "id": "call_2",
                "type": "function",
                "function": { "name": "quest_complete", "arguments": "{\"title\":\"Пропавший сын\"}" }
            }]
        });
        let calls = parse_calls(&message);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].name, "quest_complete");
        assert_eq!(calls[1].arguments["title"], "Пропавший сын");
    }

    #[test]
    fn a_plain_answer_calls_nothing() {
        assert!(parse_calls(&json!({ "role": "assistant", "content": "просто текст" })).is_empty());
    }

    #[test]
    fn broken_arguments_do_not_break_the_turn() {
        let message = json!({
            "tool_calls": [{ "id": "x", "function": { "name": "quests_open", "arguments": "{сломано" } }]
        });
        let calls = parse_calls(&message);
        assert_eq!(calls.len(), 1, "вызов остаётся, аргументы просто пустые");
        assert_eq!(calls[0].arguments, json!({}));
    }

    #[test]
    fn the_answer_goes_back_in_the_shape_the_provider_expects() {
        let call = ToolCall { id: "call_7".into(), name: "quests_open".into(), arguments: json!({}) };
        let message = result_message(&call, &json!({ "quests": [] }));
        assert_eq!(message["role"], "tool");
        assert_eq!(message["tool_call_id"], "call_7");
        // Содержимое — строка, а не объект: так требует договор.
        assert!(message["content"].is_string());
    }
}
