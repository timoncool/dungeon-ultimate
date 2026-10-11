//! Язык окна для вопросов оболочки (обновление): окно сообщает его после загрузки, как в Dub Studio.

use std::sync::RwLock;

use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

static LANG: RwLock<Option<String>> = RwLock::new(None);

const LANGS: [&str; 7] = ["ru", "en", "es", "fr", "de", "zh", "ja"];

#[derive(Deserialize)]
pub struct UiLanguage {
    lang: String,
}

pub async fn report(Json(body): Json<UiLanguage>) -> Json<Value> {
    let lang = body.lang.trim().to_ascii_lowercase();
    let known = LANGS.contains(&lang.as_str());
    if known {
        if let Ok(mut slot) = LANG.write() {
            *slot = Some(lang.clone());
        }
    }
    Json(json!({ "lang": lang, "known": known }))
}

/// Окно уже сообщило свой язык.
pub fn reported() -> bool {
    LANG.read().map(|slot| slot.is_some()).unwrap_or(false)
}

pub fn current() -> String {
    LANG.read().ok().and_then(|slot| slot.clone()).unwrap_or_else(|| "ru".into())
}

/// Текст вопроса оболочки на языке окна; `{version}`, `{error}` подставляются.
pub fn text(key: &str, args: &[(&str, &str)]) -> String {
    let template = template(&current(), key);
    args.iter().fold(template.to_string(), |text, (name, value)| text.replace(&format!("{{{name}}}"), value))
}

fn template(lang: &str, key: &str) -> &'static str {
    match (lang, key) {
        ("ru", "update-title") => "Обновление Dungeon Ultimate",
        ("ru", "update-install-now") => "Вышла версия {version}. Обновить сейчас? Игра перезапустится.",
        ("ru", "update-failed") => "Не удалось обновить: {error}\nОткрыть страницу загрузки?",
        ("ru", "update-button-install") => "Обновить",
        ("ru", "update-button-open") => "Открыть",
        ("ru", "update-button-later") => "Позже",
        ("en", "update-title") => "Dungeon Ultimate update",
        ("en", "update-install-now") => "Version {version} is out. Update now? The game will restart.",
        ("en", "update-failed") => "The update failed: {error}\nOpen the download page?",
        ("en", "update-button-install") => "Update",
        ("en", "update-button-open") => "Open",
        ("en", "update-button-later") => "Later",
        ("es", "update-title") => "Actualización de Dungeon Ultimate",
        ("es", "update-install-now") => "Salió la versión {version}. ¿Actualizar ahora? El juego se reiniciará.",
        ("es", "update-failed") => "No se pudo actualizar: {error}\n¿Abrir la página de descarga?",
        ("es", "update-button-install") => "Actualizar",
        ("es", "update-button-open") => "Abrir",
        ("es", "update-button-later") => "Más tarde",
        ("fr", "update-title") => "Mise à jour de Dungeon Ultimate",
        ("fr", "update-install-now") => "La version {version} est sortie. Mettre à jour maintenant ? Le jeu va redémarrer.",
        ("fr", "update-failed") => "La mise à jour a échoué : {error}\nOuvrir la page de téléchargement ?",
        ("fr", "update-button-install") => "Mettre à jour",
        ("fr", "update-button-open") => "Ouvrir",
        ("fr", "update-button-later") => "Plus tard",
        ("de", "update-title") => "Dungeon-Ultimate-Update",
        ("de", "update-install-now") => "Version {version} ist da. Jetzt aktualisieren? Das Spiel startet neu.",
        ("de", "update-failed") => "Das Update ist fehlgeschlagen: {error}\nDownloadseite öffnen?",
        ("de", "update-button-install") => "Aktualisieren",
        ("de", "update-button-open") => "Öffnen",
        ("de", "update-button-later") => "Später",
        ("zh", "update-title") => "Dungeon Ultimate 更新",
        ("zh", "update-install-now") => "版本 {version} 已发布。现在更新吗？游戏会重启。",
        ("zh", "update-failed") => "更新失败：{error}\n打开下载页面吗？",
        ("zh", "update-button-install") => "更新",
        ("zh", "update-button-open") => "打开",
        ("zh", "update-button-later") => "稍后",
        ("ja", "update-title") => "Dungeon Ultimate の更新",
        ("ja", "update-install-now") => "バージョン {version} が出ました。今すぐ更新しますか？ゲームは再起動します。",
        ("ja", "update-failed") => "更新に失敗しました：{error}\nダウンロードページを開きますか？",
        ("ja", "update-button-install") => "更新",
        ("ja", "update-button-open") => "開く",
        ("ja", "update-button-later") => "あとで",
        (_, key) => template("en", key_or_empty(key)),
    }
}

fn key_or_empty(key: &str) -> &str {
    match key {
        "update-title" | "update-install-now" | "update-failed" | "update-button-install" | "update-button-open"
        | "update-button-later" => key,
        _ => "update-title",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_every_question() {
        for lang in LANGS {
            for key in ["update-title", "update-install-now", "update-failed", "update-button-install", "update-button-open", "update-button-later"] {
                assert!(!template(lang, key).is_empty(), "{lang}/{key}");
            }
            assert!(template(lang, "update-install-now").contains("{version}"), "{lang}");
            assert!(template(lang, "update-failed").contains("{error}"), "{lang}");
        }
    }
}
