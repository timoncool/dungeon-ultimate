//! Ключ Google для озвучки Gemini — как в Dub Studio.
//!
//! Переменная окружения `GEMINI_API_KEY` всегда главнее: скриптовый запуск не читает и не
//! перезаписывает ключ игрока. Сохранённый ключ лежит в `secrets/google-api-key` рядом с игрой
//! и переезжает вместе с папкой. Наружу значение не отдаётся: окно и агент узнают только,
//! задан ли ключ и откуда.

use std::path::{Path, PathBuf};

use serde::Serialize;

pub const GOOGLE_ENV_VAR: &str = "GEMINI_API_KEY";
const GOOGLE_FILE: &str = "google-api-key";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSource {
    Environment,
    LocalStore,
}

fn secrets_dir(root: &Path) -> PathBuf {
    root.join("secrets")
}

pub fn google_api_key(root: &Path) -> Option<(String, CredentialSource)> {
    if let Some(key) = std::env::var(GOOGLE_ENV_VAR).ok().map(|key| key.trim().to_owned()).filter(|key| !key.is_empty()) {
        return Some((key, CredentialSource::Environment));
    }
    let stored = std::fs::read_to_string(secrets_dir(root).join(GOOGLE_FILE)).ok()?;
    let stored = stored.trim().to_owned();
    (!stored.is_empty()).then_some((stored, CredentialSource::LocalStore))
}

/// Сохранить или стереть ключ. Ключ из окружения не затеняется молча: игрок не должен думать,
/// что заменил ключ, которым процесс на деле не пользуется.
pub fn store_google_api_key(root: &Path, api_key: Option<&str>) -> Result<Option<CredentialSource>, String> {
    if std::env::var(GOOGLE_ENV_VAR).is_ok_and(|key| !key.trim().is_empty()) {
        return Err(format!("{GOOGLE_ENV_VAR} задан в окружении и главнее; уберите его, чтобы хранить ключ в игре"));
    }
    let path = secrets_dir(root).join(GOOGLE_FILE);
    match api_key.map(str::trim).filter(|key| !key.is_empty()) {
        None => {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("стереть ключ Google: {error}")),
            }
            Ok(None)
        }
        Some(key) => {
            if key.contains(['\n', '\r']) {
                return Err("ключ Google должен быть одной строкой".into());
            }
            std::fs::create_dir_all(secrets_dir(root)).map_err(|error| format!("создать папку ключей: {error}"))?;
            std::fs::write(&path, key).map_err(|error| format!("сохранить ключ Google: {error}"))?;
            Ok(Some(CredentialSource::LocalStore))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_key_round_trips_and_clears() {
        if std::env::var(GOOGLE_ENV_VAR).is_ok() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        assert!(google_api_key(dir.path()).is_none());
        store_google_api_key(dir.path(), Some("  key-1  ")).unwrap();
        assert_eq!(google_api_key(dir.path()).unwrap(), ("key-1".to_string(), CredentialSource::LocalStore));
        assert!(store_google_api_key(dir.path(), Some("a\nb")).is_err());
        store_google_api_key(dir.path(), None).unwrap();
        assert!(google_api_key(dir.path()).is_none());
    }
}
