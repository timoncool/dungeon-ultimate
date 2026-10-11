//! Кто пишет текст игры: своя карта, облако или агент, подключённый к MCP игры.
//!
//! Все проходы хода (проза, инструменты, механика, кадр, голоса) и служебные вызовы окна (чипы,
//! подсказки, мастер истории, генерация героя) берут клиента здесь, поэтому выбор действует на
//! всё сразу. Агент отвечает через локальный OpenAI-совместимый вход `/agent/v1/chat/completions`
//! (`mcp::agent_completions`): клиент рассказчика его не отличает от llama-server.

use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use du_llm::ChatClient;

use crate::runtime::Runtime;

static PORT: AtomicU16 = AtomicU16::new(0);

/// Порт, на котором слушает сервер: по нему клиент рассказчика находит вход для агента.
pub fn set_port(port: u16) {
    PORT.store(port, Ordering::Relaxed);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writer {
    /// llama.cpp на своей карте.
    Local,
    /// OpenRouter.
    Cloud,
    /// Агент через MCP.
    Agent,
}

pub fn writer(runtime: &Runtime) -> Writer {
    if runtime.narrator_provider == "agent" {
        Writer::Agent
    } else if crate::cloud::stage_enabled(runtime, crate::cloud::Stage::Narrator) {
        Writer::Cloud
    } else {
        Writer::Local
    }
}

/// Клиент для облака или агента; `None` — текст считает своя карта, и клиента надо брать у её
/// сервера в очереди к карте.
pub fn remote_client(runtime: &Runtime, timeout: Duration) -> Result<Option<ChatClient>, String> {
    match writer(runtime) {
        Writer::Local => Ok(None),
        Writer::Agent => agent_client(timeout).map(Some),
        Writer::Cloud => cloud_client(runtime, timeout).map(Some),
    }
}

fn agent_client(timeout: Duration) -> Result<ChatClient, String> {
    if !crate::mcp::agent_present() {
        return Err(
            "рассказчик — агент, но агент не подключён к MCP игры: подключите его или выберите рассказчика на своей карте"
                .into(),
        );
    }
    let port = PORT.load(Ordering::Relaxed);
    if port == 0 {
        return Err("сервер ещё не знает свой порт".into());
    }
    ChatClient::new(&format!("http://127.0.0.1:{port}/agent"), timeout).map_err(|error| error.to_string())
}

fn cloud_client(runtime: &Runtime, timeout: Duration) -> Result<ChatClient, String> {
    crate::cloud::require(runtime, crate::cloud::Stage::Narrator)?;
    // Размышления ПРЯЧЕМ, а не запрещаем: запрет часть моделей отвергает отказом, а если
    // рассуждения попадают в ответ, структурный проход возвращает их вместо JSON.
    let caps = crate::model_caps::caps(&runtime.openrouter_narrator_model, &runtime.openrouter_key);
    let reasoning = match (caps.reasoning, caps.reasoning_mandatory) {
        (true, false) => du_llm::Reasoning::Off,
        (true, true) => du_llm::Reasoning::Hide,
        _ => du_llm::Reasoning::AsIs,
    };
    let effort = du_llm::Effort(caps.cheapest_effort().map(str::to_string));
    Ok(ChatClient::new("https://openrouter.ai/api", timeout)
        .map_err(|error| error.to_string())?
        .with_api_key(Some(runtime.openrouter_key.clone()))
        .with_model(Some(runtime.openrouter_narrator_model.clone()))
        .with_reasoning(reasoning)
        .with_effort(effort))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_wins_over_the_cloud_and_the_card() {
        let mut runtime = Runtime::default();
        assert_eq!(writer(&runtime), Writer::Local);
        runtime.openrouter_narrator_on = true;
        assert_eq!(writer(&runtime), Writer::Cloud);
        runtime.narrator_provider = "agent".into();
        assert_eq!(writer(&runtime), Writer::Agent);
    }

    #[test]
    fn an_agent_writer_without_an_agent_is_an_error_not_the_card() {
        let runtime = Runtime { narrator_provider: "agent".into(), ..Runtime::default() };
        let Err(error) = remote_client(&runtime, Duration::from_secs(1)) else { panic!("без агента клиента нет") };
        assert!(error.contains("агент"), "{error}");
    }
}
