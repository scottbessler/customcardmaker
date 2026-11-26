//! Invariants about what the server sends to the browser, checked against the
//! source rather than a running page.

const RENDER_RS: &str = include_str!("../src/render.rs");
const INDEX_CSS: [&str; 5] = [
    include_str!("../public/css/01-tokens.css"),
    include_str!("../public/css/02-base.css"),
    include_str!("../public/css/03-controls.css"),
    include_str!("../public/css/04-cards.css"),
    include_str!("../public/css/05-pages.css"),
];

/// Assets normally get their version from `asset()`. A font can't: the
/// `@font-face` src lives in static CSS that no template touches, so the
/// version has to be in the filename (`bitter-v42-latin.woff2`).
fn is_filename_versioned(url: &str) -> bool {
    url.rsplit('/').next().is_some_and(|name| {
        name.split('-')
            .any(|part| part.starts_with('v') && part[1..].chars().all(|c| c.is_ascii_digit()))
    })
}

#[test]
fn rendered_public_assets_are_versioned() {
    for attribute in [r#"src="/public/"#, r#"href="/public/"#] {
        for (index, _) in RENDER_RS.match_indices(attribute) {
            let value = &RENDER_RS[index + attribute.len() - "/public/".len()..];
            let url = &value[..value.find('"').expect("attribute should be closed")];
            assert!(
                is_filename_versioned(url),
                "{url} is rendered unversioned: use asset() or put the version in the filename"
            );
        }
    }
}

#[test]
fn preloaded_font_matches_the_font_face_it_preloads() {
    let start = RENDER_RS
        .find(r#"<link rel="preload" href=""#)
        .expect("render.rs should preload the UI font");
    let url = &RENDER_RS[start + r#"<link rel="preload" href=""#.len()..];
    let url = &url[..url.find('"').expect("preload href should be closed")];
    assert!(
        INDEX_CSS[0].contains(&format!("url({url})")),
        "{url} is preloaded but no @font-face uses it"
    );
}

#[test]
fn every_stylesheet_the_shell_links_exists() {
    for sheet in [
        "01-tokens.css",
        "02-base.css",
        "03-controls.css",
        "04-cards.css",
        "05-pages.css",
    ] {
        assert!(
            RENDER_RS.contains(&format!("/public/css/{sheet}")),
            "{sheet} is not linked by the shell"
        );
    }
}

/// Every colour in the stylesheets lives in the tokens file, so a restyle is
/// one file. Hex literals anywhere else are a token that was never named.
#[test]
fn colours_are_only_defined_in_tokens() {
    for (name, css) in ["02-base", "03-controls", "04-cards", "05-pages"]
        .iter()
        .zip(&INDEX_CSS[1..])
    {
        for line in css.lines() {
            let code = line.split("/*").next().unwrap_or("");
            assert!(
                !code.contains('#') || code.contains("url(#"),
                "{name}.css has a raw colour: {line}"
            );
        }
    }
}
