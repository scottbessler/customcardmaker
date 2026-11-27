//! The card document: everything a designer types into the editor.
//!
//! A card is stored whole, as one JSON value, and read whole. Nothing queries
//! inside it, so the shape can keep changing without a migration. What a card
//! *has* (which fields, and which a finished card must have) is its card type's
//! business (`cardtype.rs`); how it is *drawn* is a template's. What the frame
//! looks like (colour, hybrid, land, artifact) is a rendering concern derived
//! from the mana cost and type line in `public/frame.js`, not stored.
//!
//! On the wire a card is one flat object: the keys below, plus the keys its type
//! lists (`name`, `mana_cost`, ...). See `CARDTYPES.md`.

use crate::cardtype::{self, CardType, Kind};
use crate::error::AppError;
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

/// The choices the editor offers, sent to the page beside the limits: the card types
/// (with their fields) and the templates (with the types each works with).
#[derive(Debug, Serialize)]
pub struct Choices {
    pub cardtypes: &'static [&'static CardType],
    pub templates: &'static [cardtype::Template],
    pub frames: &'static [(&'static str, &'static str)],
    pub default_template: &'static str,
    pub default_cardtype: &'static str,
}

pub const CHOICES: Choices = Choices {
    cardtypes: cardtype::TYPES,
    templates: cardtype::TEMPLATES,
    frames: FRAMES,
    default_template: DEFAULT_TEMPLATE,
    default_cardtype: cardtype::DEFAULT,
};

/// The longest each field may be, by key, over every type: the form reads it so it never
/// keeps a second, hand-copied idea of a bound.
pub fn limits() -> BTreeMap<&'static str, usize> {
    cardtype::TYPES
        .iter()
        .flat_map(|ty| ty.fields.iter().map(|field| (field.key, field.max)))
        .collect()
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

/// What a card holds beyond its type's fields; every card has these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    /// A card type's id (`spell`, `creature`, ...): which fields this card has.
    pub cardtype: String,
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
    /// The other face of a double-faced (transform) card, for a type that allows one:
    /// a card of its own. A back face has no back of its own.
    pub back: Option<Box<Card>>,
    /// The type's fields, by key. Every field the type lists is present, empty if unset.
    #[serde(flatten)]
    pub fields: BTreeMap<String, String>,
}

/// A card as stored or sent, before it is read as a `Card`: it may be a legacy document
/// with a `layout` and no `cardtype`.
#[derive(Deserialize, Default)]
#[serde(default)]
struct CardDoc {
    cardtype: String,
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
        let doc = CardDoc::deserialize(deserializer)?;
        let mut card = Card {
            cardtype: doc.cardtype,
            template: doc.template,
            frame: doc.frame,
            rarity: doc.rarity,
            art: doc.art,
            art_is_card: doc.art_is_card,
            keep_printed_text: doc.keep_printed_text,
            back: doc.back,
            fields: doc.fields,
        };
        card.read_legacy();
        card.fill();
        Ok(card)
    }
}

/// A blank spell: what a new card is.
impl Default for Card {
    fn default() -> Self {
        Card::of(cardtype::DEFAULT)
    }
}

impl Card {
    /// A blank card of a type, every field empty.
    pub fn of(cardtype: &str) -> Self {
        let mut card = Card {
            cardtype: cardtype.to_string(),
            template: String::new(),
            frame: String::new(),
            rarity: Rarity::default(),
            art: None,
            art_is_card: false,
            keep_printed_text: false,
            back: None,
            fields: BTreeMap::new(),
        };
        card.fill();
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

    /// The type's required fields that are still empty: what stands between this card
    /// and a finished one. A draft may be missing any of them.
    pub fn missing(&self) -> Vec<&'static str> {
        let Some(ty) = cardtype::get(&self.cardtype) else {
            return Vec::new();
        };
        ty.fields
            .iter()
            .filter(|field| field.required && self.get(field.key).trim().is_empty())
            .map(|field| field.key)
            .collect()
    }

