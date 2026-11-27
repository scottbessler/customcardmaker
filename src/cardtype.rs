//! Card types: what a card *has*, apart from how it is drawn (see `CARDTYPES.md`).
//!
//! A type is a list of fields. Each has a key, a kind (how the editor asks for it and
//! what it may hold), a bound on its length, and whether a *finished* card must have it
//! (a draft need not). A type also says whether a card of it may have a back face.
//! Nothing here knows about looks; a template names the types it works with, never the
//! other way round.

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

pub const NAME: Field = Field {
    key: "name",
    label: "Name",
    kind: Kind::Line,
    required: true,
    max: 80,
};
pub const MANA_COST: Field = Field {
    key: "mana_cost",
    label: "Mana cost",
    kind: Kind::Cost,
    required: false,
    max: 40,
};
pub const TYPE_LINE: Field = Field {
    key: "type_line",
    label: "Type line",
    kind: Kind::Typeline,
    required: true,
    max: 80,
};
pub const RULES_TEXT: Field = Field {
    key: "rules_text",
    label: "Rules text",
    kind: Kind::Text,
    required: false,
    max: 1500,
};
pub const FLAVOR_TEXT: Field = Field {
    key: "flavor_text",
    label: "Flavor text",
    kind: Kind::Text,
    required: false,
    max: 600,
};
pub const POWER_TOUGHNESS: Field = Field {
    key: "power_toughness",
    label: "Power / toughness",
    kind: Kind::Stat,
    required: true,
    max: 12,
};
pub const LOYALTY: Field = Field {
    key: "loyalty",
    label: "Loyalty",
    kind: Kind::Stat,
    required: true,
    max: 4,
};
pub const ARTIST: Field = Field {
    key: "artist",
    label: "Artist",
    kind: Kind::Line,
    required: false,
    max: 80,
};

#[derive(Debug, Serialize)]
pub struct CardType {
    pub id: &'static str,
    pub label: &'static str,
    /// A card of this type may carry a back face (a transform card).
    pub back: bool,
    pub fields: &'static [Field],
}

pub const SPELL: CardType = CardType {
    id: "spell",
    label: "Spell",
    back: true,
    fields: &[NAME, MANA_COST, TYPE_LINE, RULES_TEXT, FLAVOR_TEXT, ARTIST],
};

pub const CREATURE: CardType = CardType {
    id: "creature",
    label: "Creature",
    back: true,
    fields: &[
        NAME,
        MANA_COST,
        TYPE_LINE,
        RULES_TEXT,
        FLAVOR_TEXT,
        POWER_TOUGHNESS,
        ARTIST,
    ],
};

pub const PLANESWALKER: CardType = CardType {
    id: "planeswalker",
    label: "Planeswalker",
    back: true,
    fields: &[
        NAME,
        MANA_COST,
        TYPE_LINE,
        RULES_TEXT,
        FLAVOR_TEXT,
        LOYALTY,
        ARTIST,
    ],
};

/// A token has no mana cost, and no back.
pub const TOKEN: CardType = CardType {
    id: "token",
    label: "Token",
    back: false,
    fields: &[
        NAME,
        TYPE_LINE,
        RULES_TEXT,
        FLAVOR_TEXT,
        POWER_TOUGHNESS,
        ARTIST,
    ],
};

pub const TYPES: &[&CardType] = &[&SPELL, &CREATURE, &PLANESWALKER, &TOKEN];

/// What a new card is, and what a legacy card with nothing to say otherwise is.
pub const DEFAULT: &str = "spell";

pub fn get(id: &str) -> Option<&'static CardType> {
    TYPES.iter().copied().find(|ty| ty.id == id)
}

impl CardType {
    pub fn field(&self, key: &str) -> Option<&'static Field> {
        self.fields.iter().find(|field| field.key == key)
    }
}

/// A look a card may wear, and the types it works with. The set of templates is data
/// the editor and the checks both read; the XML format that will replace this list is in
/// `CARDTYPES.md`.
#[derive(Debug, Serialize)]
pub struct Template {
    pub id: &'static str,
    pub label: &'static str,
    pub cardtypes: &'static [&'static str],
}

const ALL: &[&str] = &["spell", "creature", "planeswalker", "token"];

pub const TEMPLATES: &[Template] = &[
    Template {
        id: "modern",
        label: "Modern",
        cardtypes: ALL,
    },
    Template {
        id: "classic",
        label: "Classic",
        cardtypes: ALL,
    },
    Template {
        id: "borderless",
        label: "Borderless",
        cardtypes: ALL,
    },
    Template {
        id: "full-art",
        label: "Full art",
        cardtypes: ALL,
    },
    // Chapters down the left, art on the right: it reads a spell's rules text as chapters.
    Template {
        id: "saga",
        label: "Saga",
        cardtypes: &["spell"],
    },
];

pub fn template(id: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|template| template.id == id)
}

impl Template {
    pub fn works_with(&self, cardtype: &str) -> bool {
        self.cardtypes.contains(&cardtype)
    }
}

/// A set's default must be a look every type can wear.
pub fn is_set_default(id: &str) -> bool {
    template(id).is_some_and(|template| TYPES.iter().all(|ty| template.works_with(ty.id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_names_real_types() {
        for template in TEMPLATES {
            assert!(
                !template.cardtypes.is_empty(),
                "{} works with nothing",
                template.id
            );
            for ty in template.cardtypes {
                assert!(
                    get(ty).is_some(),
                    "{} names a type that is not there: {ty}",
                    template.id
                );
            }
        }
    }

    #[test]
    fn a_set_default_is_a_look_every_type_can_wear() {
        assert!(is_set_default("modern"));
        assert!(!is_set_default("saga"), "a saga is only for spells");
        assert!(!is_set_default("neon"));
    }

    #[test]
    fn types_differ_by_their_fields() {
        assert!(SPELL.field("power_toughness").is_none());
        assert!(
            CREATURE
                .field("power_toughness")
                .is_some_and(|f| f.required)
        );
        assert!(PLANESWALKER.field("loyalty").is_some());
        assert!(TOKEN.field("mana_cost").is_none() && !TOKEN.back);
        assert!(get("spell").is_some() && get("saga").is_none());
    }
}
