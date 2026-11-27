//! Templates: how a card is drawn (see `CARDTYPES.md`).
//!
//! A template is a restricted XML file in `public/templates/`. It names the features a card
//! must have to wear it (`requires`) and the ones it has nowhere to draw (`excludes`), says
//! which frame it uses, and either has no body (the standard grid layout the stylesheet draws
//! for that frame) or places each field on the card in card-width units. The same files are
//! embedded here, for the server to check a card against, and sent to the page, for the
//! browser to draw from, so the rule is written once.
//!
//! The format is closed on purpose: an element or attribute that is not listed is an error,
//! which is what would make accepting a template from a person safe later.

use crate::cardtype;
use roxmltree::{Document, Node};
use serde::Serialize;
use std::sync::OnceLock;

/// Every built-in template, in the order the editor offers them.
const FILES: &[&str] = &[
    include_str!("../public/templates/modern.xml"),
    include_str!("../public/templates/classic.xml"),
    include_str!("../public/templates/borderless.xml"),
    include_str!("../public/templates/full-art.xml"),
    include_str!("../public/templates/nexus.xml"),
    include_str!("../public/templates/planeswalker.xml"),
    include_str!("../public/templates/saga.xml"),
    include_str!("../public/templates/token.xml"),
];

/// The frames a template may use. `inherit` is the set's look.
const FRAMES: &[&str] = &["modern", "classic", "borderless", "full-art", "inherit"];

#[derive(Debug, Serialize)]
pub struct Template {
    pub id: String,
    pub label: String,
    /// Features a card must have on to wear it.
    pub requires: Vec<String>,
    /// Features a card must not have on, because the template has nowhere to draw them.
    pub excludes: Vec<String>,
    pub frame: String,
    /// Its source, which the page parses to draw it.
    pub xml: &'static str,
}

impl Template {
    /// Whether a card with these features on can wear it.
    pub fn works_with(&self, features: &[String]) -> bool {
        let has = |id: &String| features.contains(id);
        self.requires.iter().all(has) && !self.excludes.iter().any(has)
    }

    /// A look: a frame of its own that any card can wear, so a set may make it its default. A
    /// template that takes its frame from the card (`inherit`) is a layout, not a look.
    pub fn is_look(&self) -> bool {
        self.requires.is_empty() && self.excludes.is_empty() && self.frame != "inherit"
    }
}

/// All the built-in templates. They are part of the program, so one that does not parse is a
/// bug caught by this module's tests, not something to recover from.
pub fn all() -> &'static [Template] {
    static LOADED: OnceLock<Vec<Template>> = OnceLock::new();
    LOADED.get_or_init(|| {
        FILES
            .iter()
            .map(|xml| parse(xml).unwrap_or_else(|e| panic!("a built-in template is invalid: {e}")))
            .collect()
    })
}

pub fn get(id: &str) -> Option<&'static Template> {
    all().iter().find(|template| template.id == id)
}

fn attrs(node: Node<'_, '_>, allowed: &[&str]) -> Result<(), String> {
    for attribute in node.attributes() {
        if !allowed.contains(&attribute.name()) {
            return Err(format!(
                "<{}> has no attribute \"{}\"",
                node.tag_name().name(),
                attribute.name()
            ));
        }
    }
    Ok(())
}

fn number(
    node: Node<'_, '_>,
    name: &str,
    range: std::ops::RangeInclusive<f64>,
) -> Result<(), String> {
    let value = node
        .attribute(name)
        .ok_or_else(|| format!("<{}> needs {name}", node.tag_name().name()))?;
    let parsed: f64 = value
        .parse()
        .map_err(|_| format!("{name}=\"{value}\" is not a number"))?;
    if !range.contains(&parsed) {
        return Err(format!("{name}=\"{value}\" is off the card"));
    }
    Ok(())
}

fn list(value: Option<&str>) -> Result<Vec<String>, String> {
    let ids: Vec<String> = value
        .unwrap_or_default()
        .split_whitespace()
        .map(String::from)
        .collect();
    for id in &ids {
        if cardtype::feature(id).is_none() {
            return Err(format!("\"{id}\" is not a card feature"));
        }
    }
    Ok(ids)
}

