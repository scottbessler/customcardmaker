//! The card document: everything a designer types into the editor.
//!
//! A card is stored whole, as one JSON value, and read whole. Nothing queries
//! inside it, so the shape can keep changing without a migration — every field
//! is `#[serde(default)]` for exactly that reason. What the frame *looks* like
//! (colour, hybrid, land, artifact) is a rendering concern derived from the
//! mana cost and type line in `public/frame.js`, not stored.

use crate::error::AppError;
use serde::{Deserialize, Serialize};

pub const MAX_NAME: usize = 80;
pub const MAX_MANA_COST: usize = 40;
pub const MAX_TYPE_LINE: usize = 80;
pub const MAX_RULES_TEXT: usize = 1500;
pub const MAX_FLAVOR_TEXT: usize = 600;
pub const MAX_POWER_TOUGHNESS: usize = 12;
pub const MAX_ARTIST: usize = 80;

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
}
