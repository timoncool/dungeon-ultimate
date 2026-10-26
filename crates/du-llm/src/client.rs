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

#[derive(Clone)]
pub struct ChatClient {
    endpoint: String,
    http: reqwest::blocking::Client,
    api_key: Option<String>,
    model: Option<String>,
    reasoning: Reasoning,
    effort: Effort,
    /// Инструменты, которыми модель вправе пользоваться в этом вызове.
    tools: Vec<crate::tools::Tool>,
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
            tools: Vec::new(),
        })
    }

    pub fn with_api_key(mut self, key: Option<String>) -> Self {
        self.api_key = key.map(|key| key.trim().to_string()).filter(|key| !key.is_empty());
        self
    }

    /// Как обходиться с размышлениями модели.
    pub fn with_reasoning(mut self, reasoning: Reasoning) -> Self {
        self.reasoning = reasoning;
        self
    }

    /// Какой уровень усилия просить — строго из тех, что перечислила схема модели.
    /// Дать модели инструменты на этот вызов.
    pub fn with_tools(mut self, tools: Vec<crate::tools::Tool>) -> Self {
        self.tools = tools;
        self
    }

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
        // Инструменты: модель сама решает, звать ли их. `auto` — договор провайдера.
        if !self.tools.is_empty() {
            body.insert(
                "tools".into(),
                Value::Array(self.tools.iter().map(crate::tools::Tool::to_value).collect()),
            );
            body.insert("tool_choice".into(), json!("auto"));
        }
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

    /// Вызов с инструментами: возвращает и текст, и просьбы модели что-то сделать.
    ///
    /// Сообщения принимаем уже готовыми значениями, потому что в разговор с инструментами
    /// вплетаются чужие роли — `assistant` с вызовами и `tool` с ответами, — а обычное
    /// `Message` их не описывает.
    pub fn chat_tools(
        &self,
        messages: &[Value],
        sampling: &Sampling,
    ) -> Result<(String, Vec<crate::tools::ToolCall>), LlmError> {
        let mut body = self.body(&[], sampling, false);
        body.insert("messages".into(), Value::Array(messages.to_vec()));
        let body = Value::Object(body);

        let response = self
            .post(&body)
            .send()
            .map_err(|error| LlmError::Http(error.to_string()))?;
        let status = response.status();
        let text = response.text().map_err(|error| LlmError::Http(format!("чтение ответа: {error}")))?;
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            // Прокси и шлюзы отвечают HTML: код и начало тела ценнее, чем «разбор ответа».
            Err(error) if status.is_success() => return Err(LlmError::Http(format!("разбор ответа: {error}"))),
            Err(_) => {
                let head: String = text.trim().chars().take(200).collect();
                return Err(LlmError::Http(format!("{status}: {head}")));
            }
        };
        if !status.is_success() {
            let message = value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("провайдер отказал");
            return Err(LlmError::Status { code: status.as_u16(), detail: message.to_string() });
        }
        let message = value.pointer("/choices/0/message").cloned().unwrap_or(Value::Null);
        let text = message.get("content").and_then(Value::as_str).unwrap_or_default().to_string();
        Ok((text, crate::tools::parse_calls(&message)))
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
                        return Err(LlmError::Status { code, detail: truncate(&detail) });
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

    pub fn chat_stream(
        &self,
        messages: &[Message],
        sampling: &Sampling,
        mut on_delta: impl FnMut(&str) -> bool,
    ) -> Result<String, LlmError> {
        let body = Value::Object(self.body(messages, sampling, true));
        // Повтор — только до первого куска текста: однослотовый локальный сервер сбрасывает
        // соединение проигравшему из двух одновременных запросов, как и в буферизованном вызове.
        let mut attempt = 0;
        let response = loop {
            attempt += 1;
            match self.post(&body).send() {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        break response;
                    }
                    let detail = response.text().unwrap_or_default();
                    let code = status.as_u16();
                    if !TRANSIENT.contains(&code) || attempt >= MAX_ATTEMPTS {
                        return Err(LlmError::Status { code, detail: truncate(&detail) });
                    }
                }
                Err(error) => {
                    if error.is_timeout() || attempt >= MAX_ATTEMPTS {
                        return Err(LlmError::StreamSend(send_failure(&error)));
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(350 * attempt as u64));
        };

        let mut full = String::new();
        let mut outcome = StreamOutcome::default();
        let reader = BufReader::new(response);
        for line in reader.lines() {
            let line = line.map_err(|e| LlmError::StreamBroken(e.to_string()))?;
            let Some(payload) = line.strip_prefix("data:") else { continue };
            let payload = payload.trim();
            if payload == "[DONE]" {
                outcome.done = true;
                break;
            }
            if payload.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<Value>(payload) else { continue };
            // Провайдер, уже ответивший 200, сообщает об ошибке внутри потока.
            if let Some(error) = value.get("error") {
                let message = error
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| error.to_string());
                return Err(LlmError::Api(format!("ошибка в потоке: {}", truncate(&message))));
            }
            if let Some(reason) = value.pointer("/choices/0/finish_reason").and_then(Value::as_str) {
                outcome.finish = Some(reason.to_string());
            }
            let delta = value.pointer("/choices/0/delta").cloned().unwrap_or(Value::Null);
            for key in ["reasoning_content", "reasoning"] {
                if let Some(thought) = delta.get(key).and_then(Value::as_str) {
                    outcome.reasoning_chars += thought.chars().count();
                }
            }
            if delta.get("tool_calls").and_then(Value::as_array).is_some_and(|calls| !calls.is_empty()) {
                outcome.tool_call = true;
            }
            let text = delta.get("content").and_then(Value::as_str).unwrap_or_default();
            if text.is_empty() {
                continue;
            }
            full.push_str(text);
            if !on_delta(text) {
                break;
            }
        }
        if full.is_empty() {
            return Err(LlmError::Empty(outcome.why_empty()));
        }
        Ok(full)
    }
}

