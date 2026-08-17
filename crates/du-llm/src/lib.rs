//! Текстовая часть Dungeon Ultimate: локальный сайдкар llama-server и клиент к любому
//! OpenAI-совместимому бэкенду.
//!
//! Ход истории многопроходный: нарратор пишет ТОЛЬКО чистую прозу и отдаёт её потоком, а
//! механику хода, кадр и подсказки действий добывают отдельные вызовы, ограниченные
//! JSON-схемой. Так модель не протаскивает служебные блоки в текст истории.

pub mod client;
pub mod server;

pub use client::{
    Effort, Reasoning,chat_endpoint, parse_json_lenient, ChatClient, Message, Part, Sampling};
pub use server::{kill_orphans, resolve_llama_bin, LlamaServer, ServerOpts};

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("не удалось запустить модель: {0}")]
    Spawn(String),
    #[error("сеть: {0}")]
    Http(String),
    #[error("модель ответила ошибкой: {0}")]
    Api(String),
}
