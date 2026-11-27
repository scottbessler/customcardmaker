//! What a card *has*, apart from how it is drawn (see `CARDTYPES.md`).
//!
//! Every card has the same base fields. On top of them it turns on **features**: groups
//! of fields a card may or may not have (a mana cost, power/toughness, loyalty, a defense
//! counter). A card's "type", in the sense of which fields it has and which a finished
//! card must have, is just the features it has on, so a creature planeswalker is a card
//! with both stats and needs no type of its own. Each field has a key, a kind (how the
//! editor asks for it and what it may hold), a bound on its length, and whether a
//! *finished* card must have it (a draft need not).
//!
//! A template (`template.rs`) names the features it needs and the ones it cannot draw;
//! nothing here knows about looks.

use serde::Serialize;

/// How a field is asked for and what it may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// One line of text.
    Line,
    /// Braced mana symbols, `{2}{U}`.
    Cost,
    /// The type line.
    Typeline,
    /// Paragraphs, one per line; symbols allowed.
    Text,
    /// A short value: `2/3`, `4`, `X`.
    Stat,
}

#[derive(Debug, Serialize)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    /// A finished card must have it. A draft may be empty.
    pub required: bool,
    /// The most characters it may hold.
    pub max: usize,
}

const NAME: Field = Field {
    key: "name",
    label: "Name",
    kind: Kind::Line,
    required: true,
    max: 80,
};
const MANA_COST: Field = Field {
    key: "mana_cost",
    label: "Mana cost",
    kind: Kind::Cost,
    required: false,
    max: 40,
};
const TYPE_LINE: Field = Field {
    key: "type_line",
    label: "Type line",
    kind: Kind::Typeline,
    required: true,
    max: 80,
};
const RULES_TEXT: Field = Field {
    key: "rules_text",
    label: "Rules text",
    kind: Kind::Text,
    required: false,
    max: 1500,
};
const FLAVOR_TEXT: Field = Field {
    key: "flavor_text",
    label: "Flavor text",
    kind: Kind::Text,
    required: false,
    max: 600,
};
const POWER_TOUGHNESS: Field = Field {
    key: "power_toughness",
    label: "Power / toughness",
    kind: Kind::Stat,
    required: true,
    max: 12,
};
const LOYALTY: Field = Field {
    key: "loyalty",
    label: "Loyalty",
    kind: Kind::Stat,
    required: true,
    max: 4,
};
const DEFENSE: Field = Field {
    key: "defense",
    label: "Defense",
    kind: Kind::Stat,
    required: true,
    max: 4,
};
const ARTIST: Field = Field {
    key: "artist",
    label: "Artist",
    kind: Kind::Line,
    required: false,
    max: 80,
};

/// Every card has these.
pub const BASE: &[Field] = &[NAME, TYPE_LINE, RULES_TEXT, FLAVOR_TEXT, ARTIST];

/// A group of fields a card may have.
#[derive(Debug, Serialize)]
pub struct Feature {
    pub id: &'static str,
    pub label: &'static str,
    /// A new card has it: most cards have a mana cost, so a token is the exception.
    pub default: bool,
    pub fields: &'static [Field],
}

pub const FEATURES: &[Feature] = &[
    Feature {
        id: "cost",
        label: "Mana cost",
        default: true,
        fields: &[MANA_COST],
    },
    Feature {
        id: "power_toughness",
        label: "Power / toughness",
        default: false,
        fields: &[POWER_TOUGHNESS],
    },
    Feature {
        id: "loyalty",
        label: "Loyalty",
        default: false,
        fields: &[LOYALTY],
    },
    Feature {
        id: "defense",
        label: "Defense",
        default: false,
        fields: &[DEFENSE],
    },
];

pub fn feature(id: &str) -> Option<&'static Feature> {
    FEATURES.iter().find(|feature| feature.id == id)
}

/// The features a new card has on.
pub fn default_features() -> Vec<String> {
    FEATURES
        .iter()
        .filter(|feature| feature.default)
        .map(|feature| feature.id.to_string())
        .collect()
}

/// The fields a card with these features on has, in order: the base first.
pub fn fields_for(features: &[String]) -> Vec<&'static Field> {
    BASE.iter()
        .chain(
            FEATURES
                .iter()
                .filter(|feature| features.iter().any(|id| id == feature.id))
                .flat_map(|feature| feature.fields.iter()),
        )
        .collect()
}

/// The longest each field may be, by key, over every field there is.
pub fn limits() -> std::collections::BTreeMap<&'static str, usize> {
    BASE.iter()
        .chain(FEATURES.iter().flat_map(|feature| feature.fields.iter()))
        .map(|field| (field.key, field.max))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(ids: &[&str]) -> Vec<String> {
        ids.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_card_has_the_base_and_the_features_it_turns_on() {
        let keys = |features: &[&str]| -> Vec<&str> {
            fields_for(&on(features)).iter().map(|f| f.key).collect()
        };
        assert_eq!(
            keys(&[]),
            ["name", "type_line", "rules_text", "flavor_text", "artist"]
        );
        assert!(keys(&["cost"]).contains(&"mana_cost"));
        // Stats combine: a creature planeswalker needs no type of its own.
        let both = keys(&["power_toughness", "loyalty"]);
        assert!(both.contains(&"power_toughness") && both.contains(&"loyalty"));
        assert_eq!(default_features(), ["cost"]);
    }
}
