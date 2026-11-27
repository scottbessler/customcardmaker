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
    // A stranger is told nothing: private art answers as if it were not there.
    assert_eq!(
        raw(&app, "GET", &uri, vec![], None).await.0,
        StatusCode::NOT_FOUND
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

// ---- bulk edits ----------------------------------------------------------

/// A set with `names` as its cards; returns the set id and the card ids.
async fn set_with(
    app: &Router,
    who: &str,
    name: &str,
    code: &str,
    names: &[&str],
) -> (String, Vec<String>) {
    let set = call(
        app,
        "POST",
        "/api/sets",
        Some(json!({"name": name, "code": code})),
        Some(who),
    )
    .await;
    let id = set.body["id"].as_str().unwrap().to_string();
    let mut cards = Vec::new();
    for card in names {
        let made = call(
            app,
            "POST",
            &format!("/api/sets/{id}/cards"),
            Some(json!({"name": card})),
            Some(who),
        )
        .await;
        cards.push(made.body["id"].as_str().unwrap().to_string());
    }
    (id, cards)
}

async fn names(app: &Router, who: &str, set: &str) -> Vec<String> {
    let detail = call(app, "GET", &format!("/api/sets/{set}"), None, Some(who)).await;
    detail.body["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| format!("{}#{}", c["card"]["name"].as_str().unwrap(), c["position"]))
        .collect()
}

#[tokio::test]
async fn cards_are_deleted_in_bulk_and_the_rest_renumbered() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let (set, ids) = set_with(&app, &me, "Dusk", "DSK", &["A", "B", "C", "D"]).await;

    let reply = call(
        &app,
        "POST",
        "/api/cards/delete",
        Some(json!({"ids": [ids[0], ids[2]]})),
        Some(&me),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["deleted"], 2);
    assert_eq!(names(&app, &me, &set).await, ["B#1", "D#2"]);
}

#[tokio::test]
async fn cards_move_to_an_existing_set_a_new_one_or_are_copied() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let (from, ids) = set_with(&app, &me, "From", "FRM", &["A", "B", "C"]).await;
    let (to, _) = set_with(&app, &me, "To", "TOO", &["X"]).await;

    let moved = call(
        &app,
        "POST",
        "/api/cards/transfer",
        Some(json!({"ids": [ids[0]], "to": {"kind": "existing", "set": to}})),
        Some(&me),
    )
    .await;
    assert_eq!(moved.status, StatusCode::OK);
    assert_eq!(moved.body["count"], 1);
    assert_eq!(moved.body["set"]["card_count"], 2);
    assert_eq!(names(&app, &me, &to).await, ["X#1", "A#2"]);
    assert_eq!(names(&app, &me, &from).await, ["B#1", "C#2"]);

    let copied = call(
        &app,
        "POST",
        "/api/cards/transfer",
        Some(json!({"ids": [ids[1]], "to": {"kind": "existing", "set": to}, "copy": true})),
        Some(&me),
    )
    .await;
    assert_eq!(copied.status, StatusCode::OK);
    assert_eq!(names(&app, &me, &to).await, ["X#1", "A#2", "B#3"]);
    assert_eq!(
        names(&app, &me, &from).await,
        ["B#1", "C#2"],
        "a copy leaves the original"
    );

    let fresh = call(
        &app,
        "POST",
        "/api/cards/transfer",
        Some(json!({"ids": [ids[2]], "to": {"kind": "new", "name": "  Extras ", "code": "ext"}})),
        Some(&me),
    )
    .await;
    assert_eq!(fresh.status, StatusCode::OK);
    assert_eq!(fresh.body["set"]["name"], "Extras");
    assert_eq!(fresh.body["set"]["code"], "EXT");
    assert_eq!(
        names(&app, &me, fresh.body["set"]["id"].as_str().unwrap()).await,
        ["C#1"]
    );
}

#[tokio::test]
async fn bad_bulk_requests_are_refused_with_a_reason() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let (set, ids) = set_with(&app, &me, "Dusk", "DSK", &["A"]).await;

    let none = call(
        &app,
        "POST",
        "/api/cards/delete",
        Some(json!({"ids": []})),
        Some(&me),
    )
    .await;
    assert_eq!(none.status, StatusCode::BAD_REQUEST);
    assert!(
        none.body["error"]
            .as_str()
            .unwrap()
            .contains("at least one")
    );

    let many: Vec<String> = (0..501).map(|_| Uuid::new_v4().to_string()).collect();
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/cards/delete",
            Some(json!({"ids": many})),
            Some(&me)
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );

    let bad_code = call(
        &app,
        "POST",
        "/api/cards/transfer",
        Some(json!({"ids": [ids[0]], "to": {"kind": "new", "name": "Ok", "code": "x"}})),
        Some(&me),
    )
    .await;
    assert_eq!(bad_code.status, StatusCode::BAD_REQUEST);

    let same = call(
        &app,
        "POST",
        "/api/cards/transfer",
        Some(json!({"ids": [ids[0]], "to": {"kind": "existing", "set": set}})),
        Some(&me),
    )
    .await;
    assert_eq!(same.status, StatusCode::BAD_REQUEST);
    assert!(same.body["error"].as_str().unwrap().contains("already"));

    // Every one of those left the card where it was.
    assert_eq!(names(&app, &me, &set).await, ["A#1"]);
}