    /// Documents written before card types existed carry a `layout` and every field, used
    /// or not. Read them as the type they were (see `CARDTYPES.md`) and keep only that
    /// type's fields.
    fn read_legacy(&mut self) {
        let layout = self.fields.remove("layout");
        if !self.cardtype.is_empty() {
            return;
        }
        self.cardtype = match layout.as_deref() {
            Some("token") => "token",
            Some("planeswalker") => "planeswalker",
            _ if !self.get("power_toughness").is_empty() => "creature",
            _ => cardtype::DEFAULT,
        }
        .to_string();
        if layout.as_deref() == Some("saga") && self.template.is_empty() {
            self.template = "saga".to_string();
        }
        if let Some(ty) = cardtype::get(&self.cardtype) {
            self.fields.retain(|key, _| ty.field(key).is_some());
        }
        if let Some(back) = &mut self.back {
            // A back face is a card too, with its own legacy `layout`.
            if back.cardtype.is_empty() {
                back.read_legacy();
            }
        }
    }

    /// Every field its type lists is present, so a form can read any of them.
    fn fill(&mut self) {
        if let Some(ty) = cardtype::get(&self.cardtype) {
            for field in ty.fields {
                self.fields.entry(field.key.to_string()).or_default();
            }
        }
    }

    /// The same card as another type, keeping what the two have in common and dropping what
    /// the new type has no field for.
    pub fn as_type(&self, cardtype: &str) -> Card {
        let mut card = Card {
            cardtype: cardtype.to_string(),
            fields: BTreeMap::new(),
            ..self.clone()
        };
        if let Some(ty) = cardtype::get(cardtype) {
            for field in ty.fields {
                card.set(field.key, self.get(field.key));
            }
            if !ty.back {
                card.back = None;
            }
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
        let ty = cardtype::get(&self.cardtype)
            .ok_or_else(|| AppError::bad_request("That is not a card type."))?;
        for (key, value) in &self.fields {
            let field = ty.field(key).ok_or_else(|| {
                AppError::bad_request(format!("A {} has no \"{key}\" field.", ty.label))
            })?;
            within(field.label, value, field.max, field.kind == Kind::Text)?;
        }
        if !self.frame.is_empty() && !is_frame(&self.frame) {
            return Err(AppError::bad_request("That is not a frame colour."));
        }
        if !self.template.is_empty() {
            let template = cardtype::template(&self.template)
                .ok_or_else(|| AppError::bad_request("That is not a card template."))?;
            if !template.works_with(ty.id) {
                return Err(AppError::bad_request(format!(
                    "The {} template does not work with a {}.",
                    template.label, ty.label
                )));
            }
        }
        if let Some(back) = &self.back {
            if !ty.back {
                return Err(AppError::bad_request(format!(
                    "A {} cannot have a back face.",
                    ty.label
                )));
            }
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

    #[test]
    fn a_blank_card_is_a_valid_draft() {
        let blank = Card::of("spell");
        assert!(blank.validate().is_ok());
        assert_eq!(blank.missing(), ["name", "type_line"]);
    }

    #[test]
    fn missing_fields_default_so_old_documents_keep_loading() {
        let card = card(json!({"name": "Llanowar Elves"}));
        assert_eq!(card.get("name"), "Llanowar Elves");
        assert_eq!(card.rarity, Rarity::Common);
        assert_eq!(card.cardtype, "spell");
    }

    #[test]
    fn a_legacy_document_is_read_as_the_type_it_was() {
        let creature =
            card(json!({"name": "A", "power_toughness": "2/2", "layout": "normal", "loyalty": ""}));
        assert_eq!(creature.cardtype, "creature");
        assert!(
            !creature.fields.contains_key("loyalty"),
            "a creature has no loyalty"
        );
        assert!(!creature.fields.contains_key("layout"));
        let walker = card(json!({"name": "B", "layout": "planeswalker", "loyalty": "4"}));
        assert_eq!(
            (walker.cardtype.as_str(), walker.get("loyalty")),
            ("planeswalker", "4")
        );
        let token = card(json!({"layout": "token", "power_toughness": "1/1"}));
        assert_eq!(token.cardtype, "token");
        let saga = card(json!({"name": "C", "layout": "saga"}));
        assert_eq!(
            (saga.cardtype.as_str(), saga.template.as_str()),
            ("spell", "saga")
        );
        let flipped = card(json!({"name": "D", "back": {"name": "E", "layout": "saga"}}));
        assert_eq!(flipped.back.unwrap().template, "saga");
        // A document that already has a type is left as it is.
        let typed = card(json!({"cardtype": "creature", "name": "F"}));
        assert_eq!(typed.cardtype, "creature");
        assert_eq!(typed.get("power_toughness"), "");
    }

    #[test]
    fn rules_text_may_break_lines_but_a_name_may_not() {
        let mut card = Card::of("spell").with("rules_text", "Flying\nHaste");
        assert!(card.validate().is_ok());
        card.set("name", "Two\nLines");
        assert!(card.validate().is_err());
    }

    #[test]
    fn fields_are_bounded_by_their_type() {
        let long = Card::of("spell").with("rules_text", "x".repeat(1501));
        assert!(long.validate().is_err());
        assert!(
            Card::of("planeswalker")
                .with("loyalty", "12345")
                .validate()
                .is_err()
        );
    }

    #[test]
    fn a_card_may_only_have_the_fields_its_type_lists() {
        // A spell has no power/toughness; a creature does.
        assert!(
            Card::of("spell")
                .with("power_toughness", "2/2")
                .validate()
                .is_err()
        );
        assert!(
            Card::of("creature")
                .with("power_toughness", "2/2")
                .validate()
                .is_ok()
        );
        assert!(
            Card::of("token")
                .with("mana_cost", "{1}")
                .validate()
                .is_err()
        );
        assert!(Card::of("neon").validate().is_err());
    }

    #[test]
    fn a_creature_needs_power_toughness_to_be_finished() {
        let creature = Card::of("creature")
            .with("name", "A")
            .with("type_line", "Creature");
        assert_eq!(creature.missing(), ["power_toughness"]);
        assert!(creature.with("power_toughness", "1/1").missing().is_empty());
    }

    #[test]
    fn a_card_may_only_name_a_frame_and_template_that_exist() {
        let mut ok = Card::of("spell");
        ok.frame = "gold".into();
        ok.template = "classic".into();
        assert!(ok.validate().is_ok());
        let mut bad_frame = Card::of("spell");
        bad_frame.frame = "purple".into();
        assert!(bad_frame.validate().is_err());
        let mut bad_template = Card::of("spell");
        bad_template.template = "neon".into();
        assert!(bad_template.validate().is_err());
    }

    #[test]
    fn a_template_must_work_with_the_cards_type() {
        let mut saga = Card::of("spell");
        saga.template = "saga".into();
        assert!(saga.validate().is_ok());
        let mut creature = Card::of("creature");
        creature.template = "saga".into();
        assert!(
            creature.validate().is_err(),
            "a saga template is for spells"
        );
    }

    #[test]
    fn a_card_may_have_a_back_face_when_its_type_allows_one() {
        let mut ok = Card::of("spell").with("name", "Front");
        ok.back = Some(Box::new(Card::of("creature").with("name", "Back")));
        assert!(ok.validate().is_ok());

        let mut nested = Card::of("spell");
        let mut back = Card::of("spell");
        back.back = Some(Box::new(Card::of("spell")));
        nested.back = Some(Box::new(back));
        assert!(nested.validate().is_err());

        let mut bad_back = Card::of("spell");
        let mut inner = Card::of("spell");
        inner.template = "neon".into();
        bad_back.back = Some(Box::new(inner));
        assert!(
            bad_back.validate().is_err(),
            "the back is checked like a front"
        );

        let mut token = Card::of("token");
        token.back = Some(Box::new(Card::of("spell")));
        assert!(token.validate().is_err(), "a token has no back");
    }

    #[test]
    fn changing_type_keeps_what_the_two_share() {
        let creature = Card::of("creature")
            .with("name", "A")
            .with("rules_text", "Flying")
            .with("power_toughness", "2/2");
        let spell = creature.as_type("spell");
        assert_eq!(spell.cardtype, "spell");
        assert_eq!(
            (spell.get("name"), spell.get("rules_text")),
            ("A", "Flying")
        );
        assert!(!spell.fields.contains_key("power_toughness"));
        assert!(spell.validate().is_ok());
        let token = Card::of("spell").with("mana_cost", "{1}").as_type("token");
        assert!(!token.fields.contains_key("mana_cost"));
    }

    #[test]
    fn a_card_round_trips_as_one_flat_object() {
        let original = Card::of("creature")
            .with("name", "A")
            .with("power_toughness", "1/1");
        let value = serde_json::to_value(&original).unwrap();
        assert_eq!(value["name"], "A");
        assert_eq!(value["cardtype"], "creature");
        assert_eq!(serde_json::from_value::<Card>(value).unwrap(), original);
    }
}
