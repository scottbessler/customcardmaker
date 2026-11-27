//! The card document: everything a designer types into the editor.
//!
//! A card is stored whole, as one JSON value, and read whole. Nothing queries
//! inside it, so the shape can keep changing without a migration. What a card
//! *has* (which fields, and which a finished card must have) is down to the features
//! it has on (`cardtype.rs`); how it is *drawn* is a template's. What the frame
//! looks like (colour, hybrid, land, artifact) is a rendering concern derived
//! from the mana cost and type line in `public/frame.js`, not stored.
//!
//! On the wire a card is one flat object: the keys below, plus the keys its type
//! of its features (`name`, `mana_cost`, ...). See `CARDTYPES.md`.

use crate::cardtype::{self, Kind};
use crate::error::AppError;
use crate::template;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// What a card with no template of its own, in a set with no choice, looks like.
pub const DEFAULT_TEMPLATE: &str = "modern";

/// Frame colours an author may force, as (id, label). A card that names none takes
/// its frame from its mana cost (see `public/frame.js`).
pub const FRAMES: &[(&str, &str)] = &[
    ("w", "White"),
    ("u", "Blue"),
    ("b", "Black"),
    ("r", "Red"),
    ("g", "Green"),
    ("gold", "Gold"),
    ("artifact", "Artifact"),
    ("land", "Land"),
    ("colorless", "Colourless"),
];

pub fn is_frame(id: &str) -> bool {
    FRAMES.iter().any(|(known, _)| *known == id)
}

/// The choices the editor offers, sent to the page beside the limits: the base fields and the
/// features. (The templates go in a script of their own; every page that draws a card needs them.)
#[derive(Debug, Serialize)]
pub struct Choices {
    pub base: &'static [cardtype::Field],
    pub features: &'static [cardtype::Feature],
    pub frames: &'static [(&'static str, &'static str)],
    pub default_template: &'static str,
}

pub fn choices() -> Choices {
    Choices {
        base: cardtype::BASE,
        features: cardtype::FEATURES,
        frames: FRAMES,
        default_template: DEFAULT_TEMPLATE,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Mythic,
}

/// What a card holds beyond its fields; every card has these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    /// The feature groups this card has on (`cost`, `power_toughness`, ...): which fields it
    /// has beyond the base ones. Sorted, with no repeats.
    pub features: Vec<String>,
    /// How it is drawn; empty means the set's default.
    pub template: String,
    /// A frame colour forced by the author; empty means "from the mana cost".
    pub frame: String,
    pub rarity: Rarity,
    /// An upload (`/api/art/<id>`) shown in the art box. Only the owner's own
    /// uploads may be referenced; the route checks, the document just holds it.
    pub art: Option<Uuid>,
    /// The picture is a whole printed card (a scan, or an MTG Nexus render), not
    /// just art: it fills the card, and our plates and text box are laid over it.
    pub art_is_card: bool,
    /// With `art_is_card`: the words printed on the picture are kept, and nothing
    /// of ours is drawn over it.
    pub keep_printed_text: bool,
    /// The other face of a double-faced (transform) card: a card of its own. A back face
    /// has no back of its own.
    pub back: Option<Box<Card>>,
    /// The fields, by key. Every field the card's features allow is present, empty if unset.
    #[serde(flatten)]
    pub fields: BTreeMap<String, String>,
}

/// A card as stored or sent, before it is read as a `Card`: it may be a legacy document with a
/// `layout` and no `features`.
#[derive(Deserialize, Default)]
#[serde(default)]
struct CardDoc {
    features: Option<Vec<String>>,
    template: String,
    frame: String,
    rarity: Rarity,
    art: Option<Uuid>,
    art_is_card: bool,
    keep_printed_text: bool,
    back: Option<Box<Card>>,
    #[serde(flatten)]
    fields: BTreeMap<String, String>,
}