#[tokio::test]
async fn bulk_edits_need_a_session_and_only_touch_your_own_cards() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let bob = register(&app, "Bob").await;
    let (mine, ids) = set_with(&app, &ada, "Mine", "MNE", &["A", "B"]).await;
    let (yours, _) = set_with(&app, &bob, "Yours", "YRS", &[]).await;

    assert_eq!(
        call(
            &app,
            "POST",
            "/api/cards/delete",
            Some(json!({"ids": ids})),
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/cards/transfer",
            Some(json!({"ids": ids, "to": {"kind": "existing", "set": yours}})),
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );

    // Bob can neither delete Ada's cards nor carry them off into his own set.
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/cards/delete",
            Some(json!({"ids": ids})),
            Some(&bob)
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    let stolen = call(
        &app,
        "POST",
        "/api/cards/transfer",
        Some(json!({"ids": ids, "to": {"kind": "existing", "set": yours}})),
        Some(&bob),
    )
    .await;
    assert_eq!(stolen.status, StatusCode::NOT_FOUND);
    let copied = call(&app, "POST", "/api/cards/transfer", Some(json!({"ids": ids, "to": {"kind": "new", "name": "Mine now", "code": "NOW"}, "copy": true})), Some(&bob)).await;
    assert_eq!(copied.status, StatusCode::NOT_FOUND);

    assert_eq!(names(&app, &ada, &mine).await, ["A#1", "B#2"]);
    assert_eq!(
        call(&app, "GET", "/api/sets", None, Some(&bob))
            .await
            .body
            .as_array()
            .unwrap()
            .len(),
        1,
        "no stray set was left behind"
    );
}

// ---- the gallery ---------------------------------------------------------

