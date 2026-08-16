//! Проверки маршрутов: поднимаем сервер на настоящем сокете и ходим по нему как клиент.

use std::net::{Ipv4Addr, SocketAddr};

use crate::{router, state::AppState};

struct Harness {
    address: SocketAddr,
    /// Каталог сгенерированного: тестам нужно класть в него файлы и проверять, что с ними стало.
    generated: std::path::PathBuf,
    _dir: tempfile::TempDir,
}

async fn start() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(dir.path()).unwrap();
    let generated = state.generated.clone();
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router(state)).await;
    });
    Harness { address, generated, _dir: dir }
}

async fn call(
    harness: &Harness,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> (u16, serde_json::Value) {
    let url = format!("http://{}{path}", harness.address);
    let client = reqwest::Client::new();
    let mut request = client.request(method.parse().unwrap(), url);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let value = serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text));
    (status, value)
}

#[tokio::test]
async fn health_reports_what_is_ready() {
    let harness = start().await;
    let (status, body) = call(&harness, "GET", "/api/health", None).await;
    assert_eq!(status, 200);
    assert_eq!(body["ok"], true);
    // Весов во временном каталоге нет, и сервер обязан честно это показать, а не молчать.
    assert_eq!(body["image"]["ready"], false);
    assert_eq!(body["text"]["ready"], false);
}

#[tokio::test]
async fn a_chat_can_be_created_read_renamed_and_deleted() {
    let harness = start().await;

    let (status, body) = call(
        &harness,
        "POST",
        "/api/chats",
        Some(serde_json::json!({ "title": "Первая история" })),
    )
    .await;
    assert_eq!(status, 200);
    let id = body["chat"]["id"].as_str().unwrap().to_string();

    let (status, body) = call(&harness, "GET", &format!("/api/chats/{id}"), None).await;
    assert_eq!(status, 200);
    assert_eq!(body["chat"]["title"], "Первая история");

    let (status, body) = call(
        &harness,
        "PATCH",
        &format!("/api/chats/{id}"),
        Some(serde_json::json!({ "title": "Переименована" })),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["chat"]["title"], "Переименована");

    let (status, _) = call(&harness, "DELETE", &format!("/api/chats/{id}"), None).await;
    assert_eq!(status, 200);

    let (status, _) = call(&harness, "GET", &format!("/api/chats/{id}"), None).await;
    assert_eq!(status, 404, "удалённый чат обязан отдавать 404");
}

#[tokio::test]
async fn settings_survive_a_round_trip_through_the_api() {
    let harness = start().await;
    let (_, body) = call(&harness, "POST", "/api/chats", Some(serde_json::json!({}))).await;
    let id = body["chat"]["id"].as_str().unwrap().to_string();

    let mut settings = body["chat"]["settings"].clone();
    settings["world"] = serde_json::json!("Затопленная крипта");
    settings["responseLength"] = serde_json::json!("epic");

    let (status, body) = call(
        &harness,
        "PATCH",
        &format!("/api/chats/{id}"),
        Some(serde_json::json!({ "settings": settings })),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["chat"]["settings"]["world"], "Затопленная крипта");
    assert_eq!(body["chat"]["settings"]["responseLength"], "epic");
}

