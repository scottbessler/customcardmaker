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
    app::router(AppState::open(&dir, true).await.unwrap())
}

struct Reply {
    status: StatusCode,
    body: Value,
    cookie: Option<String>,
}

/// `who` is a session cookie as `register` returned it, sent back verbatim.
async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    who: Option<&str>,
) -> Reply {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(cookie) = who {
        request = request.header(header::COOKIE, cookie);
    }
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
    let cookie = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("sid=") && !v.starts_with("sid=;"))
        .map(|v| v.split(';').next().unwrap().to_string());
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    Reply {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
    }
}

/// Registers an account (passkeys are off in tests) and returns its session.
async fn register(app: &Router, name: &str) -> String {
    let reply = call(
        app,
        "POST",
        "/auth/register/begin",
        Some(json!({"username": name})),
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    reply.cookie.expect("registering signs you in")
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
async fn the_api_turns_a_stranger_away() {
    let app = app().await;
    for (method, uri) in [
        ("GET", "/api/sets"),
        ("POST", "/api/sets/x/cards"),
        ("DELETE", "/api/cards/x"),
    ] {
        let reply = call(&app, method, uri, None, None).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert!(reply.body["error"].is_string());
    }
}

#[tokio::test]
async fn a_name_registers_once_and_signs_in_again() {
    let app = app().await;
    register(&app, "Ada").await;

    let again = call(
        &app,
        "POST",
        "/auth/register/begin",
        Some(json!({"username": "ada"})),
        None,
    )
    .await;
    assert_eq!(
        again.status,
        StatusCode::CONFLICT,
        "case does not make a new name"
    );

    let back = call(
        &app,
        "POST",
        "/auth/login/begin",
        Some(json!({"username": "ADA"})),
        None,
    )
    .await;
    assert_eq!(back.status, StatusCode::OK);
    assert!(back.cookie.is_some());

    let nobody = call(
        &app,
        "POST",
        "/auth/login/begin",
        Some(json!({"username": "grace"})),
        None,
    )
    .await;
    assert_eq!(nobody.status, StatusCode::NOT_FOUND);

    let blank = call(
        &app,
        "POST",
        "/auth/register/begin",
        Some(json!({"username": " "})),
        None,
    )
    .await;
    assert_eq!(blank.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_home_page_follows_the_session() {
    let app = app().await;
    let html = |reply: axum::response::Response| async move {
        String::from_utf8(
            to_bytes(reply.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    };
    let stranger = html(
        app.clone()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert!(stranger.contains("register-form"));
    assert!(!stranger.contains("Sign out"));

    let cookie = register(&app, "Ada").await;
    let request = Request::get("/")
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    let member = html(app.oneshot(request).await.unwrap()).await;
    assert!(member.contains("sets-page"));
    assert!(member.contains("Sign out"));
}

#[tokio::test]
async fn a_set_and_its_cards_round_trip() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let me = Some(me.as_str());

    let set = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": " Dusk ", "code": "dsk"})),
        me,
    )
    .await;
    assert_eq!(set.status, StatusCode::CREATED);
    assert_eq!(set.body["name"], "Dusk");
    assert_eq!(set.body["code"], "DSK");
    let id = set.body["id"].as_str().unwrap().to_string();

    // No body at all is a blank card.
    let card = call(&app, "POST", &format!("/api/sets/{id}/cards"), None, me).await;
    assert_eq!(card.status, StatusCode::CREATED);
    assert_eq!(card.body["position"], 1);
    assert_eq!(card.body["card"]["rarity"], "common");
    let card_id = card.body["id"].as_str().unwrap().to_string();

    let draft = json!({"name": "Mist Warden", "mana_cost": "{1}{U}", "rarity": "rare", "rules_text": "Flying"});
    let saved = call(
        &app,
        "PUT",
        &format!("/api/cards/{card_id}"),
        Some(draft),
        me,
    )
    .await;
    assert_eq!(saved.status, StatusCode::OK);
    assert_eq!(saved.body["card"]["name"], "Mist Warden");

    let detail = call(&app, "GET", &format!("/api/sets/{id}"), None, me).await;
    assert_eq!(detail.body["set"]["card_count"], 1);
    assert_eq!(detail.body["cards"][0]["card"]["rules_text"], "Flying");

    let listed = call(&app, "GET", "/api/sets", None, me).await;
    assert_eq!(listed.body[0]["code"], "DSK");

    assert_eq!(
        call(&app, "DELETE", &format!("/api/cards/{card_id}"), None, me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "DELETE", &format!("/api/sets/{id}"), None, me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", &format!("/api/sets/{id}"), None, me)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn one_account_cannot_reach_anothers_set() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let bob = register(&app, "Bob").await;
    let (ada, bob) = (Some(ada.as_str()), Some(bob.as_str()));

    let set = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "Mine", "code": "MNE"})),
        ada,
    )
    .await;
    let id = set.body["id"].as_str().unwrap().to_string();
    let card = call(&app, "POST", &format!("/api/sets/{id}/cards"), None, ada).await;
    let card_id = card.body["id"].as_str().unwrap().to_string();

    assert_eq!(
        call(&app, "GET", "/api/sets", None, bob).await.body,
        json!([])
    );
    for (method, uri, body) in [
        ("GET", format!("/api/sets/{id}"), None),
        (
            "PATCH",
            format!("/api/sets/{id}"),
            Some(json!({"name": "Stolen"})),
        ),
        ("DELETE", format!("/api/sets/{id}"), None),
        ("POST", format!("/api/sets/{id}/cards"), Some(json!({}))),
        (
            "PUT",
            format!("/api/cards/{card_id}"),
            Some(json!({"name": "Edited"})),
        ),
        ("DELETE", format!("/api/cards/{card_id}"), None),
    ] {
        let reply = call(&app, method, &uri, body, bob).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{method} {uri}");
    }

    // And Ada's set is exactly as she left it.
    let detail = call(&app, "GET", &format!("/api/sets/{id}"), None, ada).await;
    assert_eq!(detail.body["set"]["name"], "Mine");
    assert_eq!(detail.body["cards"][0]["card"]["name"], "");
}

#[tokio::test]
async fn bad_input_is_a_json_400_not_a_page() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let me = Some(me.as_str());
    let reply = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "", "code": "ABC"})),
        me,
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert!(reply.body["error"].as_str().unwrap().contains("name"));
    let reply = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "Ok", "code": "x"})),
        me,
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_set_page_is_only_for_its_owner() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let bob = register(&app, "Bob").await;
    let set = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "Mine", "code": "MNE"})),
        Some(&ada),
    )
    .await;
    let uri = format!("/sets/{}", set.body["id"].as_str().unwrap());

    let page = |cookie: Option<String>| {
        let app = app.clone();
        let uri = uri.clone();
        async move {
            let mut request = Request::get(uri);
            if let Some(cookie) = cookie {
                request = request.header(header::COOKIE, cookie);
            }
            app.oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
        }
    };
    assert_eq!(page(Some(ada)).await.status(), StatusCode::OK);
    assert_eq!(page(Some(bob)).await.status(), StatusCode::NOT_FOUND);
    let stranger = page(None).await;
    assert!(
        stranger.status().is_redirection(),
        "a stranger is sent to sign in"
    );
}

