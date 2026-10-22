//! Текстовая часть Dungeon Ultimate: локальный сайдкар llama-server и клиент к любому
//! OpenAI-совместимому бэкенду.
//!
//! Ход истории многопроходный: нарратор пишет ТОЛЬКО чистую прозу и отдаёт её потоком, а
//! механику хода, кадр и подсказки действий добывают отдельные вызовы, ограниченные
//! JSON-схемой. Так модель не протаскивает служебные блоки в текст истории.

pub mod client;
pub mod server;

pub use client::{
    EmptyStream, Effort, Reasoning, chat_endpoint, parse_json_lenient, ChatClient, Message, Part, Sampling};
pub mod tools;
pub use tools::{Tool, ToolCall};
pub use server::{kill_orphans, resolve_llama_bin, LlamaServer, ServerOpts};

/// Ошибки текстовой модели. Тексты — для журнала и статистики; окно игрока переводит их по
/// виду ошибки и её полям.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("не удалось запустить модель: llama-server не найден: {}", .0.display())]
    BinMissing(std::path::PathBuf),
    #[error("не удалось запустить модель: модель не найдена: {}", .0.display())]
    ModelMissing(std::path::PathBuf),
    #[error("не удалось запустить модель: {0}")]
    Spawn(String),
    #[error("сеть: {0}")]
    Http(String),
    /// Запрос потока не дошёл до сервера модели.
    #[error("сеть: стриминг: {0}")]
    StreamSend(String),
    /// Поток оборвался посреди ответа.
    #[error("сеть: обрыв потока: {0}")]
    StreamBroken(String),
    /// Сервер модели ответил кодом HTTP: 401 и 403 — отказ в ключе, остальное — его сбой.
    #[error("модель ответила ошибкой: {code}: {detail}")]
    Status { code: u16, detail: String },
    #[error("модель ответила ошибкой: {0}")]
    Api(String),
    /// Поток закончился без единого слова текста.
    #[error("модель ответила ошибкой: {0}")]
    Empty(client::EmptyStream),
}