impl<'de> Deserialize<'de> for Card {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut doc = CardDoc::deserialize(deserializer)?;
        let layout = doc.fields.remove("layout").unwrap_or_default();
        // Documents from before features carried a `layout` and every field, used or not.
        // Read them as the card they were (see `CARDTYPES.md`).
        let legacy = doc.features.is_none();
        let features = doc.features.unwrap_or_else(|| {
            let has = |key: &str| doc.fields.get(key).is_some_and(|value| !value.is_empty());
            let mut features = Vec::new();
            if layout != "token" {
                features.push("cost".to_string());
            }
            if has("power_toughness") {
                features.push("power_toughness".to_string());
            }
            if layout == "planeswalker" || has("loyalty") {
                features.push("loyalty".to_string());
            }
            features
        });
        if (layout == "saga" || layout == "token") && doc.template.is_empty() {
            doc.template = layout;
        }
        let mut card = Card {
            features,
            template: doc.template,
            frame: doc.frame,
            rarity: doc.rarity,
            art: doc.art,
            art_is_card: doc.art_is_card,
            keep_printed_text: doc.keep_printed_text,
            back: doc.back,
            fields: doc.fields,
        };
        if legacy {
            // It sent every field whether it used it or not; keep the ones it has a place for.
            let allowed = cardtype::fields_for(&card.features);
            card.fields
                .retain(|key, _| allowed.iter().any(|field| field.key == key));
        }
        card.normalise();
        Ok(card)
    }
}

/// A blank card: the features a new card has, every field empty.
impl Default for Card {
    fn default() -> Self {
        Card::with_features(&cardtype::default_features())
    }
}

impl Card {
    /// A blank card with these features on, every field empty.
    pub fn with_features(features: &[String]) -> Self {
        let mut card = Card {
            features: features.to_vec(),
            template: String::new(),
            frame: String::new(),
            rarity: Rarity::default(),
            art: None,
            art_is_card: false,
            keep_printed_text: false,
            back: None,
            fields: BTreeMap::new(),
        };
        card.normalise();
        card
    }

    /// The value of a field, or empty if the card has none.
    pub fn get(&self, key: &str) -> &str {
        self.fields.get(key).map_or("", String::as_str)
    }

    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        self.fields.insert(key.to_string(), value.into());
    }

    /// This card with a field set; for building one in a line.
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.set(key, value);
        self
    }

    /// This card wearing a template; for building one in a line.
    pub fn wearing(mut self, template: &str) -> Self {
        self.template = template.to_string();
        self
    }

    /// This card with a frame colour forced.
    pub fn framed(mut self, frame: &str) -> Self {
        self.frame = frame.to_string();
        self
    }

    pub fn has(&self, feature: &str) -> bool {
        self.features.iter().any(|id| id == feature)
    }

    /// What the card has, for a template to check its `requires` against: the features it
    /// turned on, plus a second face when it has one.
    pub fn has_all(&self) -> Vec<String> {
        let mut ids = self.features.clone();
        if self.back.is_some() {
            ids.push(template::SECOND_FACE.to_string());
        }
        ids
    }

    /// The required fields that are still empty: what stands between this card and a
    /// finished one. A draft may be missing any of them.
    pub fn missing(&self) -> Vec<&'static str> {
        cardtype::fields_for(&self.features)
            .into_iter()
            .filter(|field| field.required && self.get(field.key).trim().is_empty())
            .map(|field| field.key)
            .collect()
    }

    /// Features sorted and unique, and every field the features allow present: so a form can
    /// read any of them, and an unknown feature is left for `validate` to refuse.
    fn normalise(&mut self) {
        self.features.sort();
        self.features.dedup();
        for field in cardtype::fields_for(&self.features) {
            self.fields.entry(field.key.to_string()).or_default();
        }
    }

    /// The same card with other features on: what the two share is kept, a field with no place
    /// on the new card is dropped, and a template the new card cannot wear goes back to the
    /// set's default.
    pub fn changed_to(&self, features: &[String]) -> Card {
        let mut card = Card {
            features: features.to_vec(),
            fields: BTreeMap::new(),
            ..self.clone()
        };
        card.normalise();
        for field in cardtype::fields_for(&card.features) {
            card.set(field.key, self.get(field.key));
        }
        if template::get(&card.template).is_some_and(|t| !t.works_with(&card.has_all())) {
            card.template.clear();
        }
        card
    }
}

fn within(field: &str, value: &str, max: usize, multiline: bool) -> Result<(), AppError> {
    if value.chars().count() > max {
        return Err(AppError::bad_request(format!(
            "{field} is longer than {max} characters"
        )));
    }
    let allowed = |c: char| !c.is_control() || (multiline && c == '\n');
    if !value.chars().all(allowed) {
        return Err(AppError::bad_request(format!(
            "{field} has a control character in it"
        )));
    }
    Ok(())
}

