//! Reading a Cockatrice card database.
//!
//! Cockatrice and MTG Nexus export a set as one XML document: a `<sets>` block
//! naming the set, then one `<card>` per card. Only the fields the editor knows
//! are taken; everything else in the file is ignored rather than rejected, so a
//! file from a newer exporter still imports.

use crate::card::{Card, Rarity};
use crate::cardtype;
use crate::error::AppError;
use roxmltree::Node;

/// One card, plus the picture the file points at. The URL is kept beside the
/// card rather than in it: `Card` holds an uploaded image's id, and the URL
/// only becomes one if the file is downloaded.
#[derive(Debug)]
pub struct ImportedCard {
    pub card: Card,
    /// The `picurl` on the card's `<set>`, if the file gives one.
    pub image_url: Option<String>,
    /// A layout Nexus draws in a way we have no geometry for, so our text cannot be
    /// laid over its picture: the picture is kept as printed.
    pub printed_layout: bool,
    /// The file marks this as the back face of a double-faced card.
    pub is_back: bool,
    /// The other face's name, from `<related attach="transform">`.
    pub related: Option<String>,
    /// Once the two faces are joined: the back's picture, and whether it is kept as
    /// printed. (`card.back` holds the rest of it.)
    pub back_image_url: Option<String>,
    pub back_printed_layout: bool,
}

/// What a Cockatrice file says about the set and the cards in it.
#[derive(Debug)]
pub struct Database {
    /// `<longname>`, when the file has a `<sets>` block.
    pub set_name: Option<String>,
    /// The short code: `<sets><set><name>`, or the first card's `<set>`.
    pub set_code: Option<String>,
    pub cards: Vec<ImportedCard>,
}

pub fn parse(xml: &str) -> Result<Database, AppError> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| AppError::bad_request(format!("That is not a Cockatrice XML file: {e}")))?;

    let cards = doc
        .descendants()
        .filter(|node| node.has_tag_name("card"))
        .map(card)
        .collect::<Result<Vec<_>, _>>()?;
    let cards = join_faces(cards);
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

fn card(node: Node<'_, '_>) -> Result<ImportedCard, AppError> {
    let prop = node.children().find(|child| child.has_tag_name("prop"));
    let field = |tag| prop.and_then(|prop| text(prop, tag)).unwrap_or_default();
    let set = node.children().find(|child| child.has_tag_name("set"));

    let layout = prop
        .and_then(|prop| text(prop, "layout"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    let type_line = field("type");
    let lowered = type_line.to_ascii_lowercase();
    // What the card is, by its type first (a transform's back may be a saga), then by
    // the layout the file names.
    let ours = if lowered.contains("token") {
        "token"
    } else if lowered.contains("planeswalker") || layout == "planeswalker" {
        "planeswalker"
    } else if lowered.contains("saga") || layout == "saga" {
        "saga"
    } else {
        ""
    };
    // A layout Nexus draws in a way we have no geometry for (split, flip, adventure...):
    // its picture is kept as printed.
    let printed_layout =
        ours.is_empty() && !matches!(layout.as_str(), "" | "normal" | "transform" | "modal_dfc");

    // Our type for it: what fields it has. A saga has a spell's, drawn by a template.
    let cardtype = match ours {
        "token" => "token",
        "planeswalker" => "planeswalker",
        _ if lowered.contains("creature") => "creature",
        _ => "spell",
    };
    let mut card = Card::of(cardtype);
    card.set("name", text(node, "name").unwrap_or_default());
    card.set("type_line", type_line);
    card.set(
        "rules_text",
        paragraphs(text(node, "text").unwrap_or_default()),
    );
    for (key, tag) in [
        ("mana_cost", "manacost"),
        ("power_toughness", "pt"),
        ("loyalty", "loyalty"),
    ] {
        // Only what the type has a field for; the rest of the file's keys are ignored.
        if cardtype::get(cardtype).is_some_and(|ty| ty.field(key).is_some()) {
            card.set(key, field(tag));
        }
    }
    if ours == "saga" {
        card.template = "saga".to_string();
    }
    card.rarity = set
        .and_then(|set| set.attribute("rarity"))
        .map(rarity)
        .unwrap_or_default();

    card.validate().map_err(|e| {
        AppError::bad_request(format!("Card \"{}\": {}", card.get("name"), e.detail()))
    })?;
    Ok(ImportedCard {
        card,
        image_url: set
            .and_then(|set| set.attribute("picurl"))
            .and_then(web_url),
        printed_layout,
        is_back: prop
            .and_then(|prop| text(prop, "side"))
            .is_some_and(|side| side.eq_ignore_ascii_case("back")),
        related: node
            .children()
            .find(|child| child.has_tag_name("related"))
            .filter(|related| {
                related
                    .attribute("attach")
                    .is_none_or(|attach| attach.eq_ignore_ascii_case("transform"))
            })
            .and_then(|related| related.text())
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty()),
        back_image_url: None,
        back_printed_layout: false,
    })
}

/// Nexus writes a double-faced card as two `<card>`s, the back marked `side=back`, each
/// naming the other in `<related>`. They become one card with a back face. A back whose
/// front is not in the file (or does not name it in return) stays a card of its own.
fn join_faces(cards: Vec<ImportedCard>) -> Vec<ImportedCard> {
    let mut joined: Vec<ImportedCard> = Vec::new();
    let mut backs: Vec<ImportedCard> = Vec::new();
    for item in cards {
        if item.is_back && item.related.is_some() {
            backs.push(item);
        } else {
            joined.push(item);
        }
    }
    for back in backs {
        let front = joined.iter_mut().find(|front| {
            front.card.back.is_none()
                && front.related.as_deref() == Some(back.card.get("name"))
                && back.related.as_deref() == Some(front.card.get("name"))
        });
        match front {
            Some(front) => {
                front.back_image_url = back.image_url;
                front.back_printed_layout = back.printed_layout;
                front.card.back = Some(Box::new(back.card));
            }
            None => joined.push(back),
        }
    }
    joined
}

/// Only an `http`/`https` URL is worth fetching; anything else (a `file:` or a
/// bare path) is ignored rather than handed to the HTTP client.
fn web_url(value: &str) -> Option<String> {
    let value = value.trim();
    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))?;
    (!rest.is_empty() && !rest.starts_with('/')).then(|| value.to_string())
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