/// Registers with a display name that differs from the username, so a leak of
/// the username into public pages is visible.
async fn register_named(app: &Router, username: &str, display: &str) -> String {
    let reply = call(
        app,
        "POST",
        "/auth/register/begin",
        Some(json!({"username": username, "display_name": display})),
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    reply.cookie.unwrap()
}

async fn publish(app: &Router, who: &str, set: &str, public: bool) -> Reply {
    call(
        app,
        "PATCH",
        &format!("/api/sets/{set}"),
        Some(json!({"public": public})),
        Some(who),
    )
    .await
}

#[tokio::test]
async fn a_set_is_private_until_its_owner_publishes_it() {
    let app = app().await;
    let ada = register_named(&app, "ada_private_login", "Ada L").await;
    let bob = register(&app, "Bob").await;
    let (set, _) = set_with(&app, &ada, "Dusk", "DSK", &["A", "B"]).await;

    let detail = call(&app, "GET", &format!("/api/sets/{set}"), None, Some(&ada)).await;
    assert_eq!(detail.body["set"]["public"], false);
    assert_eq!(
        call(&app, "GET", "/api/gallery", None, None).await.body,
        json!([])
    );
    assert_eq!(
        call(&app, "GET", &format!("/api/gallery/{set}"), None, None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    // Only the owner can publish it.
    assert_eq!(
        publish(&app, &bob, &set, true).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(publish(&app, &ada, &set, true).await.body["public"], true);

    // Anyone, signed in or not, can now list it and read it, and sees the
    // display name, never the login name.
    let list = call(&app, "GET", "/api/gallery", None, None).await;
    assert_eq!(list.body.as_array().unwrap().len(), 1);
    assert_eq!(list.body[0]["name"], "Dusk");
    assert_eq!(list.body[0]["by"], "Ada L");
    assert_eq!(list.body[0]["card_count"], 2);
    assert_eq!(list.body[0]["preview"]["name"], "A");
    let whole = call(&app, "GET", &format!("/api/gallery/{set}"), None, None).await;
    assert_eq!(whole.status, StatusCode::OK);
    assert_eq!(whole.body["cards"].as_array().unwrap().len(), 2);
    for reply in [&list, &whole] {
        assert!(
            !reply.body.to_string().contains("ada_private_login"),
            "the username must not reach a public page"
        );
    }

    // And private again, at once.
    publish(&app, &ada, &set, false).await;
    assert_eq!(
        call(&app, "GET", "/api/gallery", None, None).await.body,
        json!([])
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/gallery/{set}"),
            None,
            Some(&bob)
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_public_set_with_nothing_in_it_is_not_listed_yet() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let (set, _) = set_with(&app, &ada, "Empty", "EMP", &[]).await;
    publish(&app, &ada, &set, true).await;
    assert_eq!(
        call(&app, "GET", "/api/gallery", None, None).await.body,
        json!([])
    );
    call(
        &app,
        "POST",
        &format!("/api/sets/{set}/cards"),
        None,
        Some(&ada),
    )
    .await;
    assert_eq!(
        call(&app, "GET", "/api/gallery", None, None)
            .await
            .body
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn art_is_public_only_while_a_public_set_uses_it() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let bob = register(&app, "Bob").await;

    // Two uploads: one goes on a card in a set, one is never used.
    let used = upload(&app, &ada, PNG).await.1["id"]
        .as_str()
        .unwrap()
        .to_string();
    let unused = upload(&app, &ada, PNG).await.1["id"]
        .as_str()
        .unwrap()
        .to_string();
    let set = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "Art", "code": "ART"})),
        Some(&ada),
    )
    .await
    .body["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &app,
        "POST",
        &format!("/api/sets/{set}/cards"),
        Some(json!({"name": "Pic", "art": used})),
        Some(&ada),
    )
    .await;

    let get = |who: Option<&str>, id: &str| {
        let app = app.clone();
        let uri = format!("/api/art/{id}");
        let who = who.map(str::to_string);
        async move { raw(&app, "GET", &uri, vec![], who.as_deref()).await.0 }
    };
    // Private: only the owner.
    assert_eq!(get(Some(&ada), &used).await, StatusCode::OK);
    assert_eq!(get(Some(&bob), &used).await, StatusCode::NOT_FOUND);
    assert_eq!(get(None, &used).await, StatusCode::NOT_FOUND);

    publish(&app, &ada, &set, true).await;
    // The card's art is everyone's now; the other upload still is not.
    assert_eq!(get(Some(&bob), &used).await, StatusCode::OK);
    assert_eq!(get(None, &used).await, StatusCode::OK);
    assert_eq!(
        get(None, &unused).await,
        StatusCode::NOT_FOUND,
        "publishing a set must not publish the rest of the uploads"
    );
    assert_eq!(get(Some(&bob), &unused).await, StatusCode::NOT_FOUND);

    publish(&app, &ada, &set, false).await;
    assert_eq!(get(None, &used).await, StatusCode::NOT_FOUND);
    assert_eq!(get(Some(&bob), &used).await, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn gallery_pages_are_for_everyone_but_only_for_public_sets() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let (public, _) = set_with(&app, &ada, "Out", "OUT", &["A"]).await;
    let (private, _) = set_with(&app, &ada, "In", "INN", &["A"]).await;
    publish(&app, &ada, &public, true).await;

    let page = |uri: String, who: Option<String>| {
        let app = app.clone();
        async move {
            let mut request = Request::get(uri);
            if let Some(cookie) = who {
                request = request.header(header::COOKIE, cookie);
            }
            let response = app
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            let status = response.status();
            let html = String::from_utf8(
                to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();
            (status, html)
        }
    };
    let (status, html) = page("/gallery".into(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("gallery-page"));
    let (status, html) = page(format!("/gallery/{public}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("gallery-set-page"));
    // Even its owner reads her own private set through /sets, not here.
    assert_eq!(
        page(format!("/gallery/{private}"), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        page(format!("/gallery/{private}"), Some(ada.clone()))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // A signed-in visitor sees their own header on it.
    assert!(
        page("/gallery".into(), Some(ada))
            .await
            .1
            .contains("Sign out")
    );
}

// ---- importing a Cockatrice file -----------------------------------------

/// A one-route server that answers a real PNG, so an import can be given a
/// `picurl` that actually resolves. Returns the picture's URL.
async fn picture_server() -> String {
    use axum::routing::get;
    let app = Router::new().route(
        "/card.png",
        get(|| async { ([(header::CONTENT_TYPE, "image/png")], PNG) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}/card.png")
}

fn cockatrice_file(picture: Option<&str>) -> Value {
    let picurl = picture
        .map(|url| format!(r#" picurl="{url}""#))
        .unwrap_or_default();
    json!({
        "xml": format!(
            r#"<cockatrice_carddatabase version="4"><sets><set><name>IMP</name><longname>Imported</longname></set></sets><cards>
            <card><name>Art Bird</name><text>Flying</text><prop><type>Creature — Bird</type><manacost>{{1}}{{W}}</manacost><pt>1/1</pt></prop><set rarity="common"{picurl}>IMP</set></card>
            <card><name>No Art</name><text>Nothing</text><prop><type>Sorcery</type></prop><set rarity="rare">IMP</set></card>
            </cards></cockatrice_carddatabase>"#
        )
    })
}

#[tokio::test]
async fn a_cockatrice_file_becomes_a_set_owned_by_its_importer() {
    let app = app().await;
    let me = register(&app, "Ada").await;

    let reply = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some(cockatrice_file(None)),
        Some(&me),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
    assert_eq!(reply.body["cards_imported"], 2);
    assert_eq!(reply.body["images_imported"], 0);
    assert_eq!(reply.body["set_code"], "IMP");
    assert_eq!(
        reply.body["set_name"], "Imported",
        "the file's long name is used when none is given"
    );

    let set = reply.body["set_id"].as_str().unwrap();
    let detail = call(&app, "GET", &format!("/api/sets/{set}"), None, Some(&me)).await;
    let cards = detail.body["cards"].as_array().unwrap();
    assert_eq!(cards[0]["card"]["name"], "Art Bird");
    assert_eq!(cards[0]["card"]["mana_cost"], "{1}{W}");
    assert_eq!(cards[1]["card"]["rarity"], "rare");

    // A stranger cannot import, and the code from the file is upper-cased.
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/import/cockatrice",
            Some(cockatrice_file(None)),
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    let lowercase = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some(json!({"xml": "<cockatrice_carddatabase><cards><card><name>A</name><set>abc</set></card></cards></cockatrice_carddatabase>"})),
        Some(&me),
    )
    .await;
    assert_eq!(lowercase.body["set_code"], "ABC");
}

#[tokio::test]
async fn previewing_an_import_creates_nothing_and_says_what_it_would_do() {
    let app = app().await;
    let me = register(&app, "Ada").await;

    let reply = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some({
            let mut body = cockatrice_file(Some("https://example.test/bird.jpg"));
            body["images"] = json!(true);
            body["preview"] = json!(true);
            body
        }),
        Some(&me),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    assert_eq!(reply.body["preview"], true);
    assert!(reply.body["set_id"].is_null(), "a preview creates nothing");
    assert_eq!(reply.body["cards_imported"], 2);
    assert_eq!(reply.body["images_imported"], 1, "counted, not fetched");
    assert_eq!(reply.body["set_code"], "IMP");
    assert_eq!(reply.body["set_name"], "Imported");

    let cards = reply.body["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0]["name"], "Art Bird");
    assert_eq!(cards[0]["type_line"], "Creature — Bird");
    assert_eq!(cards[0]["rarity"], "common");
    assert_eq!(cards[0]["features"], json!(["cost", "power_toughness"]));
    assert_eq!(cards[0]["picture"], true);
    assert_eq!(cards[1]["picture"], false);
    assert_eq!(cards[1]["rarity"], "rare");
    // One card has no picture in the file, and the preview says so.
    assert!(
        reply.body["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note
                .as_str()
                .unwrap()
                .contains("No picture in the file for 1 card")),
        "{:?}",
        reply.body["notes"]
    );

    // Nothing was written: no set, and no art.
    assert_eq!(
        call(&app, "GET", "/api/sets", None, Some(&me)).await.body,
        json!([])
    );
}

#[tokio::test]
async fn a_preview_says_what_the_file_had_that_we_could_not_keep() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let xml = r#"<cockatrice_carddatabase><cards>
    <card><name>Cut</name><text>Deal 2.</text><prop><layout>split</layout><side>front</side><type>Instant</type><manacost>{R}</manacost></prop><set rarity="special" picurl="https://example.test/e.jpg">ABC</set></card>
    <card><name>Orphan</name><text>No front.</text><prop><layout>transform</layout><side>back</side><type>Instant</type></prop><set rarity="rare">ABC</set><related attach="transform">Missing</related></card>
    </cards></cockatrice_carddatabase>"#;

    let reply = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some(json!({"xml": xml, "preview": true, "images": true})),
        Some(&me),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    let notes: Vec<&str> = reply.body["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|note| note.as_str().unwrap())
        .collect();
    assert!(
        notes
            .iter()
            .any(|note| note.contains("layout we do not draw") && note.contains("Cut")),
        "{notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|note| note.contains("no front") && note.contains("Orphan")),
        "{notes:?}"
    );
    assert!(
        notes.iter().any(|note| note.contains("special")),
        "{notes:?}"
    );

    // The card we cannot draw is marked, so the page can say its picture is kept as printed.
    let cards = reply.body["cards"].as_array().unwrap();
    assert_eq!(cards[0]["printed"], true);
    assert_eq!(cards[1]["printed"], false);
}

/// A picture server that answers only after `delay`, so an import is still
/// running when the next request arrives.
async fn slow_picture_server(delay: std::time::Duration) -> String {
    use axum::routing::get;
    let app = Router::new().route(
        "/card.png",
        get(move || async move {
            tokio::time::sleep(delay).await;
            ([(header::CONTENT_TYPE, "image/png")], PNG)
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}/card.png")
}

/// Polls a job until it stops running, and returns its last progress.
async fn wait_for(app: &Router, who: &str, job: &str) -> Value {
    let mut progress = Value::Null;
    for _ in 0..300 {
        let reply = call(
            app,
            "GET",
            &format!("/api/import/cockatrice/{job}"),
            None,
            Some(who),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
        progress = reply.body;
        if progress["state"] != "running" {
            return progress;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the import never finished: {progress}");
}

#[tokio::test]
async fn a_background_import_reports_progress_and_ends_with_the_set() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let url = picture_server().await;

    let started = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some({
            let mut body = cockatrice_file(Some(&url));
            body["images"] = json!(true);
            body["background"] = json!(true);
            body
        }),
        Some(&me),
    )
    .await;
    assert_eq!(started.status, StatusCode::ACCEPTED, "{:?}", started.body);
    let job = started.body["job_id"].as_str().unwrap().to_string();

    let progress = wait_for(&app, &me, &job).await;
    assert_eq!(progress["state"], "done", "{progress}");
    assert_eq!(progress["cards_total"], 2);
    assert_eq!(progress["cards_done"], 2);
    assert_eq!(progress["pictures_total"], 1);
    assert_eq!(progress["pictures_done"], 1);
    assert_eq!(progress["pictures_tried"], 1);
    assert_eq!(progress["set_code"], "IMP");

    // The set it made is really there.
    let set = progress["set_id"].as_str().unwrap();
    let detail = call(&app, "GET", &format!("/api/sets/{set}"), None, Some(&me)).await;
    assert_eq!(detail.body["cards"].as_array().unwrap().len(), 2);

    // A job is the account's own: a stranger is turned away, another account
    // answers as if it were not there.
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/import/cockatrice/{job}"),
            None,
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    let bob = register(&app, "Bob").await;
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/import/cockatrice/{job}"),
            None,
            Some(&bob)
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/import/cockatrice/{}", Uuid::new_v4()),
            None,
            Some(&me)
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn only_one_import_runs_at_a_time_per_account() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let url = slow_picture_server(std::time::Duration::from_millis(300)).await;

    let body = || {
        let mut body = cockatrice_file(Some(&url));
        body["images"] = json!(true);
        body["background"] = json!(true);
        body
    };
    let first = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some(body()),
        Some(&me),
    )
    .await;
    assert_eq!(first.status, StatusCode::ACCEPTED, "{:?}", first.body);

    let second = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some(body()),
        Some(&me),
    )
    .await;
    assert_eq!(second.status, StatusCode::CONFLICT, "{:?}", second.body);
    assert!(
        second.body["error"].as_str().unwrap().contains("already"),
        "{:?}",
        second.body
    );

    // Another account is not blocked by mine.
    let bob = register(&app, "Bob").await;
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/import/cockatrice",
            Some(body()),
            Some(&bob)
        )
        .await
        .status,
        StatusCode::ACCEPTED
    );

    // Let the first finish, so the test leaves nothing running.
    let job = first.body["job_id"].as_str().unwrap().to_string();
    assert_eq!(wait_for(&app, &me, &job).await["state"], "done");
}

#[tokio::test]
async fn a_background_import_names_the_pictures_it_could_not_fetch() {
    let app = app().await;
    let me = register(&app, "Ada").await;

    let started = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some({
            let mut body = cockatrice_file(Some("http://127.0.0.1:9/nothing.png"));
            body["images"] = json!(true);
            body["background"] = json!(true);
            body
        }),
        Some(&me),
    )
    .await;
    let job = started.body["job_id"].as_str().unwrap().to_string();

    let progress = wait_for(&app, &me, &job).await;
    assert_eq!(progress["state"], "done", "{progress}");
    assert_eq!(progress["pictures_done"], 0);
    assert_eq!(progress["pictures_tried"], 1);
    let failures = progress["failures"].as_array().unwrap();
    assert_eq!(failures.len(), 1, "{failures:?}");
    let failure = failures[0].as_str().unwrap();
    assert!(
        failure.contains("Art Bird") && failure.contains("nothing.png"),
        "{failure}"
    );
}

#[tokio::test]
async fn importing_can_download_each_card_picture() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let url = picture_server().await;

    let reply = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some({
            let mut body = cockatrice_file(Some(&url));
            body["images"] = json!(true);
            body
        }),
        Some(&me),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
    assert_eq!(reply.body["cards_imported"], 2);
    assert_eq!(reply.body["images_imported"], 1);
    // The one card the file has no picture for is reported, as in a preview.
    assert_eq!(
        reply.body["notes"],
        json!(["No picture in the file for 1 card."])
    );

    let set = reply.body["set_id"].as_str().unwrap();
    let detail = call(&app, "GET", &format!("/api/sets/{set}"), None, Some(&me)).await;
    let cards = detail.body["cards"].as_array().unwrap();
    let art = cards[0]["card"]["art"]
        .as_str()
        .expect("the card kept its picture");
    assert!(cards[1]["card"]["art"].is_null(), "no URL, no art");

    // The picture is now an ordinary upload of the importer's.
    let (status, headers, bytes) =
        raw(&app, "GET", &format!("/api/art/{art}"), vec![], Some(&me)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, PNG);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
}

#[tokio::test]
async fn a_picture_that_cannot_be_fetched_is_skipped_not_fatal() {
    let app = app().await;
    let me = register(&app, "Ada").await;

    // A URL nothing answers on, and one that answers with something that is not
    // an image. Both cards import; neither gets art.
    let dead = "http://127.0.0.1:9/nothing.png";
    let reply = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some({
            let mut body = cockatrice_file(Some(dead));
            body["images"] = json!(true);
            body
        }),
        Some(&me),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
    assert_eq!(reply.body["images_imported"], 0);

    let set = reply.body["set_id"].as_str().unwrap();
    let detail = call(&app, "GET", &format!("/api/sets/{set}"), None, Some(&me)).await;
    assert!(detail.body["cards"][0]["card"]["art"].is_null());
}

// ---- templates and frame colours -----------------------------------------

#[tokio::test]
async fn a_set_has_a_default_template_its_owner_can_change() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let bob = register(&app, "Bob").await;
    let (set, _) = set_with(&app, &ada, "Looks", "LOK", &["A"]).await;

    let detail = call(&app, "GET", &format!("/api/sets/{set}"), None, Some(&ada)).await;
    assert_eq!(detail.body["set"]["default_template"], "modern");

    let changed = call(
        &app,
        "PATCH",
        &format!("/api/sets/{set}"),
        Some(json!({"default_template": "classic"})),
        Some(&ada),
    )
    .await;
    assert_eq!(changed.status, StatusCode::OK);
    assert_eq!(changed.body["default_template"], "classic");
    // Changing one thing leaves the rest alone.
    assert_eq!(changed.body["name"], "Looks");
    assert_eq!(changed.body["public"], false);

    let bad = call(
        &app,
        "PATCH",
        &format!("/api/sets/{set}"),
        Some(json!({"default_template": "neon"})),
        Some(&ada),
    )
    .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        call(
            &app,
            "PATCH",
            &format!("/api/sets/{set}"),
            Some(json!({"default_template": "classic"})),
            Some(&bob)
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );

    // It reaches the gallery, so a public set is drawn the way its maker chose.
    call(
        &app,
        "PATCH",
        &format!("/api/sets/{set}"),
        Some(json!({"public": true})),
        Some(&ada),
    )
    .await;
    let list = call(&app, "GET", "/api/gallery", None, None).await;
    assert_eq!(list.body[0]["default_template"], "classic");
    let whole = call(&app, "GET", &format!("/api/gallery/{set}"), None, None).await;
    assert_eq!(whole.body["default_template"], "classic");
}

#[tokio::test]
async fn a_card_keeps_the_frame_and_template_it_names_and_may_name_only_real_ones() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let (set, ids) = set_with(&app, &ada, "Looks", "LOK", &["A"]).await;

    let saved = call(
        &app,
        "PUT",
        &format!("/api/cards/{}", ids[0]),
        Some(json!({"name": "A", "frame": "gold", "template": "full-art"})),
        Some(&ada),
    )
    .await;
    assert_eq!(saved.status, StatusCode::OK);
    assert_eq!(
        (
            saved.body["card"]["frame"].as_str(),
            saved.body["card"]["template"].as_str()
        ),
        (Some("gold"), Some("full-art"))
    );
    let detail = call(&app, "GET", &format!("/api/sets/{set}"), None, Some(&ada)).await;
    assert_eq!(detail.body["cards"][0]["card"]["frame"], "gold");

    // A blank card names neither: from its cost, and the set's choice.
    let blank = call(
        &app,
        "POST",
        &format!("/api/sets/{set}/cards"),
        None,
        Some(&ada),
    )
    .await;
    assert_eq!(
        (
            blank.body["card"]["frame"].as_str(),
            blank.body["card"]["template"].as_str()
        ),
        (Some(""), Some(""))
    );

    for bad in [
        json!({"frame": "purple"}),
        json!({"template": "neon"}),
        json!({"frame": "gold; drop"}),
    ] {
        assert_eq!(
            call(
                &app,
                "PUT",
                &format!("/api/cards/{}", ids[0]),
                Some(bad),
                Some(&ada)
            )
            .await
            .status,
            StatusCode::BAD_REQUEST
        );
    }
}

#[tokio::test]
async fn the_frames_gallery_exists_only_in_a_development_build() {
    let dir = std::env::temp_dir().join(format!("customcardmaker-{}", Uuid::new_v4()));
    let get = |app: Router| async move {
        app.oneshot(Request::get("/dev/frames").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    };
    let dev = app::router(AppState::open(&dir, true).await.unwrap());
    assert_eq!(get(dev).await, StatusCode::OK);
    let prod = app::router(AppState::open(&dir, false).await.unwrap());
    assert_eq!(
        get(prod.clone()).await,
        StatusCode::NOT_FOUND,
        "a visitor to the real site has nothing there"
    );
    // And the old address is gone everywhere.
    let old = prod
        .oneshot(Request::get("/frames").body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status();
    assert_eq!(old, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn pictures_can_be_marked_as_whole_cards_on_import() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let url = picture_server().await;

    let import = |whole: bool| {
        let url = url.clone();
        let app = app.clone();
        let me = me.clone();
        async move {
            let mut body = cockatrice_file(Some(&url));
            body["images"] = json!(true);
            body["whole_cards"] = json!(whole);
            let reply = call(
                &app,
                "POST",
                "/api/import/cockatrice",
                Some(body),
                Some(&me),
            )
            .await;
            assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
            let set = reply.body["set_id"].as_str().unwrap().to_string();
            call(&app, "GET", &format!("/api/sets/{set}"), None, Some(&me))
                .await
                .body
        }
    };

    let whole = import(true).await;
    assert_eq!(whole["cards"][0]["card"]["art_is_card"], true);
    // A card with no picture has nothing to be a whole card.
    assert_eq!(whole["cards"][1]["card"]["art_is_card"], false);

    let art_only = import(false).await;
    assert_eq!(art_only["cards"][0]["card"]["art_is_card"], false);
}

#[tokio::test]
async fn a_card_has_the_features_it_turns_on_and_only_their_fields() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let (_set, ids) = set_with(&app, &ada, "Looks", "LOK", &["A"]).await;
    let put = |body: Value| {
        let app = app.clone();
        let ada = ada.clone();
        let id = ids[0].clone();
        async move {
            call(
                &app,
                "PUT",
                &format!("/api/cards/{id}"),
                Some(body),
                Some(&ada),
            )
            .await
        }
    };

    let saved = put(json!({"features": ["cost", "loyalty"], "name": "A", "loyalty": "4"})).await;
    assert_eq!(saved.status, StatusCode::OK);
    assert_eq!(saved.body["card"]["features"], json!(["cost", "loyalty"]));
    assert_eq!(saved.body["card"]["loyalty"], "4");

    for bad in [
        // No such feature; a field the features do not allow; a field too long.
        json!({"features": ["neon"]}),
        json!({"features": ["cost"], "power_toughness": "2/2"}),
        json!({"features": ["power_toughness"], "mana_cost": "{1}"}),
        json!({"features": ["loyalty"], "loyalty": "12345"}),
        // A token has no mana cost; a saga has no stats.
        json!({"features": ["cost"], "template": "token"}),
        json!({"features": ["power_toughness"], "template": "saga"}),
    ] {
        assert_eq!(put(bad).await.status, StatusCode::BAD_REQUEST);
    }
    // Stats combine, and every field the features allow comes back, empty if unset.
    let both = put(json!({"features": ["power_toughness", "loyalty"], "name": "Gideon"})).await;
    assert_eq!(both.status, StatusCode::OK);
    assert_eq!(both.body["card"]["power_toughness"], "");
    assert!(both.body["card"].get("mana_cost").is_none());
    let token = put(json!({"features": ["power_toughness"], "template": "token"})).await;
    assert_eq!(token.status, StatusCode::OK);
}

#[tokio::test]
async fn a_card_saved_before_features_existed_is_read_as_the_card_it_was() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let (_set, ids) = set_with(&app, &ada, "Old", "OLD", &["A"]).await;
    // What an old client sent: a `layout`, and every field whether the card used it or not.
    let old = json!({"name": "Walker", "layout": "planeswalker", "loyalty": "5", "power_toughness": "", "mana_cost": "{2}{W}"});
    let put = call(
        &app,
        "PUT",
        &format!("/api/cards/{}", ids[0]),
        Some(old),
        Some(&ada),
    )
    .await;
    assert_eq!(put.status, StatusCode::OK, "{:?}", put.body);
    assert_eq!(put.body["card"]["features"], json!(["cost", "loyalty"]));
    assert_eq!(put.body["card"]["loyalty"], "5");
    assert!(put.body["card"].get("layout").is_none());
    assert!(put.body["card"].get("power_toughness").is_none());
}

#[tokio::test]
async fn a_back_face_is_part_of_the_card_and_its_art_follows_the_same_rules() {
    let app = app().await;
    let ada = register(&app, "Ada").await;
    let bob = register(&app, "Bob").await;

    let front_art = upload(&app, &ada, PNG).await.1["id"]
        .as_str()
        .unwrap()
        .to_string();
    let back_art = upload(&app, &ada, PNG).await.1["id"]
        .as_str()
        .unwrap()
        .to_string();
    let set = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "Two", "code": "TWO"})),
        Some(&ada),
    )
    .await
    .body["id"]
        .as_str()
        .unwrap()
        .to_string();
    let made = call(
        &app,
        "POST",
        &format!("/api/sets/{set}/cards"),
        Some(json!({"name": "Day", "art": front_art, "back": {"name": "Night", "art": back_art, "template": "saga"}})),
        Some(&ada),
    )
    .await;
    assert_eq!(made.status, StatusCode::CREATED, "{:?}", made.body);
    let card = made.body["id"].as_str().unwrap().to_string();
    assert_eq!(made.body["card"]["back"]["name"], "Night");
    assert_eq!(made.body["card"]["back"]["template"], "saga");

    // A back face is checked like a front, and has no back of its own.
    for bad in [
        json!({"back": {"features": ["neon"]}}),
        json!({"back": {"back": {"name": "Deeper"}}}),
    ] {
        let put = call(
            &app,
            "PUT",
            &format!("/api/cards/{card}"),
            Some(bad),
            Some(&ada),
        )
        .await;
        assert_eq!(put.status, StatusCode::BAD_REQUEST);
    }

    // Someone else's upload cannot be put on the back.
    let bobs = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "Bobs", "code": "BOB"})),
        Some(&bob),
    )
    .await
    .body["id"]
        .as_str()
        .unwrap()
        .to_string();
    let stolen = call(
        &app,
        "POST",
        &format!("/api/sets/{bobs}/cards"),
        Some(json!({"back": {"art": back_art}})),
        Some(&bob),
    )
    .await;
    assert_eq!(stolen.status, StatusCode::BAD_REQUEST);

    // Publishing the set shows the back's art too, and credits for both faces travel with it.
    let get = |who: Option<&str>, id: &str| {
        let app = app.clone();
        let uri = format!("/api/art/{id}");
        let who = who.map(str::to_string);
        async move { raw(&app, "GET", &uri, vec![], who.as_deref()).await.0 }
    };
    assert_eq!(get(None, &back_art).await, StatusCode::NOT_FOUND);
    publish(&app, &ada, &set, true).await;
    assert_eq!(get(None, &back_art).await, StatusCode::OK);
    assert_eq!(get(None, &front_art).await, StatusCode::OK);
    publish(&app, &ada, &set, false).await;
    assert_eq!(get(None, &back_art).await, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn importing_a_double_faced_card_brings_both_faces_and_both_pictures() {
    let app = app().await;
    let me = register(&app, "Ada").await;
    let url = picture_server().await;
    let xml = format!(
        r#"<cockatrice_carddatabase><cards>
<card><name>Day</name><text>Flip.</text><prop><layout>transform</layout><side>front</side><type>Creature — Human</type><manacost>{{1}}{{W}}</manacost><pt>1/1</pt></prop><set rarity="rare" picurl="{url}">ABC</set><related attach="transform">Night</related></card>
<card><name>Night</name><text>Flipped.</text><prop><layout>transform</layout><side>back</side><type>Creature — Werewolf</type><pt>2/2</pt></prop><set rarity="rare" picurl="{url}">ABC</set><related attach="transform">Day</related></card>
</cards></cockatrice_carddatabase>"#
    );
    let reply = call(
        &app,
        "POST",
        "/api/import/cockatrice",
        Some(json!({"xml": xml, "set_code": "ABC", "images": true, "whole_cards": true})),
        Some(&me),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
    assert_eq!(reply.body["cards_imported"], 1, "two faces are one card");
    assert_eq!(reply.body["images_imported"], 2);

    let set = reply.body["set_id"].as_str().unwrap();
    let detail = call(&app, "GET", &format!("/api/sets/{set}"), None, Some(&me)).await;
    let card = &detail.body["cards"][0]["card"];
    assert_eq!(card["name"], "Day");
    assert_eq!(card["back"]["name"], "Night");
    let (front, back) = (
        card["art"].as_str().unwrap(),
        card["back"]["art"].as_str().unwrap(),
    );
    assert_ne!(front, back, "each face has a picture of its own");
    assert_eq!(card["art_is_card"], true);
    assert_eq!(card["back"]["art_is_card"], true);
    // Both pictures are the importer's own uploads, credits included.
    assert!(detail.body["credits"].is_object());
}