/// Что пришло в потоке, кроме текста: по этому видно, почему текста нет.
#[derive(Debug, Default)]
struct StreamOutcome {
    finish: Option<String>,
    reasoning_chars: usize,
    tool_call: bool,
    done: bool,
}

impl StreamOutcome {
    fn why_empty(&self) -> EmptyStream {
        let finish = self.finish.clone().unwrap_or_else(|| "нет".to_string());
        if self.tool_call {
            return EmptyStream::ToolCall { finish };
        }
        if self.reasoning_chars > 0 {
            return EmptyStream::OnlyThinking { chars: self.reasoning_chars, finish };
        }
        match self.finish.as_deref() {
            Some("content_filter") => EmptyStream::Filtered,
            Some(_) => EmptyStream::Finished { finish },
            None if self.done => EmptyStream::NoReason,
            None => EmptyStream::Cut,
        }
    }
}

/// Почему поток рассказчика кончился без текста.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmptyStream {
    #[error("модель позвала инструмент вместо текста (finish_reason={finish})")]
    ToolCall { finish: String },
    #[error(
        "модель только размышляла ({chars} знаков) и не написала текст (finish_reason={finish}): \
         выключите у неё размышления или выберите модель без них"
    )]
    OnlyThinking { chars: usize, finish: String },
    #[error("провайдер отфильтровал ответ (content_filter)")]
    Filtered,
    #[error("модель вернула пустой текст (finish_reason={finish})")]
    Finished { finish: String },
    #[error("модель вернула пустой текст без причины завершения")]
    NoReason,
    #[error("поток оборвался, не дав ни текста, ни причины завершения")]
    Cut,
}

fn truncate(text: &str) -> String {
    text.chars().take(500).collect()
}

/// Почему запрос не дошёл до сервера модели: верхний текст reqwest («error sending request for url (…)») причины не
/// называет, она — в цепочке источников (отказ соединения, таймаут, DNS, TLS).
fn send_failure(error: &reqwest::Error) -> String {
    let what = if error.is_timeout() {
        "нет ответа (таймаут)"
    } else if error.is_connect() {
        "нет соединения"
    } else {
        "запрос не ушёл"
    };
    let host = error
        .url()
        .and_then(|url| url.host_str().map(|host| url.port().map_or_else(|| host.to_string(), |port| format!("{host}:{port}"))))
        .unwrap_or_default();
    let mut causes: Vec<String> = Vec::new();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        let text = cause.to_string();
        if !causes.iter().any(|known| known.contains(&text)) {
            causes.push(text);
        }
        source = cause.source();
    }
    let cause = if causes.is_empty() { error.to_string() } else { causes.join(": ") };
    format!("{what} с {host}: {}", truncate(&cause))
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
    fn a_stream_to_a_dead_server_names_the_refused_connection() {
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        let client = ChatClient::new(&format!("http://127.0.0.1:{port}"), Duration::from_secs(5)).unwrap();
        let error = client.chat_stream(&[Message::user("hi")], &Sampling::new(0.7, 16), |_| true).unwrap_err();
        let text = error.to_string();
        assert!(matches!(error, LlmError::StreamSend(_)), "{text}");
        assert!(text.contains(&format!("нет соединения с 127.0.0.1:{port}")) && !text.contains("error sending request"), "{text}");
    }

    /// Поднять разовый HTTP-сервер, отвечающий потоком SSE из готовых строк.
    fn serve_stream(lines: &'static [&'static str]) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 65536];
            let _ = socket.read(&mut request);
            let body: String = lines.iter().map(|line| format!("data: {line}\n\n")).collect();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(head.as_bytes());
            let _ = socket.write_all(body.as_bytes());
        });
        format!("http://{address}")
    }

    fn stream(lines: &'static [&'static str]) -> Result<String, LlmError> {
        let client = ChatClient::new(&serve_stream(lines), Duration::from_secs(5)).unwrap();
        client.chat_stream(&[Message::user("ход")], &Sampling::new(0.5, 100), |_| true)
    }

    #[test]
    fn a_stream_without_text_says_why() {
        let thinking = stream(&[
            r#"{"choices":[{"delta":{"reasoning_content":"думаю, думаю"}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#,
            "[DONE]",
        ])
        .unwrap_err()
        .to_string();
        assert!(thinking.contains("размышляла") && thinking.contains("length"), "{thinking}");

        let tool = stream(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"roll_check"}}]}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
            "[DONE]",
        ])
        .unwrap_err()
        .to_string();
        assert!(tool.contains("инструмент"), "{tool}");

        let failed = stream(&[r#"{"error":{"message":"Provider returned error","code":502}}"#])
            .unwrap_err()
            .to_string();
        assert!(failed.contains("Provider returned error"), "{failed}");
    }

    #[test]
    fn a_stream_with_text_returns_it_whole() {
        let text = stream(&[
            r#"{"choices":[{"delta":{"content":"Дверь "}}]}"#,
            r#"{"choices":[{"delta":{"content":"скрипнула."},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ])
        .unwrap();
        assert_eq!(text, "Дверь скрипнула.");
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