/// Nexus separates paragraphs with a blank line, and a card's rules text is one
/// paragraph per line, so a blank line would only draw as a gap; and a line may
/// carry trailing spaces.
fn paragraphs(text: String) -> String {
    text.lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
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
        // Nexus's "special" is a bonus-sheet rarity we have no pip for; mythic is the
        // nearest, being the one reserved for a set's showpieces.
        "mythic" | "special" => Rarity::Mythic,
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
      <set rarity="mythic" num="5" picurl="https://example.test/vivi.jpg">RND</set>
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

        let first = &db.cards[0].card;
        assert_eq!(first.get("name"), "Vivi's Friend");
        assert_eq!(first.get("mana_cost"), "{2}{W}{W}");
        assert_eq!(first.get("type_line"), "Creature — Bird");
        assert_eq!(first.get("power_toughness"), "2/2");
        assert_eq!(
            first.get("rules_text"),
            "Birds you control have +1+1 and haste"
        );
        assert_eq!(first.rarity, Rarity::Mythic);
        assert_eq!(
            db.cards[0].image_url.as_deref(),
            Some("https://example.test/vivi.jpg")
        );

        assert_eq!(db.cards[1].card.rarity, Rarity::Rare);
        assert_eq!(db.cards[1].card.get("power_toughness"), "");
        assert_eq!(db.cards[1].image_url, None);
    }

    #[test]
    fn only_web_urls_are_kept_as_pictures() {
        assert_eq!(
            web_url("https://x.test/a.jpg").as_deref(),
            Some("https://x.test/a.jpg")
        );
        assert_eq!(
            web_url("http://x.test/a.jpg").as_deref(),
            Some("http://x.test/a.jpg")
        );
        assert_eq!(web_url("file:///etc/passwd"), None);
        assert_eq!(web_url("/local/a.jpg"), None);
        assert_eq!(web_url("https://"), None);
        assert_eq!(web_url(""), None);
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
        assert_eq!(db.cards[0].card.get("rules_text"), "one\ntwo\nthree");
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
            "x".repeat(1501)
        );
        let err = parse(&xml).unwrap_err();
        assert!(err.detail().contains("Card \"A\""));
    }

    /// Shaped like a real MTG Nexus export: `layout` and `side` in `prop`, blank lines
    /// between paragraphs, a `special` rarity, loyalty, and a token with no cost.
    const NEXUS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><cockatrice_carddatabase version="4"><sets><set><name>ALA</name><longname>animals</longname></set></sets><cards>
<card><name>Bandage</name><text>Enchant Creature

Enchanted Creature Gains +0+2  

