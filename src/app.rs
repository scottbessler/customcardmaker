use crate::{
    art::{self, ArtStore},
    auth,
    db::Db,
    render, routes,
    sets::SetStore,
    users::UserStore,
};
use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{DefaultBodyLimit, FromRef, Request},
    http::{HeaderValue, header::CACHE_CONTROL},
    middleware::{Next, from_fn},
    response::Response,
    routing::{get, post, put},
};
use axum_extra::extract::cookie::Key;
use std::{env, io, sync::Arc, time::Instant};
use tokio::net::TcpListener;
use tower_http::{services::ServeDir, trace::TraceLayer};
use webauthn_rs::prelude::{Url, Webauthn, WebauthnBuilder};

const DEFAULT_PORT: u16 = 8080;
const PORT_SCAN_LIMIT: u16 = 100;

const LOCAL_SESSION_SECRET: &str =
    "customcardmaker-local-development-session-secret-v1-keep-browser-sessions-across-restarts";

#[derive(Clone)]
pub struct AppState {
    pub sets: SetStore,
    pub users: UserStore,
    pub art: ArtStore,
    pub webauthn: Arc<Webauthn>,
    pub key: Key,
    /// Sign in by username alone. For development and tests, where no
    /// authenticator can be driven; never set in production.
    pub passkey_disabled: bool,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

impl AppState {
    pub async fn open(data: impl AsRef<std::path::Path>, passkey_disabled: bool) -> Result<Self> {
        let db = Db::open(data).await?;
        Ok(Self {
            sets: SetStore::new(db.clone()),
            users: UserStore::new(db.clone()),
            art: ArtStore::open(db).await?,
            webauthn: Arc::new(build_webauthn()?),
            key: load_key(),
            passkey_disabled,
        })
    }
}

pub fn router(s: AppState) -> Router {
    Router::new()
        .route("/", get(routes::index))
        .route("/frames", get(routes::frames))
        .route("/sets/{id}", get(routes::set_page))
        .route("/healthcheck", get(routes::healthcheck))
        .route("/auth/register/begin", post(auth::register_begin))
        .route("/auth/register/finish", post(auth::register_finish))
        .route("/auth/login/begin", post(auth::login_begin))
        .route("/auth/login/finish", post(auth::login_finish))
        .route("/auth/logout", post(auth::logout))
        .route("/sw.js", get(routes::service_worker))
        .route("/api/sets", get(routes::list_sets).post(routes::create_set))
        .route(
            "/api/sets/{id}",
            get(routes::get_set)
                .patch(routes::patch_set)
                .delete(routes::delete_set),
        )
        .route("/api/sets/{id}/cards", post(routes::add_card))
        .route("/api/import/cockatrice", post(routes::import_cockatrice))
        .route(
            "/api/art",
            post(routes::upload_art).layer(DefaultBodyLimit::max(art::MAX_BYTES)),
        )
        .route("/api/art/{id}", get(routes::get_art))
        .route(
            "/api/cards/{id}",
            put(routes::save_card).delete(routes::delete_card),
        )
        .nest_service("/public", ServeDir::new("public"))
        .layer(from_fn(cache_control))
        .layer(from_fn(log_request))
        .layer(TraceLayer::new_for_http())
        .with_state(s)
}

/// Only a URL that carries the release version may be cached forever. The
/// islands import their helpers by bare path, which the import map versions;
/// anything that arrives without a `?v=` has to revalidate, otherwise an
/// installed PWA holds a stale copy across releases.
fn cache_directive(path: &str, query: Option<&str>, release: bool) -> &'static str {
    let versioned = query.is_some_and(|query| {
        query
            .split('&')
            .any(|pair| pair.strip_prefix("v=").is_some_and(|v| !v.is_empty()))
    });
    if release && path.starts_with("/public/") && versioned {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

async fn cache_control(req: Request, next: Next) -> Response {
    let directive = cache_directive(req.uri().path(), req.uri().query(), !cfg!(debug_assertions));
    let mut r = next.run(req).await;
    // A handler that knows better says so: uploaded art is immutable per id.
    if !r.headers().contains_key(CACHE_CONTROL) {
        r.headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static(directive));
    }
    r
}

/// A response past this is worth a WARN of its own, so a slow path is visible
/// in the logs without anybody going looking for it.
const SLOW_REQUEST_MS: u128 = 250;

async fn log_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = Instant::now();
    let r = next.run(req).await;
    let elapsed_ms = start.elapsed().as_millis();
    tracing::info!(%method, path, status = r.status().as_u16(), elapsed_ms, "request");
    if elapsed_ms >= SLOW_REQUEST_MS {
        tracing::warn!(%method, path, elapsed_ms, "slow request");
    }
    r
}

