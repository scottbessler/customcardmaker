//! The card document: everything a designer types into the editor.
//!
//! A card is stored whole, as one JSON value, and read whole. Nothing queries
//! inside it, so the shape can keep changing without a migration — every field
//! is `#[serde(default)]` for exactly that reason. What the frame *looks* like
//! (colour, hybrid, land, artifact) is a rendering concern derived from the
//! mana cost and type line in `public/frame.js`, not stored.

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_NAME: usize = 80;
pub const MAX_MANA_COST: usize = 40;
pub const MAX_TYPE_LINE: usize = 80;
pub const MAX_RULES_TEXT: usize = 1500;
pub const MAX_FLAVOR_TEXT: usize = 600;
pub const MAX_POWER_TOUGHNESS: usize = 12;
pub const MAX_ARTIST: usize = 80;

/// The looks a card can wear, as (id, label). One list: the picker in the editor
/// and the check on what a card may name both read it, so they cannot disagree.
pub const TEMPLATES: &[(&str, &str)] = &[
    ("modern", "Modern"),
    ("classic", "Classic"),
    ("borderless", "Borderless"),
    ("full-art", "Full art"),
];

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

pub fn is_template(id: &str) -> bool {
    TEMPLATES.iter().any(|(known, _)| *known == id)
}

pub fn is_frame(id: &str) -> bool {
    FRAMES.iter().any(|(known, _)| *known == id)
}

/// The choices the editor offers, sent to the page alongside the limits.
#[derive(Debug, Serialize)]
pub struct Choices {
    pub templates: &'static [(&'static str, &'static str)],
    pub frames: &'static [(&'static str, &'static str)],
    pub default_template: &'static str,
}

pub const CHOICES: Choices = Choices {
    templates: TEMPLATES,
    frames: FRAMES,
    default_template: DEFAULT_TEMPLATE,
};

/// The bounds the editor enforces, sent to the page so the form never has a
/// second, hand-copied idea of how long a field may be.
#[derive(Debug, Serialize)]
pub struct Limits {
    pub name: usize,
    pub mana_cost: usize,
    pub type_line: usize,
    pub rules_text: usize,
    pub flavor_text: usize,
    pub power_toughness: usize,
    pub artist: usize,
}

pub const LIMITS: Limits = Limits {
    name: MAX_NAME,
    mana_cost: MAX_MANA_COST,
    type_line: MAX_TYPE_LINE,
    rules_text: MAX_RULES_TEXT,
    flavor_text: MAX_FLAVOR_TEXT,
    power_toughness: MAX_POWER_TOUGHNESS,
    artist: MAX_ARTIST,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Mythic,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Card {
    pub name: String,
    /// Braced symbols, as printed on a card: `{2}{U}{U}`.
    pub mana_cost: String,
    pub type_line: String,
    pub rarity: Rarity,
    /// One paragraph per line. `{T}` and friends are symbols.
    pub rules_text: String,
    pub flavor_text: String,
    /// `2/3` for a creature, empty otherwise.
    pub power_toughness: String,
    pub artist: String,
    /// An upload (`/api/art/<id>`) shown in the art box. Only the owner's own
    /// uploads may be referenced; the route checks, the document just holds it.
    pub art: Option<Uuid>,
    /// A frame colour forced by the author; empty means "from the mana cost".
    pub frame: String,
    /// The look of the card; empty means "the set's default".
    pub template: String,
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
    /// so this bounds the document rather than demanding it be finished.
    pub fn validate(&self) -> Result<(), AppError> {
        within("name", &self.name, MAX_NAME, false)?;
        within("mana cost", &self.mana_cost, MAX_MANA_COST, false)?;
        within("type line", &self.type_line, MAX_TYPE_LINE, false)?;
        within("rules text", &self.rules_text, MAX_RULES_TEXT, true)?;
        within("flavor text", &self.flavor_text, MAX_FLAVOR_TEXT, true)?;
        within(
            "power/toughness",
            &self.power_toughness,
            MAX_POWER_TOUGHNESS,
            false,
        )?;
        within("artist", &self.artist, MAX_ARTIST, false)?;
        if !self.frame.is_empty() && !is_frame(&self.frame) {
            return Err(AppError::bad_request("That is not a frame colour."));
        }
        if !self.template.is_empty() && !is_template(&self.template) {
            return Err(AppError::bad_request("That is not a card template."));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_card_is_a_valid_draft() {
        assert!(Card::default().validate().is_ok());
    }

    #[test]
    fn missing_fields_default_so_old_documents_keep_loading() {
        let card: Card = serde_json::from_str(r#"{"name":"Llanowar Elves"}"#).unwrap();
        assert_eq!(card.name, "Llanowar Elves");
        assert_eq!(card.rarity, Rarity::Common);
    }

    #[test]
    fn rules_text_may_break_lines_but_a_name_may_not() {
        let mut card = Card {
            rules_text: "Flying\nHaste".into(),
            ..Card::default()
        };
        assert!(card.validate().is_ok());
        card.name = "Two\nLines".into();
        assert!(card.validate().is_err());
    }

    #[test]
    fn fields_are_bounded() {
        let card = Card {
            rules_text: "x".repeat(MAX_RULES_TEXT + 1),
            ..Card::default()
        };
        assert!(card.validate().is_err());
    }

    #[test]
    fn a_card_may_only_name_a_frame_and_template_that_exist() {
        let ok = Card {
            frame: "gold".into(),
            template: "classic".into(),
            ..Card::default()
        };
        assert!(ok.validate().is_ok());
        let auto = Card::default();
        assert!(
            auto.frame.is_empty() && auto.template.is_empty(),
            "the default is: from the cost, and the set's choice"
        );
        assert!(
            Card {
                frame: "purple".into(),
                ..Card::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Card {
                template: "../../x".into(),
                ..Card::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn the_default_template_is_one_of_the_templates() {
        assert!(is_template(DEFAULT_TEMPLATE));
        let ids: Vec<_> = TEMPLATES.iter().map(|(id, _)| id).collect();
        assert_eq!(
            ids.len(),
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            "ids are unique"
        );
    }
}