You can only Activate this Ability: Sacrifice Bandage
</text><prop><layout>normal</layout><side>front</side><type>Enchantment — Aura</type><manacost>{1}{W}</manacost></prop><set rarity="special" picurl="https://example.test/a.jpg" num="1">ALA</set></card>
<card><name>Dog</name><text></text><prop><layout>normal</layout><side>front</side><type>Token Creature — Dog</type><pt>1/1</pt></prop><set rarity="mythic" picurl="https://example.test/b.jpg" num="2">ALA</set></card>
<card><name>Munchkin</name><text>+1: Add {B}{B}</text><prop><layout>planeswalker</layout><side>front</side><type>Legendary Planeswalker — Bird</type><manacost>{2}{W}{W}{W}</manacost><loyalty>4</loyalty></prop><set rarity="mythic" picurl="https://example.test/c.jpg" num="3">ALA</set></card>
<card><name>Jorgan</name><text>I: Draw.</text><prop><layout>transform</layout><side>back</side><type>Enchantment — Saga</type></prop><set rarity="uncommon" picurl="https://example.test/d.jpg" num="4">ALA</set></card>
<card><name>Cut</name><text>Deal 2.</text><prop><layout>split</layout><side>front</side><type>Instant</type><manacost>{R}</manacost></prop><set rarity="common" picurl="https://example.test/e.jpg" num="5">ALA</set></card>
</cards></cockatrice_carddatabase>"#;

    #[test]
    fn a_real_nexus_export_reads_cleanly() {
        let db = parse(NEXUS).unwrap();
        let cards: Vec<_> = db.cards.iter().collect();
        // Blank lines and trailing spaces are not paragraphs.
        assert_eq!(
            cards[0].card.get("rules_text"),
            "Enchant Creature\nEnchanted Creature Gains +0+2\nYou can only Activate this Ability: Sacrifice Bandage"
        );
        // "special" has no pip of its own; it is shown as mythic.
        assert_eq!(cards[0].card.rarity, Rarity::Mythic);
        // Token and planeswalker are types of our own now; a saga is a spell drawn by the
        // saga template.
        let types: Vec<_> = cards.iter().map(|c| c.card.cardtype.as_str()).collect();
        assert_eq!(types, ["spell", "token", "planeswalker", "spell", "spell"]);
        assert_eq!(cards[3].card.template, "saga");
        assert_eq!(cards[2].card.get("loyalty"), "4");
        // One we have no geometry for keeps its picture as printed.
        let printed: Vec<_> = cards.iter().map(|c| c.printed_layout).collect();
        assert_eq!(printed, [false, false, false, false, true]);
    }

    #[test]
    fn the_two_faces_of_a_transform_card_become_one_card() {
        let xml = r#"<cockatrice_carddatabase><cards>
<card><name>Day</name><text>Flip me.</text><prop><layout>transform</layout><side>front</side><type>Creature — Human</type><manacost>{1}{W}</manacost><pt>1/1</pt></prop><set rarity="rare" picurl="https://example.test/day-a.jpg" num="7">ABC</set><related attach="transform">Night</related></card>
<card><name>Solo</name><text>Alone.</text><prop><layout>normal</layout><side>front</side><type>Instant</type></prop><set rarity="common">ABC</set></card>
<card><name>Night</name><text>Flipped.</text><prop><layout>transform</layout><side>back</side><type>Creature — Werewolf</type><pt>2/2</pt></prop><set rarity="rare" picurl="https://example.test/day-b.jpg" num="7">ABC</set><related attach="transform">Day</related></card>
<card><name>Orphan</name><text>No front.</text><prop><layout>transform</layout><side>back</side><type>Instant</type></prop><set rarity="rare">ABC</set><related attach="transform">Missing</related></card>
</cards></cockatrice_carddatabase>"#;
        let db = parse(xml).unwrap();
        let names: Vec<_> = db.cards.iter().map(|c| c.card.get("name")).collect();
        // The back leaves the list; a back with no front in the file stays, at the end.
        assert_eq!(names, ["Day", "Solo", "Orphan"]);
        let day = &db.cards[0];
        let back = day.card.back.as_ref().expect("Day has a back face");
        assert_eq!(
            (back.get("name"), back.get("power_toughness")),
            ("Night", "2/2")
        );
        assert_eq!(
            day.image_url.as_deref(),
            Some("https://example.test/day-a.jpg")
        );
        assert_eq!(
            day.back_image_url.as_deref(),
            Some("https://example.test/day-b.jpg")
        );
        assert!(db.cards[1].card.back.is_none());
    }
}
