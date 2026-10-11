//! Журнал «Активность»: что агент сделал в игре через MCP — как журнал студий. Окно показывает
//! его в боковой панели; строки переживают перезапуск (`activity.jsonl` в корне данных).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::state::AppState;

/// Сколько последних строк держим и отдаём окну.
const KEEP: usize = 300;

static FILE: OnceLock<PathBuf> = OnceLock::new();
static LINES: Mutex<Vec<Value>> = Mutex::new(Vec::new());

pub fn start(root: &Path) {
    let path = root.join("activity.jsonl");
    if let Ok(text) = std::fs::read_to_string(&path) {
        let mut lines: Vec<Value> = text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect();
        let skip = lines.len().saturating_sub(KEEP);
        lines.drain(..skip);
        *LINES.lock().unwrap_or_else(|e| e.into_inner()) = lines;
    }
    let _ = FILE.set(path);
}

/// Записать действие агента: тул, его аргументы без длинных текстов и итог.
pub fn record(tool: &str, args: &Value, ok: bool, summary: &str) {
    let line = json!({
        "at": chrono::Utc::now().to_rfc3339(),
        "source": "agent",
        "tool": tool,
        "args": short(args),
        "ok": ok,
        "summary": summary.chars().take(300).collect::<String>(),
    });
    if let Some(path) = FILE.get() {
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| writeln!(file, "{line}"));
        if let Err(error) = written {
            tracing::error!("журнал активности не записан: {error}");
        }
    }
    let mut lines = LINES.lock().unwrap_or_else(|e| e.into_inner());
    lines.push(line);
    let skip = lines.len().saturating_sub(KEEP);
    lines.drain(..skip);
    drop(lines);
    crate::mcp::announce("activity");
}

/// Длинные тексты в журнал не идут: хватит начала.
fn short(args: &Value) -> Value {
    match args {
        Value::String(text) if text.chars().count() > 160 => {
            Value::String(format!("{}…", text.chars().take(160).collect::<String>()))
        }
        Value::Object(map) => Value::Object(map.iter().map(|(key, value)| (key.clone(), short(value))).collect()),
        Value::Array(list) => Value::Array(list.iter().take(20).map(short).collect()),
        other => other.clone(),
    }
}

pub async fn list(State(_state): State<AppState>) -> Json<Value> {
    let lines = LINES.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Json(json!({ "lines": lines }))
}

#[cfg(test)]
mod tests {
    use super::short;
    use serde_json::json;

    #[test]
    fn long_texts_are_cut_in_the_journal() {
        let long = "а".repeat(500);
        let value = short(&json!({ "text": long, "chat_id": "c1" }));
        assert!(value["text"].as_str().unwrap().chars().count() <= 161);
        assert_eq!(value["chat_id"], "c1");
    }
}
