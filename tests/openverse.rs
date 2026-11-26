//! Searching and importing Creative Commons art, against a fake Openverse on a
//! local port: no network, and every awkward answer a real one might give.

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Path, RawQuery, State},
    http::{HeaderValue, Request, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use customcardmaker::{
    app::{self, AppState},
    art,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n pretend this is a picture";

// Ids the fake treats specially.
const OK: &str = "11111111-1111-1111-1111-111111111111";
const ND: &str = "22222222-2222-2222-2222-222222222222";
const MATURE: &str = "33333333-3333-3333-3333-333333333333";
const HUGE: &str = "44444444-4444-4444-4444-444444444444";
const NOT_AN_IMAGE: &str = "55555555-5555-5555-5555-555555555555";
const REDIRECTS: &str = "66666666-6666-6666-6666-666666666666";
const NC: &str = "77777777-7777-7777-7777-777777777777";

#[derive(Clone, Default)]
struct Fake {
    searches: Arc<Mutex<Vec<String>>>,
    base: Arc<Mutex<String>>,
}

fn record(id: &str, base: &str) -> Value {
    let (license, version, mature) = match id {
        ND => ("by-nd", "2.0", false),
        MATURE => ("by", "2.0", true),
        NC => ("by-nc", "2.0", false),
        _ => ("by", "2.0", false),
    };
    json!({
        "id": id, "title": "Forest\u{0}  at dusk", "creator": "Jane Painter",
        "license": license, "license_version": version, "mature": mature,
        "license_url": format!("https://creativecommons.org/licenses/{license}/{version}/"),
        "thumbnail": format!("{base}/v1/images/{id}/thumb/"),
        "foreign_landing_url": "https://www.flickr.com/photos/jane/1", "width": 800, "height": 600,
    })
}

async fn search(State(fake): State<Fake>, RawQuery(query): RawQuery) -> Response {
    let query = query.unwrap_or_default();
    fake.searches.lock().unwrap().push(query.clone());
    if query.contains("q=busy") {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    if query.contains("q=down") {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let base = fake.base.lock().unwrap().clone();
    let mut results: Vec<Value> = [OK, ND, MATURE, NC]
        .iter()
        .map(|id| record(id, &base))
        .collect();
    // Records a hostile or careless index might hold.
    let mut foreign = record("88888888-8888-8888-8888-888888888888", &base);
    foreign["thumbnail"] = json!("https://evil.example.net/x.jpg");
    let mut script = record("99999999-9999-9999-9999-999999999999", &base);
    script["foreign_landing_url"] = json!("javascript:alert(1)");
    let mut bad_id = record(OK, &base);
    bad_id["id"] = json!("../../etc/passwd");
    results.extend([foreign, script, bad_id]);
    Json(json!({ "result_count": 240, "page_count": 12, "results": results })).into_response()
}

async fn detail(State(fake): State<Fake>, Path(id): Path<String>) -> Response {
    let base = fake.base.lock().unwrap().clone();
    if id == "00000000-0000-0000-0000-000000000000" {
        return StatusCode::NOT_FOUND.into_response();
    }
    Json(record(&id, &base)).into_response()
}

async fn thumb(Path(id): Path<String>) -> Response {
    match id.as_str() {
        HUGE => {
            let mut body = PNG.to_vec();
            body.resize(art::MAX_BYTES + 10, 0);
            ([(header::CONTENT_TYPE, "image/png")], body).into_response()
        }
        NOT_AN_IMAGE => (
            [(header::CONTENT_TYPE, "image/png")],
            "<html>not a picture</html>",
        )
            .into_response(),
        REDIRECTS => {
            let mut response = StatusCode::FOUND.into_response();
            response.headers_mut().insert(
                header::LOCATION,
                HeaderValue::from_static("https://evil.example.net/steal.png"),
            );
            response
        }
        _ => ([(header::CONTENT_TYPE, "image/png")], PNG.to_vec()).into_response(),
    }
}

async fn fake() -> (Fake, String) {
    let state = Fake::default();
    let app = Router::new()
        .route("/v1/images/", get(search))
        .route("/v1/images/{id}/", get(detail))
        .route("/v1/images/{id}/thumb/", get(thumb))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    *state.base.lock().unwrap() = base.clone();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (state, base)
}

async fn app(base: &str) -> Router {
    let dir = std::env::temp_dir().join(format!("customcardmaker-ov-{}", Uuid::new_v4()));
    app::router(
        AppState::open(&dir, true)
            .await
            .unwrap()
            .with_openverse(base)
            .unwrap(),
    )
}

struct Reply {
    status: StatusCode,
    body: Value,
    raw: Vec<u8>,
    cookie: Option<String>,
}

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
    let raw = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    Reply {
        status,
        body: serde_json::from_slice(&raw).unwrap_or(Value::Null),
        raw,
        cookie,
    }
}

async fn register(app: &Router, name: &str) -> String {
    call(
        app,
        "POST",
        "/auth/register/begin",
        Some(json!({"username": name})),
        None,
    )
    .await
    .cookie
    .unwrap()
}

#[tokio::test]
async fn results_are_cleaned_and_only_what_a_card_may_use_is_offered() {
    let (fake, base) = fake().await;
    let app = app(&base).await;
    let me = register(&app, "Ada").await;

    let reply = call(&app, "GET", "/api/art/search?q=forest", None, Some(&me)).await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    let ids: Vec<&str> = reply.body["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    // The ND, mature and non-commercial records, the one with a foreign
    // thumbnail and the one with a bad id are all left out; two remain.
    assert_eq!(ids, [OK, "99999999-9999-9999-9999-999999999999"]);
    let first = &reply.body["results"][0];
    assert_eq!(
        first["title"], "Forest  at dusk",
        "control characters are stripped"
    );
    assert_eq!(first["creator"], "Jane Painter");
    assert_eq!(first["license_label"], "CC BY 2.0");
    assert_eq!(
        first["license_url"],
        "https://creativecommons.org/licenses/by/2.0/"
    );
    assert!(
        first["thumbnail"].as_str().unwrap().starts_with(&base),
        "thumbnails come only from the configured host"
    );
    assert_eq!(
        reply.body["results"][1]["landing_url"], "",
        "a javascript: link is dropped, not passed on"
    );
    assert_eq!(reply.body["total"], 240);

    // What was asked of Openverse: the safe licences, no mature work, formats we store.
    let sent = fake.searches.lock().unwrap()[0].clone();
    assert!(sent.contains("license=cc0%2Cpdm%2Cby%2Cby-sa"), "{sent}");
    assert!(
        !sent.contains("nc"),
        "non-commercial is not asked for by default: {sent}"
    );
    assert!(
        sent.contains("mature=false") && sent.contains("extension=jpg%2Cpng"),
        "{sent}"
    );
}

#[tokio::test]
async fn non_commercial_licences_are_offered_only_when_asked_for() {
    let (fake, base) = fake().await;
    let app = app(&base).await;
    let me = register(&app, "Ada").await;
    let reply = call(
        &app,
        "GET",
        "/api/art/search?q=forest&nc=true",
        None,
        Some(&me),
    )
    .await;
    let ids: Vec<&str> = reply.body["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&NC), "the by-nc record appears when asked for");
    assert!(!ids.contains(&ND), "NoDerivatives never does");
    assert!(fake.searches.lock().unwrap()[0].contains("by-nc"));
}

#[tokio::test]
async fn search_needs_a_session_and_a_sensible_question() {
    let (_fake, base) = fake().await;
    let app = app(&base).await;
    assert_eq!(
        call(&app, "GET", "/api/art/search?q=forest", None, None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let me = register(&app, "Ada").await;
    for uri in [
        "/api/art/search?q=",
        "/api/art/search?q=%20%20",
        "/api/art/search?q=forest&page=0",
        "/api/art/search?q=forest&page=99",
    ] {
        assert_eq!(
            call(&app, "GET", uri, None, Some(&me)).await.status,
            StatusCode::BAD_REQUEST,
            "{uri}"
        );
    }
    let long = "x".repeat(101);
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/art/search?q={long}"),
            None,
            Some(&me)
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn the_same_search_is_answered_from_memory() {
    let (fake, base) = fake().await;
    let app = app(&base).await;
    let me = register(&app, "Ada").await;
    for _ in 0..3 {
        assert_eq!(
            call(&app, "GET", "/api/art/search?q=Forest", None, Some(&me))
                .await
                .status,
            StatusCode::OK
        );
    }
    assert_eq!(
        fake.searches.lock().unwrap().len(),
        1,
        "Openverse's anonymous quota is shared; do not spend it twice"
    );
}

#[tokio::test]
async fn one_account_cannot_search_without_limit() {
    let (_fake, base) = fake().await;
    let app = app(&base).await;
    let (ada, bob) = (register(&app, "Ada").await, register(&app, "Bob").await);
    for n in 0..30 {
        assert_eq!(
            call(
                &app,
                "GET",
                &format!("/api/art/search?q=word{n}"),
                None,
                Some(&ada)
            )
            .await
            .status,
            StatusCode::OK,
            "search {n}"
        );
    }
    let refused = call(
        &app,
        "GET",
        "/api/art/search?q=one-too-many",
        None,
        Some(&ada),
    )
    .await;
    assert_eq!(refused.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(refused.body["error"].as_str().unwrap().contains("Wait"));
    assert_eq!(
        call(&app, "GET", "/api/art/search?q=fine", None, Some(&bob))
            .await
            .status,
        StatusCode::OK,
        "someone else is unaffected"
    );
}

#[tokio::test]
async fn when_openverse_is_busy_or_down_the_reader_is_told_plainly() {
    let (_fake, base) = fake().await;
    let app = app(&base).await;
    let me = register(&app, "Ada").await;
    let busy = call(&app, "GET", "/api/art/search?q=busy", None, Some(&me)).await;
    assert_eq!(busy.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(busy.body["error"].as_str().unwrap().contains("busy"));
    let down = call(&app, "GET", "/api/art/search?q=down", None, Some(&me)).await;
    assert_eq!(down.status, StatusCode::BAD_GATEWAY);
    assert!(
        !down.body["error"].as_str().unwrap().contains("500"),
        "an upstream status is not for the reader"
    );
    // And with nothing listening at all.
    let gone = customcardmaker_app_with_dead_upstream().await;
    let me = register(&gone, "Grace").await;
    assert_eq!(
        call(&gone, "GET", "/api/art/search?q=forest", None, Some(&me))
            .await
            .status,
        StatusCode::BAD_GATEWAY
    );
}

async fn customcardmaker_app_with_dead_upstream() -> Router {
    // Port 1 is never open.
    app("http://127.0.0.1:1").await
}

#[tokio::test]
async fn a_picked_image_is_brought_home_with_its_credit() {
    let (_fake, base) = fake().await;
    let app = app(&base).await;
    let ada = register(&app, "Ada").await;
    let bob = register(&app, "Bob").await;

    let got = call(
        &app,
        "POST",
        "/api/art/openverse",
        Some(json!({"id": OK})),
        Some(&ada),
    )
    .await;
    assert_eq!(got.status, StatusCode::CREATED, "{:?}", got.body);
    assert_eq!(got.body["credit"]["creator"], "Jane Painter");
    assert_eq!(got.body["credit"]["license_label"], "CC BY 2.0");
    assert_eq!(
        got.body["credit"]["source_url"],
        "https://www.flickr.com/photos/jane/1"
    );
    let art = got.body["id"].as_str().unwrap().to_string();

    // It is stored like any upload: the owner's, and served from here.
    let served = call(&app, "GET", &format!("/api/art/{art}"), None, Some(&ada)).await;
    assert_eq!(
        (served.status, served.raw.as_slice()),
        (StatusCode::OK, PNG)
    );
    assert_eq!(
        call(&app, "GET", &format!("/api/art/{art}"), None, Some(&bob))
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    // The credit follows the card: in her set, and in the gallery once public.
    let set = call(
        &app,
        "POST",
        "/api/sets",
        Some(json!({"name": "Borrowed", "code": "BRW"})),
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
        Some(json!({"name": "Pic", "art": art})),
        Some(&ada),
    )
    .await;
    let mine = call(&app, "GET", &format!("/api/sets/{set}"), None, Some(&ada)).await;
    assert_eq!(mine.body["credits"][&art]["creator"], "Jane Painter");
    assert_eq!(
        call(&app, "GET", &format!("/api/gallery/{set}"), None, None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    call(
        &app,
        "PATCH",
        &format!("/api/sets/{set}"),
        Some(json!({"public": true})),
        Some(&ada),
    )
    .await;
    let public = call(&app, "GET", &format!("/api/gallery/{set}"), None, None).await;
    assert_eq!(public.status, StatusCode::OK, "{:?}", public.body);
    assert_eq!(public.body["credits"][&art]["license_label"], "CC BY 2.0");
    assert_eq!(public.body["credits"][&art]["title"], "Forest  at dusk");
}

#[tokio::test]
async fn a_pick_that_a_card_may_not_use_is_refused() {
    let (_fake, base) = fake().await;
    let app = app(&base).await;
    let me = register(&app, "Ada").await;
    let pick = |id: &str| {
        call(
            &app,
            "POST",
            "/api/art/openverse",
            Some(json!({"id": id})),
            Some(&me),
        )
    };

    assert_eq!(
        pick(ND).await.status,
        StatusCode::BAD_REQUEST,
        "a card crops its art; that is a derivative"
    );
    assert_eq!(pick(MATURE).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(pick("not-an-id").await.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        pick("../../etc/passwd").await.status,
        StatusCode::BAD_REQUEST,
        "an id never reaches a URL unless it is a uuid"
    );
    assert_eq!(
        pick("00000000-0000-0000-0000-000000000000").await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        pick(HUGE).await.status,
        StatusCode::BAD_REQUEST,
        "no larger than an upload may be"
    );
    assert_eq!(
        pick(NOT_AN_IMAGE).await.status,
        StatusCode::BAD_GATEWAY,
        "the bytes decide, not the label"
    );
    assert_eq!(
        pick(REDIRECTS).await.status,
        StatusCode::BAD_GATEWAY,
        "a redirect off the configured host is not followed"
    );
    // A non-commercial one is fine to import: it was offered only when asked for.
    assert_eq!(pick(NC).await.status, StatusCode::CREATED);
}

#[tokio::test]
async fn importing_needs_a_session() {
    let (_fake, base) = fake().await;
    let app = app(&base).await;
    let reply = call(
        &app,
        "POST",
        "/api/art/openverse",
        Some(json!({"id": OK})),
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}
