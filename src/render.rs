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
        "/public/card-face.js",
        "/public/frame.js",
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

const STYLESHEETS: [&str; 4] = [
    "/public/css/01-tokens.css",
    "/public/css/02-base.css",
    "/public/css/03-controls.css",
    "/public/css/04-cards.css",
];

/// `island` is the module that brings the page to life; `mount` is the
/// attributes on the element it mounts into.
pub fn layout(title: &str, body: &str, island: Option<&str>) -> String {
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
    format!(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,user-scalable=no,viewport-fit=cover"><meta name="theme-color" content="#1a1630"><meta name="apple-mobile-web-app-capable" content="yes"><meta name="apple-mobile-web-app-status-bar-style" content="black-translucent"><title>{title}</title><link rel="manifest" href="{manifest}"><link rel="icon" href="{icon}"><link rel="apple-touch-icon" href="{touch}"><link rel="preload" href="/public/vendor/bitter-v42-latin.woff2" as="font" type="font/woff2" crossorigin>{styles}{import_map}</head><body><main class="page"><header class="site-header"><a class="brand" href="/">customcardmaker</a></header>{body}</main>{script}</body></html>"##,
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
    )
}

pub fn sets_page() -> String {
    layout(
        "Your sets",
        r#"<div id="app" class="sets-page"></div>"#,
        Some("/public/sets.js"),
    )
}

pub fn set_page(id: Uuid, name: &str) -> String {
    layout(
        name,
        &format!(r#"<div id="app" class="set-page" data-set-id="{id}"></div>"#),
        Some("/public/set.js"),
    )
}
