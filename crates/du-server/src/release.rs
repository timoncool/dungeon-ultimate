//! Проверка обновлений.
//!
//! Игра ставится папкой и обновляется скриптом `update.bat`, поэтому узнать о новой версии
//! ей неоткуда — кроме как спросить. Спрашиваем у GitHub про последний релиз и сравниваем с
//! версией, вшитой при сборке.
//!
//! Ответ кэшируем на полдня: чаще нет смысла, а без кэша каждый запуск игры дёргал бы сеть.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

/// Версия из `Cargo.toml` — та, с которой собран этот бинарь.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

const RELEASES: &str = "https://api.github.com/repos/timoncool/dungeon-ultimate/releases/latest";
const CACHE_TTL: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub current: String,
    /// Пусто, если спросить не удалось: молчание лучше пугающей ошибки на пустом месте.
    pub latest: String,
    pub newer: bool,
    pub url: String,
    pub notes: String,
}

impl Release {
    fn unknown() -> Self {
        Self {
            current: CURRENT.to_string(),
            latest: String::new(),
            newer: false,
            url: "https://github.com/timoncool/dungeon-ultimate/releases".to_string(),
            notes: String::new(),
        }
    }
}

fn cache() -> &'static Mutex<Option<(Instant, Release)>> {
    static CACHE: OnceLock<Mutex<Option<(Instant, Release)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Разложить версию вида `v1.2.3` в числа. Хвост после третьего числа игнорируем: он бывает
/// у предрелизов и на сравнение старшинства не влияет.
fn parts(version: &str) -> (u32, u32, u32) {
    let cleaned = version.trim().trim_start_matches(['v', 'V']);
    let mut numbers = cleaned
        .split(['.', '-', '+'])
        .map(|piece| piece.chars().take_while(char::is_ascii_digit).collect::<String>())
        .filter(|piece| !piece.is_empty())
        .map(|piece| piece.parse::<u32>().unwrap_or(0));
    (numbers.next().unwrap_or(0), numbers.next().unwrap_or(0), numbers.next().unwrap_or(0))
}

/// Новее ли `candidate`, чем `current`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    parts(candidate) > parts(current)
}

fn fetch() -> Result<Release, String> {
    let response = ureq::get(RELEASES)
        // GitHub отвечает отказом на запрос без подписи клиента.
        .header("User-Agent", "dungeon-ultimate")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| error.to_string())?;
    let body = response.into_body().read_to_string().map_err(|error| error.to_string())?;
    let parsed: Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;

    let latest = parsed
        .get("tag_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if latest.is_empty() {
        return Err("в ответе нет версии".into());
    }
    let url = parsed
        .get("html_url")
        .and_then(Value::as_str)
        .unwrap_or("https://github.com/timoncool/dungeon-ultimate/releases")
        .to_string();
    // Заметки к релизу показываем коротко: в панели им места немного.
    let notes: String = parsed
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(Release { newer: is_newer(&latest, CURRENT), current: CURRENT.to_string(), latest, url, notes })
}

/// Узнать о новой версии. Сеть недоступна — возвращаем «неизвестно», а не ошибку: игре это
/// не мешает, и пугать игрока нечем.
pub fn check() -> Release {
    if let Ok(guard) = cache().lock() {
        if let Some((at, release)) = guard.as_ref() {
            if at.elapsed() < CACHE_TTL {
                return release.clone();
            }
        }
    }
    let release = fetch().unwrap_or_else(|error| {
        tracing::debug!("проверка обновлений не удалась: {error}");
        Release::unknown()
    });
    if let Ok(mut guard) = cache().lock() {
        *guard = Some((Instant::now(), release.clone()));
    }
    release
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bigger_version_wins_regardless_of_the_v_prefix() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "v0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn a_prerelease_tail_does_not_confuse_the_comparison() {
        assert!(is_newer("v0.2.0-rc.1", "0.1.9"));
        assert!(!is_newer("v0.1.0-rc.1", "0.1.0"));
    }

    #[test]
    fn garbage_is_not_treated_as_an_update() {
        assert!(!is_newer("", "0.1.0"));
        assert!(!is_newer("непонятно", "0.1.0"));
    }
}
