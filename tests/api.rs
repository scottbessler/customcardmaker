use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use customcardmaker::app::{self, AppState};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

async fn app() -> Router {
    // A fresh directory per harness: tests run in parallel, and two of them
    // opening one SQLite file would run its migrations twice.
    let dir = std::env::temp_dir().join(format!("customcardmaker-{}", Uuid::new_v4()));
    app::router(AppState::open(&dir).await.unwrap())
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(value) => {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn healthcheck_answers() {
    let app = app().await;
    let response = app
        .oneshot(Request::get("/healthcheck").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_set_and_its_cards_round_trip() {
    let app = app().await;
    let (status, set) = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": " Dusk ", "code": "dsk"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(set["name"], "Dusk");
    assert_eq!(set["code"], "DSK");
    let id = set["id"].as_str().unwrap().to_string();

    // No body at all is a blank card.
    let (status, card) = call(&app, "POST", &format!("/api/sets/{id}/cards"), None).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(card["position"], 1);
    assert_eq!(card["card"]["rarity"], "common");
    let card_id = card["id"].as_str().unwrap().to_string();

    let draft = json!({"name": "Mist Warden", "mana_cost": "{1}{U}", "rarity": "rare", "rules_text": "Flying"});
    let (status, saved) = call(&app, "PUT", &format!("/api/cards/{card_id}"), Some(draft)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["card"]["name"], "Mist Warden");

    let (_, detail) = call(&app, "GET", &format!("/api/sets/{id}"), None).await;
    assert_eq!(detail["set"]["card_count"], 1);
    assert_eq!(detail["cards"][0]["card"]["rules_text"], "Flying");

    let (_, listed) = call(&app, "GET", "/api/sets", None).await;
    assert_eq!(listed[0]["code"], "DSK");

    let (status, _) = call(&app, "DELETE", &format!("/api/cards/{card_id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, "DELETE", &format!("/api/sets/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, "GET", &format!("/api/sets/{id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bad_input_is_a_json_400_not_a_page() {
    let app = app().await;
    let (status, body) = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "", "code": "ABC"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("name"));
    let (status, _) = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "Ok", "code": "x"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_missing_set_page_is_a_404_page() {
    let app = app().await;
    let response = app
        .oneshot(
            Request::get(format!("/sets/{}", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
}