// ---- uploaded art -------------------------------------------------------

/// The 8-byte PNG signature is all the server looks at.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n not really a picture, and that is fine";

async fn raw(
    app: &Router,
    method: &str,
    uri: &str,
    body: Vec<u8>,
    who: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(cookie) = who {
        request = request.header(header::COOKIE, cookie);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let (status, headers) = (response.status(), response.headers().clone());
    (
        status,
        headers,
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}

async fn upload(app: &Router, who: &str, body: &[u8]) -> (StatusCode, Value) {
    let (status, _, bytes) = raw(app, "POST", "/api/art", body.to_vec(), Some(who)).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn art_is_served_back_to_its_owner_alone() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let bob = register(&app, "Bob").await;

    let (status, body) = upload(&app, &ada, PNG).await;
    assert_eq!(status, StatusCode::CREATED);
    let uri = format!("/api/art/{}", body["id"].as_str().unwrap());

    let (status, headers, bytes) = raw(&app, "GET", &uri, vec![], Some(&ada)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, PNG);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(
        headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    assert!(
        headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("private")
    );

    assert_eq!(
        raw(&app, "GET", &uri, vec![], Some(&bob)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        raw(&app, "GET", &uri, vec![], None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        raw(&app, "POST", "/api/art", PNG.to_vec(), None).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn only_images_of_a_known_kind_and_size_are_accepted() {
    let app = app().await;
    let ada = register(&app, "Ada").await;

    let (status, body) = upload(&app, &ada, b"<html><script>alert(1)</script>").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("PNG"));
    assert_eq!(upload(&app, &ada, b"").await.0, StatusCode::BAD_REQUEST);

    let mut huge = PNG.to_vec();
    huge.resize(customcardmaker::art::MAX_BYTES + 1, 0);
    assert_eq!(
        upload(&app, &ada, &huge).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn a_card_may_only_use_art_its_owner_uploaded() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let bob = register(&app, "Bob").await;
    let (ada, bob) = (Some(ada.as_str()), Some(bob.as_str()));

    let art = upload(&app, ada.unwrap(), PNG).await.1["id"]
        .as_str()
        .unwrap()
        .to_string();
    let theirs = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "Bobs", "code": "BOB"})),
        bob,
    )
    .await;
    let set = theirs.body["id"].as_str().unwrap().to_string();

    let stolen = call(
        &app,
        "POST",
        &format!("/api/sets/{set}/cards"),
        Some(json!({"art": art})),
        bob,
    )
    .await;
    assert_eq!(
        stolen.status,
        StatusCode::BAD_REQUEST,
        "bob cannot point at ada's upload"
    );
    let made_up = call(
        &app,
        "POST",
        &format!("/api/sets/{set}/cards"),
        Some(json!({"art": Uuid::new_v4()})),
        bob,
    )
    .await;
    assert_eq!(made_up.status, StatusCode::BAD_REQUEST);

    let mine = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "Mine", "code": "MNE"})),
        ada,
    )
    .await;
    let set = mine.body["id"].as_str().unwrap().to_string();
    let card = call(
        &app,
        "POST",
        &format!("/api/sets/{set}/cards"),
        Some(json!({"art": art})),
        ada,
    )
    .await;
    assert_eq!(card.status, StatusCode::CREATED);
    assert_eq!(card.body["card"]["art"], art.as_str());

    // Taking it off again is just saving the card without it.
    let id = card.body["id"].as_str().unwrap();
    let cleared = call(
        &app,
        "PUT",
        &format!("/api/cards/{id}"),
        Some(json!({"art": null})),
        ada,
    )
    .await;
    assert_eq!(cleared.status, StatusCode::OK);
    assert!(cleared.body["card"]["art"].is_null());
}