impl Card {
    /// A draft is allowed to be empty — a blank card is how a new one starts —
    /// so this bounds the document rather than demanding it be finished (`missing`
    /// says what is left to do).
    pub fn validate(&self) -> Result<(), AppError> {
        if let Some(unknown) = self
            .features
            .iter()
            .find(|id| cardtype::feature(id).is_none())
        {
            return Err(AppError::bad_request(format!(
                "\"{unknown}\" is not a card feature."
            )));
        }
        let allowed = cardtype::fields_for(&self.features);
        for (key, value) in &self.fields {
            let field = allowed
                .iter()
                .find(|field| field.key == key)
                .ok_or_else(|| {
                    AppError::bad_request(format!("This card has no \"{key}\" field."))
                })?;
            within(field.label, value, field.max, field.kind == Kind::Text)?;
        }
        if !self.frame.is_empty() && !is_frame(&self.frame) {
            return Err(AppError::bad_request("That is not a frame colour."));
        }
        if !self.template.is_empty() {
            let template = template::get(&self.template)
                .ok_or_else(|| AppError::bad_request("That is not a card template."))?;
            if !template.works_with(&self.has_all()) {
                return Err(AppError::bad_request(format!(
                    "The {} template does not work with this card's fields.",
                    template.label
                )));
            }
        }
        if let Some(back) = &self.back {
            if back.back.is_some() {
                return Err(AppError::bad_request(
                    "A back face cannot have a back face of its own.",
                ));
            }
            back.validate()?;
        }
        Ok(())
    }

