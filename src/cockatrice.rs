//! Reading a Cockatrice card database.
//!
//! Cockatrice and MTG Nexus export a set as one XML document: a `<sets>` block
//! naming the set, then one `<card>` per card. Only the fields the editor knows
//! are taken; everything else in the file is ignored rather than rejected, so a
//! file from a newer exporter still imports.

use crate::card::{Card, Rarity};
use crate::error::AppError;
use roxmltree::Node;

/// What a Cockatrice file says about the set and the cards in it.
#[derive(Debug)]
pub struct Database {
    /// `<longname>`, when the file has a `<sets>` block.
    pub set_name: Option<String>,
    /// The short code: `<sets><set><name>`, or the first card's `<set>`.
    pub set_code: Option<String>,
    pub cards: Vec<Card>,
}

pub fn parse(xml: &str) -> Result<Database, AppError> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| AppError::bad_request(format!("That is not a Cockatrice XML file: {e}")))?;

    let cards = doc
        .descendants()
        .filter(|node| node.has_tag_name("card"))
        .map(card)
        .collect::<Result<Vec<_>, _>>()?;
    if cards.is_empty() {
        return Err(AppError::bad_request("That file has no cards in it."));
    }

    let (set_name, set_code) = set_info(&doc);
    Ok(Database {
        set_name,
        set_code,
        cards,
    })
}

/// The `<sets>` block is the authoritative name; a file without one (or a
/// hand-made one) still names its set on every card.
fn set_info(doc: &roxmltree::Document<'_>) -> (Option<String>, Option<String>) {
    let declared = doc
        .descendants()
        .find(|node| node.has_tag_name("sets"))
        .and_then(|sets| sets.children().find(|node| node.has_tag_name("set")));

    let name = declared.and_then(|set| text(set, "longname"));
    let code = declared.and_then(|set| text(set, "name")).or_else(|| {
        doc.descendants()
            .find(|node| node.has_tag_name("card"))
            .and_then(card_set_code)
    });
    (name, code)
}

fn card(node: Node<'_, '_>) -> Result<Card, AppError> {
    let prop = node.children().find(|child| child.has_tag_name("prop"));
    let field = |tag| prop.and_then(|prop| text(prop, tag)).unwrap_or_default();

    let card = Card {
        name: text(node, "name").unwrap_or_default(),
        mana_cost: field("manacost"),
        type_line: field("type"),
        rarity: node
            .children()
            .find(|child| child.has_tag_name("set"))
            .and_then(|set| set.attribute("rarity"))
            .map(rarity)
            .unwrap_or_default(),
        rules_text: text(node, "text").unwrap_or_default(),
        power_toughness: field("pt"),
        ..Card::default()
    };

    card.validate()
        .map_err(|e| AppError::bad_request(format!("Card \"{}\": {}", card.name, e.detail())))?;
    Ok(card)
}

/// The text of a direct child, with line endings normalised so a Windows file
/// is still a valid card document.
fn text(node: Node<'_, '_>, tag: &str) -> Option<String> {
    node.children()
        .find(|child| child.has_tag_name(tag))
        .and_then(|child| child.text())
        .map(|value| {
            value
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn card_set_code(node: Node<'_, '_>) -> Option<String> {
    node.children()
        .find(|child| child.has_tag_name("set"))
        .and_then(|set| set.text())
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .map(str::to_string)
}

fn rarity(value: &str) -> Rarity {
    match value.to_ascii_lowercase().as_str() {
        "uncommon" => Rarity::Uncommon,
        "rare" => Rarity::Rare,
        "mythic" => Rarity::Mythic,
        _ => Rarity::Common,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<cockatrice_carddatabase version="4">
  <sets>
    <set>
      <name>RND</name>
      <longname>cloakscott's Random Creations</longname>
    </set>
  </sets>
  <cards>
    <card>
      <name>Vivi&apos;s Friend</name>
      <text>Birds you control have +1+1 and haste
</text>
      <prop>
        <type>Creature — Bird</type>
        <manacost>{2}{W}{W}</manacost>
        <pt>2/2</pt>
      </prop>
      <set rarity="mythic" num="5">RND</set>
    </card>
    <card>
      <name>Exquisite Blood</name>
      <text>Whenever an opponent loses life, you gain that much life.</text>
      <prop>
        <type>Enchantment</type>
        <manacost>{4}{B}</manacost>
      </prop>
      <set rarity="rare">RND</set>
    </card>
  </cards>
</cockatrice_carddatabase>"#;

    #[test]
    fn reads_the_set_and_every_card() {
        let db = parse(FILE).unwrap();
        assert_eq!(db.set_code.as_deref(), Some("RND"));
        assert_eq!(
            db.set_name.as_deref(),
            Some("cloakscott's Random Creations")
        );
        assert_eq!(db.cards.len(), 2);

        let first = &db.cards[0];
        assert_eq!(first.name, "Vivi's Friend");
        assert_eq!(first.mana_cost, "{2}{W}{W}");
        assert_eq!(first.type_line, "Creature — Bird");
        assert_eq!(first.power_toughness, "2/2");
        assert_eq!(first.rules_text, "Birds you control have +1+1 and haste");
        assert_eq!(first.rarity, Rarity::Mythic);

        assert_eq!(db.cards[1].rarity, Rarity::Rare);
        assert_eq!(db.cards[1].power_toughness, "");
    }

    #[test]
    fn a_file_without_a_sets_block_still_names_its_code() {
        let xml = r#"<cockatrice_carddatabase><cards>
            <card><name>A</name><set rarity="common">abc</set></card>
        </cards></cockatrice_carddatabase>"#;
        let db = parse(xml).unwrap();
        assert_eq!(db.set_code.as_deref(), Some("abc"));
        assert_eq!(db.set_name, None);
    }

    #[test]
    fn line_endings_are_normalised() {
        let xml = "<cockatrice_carddatabase><cards><card><name>A</name>\
                   <text>one\r\ntwo\rthree</text><set>ABC</set></card></cards>\
                   </cockatrice_carddatabase>";
        let db = parse(xml).unwrap();
        assert_eq!(db.cards[0].rules_text, "one\ntwo\nthree");
    }

    #[test]
    fn a_file_that_is_not_xml_or_has_no_cards_is_rejected() {
        assert!(parse("not xml").is_err());
        assert!(
            parse("<cockatrice_carddatabase><cards></cards></cockatrice_carddatabase>").is_err()
        );
    }

    #[test]
    fn an_overlong_card_names_itself_in_the_error() {
        let xml = format!(
            "<cockatrice_carddatabase><cards><card><name>A</name>\
             <text>{}</text><set>ABC</set></card></cards></cockatrice_carddatabase>",
            "x".repeat(crate::card::MAX_RULES_TEXT + 1)
        );
        let err = parse(&xml).unwrap_err();
        assert!(err.detail().contains("Card \"A\""));
    }
}
