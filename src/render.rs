//! The HTML shell. Pages are a header and an empty mount point: everything
//! interactive is an island in `public/`, served as native ES modules with no
//! build step.

use std::sync::OnceLock;
use uuid::Uuid;

static VERSION: OnceLock<String> = OnceLock::new();

pub fn set_asset_version(v: String) {
    let _ = VERSION.set(v);
}

fn asset(p: &str) -> String {
    format!(
        "{p}?v={}",
        VERSION.get().map(String::as_str).unwrap_or("dev")
    )
}

/// Islands import their helpers by bare path, which would otherwise be fetched
/// without the release version and cached across deploys. An import map rewrites
/// those specifiers to versioned URLs, so a release reaches every module.
fn import_map() -> String {
    let entries = [
        "/public/api.js",
        "/public/art.js",
        "/public/card-face.js",
        "/public/frame.js",
        "/public/type-builder.js",
        "/public/type-data.js",
        "/public/types.js",
        "/public/vendor/htm-preact.js",
    ]
    .iter()
    .map(|module| format!(r#""{module}":"{}""#, asset(module)))
    .collect::<Vec<_>>()
    .join(",");
    format!(r#"<script type="importmap">{{"imports":{{{entries}}}}}</script>"#)
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const STYLESHEETS: [&str; 6] = [
    "/public/vendor/mana-v1-18-0.css",
    "/public/css/01-tokens.css",
    "/public/css/02-base.css",
    "/public/css/03-controls.css",
    "/public/css/04-cards.css",
    "/public/css/05-pages.css",
];

/// `island` is the module that brings the page to life. `user` is the display
/// name of whoever is signed in, which also decides whether the header offers
/// signing out.
pub fn layout(title: &str, body: &str, island: Option<&str>, user: Option<&str>) -> String {
    let styles: String = STYLESHEETS
        .iter()
        .map(|href| format!(r#"<link rel="stylesheet" href="{}">"#, asset(href)))
        .collect();
    let script = island.map_or_else(String::new, |src| {
        format!(
            r#"<script type="module" src="{}" defer></script>"#,
            asset(src)
        )
    });
    let account = user.map_or_else(String::new, |name| {
        format!(
            r#"<div class="account"><span class="who">{}</span><form class="sign-out" method="post" action="/auth/logout"><button class="quiet" type="submit">Sign out</button></form></div>"#,
            escape(name)
        )
    });
    format!(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,user-scalable=no,viewport-fit=cover"><meta name="theme-color" content="#1a1630"><meta name="apple-mobile-web-app-capable" content="yes"><meta name="apple-mobile-web-app-status-bar-style" content="black-translucent"><title>{title}</title><link rel="manifest" href="{manifest}"><link rel="icon" href="{icon}"><link rel="apple-touch-icon" href="{touch}"><link rel="preload" href="/public/vendor/bitter-v42-latin.woff2" as="font" type="font/woff2" crossorigin>{styles}{import_map}</head><body><main class="page"><header class="site-header"><a class="brand" href="/">customcardmaker</a>{account}</header>{body}<footer class="site-footer"><p>An unofficial fan tool. Magic: The Gathering, its mana and tap symbols, and related marks are property of Wizards of the Coast; this site is not approved or endorsed by Wizards.</p></footer></main>{script}</body></html>"##,
        title = escape(title),
        manifest = asset("/public/manifest.webmanifest"),
        icon = asset("/public/icon.svg"),
        touch = asset("/public/apple-touch-icon.svg"),
        import_map = import_map(),
    )
}

pub fn error_page(title: &str, message: &str) -> String {
    layout(
        title,
        &format!(
            r#"<section class="panel"><h1>{}</h1><p>{}</p><a href="/">Back home</a></section>"#,
            escape(title),
            escape(message)
        ),
        None,
        None,
    )
}

/// What a signed-out visitor sees: the two ways in.
pub fn welcome_page() -> String {
    layout(
        "customcardmaker",
        r#"<section class="welcome"><h1>Design your own Magic set</h1><p class="muted">Build a set, add cards, and see each one as it will print. Sign in with a passkey — there is no password.</p><p id="auth-error" class="error" role="alert" hidden></p><div class="auth-forms"><form id="register-form" class="panel"><h2>Create an account</h2><label>Username<input name="username" required minlength="2" maxlength="32" autocomplete="username webauthn"></label><label>Display name<input name="display_name" maxlength="48" placeholder="Optional" autocomplete="name"></label><button type="submit">Create account</button></form><form id="login-form" class="panel"><h2>Sign in</h2><label>Username<input name="username" required autocomplete="username webauthn"></label><button type="submit">Sign in</button></form></div></section>"#,
        Some("/public/auth.js"),
        None,
    )
}

pub fn sets_page(user: &str) -> String {
    layout(
        "Your sets",
        r#"<div id="app" class="sets-page"></div>"#,
        Some("/public/sets.js"),
        Some(user),
    )
}

/// JSON bound for a `<script type="application/json">`. Only `<` can end the
/// element early, and escaping it keeps the payload valid JSON either way.
fn json_script(id: &str, value: &impl serde::Serialize) -> String {
    let json = serde_json::to_string(value).unwrap_or_else(|_| "null".into());
    format!(
        r#"<script type="application/json" id="{id}">{}</script>"#,
        json.replace('<', "\\u003c")
    )
}

pub fn set_page(id: Uuid, name: &str, user: &str) -> String {
    layout(
        name,
        &format!(
            r#"<div id="app" class="set-page" data-set-id="{id}"></div>{}"#,
            json_script("limits", &crate::card::LIMITS)
        ),
        Some("/public/set.js"),
        Some(user),
    )
}

/// Every frame at once. Not linked from anywhere: it is where a frame is
/// designed, and what its regression test looks at.
pub fn frames_page() -> String {
    layout(
        "Frames",
        r#"<div id="app" class="frames-page"></div>"#,
        Some("/public/frames.js"),
        None,
    )
}
