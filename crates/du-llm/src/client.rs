//! OpenAI-совместимый чат-клиент.
//!
//! Три режима вызова, ровно как в версии на Next.js:
//! обычный буферизованный ответ, **стриминг** (проза нарратора идёт в чат по словам) и
//! **вывод по JSON-схеме** — схема уходит в `response_format`, локальный сервер собирает
//! из неё грамматику и ограничивает сэмплирование, поэтому структурные проходы (события
//! хода, заявка на кадр, чипсы действий) не парсятся из текста руками.
//!
//! Схему модель НЕ видит: она только формирует грамматику. Форма ≠ смысл, поэтому промпт
//! обязан описывать поля словами, а результат — проверяться после разбора.

use std::io::{BufRead, BufReader};
use std::time::Duration;

use serde_json::{json, Map, Value};

use crate::LlmError;

/// Часть составного сообщения — текст или картинка для мультимодальной модели.
#[derive(Clone, Debug)]
pub enum Part {
    Text(String),
    /// PNG в base64 без префикса; уедет как `data:image/png;base64,…`.
    ImagePngB64(String),
}

impl Part {
    fn to_value(&self) -> Value {
        match self {
            Part::Text(text) => json!({ "type": "text", "text": text }),
            Part::ImagePngB64(b64) => json!({
                "type": "image_url",
                "image_url": { "url": format!("data:image/png;base64,{b64}") }
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Message {
    pub role: String,
    text: Option<String>,
    parts: Option<Vec<Part>>,
}

impl Message {
    fn of(role: &str, text: impl Into<String>) -> Self {
        Self { role: role.into(), text: Some(text.into()), parts: None }
    }

    pub fn system(text: impl Into<String>) -> Self {
        Self::of("system", text)
    }

    pub fn user(text: impl Into<String>) -> Self {
        Self::of("user", text)
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self::of("assistant", text)
    }

    /// Составное сообщение игрока: текст плюс приложенные картинки.
    pub fn user_parts(parts: Vec<Part>) -> Self {
        Self { role: "user".into(), text: None, parts: Some(parts) }
    }

    fn to_value(&self) -> Value {
        let content = match (&self.text, &self.parts) {
            (Some(text), _) => Value::String(text.clone()),
            (None, Some(parts)) => Value::Array(parts.iter().map(Part::to_value).collect()),
            _ => Value::String(String::new()),
        };
        json!({ "role": self.role, "content": content })
    }
}

/// Параметры сэмплирования одного вызова.
#[derive(Clone, Debug)]
pub struct Sampling {
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: u32,
    pub repeat_penalty: Option<f32>,
}

impl Sampling {
    pub fn new(temperature: f32, max_tokens: u32) -> Self {
        Self { temperature, top_p: 0.95, max_tokens, repeat_penalty: None }
    }
}

/// Коды, при которых имеет смысл повторить: локальный сервер однослотовый, и когда два
/// вспомогательных прохода приходят одновременно, проигравший получает сброс соединения
/// или кратковременную пятисотку. Настоящий таймаут не повторяем — модель просто занята.
const TRANSIENT: [u16; 5] = [429, 500, 502, 503, 504];
const MAX_ATTEMPTS: u32 = 3;

/// Как быть с «размышлениями» модели.
///
/// Отключать их можно НЕ у всех: часть моделей отвечает отказом «reasoning is mandatory».
/// Поэтому режим выбирает вызывающий, опираясь на то, что провайдер сообщил о модели.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Reasoning {
    /// Ничего не просим — как у модели заведено.
    #[default]
    AsIs,
    /// Думать можно, но в ответ рассуждения не класть: они съедают лимит ответа.
    Hide,
    /// Не думать вовсе — только там, где модель это разрешает.
    Off,
}

/// Уровень усилия размышлений, если модель его принимает. Строку берём из её схемы:
/// уровень «мимо списка» модель не понимает и отвечает пустотой.
#[derive(Clone, Debug, Default)]
pub struct Effort(pub Option<String>);

pub struct ChatClient {
    endpoint: String,
    http: reqwest::blocking::Client,
    api_key: Option<String>,
    model: Option<String>,
    reasoning: Reasoning,
    effort: Effort,
}

/// Привести введённый пользователем адрес к эндпоинту `/chat/completions`: принимаем и голый
/// хост, и базу с версией, и полный путь — чтобы можно было вставить то, что напечатал сервер.
pub fn chat_endpoint(base_url: &str) -> String {
    let url = base_url.trim().trim_end_matches('/');
    if url.ends_with("/chat/completions") {
        return url.to_string();
    }
    let last = url.rsplit('/').next().unwrap_or("");
    let versioned = last.starts_with('v') && last.len() > 1 && last[1..].chars().all(|c| c.is_ascii_digit());
    if versioned {
        format!("{url}/chat/completions")
    } else {
        format!("{url}/v1/chat/completions")
    }
}

impl ChatClient {
    pub fn new(base_url: &str, timeout: Duration) -> Result<Self, LlmError> {
        let http = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| LlmError::Http(e.to_string()))?;
        Ok(Self {
            endpoint: chat_endpoint(base_url),
            http,
            api_key: None,
            model: None,
            reasoning: Reasoning::AsIs,
            effort: Effort::default(),
        })
    }

    pub fn with_api_key(mut self, key: Option<String>) -> Self {
        self.api_key = key.filter(|key| !key.trim().is_empty());
        self
    }

    /// Как обходиться с размышлениями модели.
    pub fn with_reasoning(mut self, reasoning: Reasoning) -> Self {
        self.reasoning = reasoning;
        self
    }

    /// Какой уровень усилия просить — строго из тех, что перечислила схема модели.
    pub fn with_effort(mut self, effort: Effort) -> Self {
        self.effort = effort;
        self
    }

    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model.filter(|model| !model.trim().is_empty());
        self
    }

    fn body(&self, messages: &[Message], sampling: &Sampling, stream: bool) -> Map<String, Value> {
        let mut body = Map::new();
        if let Some(model) = &self.model {
            body.insert("model".into(), json!(model));
        }
        body.insert(
            "messages".into(),
            Value::Array(messages.iter().map(Message::to_value).collect()),
        );
        body.insert("temperature".into(), json!(sampling.temperature));
        body.insert("top_p".into(), json!(sampling.top_p));
        body.insert("max_tokens".into(), json!(sampling.max_tokens));
        // Явный null llama-server отвергает четырёхсоткой, поэтому кладём только заданное.
        if let Some(penalty) = sampling.repeat_penalty {
            body.insert("repeat_penalty".into(), json!(penalty));
        }
        body.insert("stream".into(), json!(stream));
        // «Размышления» отключаем ОБОИМ бэкендам: иначе модель сжигает на них весь лимит
        // токенов, а видимый ответ приходит ПУСТЫМ — структурные проходы молча возвращают
        // ничего. Так вело себя и облако: DeepSeek расписывал ход рассуждениями на весь
        // лимит и не оставлял места под сам ответ. Поле у каждого своё: `chat_template_kwargs`
        // из мира llama.cpp, `reasoning` — договор облачного провайдера.
        if self.model.is_none() {
            body.insert("chat_template_kwargs".into(), json!({ "enable_thinking": false }));
        } else {
            match self.reasoning {
                // «enabled: false» принимают не все: Gemini и Grok отвечают отказом
                // «reasoning is mandatory». Просить его вслепую нельзя.
                Reasoning::Off => {
                    body.insert("reasoning".into(), json!({ "enabled": false, "exclude": true }));
                }
                // Думать не запрещаем, но рассуждения в ответ не кладём — иначе они съедают
                // лимит, и структурный проход возвращает пустоту.
                Reasoning::Hide => {
                    let mut block = json!({ "exclude": true });
                    // Уровень просим только тот, что модель объявила: чужой она не поймёт.
                    if let Some(effort) = self.effort.0.as_deref() {
                        block["effort"] = json!(effort);
                    }
                    body.insert("reasoning".into(), block);
                }
                Reasoning::AsIs => {}
            }
        }
        body
    }

    fn post(&self, body: &Value) -> reqwest::blocking::RequestBuilder {
        let mut request = self.http.post(&self.endpoint).json(body);
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }
        request
    }

    /// Один буферизованный вызов. Пустой ответ ошибкой не считается.
    pub fn chat(&self, messages: &[Message], sampling: &Sampling) -> Result<String, LlmError> {
        self.chat_with_format(messages, sampling, None)
    }

    fn chat_with_format(
        &self,
        messages: &[Message],
        sampling: &Sampling,
        response_format: Option<Value>,
    ) -> Result<String, LlmError> {
        let mut body = self.body(messages, sampling, false);
        if let Some(format) = response_format {
            body.insert("response_format".into(), format);
        }
        let body = Value::Object(body);

        let mut last = String::new();
        for attempt in 0..MAX_ATTEMPTS {
            match self.post(&body).send() {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        let value: Value = response
                            .json()
                            .map_err(|e| LlmError::Http(format!("разбор ответа: {e}")))?;
                        return Ok(value
                            .pointer("/choices/0/message/content")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string());
                    }
                    let detail = response.text().unwrap_or_default();
                    let code = status.as_u16();
                    if !TRANSIENT.contains(&code) {
                        return Err(LlmError::Api(format!("{code}: {}", truncate(&detail))));
                    }
                    last = format!("{code}: {}", truncate(&detail));
                }
                Err(error) => {
                    // Истёкший таймаут означает, что модель занята: повтор только добавит нагрузки.
                    if error.is_timeout() {
                        return Err(LlmError::Api(format!("таймаут запроса: {error}")));
                    }
                    last = error.to_string();
                }
            }
            if attempt + 1 < MAX_ATTEMPTS {
                std::thread::sleep(Duration::from_millis(350 * (attempt as u64 + 1)));
            }
        }
        Err(LlmError::Api(format!("не удалось за {MAX_ATTEMPTS} попытки: {last}")))
    }

    /// Вызов, ограниченный JSON-схемой. Возвращает разобранное значение.
    pub fn chat_json(
        &self,
        messages: &[Message],
        sampling: &Sampling,
        schema: &Value,
    ) -> Result<Value, LlmError> {
        // У локального сервера и у облака РАЗНЫЕ контракты структурированного ответа, и
        // это не косметика: llama.cpp понимает `json_object` со схемой рядом, а OpenRouter
        // ждёт `json_schema` со строгим режимом. Отправишь чужую форму — ограничение просто
        // не применится, и проход вернёт свободный текст вместо нужных полей.
        let format = self.response_format_for(schema);
        let raw = self.chat_with_format(messages, sampling, Some(format))?;
        // Грамматика гарантирует корректный JSON; вытаскивание фигурных скобок — страховка
        // на случай сервера, который молча проигнорировал response_format.
        parse_json_lenient(&raw).ok_or_else(|| LlmError::Api("ответ не является JSON".into()))
    }

    /// Форма ограничения ответа схемой под конкретный бэкенд.
    fn response_format_for(&self, schema: &Value) -> Value {
        if self.model.is_some() {
            json!({
                "type": "json_schema",
                "json_schema": { "name": "structured", "strict": true, "schema": schema }
            })
        } else {
            json!({ "type": "json_object", "schema": schema })
        }
    }

    /// Стриминговый вызов: `on_delta` зовётся на каждом куске текста. Возвращает полный текст.
    /// `on_delta` может вернуть `false`, чтобы оборвать генерацию (игрок нажал «стоп»).
    pub fn chat_stream(
        &self,
        messages: &[Message],
        sampling: &Sampling,
        mut on_delta: impl FnMut(&str) -> bool,
    ) -> Result<String, LlmError> {
        let body = Value::Object(self.body(messages, sampling, true));
        let response = self
            .post(&body)
            .send()
            .map_err(|e| LlmError::Http(format!("стриминг: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            let detail = response.text().unwrap_or_default();
            return Err(LlmError::Api(format!("{}: {}", status.as_u16(), truncate(&detail))));
        }

        let mut full = String::new();
        let reader = BufReader::new(response);
        for line in reader.lines() {
            let line = line.map_err(|e| LlmError::Http(format!("обрыв потока: {e}")))?;
            let Some(payload) = line.strip_prefix("data:") else { continue };
            let payload = payload.trim();
            if payload.is_empty() || payload == "[DONE]" {
                if payload == "[DONE]" {
                    break;
                }
                continue;
            }
            let Ok(value) = serde_json::from_str::<Value>(payload) else { continue };
            let delta = value
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if delta.is_empty() {
                continue;
            }
            full.push_str(delta);
            if !on_delta(delta) {
                break;
            }
        }
        Ok(full)
    }
}

fn truncate(text: &str) -> String {
    text.chars().take(500).collect()
}

/// Разобрать JSON, при неудаче попробовать первый блок в фигурных скобках.
pub fn parse_json_lenient(raw: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(raw.trim()) {
        return Some(value);
    }
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<Value>(&raw[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_host_gets_the_full_endpoint() {
        assert_eq!(chat_endpoint("http://127.0.0.1:8080"), "http://127.0.0.1:8080/v1/chat/completions");
    }

    #[test]
    fn a_versioned_base_only_gets_the_tail() {
        assert_eq!(chat_endpoint("http://127.0.0.1:8080/v1"), "http://127.0.0.1:8080/v1/chat/completions");
        assert_eq!(chat_endpoint("https://example.org/api/v2/"), "https://example.org/api/v2/chat/completions");
    }

    #[test]
    fn a_full_endpoint_is_left_alone() {
        let url = "http://127.0.0.1:8080/v1/chat/completions";
        assert_eq!(chat_endpoint(url), url);
        assert_eq!(chat_endpoint(&format!("{url}/")), url);
    }

    #[test]
    fn a_host_that_merely_starts_with_v_is_not_mistaken_for_a_version() {
        assert_eq!(chat_endpoint("http://vllm"), "http://vllm/v1/chat/completions");
    }

    #[test]
    fn both_backends_get_thinking_switched_off_in_their_own_dialect() {
        let sampling = Sampling::new(0.5, 100);
        let messages = [Message::user("привет")];

        let local = ChatClient::new("http://127.0.0.1:8080", Duration::from_secs(5)).unwrap();
        let body = local.body(&messages, &sampling, false);
        assert_eq!(
            body["chat_template_kwargs"]["enable_thinking"],
            json!(false),
            "без этого поля локальная модель уходит рассуждать и отдаёт пустой ответ"
        );

        let cloud = ChatClient::new("https://example.org", Duration::from_secs(5))
            .unwrap()
            .with_model(Some("gemma".into()));
        let body = cloud.body(&messages, &sampling, false);
        assert!(body.get("chat_template_kwargs").is_none(), "поле llama.cpp в облаке лишнее");
        assert!(body.get("reasoning").is_none(), "без явного режима поле не шлём");

        let cloud = ChatClient::new("https://example.org", Duration::from_secs(5))
            .unwrap()
            .with_model(Some("gemma".into()))
            .with_reasoning(Reasoning::Hide);
        let body = cloud.body(&messages, &sampling, false);
        assert_eq!(body["reasoning"]["exclude"], json!(true));
        assert!(
            body["reasoning"].get("enabled").is_none(),
            "часть моделей отвергает запрет размышлений — просить его нельзя"
        );
        assert_eq!(body["model"], "gemma");
    }

    #[test]
    fn each_backend_gets_the_schema_form_it_actually_honours() {
        let schema = json!({ "type": "object", "properties": { "needed": { "type": "boolean" } } });

        let local = ChatClient::new("http://127.0.0.1:8080", Duration::from_secs(5)).unwrap();
        let format = local.response_format_for(&schema);
        assert_eq!(format["type"], "json_object");
        assert_eq!(format["schema"]["type"], "object");

        let cloud = ChatClient::new("https://openrouter.ai/api", Duration::from_secs(5))
            .unwrap()
            .with_model(Some("google/gemini-3.5-flash".into()));
        let format = cloud.response_format_for(&schema);
        assert_eq!(format["type"], "json_schema", "облако игнорирует форму llama.cpp");
        assert_eq!(format["json_schema"]["strict"], true);
        assert_eq!(format["json_schema"]["schema"]["type"], "object");
    }

    #[test]
    fn json_is_salvaged_from_a_chatty_reply() {
        assert_eq!(parse_json_lenient("{\"a\":1}").unwrap()["a"], 1);
        let messy = "Конечно! Вот результат:\n```json\n{\"a\": 2}\n```\nГотово.";
        assert_eq!(parse_json_lenient(messy).unwrap()["a"], 2);
        assert!(parse_json_lenient("совсем не json").is_none());
    }

    #[test]
    fn a_message_with_an_image_becomes_a_parts_array() {
        let message = Message::user_parts(vec![
            Part::Text("кто на портрете?".into()),
            Part::ImagePngB64("QUJD".into()),
        ]);
        let value = message.to_value();
        assert_eq!(value["role"], "user");
        let parts = value["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["image_url"]["url"], "data:image/png;base64,QUJD");
    }
}