/// Reads and checks one template file.
pub fn parse(xml: &'static str) -> Result<Template, String> {
    let doc = Document::parse(xml).map_err(|e| format!("not XML: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "template" {
        return Err("the root element must be <template>".into());
    }
    attrs(root, &["id", "label", "requires", "excludes", "frame"])?;
    let id = root.attribute("id").ok_or("a template needs an id")?;
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
        return Err(format!(
            "\"{id}\" is not a template id (lowercase letters and hyphens)"
        ));
    }
    let label = root.attribute("label").ok_or("a template needs a label")?;
    let frame = root.attribute("frame").ok_or("a template needs a frame")?;
    if !FRAMES.contains(&frame) {
        return Err(format!("\"{frame}\" is not a frame"));
    }
    let requires = list(root.attribute("requires"))?;
    let excludes = list(root.attribute("excludes"))?;
    if let Some(both) = requires.iter().find(|id| excludes.contains(id)) {
        return Err(format!("it both needs and excludes {both}"));
    }

    // A template may name any field it has not excluded; a card without the feature shows it
    // empty. One that names a field nothing provides is an error.
    let named: Vec<&str> = cardtype::BASE
        .iter()
        .chain(
            cardtype::FEATURES
                .iter()
                .filter(|feature| !excludes.iter().any(|id| id == feature.id))
                .flat_map(|feature| feature.fields.iter()),
        )
        .map(|field| field.key)
        .collect();

    for element in root.children().filter(Node::is_element) {
        let name = element.tag_name().name();
        match name {
            "plate" => {
                attrs(element, &["role", "x", "y", "w", "h"])?;
                if !matches!(element.attribute("role"), Some("name" | "type")) {
                    return Err("a <plate> has role=\"name\" or role=\"type\"".into());
                }
            }
            "art" | "box" | "stat" => attrs(element, &["x", "y", "w", "h"])?,
            other => return Err(format!("<{other}> is not a template element")),
        }
        // A <stat> may leave out w and h: it is then as wide and tall as its value,
        // anchored by its bottom-right corner (which is where a printed card's P/T
        // box grows from). Both or neither.
        let sized =
            name != "stat" || element.attribute("w").is_some() == element.attribute("h").is_some();
        if !sized {
            return Err("a <stat> needs both w and h, or neither".into());
        }
        for (side, range) in [
            ("x", 0.0..=100.0),
            ("y", 0.0..=140.0),
            ("w", 0.0..=100.0),
            ("h", 0.0..=140.0),
        ] {
            if name == "stat" && (side == "w" || side == "h") && element.attribute(side).is_none() {
                continue;
            }
            number(element, side, range)?;
        }
        for child in element.children().filter(Node::is_element) {
            if child.tag_name().name() != "field" || name == "art" {
                return Err(format!(
                    "<{}> cannot hold <{}>",
                    name,
                    child.tag_name().name()
                ));
            }
            attrs(child, &["name", "as"])?;
            let key = child.attribute("name").ok_or("a <field> needs a name")?;
            if !named.contains(&key) {
                return Err(format!("\"{key}\" is not a field this template can draw"));
            }
            match child.attribute("as") {
                None => {}
                Some("abilities" | "chapters") if key == "rules_text" => {}
                Some(other) => return Err(format!("as=\"{other}\" is not a reader for {key}")),
            }
        }
    }
    Ok(Template {
        id: id.to_string(),
        label: label.to_string(),
        requires,
        excludes,
        frame: frame.to_string(),
        xml,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bad(xml: &'static str) -> String {
        parse(xml).unwrap_err()
    }

    #[test]
    fn every_built_in_template_is_valid_and_has_its_own_id() {
        let ids: Vec<_> = all().iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "modern",
                "classic",
                "borderless",
                "full-art",
                "nexus",
                "planeswalker",
                "saga",
                "token"
            ]
        );
        // A file is named for its template.
        for (file, template) in FILES.iter().zip(all()) {
            assert!(file.contains(&format!("id=\"{}\"", template.id)));
        }
    }

    #[test]
    fn the_frames_are_looks_and_the_rest_say_what_they_need() {
        assert!(get("modern").is_some_and(Template::is_look));
        // A layout takes the card's frame, so it is not a look a set can default to.
        assert!(!get("nexus").is_some_and(Template::is_look));
        let walker = get("planeswalker").unwrap();
        assert_eq!(walker.requires, ["loyalty"]);
        assert!(!walker.is_look());
        let on = |ids: &[&str]| ids.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert!(walker.works_with(&on(&["cost", "loyalty"])));
        assert!(!walker.works_with(&on(&["cost"])), "no loyalty");
        let token = get("token").unwrap();
        assert!(token.works_with(&on(&["power_toughness"])));
        assert!(!token.works_with(&on(&["cost"])), "a token has no cost");
        assert!(!get("saga").unwrap().works_with(&on(&["power_toughness"])));
    }

    #[test]
    fn the_format_is_closed() {
        assert!(
            bad(r#"<template id="a" label="A" frame="modern"><script/></template>"#)
                .contains("not a template element")
        );
        assert!(
            bad(r#"<template id="a" label="A" frame="modern" onclick="x"/>"#)
                .contains("no attribute")
        );
        assert!(
            bad(r#"<template id="A" label="A" frame="modern"/>"#).contains("not a template id")
        );
        assert!(bad(r#"<template id="a" label="A" frame="neon"/>"#).contains("not a frame"));
        assert!(
            bad(r#"<template id="a" label="A" frame="modern" requires="magic"/>"#)
                .contains("not a card feature")
        );
        assert!(bad(r#"<template id="a" label="A" frame="modern" requires="loyalty" excludes="loyalty"/>"#).contains("both needs and excludes"));
        assert!(bad(r#"<div/>"#).contains("root element"));
        assert!(bad("<template").contains("not XML"));
    }

    #[test]
    fn places_must_be_on_the_card() {
        let wrap = |body: &'static str| -> String {
            // Leak a string for the 'static the loader wants; this is a test.
            let xml: &'static str = Box::leak(
                format!(r#"<template id="a" label="A" frame="modern">{body}</template>"#)
                    .into_boxed_str(),
            );
            parse(xml).unwrap_err()
        };
        assert!(wrap(r#"<art x="0" y="0" w="50"/>"#).contains("needs h"));
        assert!(wrap(r#"<art x="a" y="0" w="50" h="50"/>"#).contains("not a number"));
        assert!(wrap(r#"<art x="0" y="999" w="50" h="50"/>"#).contains("off the card"));
        assert!(wrap(r#"<plate x="0" y="0" w="50" h="50"/>"#).contains("role"));
        assert!(
            wrap(r#"<art x="0" y="0" w="50" h="50"><field name="name"/></art>"#)
                .contains("cannot hold")
        );
    }

    #[test]
    fn a_stat_may_be_as_big_as_its_value() {
        // Both w and h, or neither.
        assert!(
            bad(r#"<template id="a" label="A" frame="modern"><stat x="1" y="2" w="3"><field name="power_toughness"/></stat></template>"#)
                .contains("both w and h")
        );
        // Neither is fine, and the place is still on the card.
        assert!(
            parse(r#"<template id="a" label="A" frame="modern"><stat x="90" y="130"><field name="power_toughness"/></stat></template>"#)
                .is_ok()
        );
        assert!(
            bad(r#"<template id="a" label="A" frame="modern"><stat x="90" y="999"><field name="power_toughness"/></stat></template>"#)
                .contains("off the card")
        );
    }

    #[test]
    fn a_template_may_only_name_fields_it_can_draw() {
        let wrap = |attrs: &'static str, body: &'static str| -> String {
            let xml: &'static str = Box::leak(
                format!(r#"<template id="a" label="A" frame="modern" {attrs}>{body}</template>"#)
                    .into_boxed_str(),
            );
            parse(xml).unwrap_err()
        };
        assert!(
            wrap(
                "",
                r#"<box x="0" y="0" w="9" h="9"><field name="colour"/></box>"#
            )
            .contains("not a field")
        );
        // A template that excludes a feature cannot name its field.
        assert!(
            wrap(
                r#"excludes="cost""#,
                r#"<box x="0" y="0" w="9" h="9"><field name="mana_cost"/></box>"#
            )
            .contains("not a field")
        );
        assert!(
            wrap(
                "",
                r#"<box x="0" y="0" w="9" h="9"><field name="name" as="abilities"/></box>"#
            )
            .contains("not a reader")
        );
    }
}