#[tokio::test]
async fn an_unknown_api_method_is_a_404_not_the_application_page() {
    let harness = start().await;
    let (status, _) = call(&harness, "GET", "/api/нет-такого", None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn a_missing_generated_file_is_a_404() {
    let harness = start().await;
    let (status, _) = call(&harness, "GET", "/generated/нет-такого.png", None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn deleting_an_unknown_message_is_reported_as_not_found() {
    let harness = start().await;
    let (status, _) = call(&harness, "DELETE", "/api/chats/чат/messages/нет-такого", None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn characters_can_be_created_updated_and_removed() {
    let harness = start().await;
    let (_, body) = call(&harness, "POST", "/api/chats", Some(serde_json::json!({}))).await;
    let chat = body["chat"]["id"].as_str().unwrap().to_string();

    let (status, body) = call(
        &harness,
        "POST",
        &format!("/api/chats/{chat}/characters"),
        Some(serde_json::json!({ "name": "Кайра", "details": "следопыт" })),
    )
    .await;
    assert_eq!(status, 200);
    let character = body["character"]["id"].as_str().unwrap().to_string();

    let (status, body) = call(
        &harness,
        "PATCH",
        &format!("/api/chats/{chat}/characters/{character}"),
        Some(serde_json::json!({ "name": "Кайра Тень", "details": "следопыт" })),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["character"]["name"], "Кайра Тень");

    let (status, _) = call(
        &harness,
        "DELETE",
        &format!("/api/chats/{chat}/characters/{character}"),
        None,
    )
    .await;
    assert_eq!(status, 200);
    let (_, body) = call(&harness, "GET", &format!("/api/chats/{chat}/characters"), None).await;
    assert!(body["characters"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn deleting_a_story_takes_its_pictures_and_clips_with_it() {
    let harness = start().await;
    let (_, body) = call(&harness, "POST", "/api/chats", Some(serde_json::json!({}))).await;
    let chat = body["chat"]["id"].as_str().unwrap().to_string();

    // Кадр истории, её клип озвучки и чужой файл, который трогать нельзя.
    let generated = harness.generated.clone();
    let frame = generated.join("кадр.png");
    let clip = generated.join(format!("tts-{chat}-0.wav"));
    let stranger = generated.join("чужой.png");
    for path in [&frame, &clip, &stranger] {
        std::fs::write(path, b"x").unwrap();
    }
    let (_, body) = call(
        &harness,
        "POST",
        &format!("/api/chats/{chat}/characters"),
        Some(serde_json::json!({
            "name": "Кайра",
            "portrait": { "id": "p", "name": "портрет", "type": "image/png", "url": "/generated/кадр.png" }
        })),
    )
    .await;
    assert!(body["character"]["portrait"]["url"].is_string());

    let (status, _) = call(&harness, "DELETE", &format!("/api/chats/{chat}"), None).await;
    assert_eq!(status, 200);
    assert!(!frame.exists(), "кадр удалённой истории остался на диске");
    assert!(!clip.exists(), "озвучка удалённой истории осталась на диске");
    assert!(stranger.exists(), "удалён чужой файл");
}

#[tokio::test]
async fn switching_the_voice_model_picks_a_voice_that_model_actually_has() {
    let harness = start().await;
    let (_, body) = call(
        &harness,
        "PUT",
        "/api/runtime",
        Some(serde_json::json!({
            "openrouterTtsModel": "openai/gpt-audio",
            "openrouterTtsVoice": "onyx"
        })),
    )
    .await;
    assert_eq!(body["runtime"]["openrouterTtsVoice"], "onyx", "свой выбор трогать нельзя");

    // Сменили модель — голос прежней у неё не существует, подставляем её собственный.
    let (_, body) = call(
        &harness,
        "PUT",
        "/api/runtime",
        Some(serde_json::json!({ "openrouterTtsModel": "google/gemini-3.1-flash-tts-preview" })),
    )
    .await;
    let voice = body["runtime"]["openrouterTtsVoice"].as_str().unwrap_or_default().to_string();
    assert_ne!(voice, "onyx", "остался голос от прежней модели");
    assert!(
        crate::voice_catalog::voices("google/gemini-3.1-flash-tts-preview")
            .unwrap()
            .iter()
            .any(|entry| entry.name == voice),
        "подставлен голос не из набора модели: {voice}"
    );
}

#[tokio::test]
async fn a_partial_runtime_patch_keeps_every_other_setting() {
    let harness = start().await;
    // Ставим облако целиком…
    let (status, _) = call(
        &harness,
        "PUT",
        "/api/runtime",
        Some(serde_json::json!({
            "openrouterKey": "ключ",
            "openrouterNarratorOn": true,
            "openrouterNarratorModel": "some/narrator",
            "openrouterImageOn": true,
            "openrouterImageModel": "some/painter",
            "narratorCtx": 8192
        })),
    )
    .await;
    assert_eq!(status, 200);

    // …и трогаем ОДИН переключатель, как это делает панель.
    let (_, body) = call(
        &harness,
        "PUT",
        "/api/runtime",
        Some(serde_json::json!({ "openrouterTtsOn": true })),
    )
    .await;
    let runtime = &body["runtime"];
    assert_eq!(runtime["openrouterTtsOn"], true, "правка не применилась");
    assert_eq!(runtime["openrouterNarratorModel"], "some/narrator", "стёрта модель рассказчика");
    assert_eq!(runtime["openrouterImageOn"], true, "сброшена стадия кадра");
    assert_eq!(runtime["narratorCtx"], 8192, "сброшен размер контекста");
    assert_eq!(body["openrouterKeySet"], true, "потерян ключ");
}

#[tokio::test]
async fn a_dnd_chat_comes_back_with_the_hero_inventory_and_journal() {
    let harness = start().await;
    let (_, body) = call(&harness, "POST", "/api/chats", Some(serde_json::json!({}))).await;
    let chat = body["chat"]["id"].as_str().unwrap().to_string();
    let (_, body) = call(
        &harness,
        "POST",
        &format!("/api/chats/{chat}/characters"),
        Some(serde_json::json!({ "name": "Кайра" })),
    )
    .await;
    let hero = body["character"]["id"].as_str().unwrap().to_string();

    // Без этих полей интерфейсу нечем наполнить полоску здоровья, инвентарь и журнал —
    // и некому нарисовать портрет героя.
    let (status, body) = call(&harness, "GET", &format!("/api/chats/{chat}"), None).await;
    assert_eq!(status, 200);
    assert_eq!(body["heroId"], hero, "герой — самый ранний персонаж чата");
    assert!(body.get("items").is_some(), "инвентарь пропал из ответа");
    assert!(body.get("events").is_some(), "журнал пропал из ответа");
}

#[tokio::test]
async fn a_partial_character_update_keeps_the_rest_of_the_sheet() {
    let harness = start().await;
    let (_, body) = call(&harness, "POST", "/api/chats", Some(serde_json::json!({}))).await;
    let chat = body["chat"]["id"].as_str().unwrap().to_string();
    let (_, body) = call(
        &harness,
        "POST",
        &format!("/api/chats/{chat}/characters"),
        Some(serde_json::json!({
            "name": "Кайра", "details": "следопыт", "inventory": "лук", "skills": "скрытность"
        })),
    )
    .await;
    let id = body["character"]["id"].as_str().unwrap().to_string();

    // Интерфейс присылает ОДИН портрет — остальной лист обязан уцелеть.
    let (status, body) = call(
        &harness,
        "PATCH",
        &format!("/api/chats/{chat}/characters/{id}"),
        Some(serde_json::json!({
            "portrait": { "id": "p", "name": "портрет", "type": "image/png", "url": "/generated/p.png" }
        })),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["character"]["name"], "Кайра");
    assert_eq!(body["character"]["details"], "следопыт");
    assert_eq!(body["character"]["inventory"], "лук");
    assert_eq!(body["character"]["skills"], "скрытность");
    assert_eq!(body["character"]["portrait"]["url"], "/generated/p.png");

    // Явный null снимает портрет, но лист опять не трогает.
    let (_, body) = call(
        &harness,
        "PATCH",
        &format!("/api/chats/{chat}/characters/{id}"),
        Some(serde_json::json!({ "portrait": null })),
    )
    .await;
    assert!(body["character"]["portrait"].is_null());
    assert_eq!(body["character"]["details"], "следопыт");
}

#[tokio::test]
async fn a_character_without_a_name_is_rejected() {
    let harness = start().await;
    let (_, body) = call(&harness, "POST", "/api/chats", Some(serde_json::json!({}))).await;
    let chat = body["chat"]["id"].as_str().unwrap().to_string();
    let (status, _) = call(
        &harness,
        "POST",
        &format!("/api/chats/{chat}/characters"),
        Some(serde_json::json!({ "name": "   " })),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn default_settings_are_served_for_a_new_story() {
    let harness = start().await;
    let (status, body) = call(&harness, "GET", "/api/settings/defaults", None).await;
    assert_eq!(status, 200);
    assert_eq!(body["settings"]["language"], "ru");
    assert_eq!(body["settings"]["rpgEnabled"], true);
}

#[tokio::test]
async fn local_data_reports_what_is_stored_and_can_be_wiped() {
    let harness = start().await;
    call(&harness, "POST", "/api/chats", Some(serde_json::json!({}))).await;

    let (status, body) = call(&harness, "GET", "/api/local-data", None).await;
    assert_eq!(status, 200);
    assert_eq!(body["chats"], 1);

    let (status, _) = call(&harness, "DELETE", "/api/local-data", None).await;
    assert_eq!(status, 200);
    let (_, body) = call(&harness, "GET", "/api/local-data", None).await;
    assert_eq!(body["chats"], 0);
}

#[tokio::test]
async fn drawing_without_weights_fails_loudly_instead_of_hanging() {
    let harness = start().await;
    let (status, body) = call(
        &harness,
        "POST",
        "/api/images",
        Some(serde_json::json!({ "prompt": "рыцарь у крипты" })),
    )
    .await;
    assert_eq!(status, 500);
    let error = body["error"].as_str().unwrap_or_default();
    assert!(error.contains("весов"), "ошибка обязана называть причину, а не быть пустой: {error}");
}