    /// Every upload this card shows, on either face.
    pub fn art_ids(&self) -> Vec<Uuid> {
        self.art
            .into_iter()
            .chain(self.back.as_ref().and_then(|back| back.art))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn card(value: serde_json::Value) -> Card {
        serde_json::from_value(value).unwrap()
    }

    fn on(ids: &[&str]) -> Vec<String> {
        ids.iter().map(ToString::to_string).collect()
    }

    fn of(ids: &[&str]) -> Card {
        Card::with_features(&on(ids))
    }

    #[test]
    fn a_blank_card_is_a_valid_draft() {
        let blank = Card::default();
        assert!(blank.validate().is_ok());
        assert_eq!(blank.features, ["cost"]);
        assert_eq!(blank.missing(), ["name", "type_line"]);
    }

    #[test]
    fn missing_fields_default_so_old_documents_keep_loading() {
        let card = card(json!({"name": "Llanowar Elves"}));
        assert_eq!(card.get("name"), "Llanowar Elves");
        assert_eq!(card.rarity, Rarity::Common);
        assert_eq!(card.features, ["cost"]);
    }

    #[test]
    fn a_legacy_document_is_read_as_the_card_it_was() {
        let creature =
            card(json!({"name": "A", "power_toughness": "2/2", "layout": "normal", "loyalty": ""}));
        assert_eq!(creature.features, ["cost", "power_toughness"]);
        assert!(
            !creature.fields.contains_key("loyalty"),
            "it had no loyalty"
        );
        assert!(!creature.fields.contains_key("layout"));
        // Every field was always sent; an empty stat does not make a feature.
        let spell = card(json!({"name": "S", "power_toughness": "", "loyalty": ""}));
        assert_eq!(spell.features, ["cost"]);
        let walker = card(json!({"name": "B", "layout": "planeswalker", "loyalty": "4"}));
        assert_eq!(walker.features, ["cost", "loyalty"]);
        assert_eq!(walker.get("loyalty"), "4");
        let token = card(json!({"layout": "token", "power_toughness": "1/1"}));
        assert_eq!(
            (token.features, token.template.as_str()),
            (on(&["power_toughness"]), "token")
        );
        let saga = card(json!({"name": "C", "layout": "saga"}));
        assert_eq!(
            (saga.features, saga.template.as_str()),
            (on(&["cost"]), "saga")
        );
        // A card that already has features is left as it is.
        let typed = card(json!({"features": ["power_toughness"], "name": "F"}));
        assert_eq!(typed.features, ["power_toughness"]);
        assert!(!typed.fields.contains_key("mana_cost"));
    }

    #[test]
    fn rules_text_may_break_lines_but_a_name_may_not() {
        let mut card = Card::default().with("rules_text", "Flying\nHaste");
        assert!(card.validate().is_ok());
        card.set("name", "Two\nLines");
        assert!(card.validate().is_err());
    }

    #[test]
    fn fields_are_bounded_by_their_field() {
        assert!(
            Card::default()
                .with("rules_text", "x".repeat(1501))
                .validate()
                .is_err()
        );
        assert!(
            of(&["loyalty"])
                .with("loyalty", "12345")
                .validate()
                .is_err()
        );
    }

    #[test]
    fn a_card_may_only_have_the_fields_its_features_allow() {
        assert!(
            Card::default()
                .with("power_toughness", "2/2")
                .validate()
                .is_err()
        );
        assert!(
            of(&["power_toughness"])
                .with("power_toughness", "2/2")
                .validate()
                .is_ok()
        );
        assert!(
            of(&["power_toughness"])
                .with("mana_cost", "{1}")
                .validate()
                .is_err(),
            "no cost feature"
        );
        let mut unknown = Card::default();
        unknown.features.push("neon".into());
        assert!(unknown.validate().is_err());
    }

    #[test]
    fn stats_combine_without_a_type_of_their_own() {
        let both = of(&["cost", "power_toughness", "loyalty"])
            .with("name", "Gideon")
            .with("type_line", "Legendary Planeswalker Creature");
        assert!(both.validate().is_ok());
        assert_eq!(both.missing(), ["power_toughness", "loyalty"]);
        assert!(
            both.with("power_toughness", "4/4")
                .with("loyalty", "3")
                .missing()
                .is_empty()
        );
    }

    #[test]
    fn a_creature_needs_power_toughness_to_be_finished() {
        let creature = of(&["cost", "power_toughness"])
            .with("name", "A")
            .with("type_line", "Creature");
        assert_eq!(creature.missing(), ["power_toughness"]);
        assert!(creature.with("power_toughness", "1/1").missing().is_empty());
    }

    #[test]
    fn a_card_may_only_name_a_frame_and_template_that_exist() {
        assert!(
            Card::default()
                .framed("gold")
                .wearing("modern")
                .validate()
                .is_ok()
        );
        assert!(Card::default().framed("purple").validate().is_err());
        assert!(Card::default().wearing("neon").validate().is_err());
    }

    #[test]
    fn a_template_must_be_able_to_draw_the_cards_features() {
        assert!(Card::default().wearing("saga").validate().is_ok());
        let with_stat = of(&["cost", "power_toughness"]).wearing("saga");
        assert!(
            with_stat.validate().is_err(),
            "a saga has no power/toughness"
        );
        assert!(
            Card::default().wearing("token").validate().is_err(),
            "a token has no mana cost"
        );
        assert!(of(&["power_toughness"]).wearing("token").validate().is_ok());
    }

    #[test]
    fn a_card_may_have_a_back_face_which_is_a_card_of_its_own() {
        let front = Card::default().with("name", "Front");
        let ok = Card {
            back: Some(Box::new(of(&["power_toughness"]).with("name", "Back"))),
            ..front
        };
        assert!(ok.validate().is_ok());

        let deeper = Card {
            back: Some(Box::new(Card::default())),
            ..Card::default()
        };
        let nested = Card {
            back: Some(Box::new(deeper)),
            ..Card::default()
        };
        assert!(nested.validate().is_err());

        let bad_back = Card {
            back: Some(Box::new(Card::default().wearing("neon"))),
            ..Card::default()
        };
        assert!(
            bad_back.validate().is_err(),
            "the back is checked like a front"
        );
    }

    #[test]
    fn changing_features_keeps_what_the_two_share() {
        let creature = of(&["cost", "power_toughness"])
            .with("name", "A")
            .with("rules_text", "Flying")
            .with("power_toughness", "2/2");
        let spell = creature.changed_to(&on(&["cost"]));
        assert_eq!(
            (spell.get("name"), spell.get("rules_text")),
            ("A", "Flying")
        );
        assert!(!spell.fields.contains_key("power_toughness"));
        assert!(spell.validate().is_ok());
        // A template that cannot draw the new features is dropped.
        let saga = Card::default().wearing("saga");
        assert_eq!(saga.changed_to(&on(&["cost", "loyalty"])).template, "");
        let token = of(&["power_toughness"]).wearing("token");
        assert_eq!(
            token
                .changed_to(&on(&["power_toughness", "loyalty"]))
                .template,
            "",
            "a token has no loyalty"
        );
    }

    #[test]
    fn a_card_round_trips_as_one_flat_object() {
        let original = of(&["cost", "power_toughness"])
            .with("name", "A")
            .with("power_toughness", "1/1");
        let value = serde_json::to_value(&original).unwrap();
        assert_eq!(value["name"], "A");
        assert_eq!(value["features"], json!(["cost", "power_toughness"]));
        assert_eq!(serde_json::from_value::<Card>(value).unwrap(), original);
    }
}