pub async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    render::set_asset_version(asset_version());
    let data = env::var("DATA_PATH").unwrap_or_else(|_| "data".into());
    let state = AppState::open(&data, env_flag("PASSKEY_DISABLED")).await?;
    let app = router(state);
    let listener = bind_listener().await?;
    let addr = listener
        .local_addr()
        .context("failed to read bound address")?;
    tracing::info!(bind_addr = %addr, "listening on http://localhost:{}", addr.port());
    println!("listening on http://localhost:{}", addr.port());
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bind_listener() -> Result<TcpListener> {
    if let Ok(port) = env::var("PORT") {
        let addr = format!("0.0.0.0:{port}");
        return TcpListener::bind(&addr)
            .await
            .with_context(|| format!("failed to bind {addr}"));
    }
    for port in DEFAULT_PORT..=DEFAULT_PORT + PORT_SCAN_LIMIT {
        let addr = format!("0.0.0.0:{port}");
        match TcpListener::bind(&addr).await {
            Ok(listener) => return Ok(listener),
            Err(e) if e.kind() == io::ErrorKind::AddrInUse => {}
            Err(e) => return Err(e).with_context(|| format!("failed to bind {addr}")),
        }
    }
    TcpListener::bind("0.0.0.0:0")
        .await
        .context("failed to bind an ephemeral port")
}

pub fn build_webauthn() -> Result<Webauthn> {
    let id = env::var("RP_ID").unwrap_or_else(|_| "localhost".into());
    let origin =
        Url::parse(&env::var("RP_ORIGIN").unwrap_or_else(|_| "http://localhost:8080".into()))
            .context("RP_ORIGIN must be a valid URL")?;
    WebauthnBuilder::new(&id, &origin)?
        .rp_name("customcardmaker")
        .build()
        .context("failed to build WebAuthn")
}

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("True")
    )
}

/// Production sets `SESSION_SECRET` (64+ bytes) so sessions survive a restart;
/// without it a release build mints a fresh key each boot, which signs
/// everyone out rather than trusting a key anybody could read in this file.
fn load_key() -> Key {
    match env::var("SESSION_SECRET") {
        Ok(secret) if secret.len() >= 64 => Key::from(secret.as_bytes()),
        Ok(_) => Key::generate(),
        Err(_) if cfg!(debug_assertions) => Key::from(LOCAL_SESSION_SECRET.as_bytes()),
        Err(_) => Key::generate(),
    }
}

/// A hash of everything under `public/`, so a release busts the cache for
/// exactly the files that changed and nobody keeps a list of them by hand.
fn asset_version() -> String {
    use std::hash::{Hash, Hasher};
    fn walk(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, files);
            } else {
                files.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(std::path::Path::new("public"), &mut files);
    files.sort();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for file in files {
        file.hash(&mut h);
        if let Ok(bytes) = std::fs::read(&file) {
            bytes.hash(&mut h);
        }
    }
    format!("{:x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_versioned_public_files_are_immutable_in_release() {
        assert_eq!(
            cache_directive("/public/set.js", Some("v=abc"), true),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_directive("/public/set.js", None, true), "no-cache");
        assert_eq!(
            cache_directive("/public/set.js", Some("v="), true),
            "no-cache"
        );
        assert_eq!(
            cache_directive("/api/sets", Some("v=abc"), true),
            "no-cache"
        );
        assert_eq!(
            cache_directive("/public/set.js", Some("v=abc"), false),
            "no-cache"
        );
    }
}
