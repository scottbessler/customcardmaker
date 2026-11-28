//! Sets and the cards in them.
//!
//! The store is a thin layer over `Db`: the set and card *rows* carry the
//! columns something orders or joins on, and the card itself stays one JSON
//! document (see `card.rs`).

use crate::{card::Card, db::Db, error::AppError};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use uuid::Uuid;

pub const MAX_SET_NAME: usize = 64;
pub const MIN_CODE: usize = 3;
pub const MAX_CODE: usize = 5;
pub const SYMBOLS: &[(&str, &str)] = &[
    ("circle", "Circle"),
    ("diamond", "Diamond"),
    ("shield", "Shield"),
    ("star", "Star"),
];
/// The most cards one bulk request may name.
pub const MAX_BULK: usize = 500;

#[derive(Debug, Clone, Serialize)]
pub struct SetSummary {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub card_count: i64,
    pub updated_at: String,
    /// Listed in the gallery for anyone to see.
    pub public: bool,
    /// The template a card with none of its own wears.
    pub default_template: String,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredCard {
    pub id: Uuid,
    pub position: i64,
    pub card: Card,
    pub updated_at: String,
}

/// What an owner may change about a set; `None` leaves a field as it is.
#[derive(Debug, Clone, Default)]
pub struct SetChanges {
    pub name: Option<String>,
    pub code: Option<String>,
    pub public: Option<bool>,
    /// Already checked against `card::is_template`.
    pub default_template: Option<String>,
    pub symbol: Option<String>,
}

/// Where a bulk transfer puts its cards.
#[derive(Debug, Clone)]
pub enum Destination {
    /// A set the account already has.
    Existing(Uuid),
    /// A set made for the occasion; the name and code are already cleaned.
    New { name: String, code: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct Transferred {
    /// The destination, as it is after the cards arrived.
    pub set: SetSummary,
    /// How many cards moved or were copied.
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetDetail {
    pub set: SetSummary,
    pub cards: Vec<StoredCard>,
}

/// A set name is a title, so it is trimmed and must say something.
pub fn clean_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("A set needs a name."));
    }
    if name.chars().count() > MAX_SET_NAME || name.chars().any(char::is_control) {
        return Err(AppError::bad_request(format!(
            "A set name is at most {MAX_SET_NAME} characters."
        )));
    }
    Ok(name.to_string())
}

/// The code printed on every card: three to five letters or digits, upper case.
pub fn clean_code(code: &str) -> Result<String, AppError> {
    let code = code.trim().to_ascii_uppercase();
    let length = code.chars().count();
    if !(MIN_CODE..=MAX_CODE).contains(&length) || !code.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return Err(AppError::bad_request(format!(
            "A set code is {MIN_CODE} to {MAX_CODE} letters or digits."
        )));
    }
    Ok(code)
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<SetSummary> {
    let id: String = row.get(0)?;
    Ok(SetSummary {
        id: id.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        code: row.get(2)?,
        updated_at: row.get(3)?,
        card_count: row.get(4)?,
        public: row.get::<_, i64>(5)? != 0,
        default_template: row.get(6)?,
        symbol: row.get(7)?,
    })
}

const SUMMARY: &str = "SELECT s.id, s.name, s.code, s.updated_at,
        (SELECT COUNT(*) FROM cards c WHERE c.set_id = s.id), s.is_public, s.default_template, s.symbol
    FROM sets s";

/// How many sets the gallery's front page shows.
pub const GALLERY_LIMIT: i64 = 60;

/// A public set as the gallery lists it: enough to pick one to open.
#[derive(Debug, Clone, Serialize)]
pub struct GalleryEntry {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub card_count: i64,
    /// The owner's display name. Never the username, which may be private.
    pub by: String,
    pub updated_at: String,
    /// The set's first card, for a thumbnail.
    pub preview: Option<Card>,
    /// What a card with no template of its own looks like in this set.
    pub default_template: String,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GalleryCard {
    pub position: i64,
    pub card: Card,
}

/// A public set, whole, for anyone to read.
#[derive(Debug, Clone, Serialize)]
pub struct GalleryDetail {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub by: String,
    pub updated_at: String,
    pub default_template: String,
    pub symbol: String,
    pub cards: Vec<GalleryCard>,
}

fn stored_card(row: &rusqlite::Row<'_>) -> Result<StoredCard> {
    let id: String = row.get(0)?;
    let doc: String = row.get(2)?;
    Ok(StoredCard {
        id: id.parse().context("card id is not a uuid")?,
        position: row.get(1)?,
        card: serde_json::from_str(&doc).context("card document is not valid")?,
        updated_at: row.get(3)?,
    })
}

/// Puts a set's cards back to 1..=n in their current order.
fn renumber(tx: &rusqlite::Transaction<'_>, set: &str) -> Result<()> {
    let mut stmt = tx.prepare("SELECT id FROM cards WHERE set_id = ?1 ORDER BY position")?;
    let ids: Vec<String> = stmt
        .query_map([set], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for (index, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE cards SET position = ?2 WHERE id = ?1",
            params![id, index as i64 + 1],
        )?;
    }
    Ok(())
}

/// One owned card: its id, set, place in the set and document. `None` if the
/// account has no such card.
struct Owned {
    id: String,
    set: String,
    position: i64,
    doc: String,
}

/// Every named card, in set-then-collector order, or `None` if even one is not
/// this account's. All or nothing: a half-applied bulk edit is worse than none.
fn owned_cards(
    tx: &rusqlite::Transaction<'_>,
    owner: Uuid,
    ids: &[Uuid],
) -> Result<Option<Vec<Owned>>> {
    let mut found = Vec::with_capacity(ids.len());
    for id in ids {
        let row = tx
            .query_row(
                "SELECT c.id, c.set_id, c.position, c.doc FROM cards c
                 JOIN sets s ON s.id = c.set_id WHERE c.id = ?1 AND s.owner_id = ?2",
                [id.to_string(), owner.to_string()],
                |row| {
                    Ok(Owned {
                        id: row.get(0)?,
                        set: row.get(1)?,
                        position: row.get(2)?,
                        doc: row.get(3)?,
                    })
                },
            )
            .optional()?;
        match row {
            Some(card) => found.push(card),
            None => return Ok(None),
        }
    }
    found.sort_by(|a, b| (&a.set, a.position).cmp(&(&b.set, b.position)));
    Ok(Some(found))
}

fn distinct(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

#[derive(Clone)]
pub struct SetStore {
    db: Db,
}

impl SetStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// An account's sets, most recently touched first.
    pub async fn list(&self, owner: Uuid) -> Result<Vec<SetSummary>> {
        self.db
            .call(move |c| {
                let mut stmt = c.prepare(&format!(
                    "{SUMMARY} WHERE s.owner_id = ?1 ORDER BY s.updated_at DESC"
                ))?;
                let rows = stmt.query_map([owner.to_string()], summary)?;
                Ok(rows.collect::<rusqlite::Result<_>>()?)
            })
            .await
    }

    pub async fn create(&self, owner: Uuid, name: String, code: String) -> Result<SetSummary> {
        let id = Uuid::new_v4();
        self.db
            .call(move |c| {
                let at = now();
                c.execute(
                    "INSERT INTO sets (id, name, code, owner_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    params![id.to_string(), name, code, owner.to_string(), at],
                )?;
                Ok(c.query_row(
                    &format!("{SUMMARY} WHERE s.id = ?1"),
                    [id.to_string()],
                    summary,
                )?)
            })
            .await
    }

    /// Creates a set and appends every card in one transaction, so a bulk
    /// import is all or nothing rather than a half-made set.
    pub async fn create_with_cards(
        &self,
        owner: Uuid,
        name: String,
        code: String,
        cards: Vec<Card>,
    ) -> Result<SetSummary> {
        let id = Uuid::new_v4();
        self.db
            .call(move |c| {
                let tx = c.transaction()?;
                let at = now();
                tx.execute(
                    "INSERT INTO sets (id, name, code, owner_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    params![id.to_string(), name, code, owner.to_string(), at],
                )?;
                for (index, card) in cards.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO cards (id, set_id, position, doc, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            Uuid::new_v4().to_string(),
                            id.to_string(),
                            index as i64 + 1,
                            serde_json::to_string(card)?,
                            at
                        ],
                    )?;
                }
                let summary = tx.query_row(
                    &format!("{SUMMARY} WHERE s.id = ?1"),
                    [id.to_string()],
                    summary,
                )?;
                tx.commit()?;
                Ok(summary)
            })
            .await
    }

    pub async fn get(&self, owner: Uuid, id: Uuid) -> Result<Option<SetDetail>> {
        self.db
            .call(move |c| {
                let id = id.to_string();
                let Some(set) = c
                    .query_row(
                        &format!("{SUMMARY} WHERE s.id = ?1 AND s.owner_id = ?2"),
                        [&id, &owner.to_string()],
                        summary,
                    )
                    .optional()?
                else {
                    return Ok(None);
                };
                let mut stmt = c.prepare(
                    "SELECT id, position, doc, updated_at FROM cards WHERE set_id = ?1 ORDER BY position",
                )?;
                let mut rows = stmt.query([&id])?;
                let mut cards = Vec::new();
                while let Some(row) = rows.next()? {
                    cards.push(stored_card(row)?);
                }
                Ok(Some(SetDetail { set, cards }))
            })
            .await
    }

    /// `None` when there is no such set.
    pub async fn rename(
        &self,
        owner: Uuid,
        id: Uuid,
        changes: SetChanges,
    ) -> Result<Option<SetSummary>> {
        self.db
            .call(move |c| {
                let id = id.to_string();
                let changed = c.execute(
                    "UPDATE sets SET name = COALESCE(?2, name), code = COALESCE(?3, code),
                            is_public = COALESCE(?6, is_public),
                            default_template = COALESCE(?7, default_template),
                            symbol = COALESCE(?8, symbol), updated_at = ?4
                     WHERE id = ?1 AND owner_id = ?5",
                    params![
                        id,
                        changes.name,
                        changes.code,
                        now(),
                        owner.to_string(),
                        changes.public,
                        changes.default_template,
                        changes.symbol
                    ],
                )?;
                if changed == 0 {
                    return Ok(None);
                }
                Ok(Some(c.query_row(
                    &format!("{SUMMARY} WHERE s.id = ?1"),
                    [&id],
                    summary,
                )?))
            })
            .await
    }

    pub async fn delete(&self, owner: Uuid, id: Uuid) -> Result<bool> {
        self.db
            .call(move |c| {
                Ok(c.execute(
                    "DELETE FROM sets WHERE id = ?1 AND owner_id = ?2",
                    [id.to_string(), owner.to_string()],
                )? > 0)
            })
            .await
    }

    /// Appends to the end of the set. `None` when there is no such set.
    pub async fn add_card(
        &self,
        owner: Uuid,
        set_id: Uuid,
        card: Card,
    ) -> Result<Option<StoredCard>> {
        let id = Uuid::new_v4();
        self.db
            .call(move |c| {
                let tx = c.transaction()?;
                let set = set_id.to_string();
                let exists = tx
                    .query_row(
                        "SELECT 1 FROM sets WHERE id = ?1 AND owner_id = ?2",
                        [&set, &owner.to_string()],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_some();
                if !exists {
                    return Ok(None);
                }
                let position: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(position), 0) + 1 FROM cards WHERE set_id = ?1",
                    [&set],
                    |row| row.get(0),
                )?;
                let at = now();
                tx.execute(
                    "INSERT INTO cards (id, set_id, position, doc, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id.to_string(), set, position, serde_json::to_string(&card)?, at],
                )?;
                tx.execute("UPDATE sets SET updated_at = ?2 WHERE id = ?1", params![set, at])?;
                tx.commit()?;
                Ok(Some(StoredCard {
                    id,
                    position,
                    card,
                    updated_at: at,
                }))
            })
            .await
    }

    /// Replaces the whole document. `None` when there is no such card.
    pub async fn save_card(&self, owner: Uuid, id: Uuid, card: Card) -> Result<Option<StoredCard>> {
        self.db
            .call(move |c| {
                let tx = c.transaction()?;
                let at = now();
                let changed = tx.execute(
                    "UPDATE cards SET doc = ?2, updated_at = ?3
                     WHERE id = ?1 AND set_id IN (SELECT id FROM sets WHERE owner_id = ?4)",
                    params![
                        id.to_string(),
                        serde_json::to_string(&card)?,
                        at,
                        owner.to_string()
                    ],
                )?;
                if changed == 0 {
                    return Ok(None);
                }
                tx.execute(
                    "UPDATE sets SET updated_at = ?2 WHERE id = (SELECT set_id FROM cards WHERE id = ?1)",
                    params![id.to_string(), at],
                )?;
                let position = tx.query_row(
                    "SELECT position FROM cards WHERE id = ?1",
                    [id.to_string()],
                    |row| row.get(0),
                )?;
                tx.commit()?;
                Ok(Some(StoredCard {
                    id,
                    position,
                    card,
                    updated_at: at,
                }))
            })
            .await
    }

    /// Removes a card and closes the gap, so collector numbers stay 1..=n.
    pub async fn delete_card(&self, owner: Uuid, id: Uuid) -> Result<bool> {
        self.db
            .call(move |c| {
                let tx = c.transaction()?;
                let id = id.to_string();
                let Some((set, position)) = tx
                    .query_row(
                        "SELECT c.set_id, c.position FROM cards c JOIN sets s ON s.id = c.set_id
                         WHERE c.id = ?1 AND s.owner_id = ?2",
                        [&id, &owner.to_string()],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                    )
                    .optional()?
                else {
                    return Ok(false);
                };
                tx.execute("DELETE FROM cards WHERE id = ?1", [&id])?;
                tx.execute(
                    "UPDATE cards SET position = position - 1 WHERE set_id = ?1 AND position > ?2",
                    params![set, position],
                )?;
                tx.execute(
                    "UPDATE sets SET updated_at = ?2 WHERE id = ?1",
                    params![set, now()],
                )?;
                tx.commit()?;
                Ok(true)
            })
            .await
    }

    /// Public sets with something in them, most recently touched first.
    pub async fn gallery(&self, search: String) -> Result<Vec<GalleryEntry>> {
        self.db
            .call(move |c| {
                let mut stmt = c.prepare(
                    "SELECT s.id, s.name, s.code, s.updated_at,
                            (SELECT COUNT(*) FROM cards c WHERE c.set_id = s.id),
                            u.display_name,
                            (SELECT doc FROM cards c WHERE c.set_id = s.id ORDER BY position LIMIT 1),
                            s.default_template, s.symbol
                     FROM sets s JOIN users u ON u.id = s.owner_id
                     WHERE s.is_public = 1
                       AND EXISTS (SELECT 1 FROM cards c WHERE c.set_id = s.id)
                       AND (?1 = '' OR instr(lower(s.name), lower(?1)) > 0
                            OR instr(lower(s.code), lower(?1)) > 0
                            OR instr(lower(u.display_name), lower(?1)) > 0)
                     ORDER BY s.updated_at DESC, s.id DESC LIMIT ?2",
                )?;
                let mut rows = stmt.query(rusqlite::params![search, GALLERY_LIMIT])?;
                let mut entries = Vec::new();
                while let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    let doc: Option<String> = row.get(6)?;
                    entries.push(GalleryEntry {
                        id: id.parse().context("set id is not a uuid")?,
                        name: row.get(1)?,
                        code: row.get(2)?,
                        updated_at: row.get(3)?,
                        card_count: row.get(4)?,
                        by: row.get(5)?,
                        preview: doc
                            .map(|doc| serde_json::from_str(&doc))
                            .transpose()
                            .context("card document is not valid")?,
                        default_template: row.get(7)?,
                        symbol: row.get(8)?,
                    });
                }
                Ok(entries)
            })
            .await
    }

    /// A public set and all its cards. `None` for one that is private or does
    /// not exist, which look the same on purpose.
    pub async fn public_set(&self, id: Uuid) -> Result<Option<GalleryDetail>> {
        self.db
            .call(move |c| {
                let id = id.to_string();
                let head = c
                    .query_row(
                        "SELECT s.name, s.code, s.updated_at, u.display_name, s.default_template, s.symbol
                         FROM sets s JOIN users u ON u.id = s.owner_id
                         WHERE s.id = ?1 AND s.is_public = 1",
                        [&id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, String>(5)?,
                            ))
                        },
                    )
                    .optional()?;
                let Some((name, code, updated_at, by, default_template, symbol)) = head else {
                    return Ok(None);
                };
                let mut stmt = c.prepare(
                    "SELECT position, doc FROM cards WHERE set_id = ?1 ORDER BY position",
                )?;
                let mut rows = stmt.query([&id])?;
                let mut cards = Vec::new();
                while let Some(row) = rows.next()? {
                    let doc: String = row.get(1)?;
                    cards.push(GalleryCard {
                        position: row.get(0)?,
                        card: serde_json::from_str(&doc).context("card document is not valid")?,
                    });
                }
                Ok(Some(GalleryDetail {
                    id: id.parse().context("set id is not a uuid")?,
                    name,
                    code,
                    by,
                    updated_at,
                    default_template,
                    symbol,
                    cards,
                }))
            })
            .await
    }

    /// Deletes several cards at once and closes the gaps in every set they
    /// were in. `None`, with nothing changed, if any of them is not this
    /// account's or does not exist.
    pub async fn delete_cards(&self, owner: Uuid, ids: Vec<Uuid>) -> Result<Option<usize>> {
        let ids = distinct(ids);
        self.db
            .call(move |c| {
                let tx = c.transaction()?;
                let Some(cards) = owned_cards(&tx, owner, &ids)? else {
                    return Ok(None);
                };
                let mut touched = std::collections::BTreeSet::new();
                for card in &cards {
                    tx.execute("DELETE FROM cards WHERE id = ?1", [&card.id])?;
                    touched.insert(card.set.clone());
                }
                let at = now();
                for set in &touched {
                    renumber(&tx, set)?;
                    tx.execute(
                        "UPDATE sets SET updated_at = ?2 WHERE id = ?1",
                        params![set, at],
                    )?;
                }
                tx.commit()?;
                Ok(Some(cards.len()))
            })
            .await
    }

    /// Moves cards to the end of another set, or copies them there, in the
    /// order they had. A move closes the gaps it leaves behind; a copy changes
    /// nothing where the originals are. `None`, with nothing changed, if a card
    /// or the destination is not this account's.
    ///
    /// Moving a card to the set it is already in does nothing and is not
    /// counted; copying it makes a duplicate at the end.
    pub async fn transfer_cards(
        &self,
        owner: Uuid,
        ids: Vec<Uuid>,
        destination: Destination,
        copy: bool,
    ) -> Result<Option<Transferred>> {
        let ids = distinct(ids);
        self.db
            .call(move |c| {
                let tx = c.transaction()?;
                let Some(cards) = owned_cards(&tx, owner, &ids)? else {
                    return Ok(None);
                };
                let at = now();
                let target = match destination {
                    Destination::Existing(id) => {
                        let id = id.to_string();
                        let there = tx
                            .query_row(
                                "SELECT 1 FROM sets WHERE id = ?1 AND owner_id = ?2",
                                [&id, &owner.to_string()],
                                |_| Ok(()),
                            )
                            .optional()?;
                        if there.is_none() {
                            return Ok(None);
                        }
                        id
                    }
                    Destination::New { name, code } => {
                        let id = Uuid::new_v4().to_string();
                        tx.execute(
                            "INSERT INTO sets (id, name, code, owner_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                            params![id, name, code, owner.to_string(), at],
                        )?;
                        id
                    }
                };
                let mut next: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(position), 0) FROM cards WHERE set_id = ?1",
                    [&target],
                    |row| row.get(0),
                )?;
                let mut sources = std::collections::BTreeSet::new();
                let mut count = 0;
                for card in &cards {
                    if copy {
                        next += 1;
                        tx.execute(
                            "INSERT INTO cards (id, set_id, position, doc, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                            params![Uuid::new_v4().to_string(), target, next, card.doc, at],
                        )?;
                    } else if card.set == target {
                        continue;
                    } else {
                        next += 1;
                        tx.execute(
                            "UPDATE cards SET set_id = ?2, position = ?3, updated_at = ?4 WHERE id = ?1",
                            params![card.id, target, next, at],
                        )?;
                        sources.insert(card.set.clone());
                    }
                    count += 1;
                }
                for set in &sources {
                    renumber(&tx, set)?;
                    tx.execute("UPDATE sets SET updated_at = ?2 WHERE id = ?1", params![set, at])?;
                }
                tx.execute("UPDATE sets SET updated_at = ?2 WHERE id = ?1", params![target, at])?;
                let set = tx.query_row(
                    &format!("{SUMMARY} WHERE s.id = ?1"),
                    [&target],
                    summary,
                )?;
                tx.commit()?;
                Ok(Some(Transferred { set, count }))
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::tests::tempdir, users::User, users::UserStore};

    /// A store plus an account to own its sets (`owner_id` is a foreign key).
    async fn store() -> (tempdir::Dir, SetStore, Uuid) {
        let dir = tempdir::Dir::new();
        let db = Db::open(dir.path()).await.unwrap();
        let owner = account(&db, "ada").await;
        (dir, SetStore::new(db), owner)
    }

    async fn account(db: &Db, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        let user = User {
            id,
            username: name.into(),
            display_name: name.into(),
            credentials: vec![],
            created_at: Utc::now(),
        };
        assert!(UserStore::new(db.clone()).insert(user).await.unwrap());
        id
    }

    fn named(name: &str) -> Card {
        Card::default().with("name", name)
    }

    #[test]
    fn codes_are_upper_cased_and_bounded() {
        assert_eq!(clean_code(" mh3 ").unwrap(), "MH3");
        assert!(clean_code("ab").is_err());
        assert!(clean_code("toolong").is_err());
        assert!(clean_code("a-b").is_err());
    }

    #[test]
    fn names_are_trimmed_and_required() {
        assert_eq!(clean_name("  Dusk  ").unwrap(), "Dusk");
        assert!(clean_name("   ").is_err());
    }

    #[tokio::test]
    async fn cards_take_collector_numbers_in_order_and_close_gaps() {
        let (_dir, store, me) = store().await;
        let set = store.create(me, "Dusk".into(), "DSK".into()).await.unwrap();
        let a = store
            .add_card(me, set.id, named("A"))
            .await
            .unwrap()
            .unwrap();
        let b = store
            .add_card(me, set.id, named("B"))
            .await
            .unwrap()
            .unwrap();
        let c = store
            .add_card(me, set.id, named("C"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!([a.position, b.position, c.position], [1, 2, 3]);

        assert!(store.delete_card(me, a.id).await.unwrap());
        let detail = store.get(me, set.id).await.unwrap().unwrap();
        let order: Vec<_> = detail
            .cards
            .iter()
            .map(|c| (c.card.get("name"), c.position))
            .collect();
        assert_eq!(order, [("B", 1), ("C", 2)]);
        assert_eq!(detail.set.card_count, 2);
    }

    #[tokio::test]
    async fn saving_replaces_the_document_whole() {
        let (_dir, store, me) = store().await;
        let set = store.create(me, "Dusk".into(), "DSK".into()).await.unwrap();
        let card = store
            .add_card(me, set.id, named("Draft"))
            .await
            .unwrap()
            .unwrap();
        let saved = Card::default()
            .with("name", "Final")
            .with("rules_text", "Flying");
        store
            .save_card(me, card.id, saved.clone())
            .await
            .unwrap()
            .unwrap();
        let detail = store.get(me, set.id).await.unwrap().unwrap();
        assert_eq!(detail.cards[0].card, saved);
        assert!(
            store
                .save_card(me, Uuid::new_v4(), saved)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn deleting_a_set_takes_its_cards_with_it() {
        let (_dir, store, me) = store().await;
        let set = store.create(me, "Dusk".into(), "DSK".into()).await.unwrap();
        let card = store
            .add_card(me, set.id, named("A"))
            .await
            .unwrap()
            .unwrap();
        assert!(store.delete(me, set.id).await.unwrap());
        assert!(store.get(me, set.id).await.unwrap().is_none());
        assert!(!store.delete_card(me, card.id).await.unwrap());
        assert!(
            store
                .add_card(me, set.id, named("B"))
                .await
                .unwrap()
                .is_none()
        );
    }

    /// Someone else's set is not forbidden, it is simply not there: every
    /// lookup that would reveal it answers as if it did not exist.
    #[tokio::test]
    async fn another_account_cannot_see_or_touch_a_set() {
        let (_dir, store, me) = store().await;
        let you = account(&store.db, "bob").await;
        let set = store.create(me, "Dusk".into(), "DSK".into()).await.unwrap();
        let card = store
            .add_card(me, set.id, named("Mine"))
            .await
            .unwrap()
            .unwrap();

        assert!(store.list(you).await.unwrap().is_empty());
        assert!(store.get(you, set.id).await.unwrap().is_none());
        assert!(
            store
                .rename(
                    you,
                    set.id,
                    SetChanges {
                        name: Some("Stolen".into()),
                        ..SetChanges::default()
                    }
                )
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .add_card(you, set.id, named("Planted"))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .save_card(you, card.id, named("Edited"))
                .await
                .unwrap()
                .is_none()
        );
        assert!(!store.delete_card(you, card.id).await.unwrap());
        assert!(!store.delete(you, set.id).await.unwrap());

        let detail = store.get(me, set.id).await.unwrap().unwrap();
        assert_eq!(detail.set.name, "Dusk");
        assert_eq!(detail.cards.len(), 1);
        assert_eq!(detail.cards[0].card.get("name"), "Mine");
    }

    /// A set of cards named by `names`, in order, and their ids.
    async fn filled(
        store: &SetStore,
        owner: Uuid,
        name: &str,
        names: &[&str],
    ) -> (SetSummary, Vec<Uuid>) {
        let set = store
            .create(owner, name.into(), "TST".into())
            .await
            .unwrap();
        let mut ids = Vec::new();
        for card in names {
            ids.push(
                store
                    .add_card(owner, set.id, named(card))
                    .await
                    .unwrap()
                    .unwrap()
                    .id,
            );
        }
        (set, ids)
    }

    async fn order(store: &SetStore, owner: Uuid, set: Uuid) -> Vec<(String, i64)> {
        let detail = store.get(owner, set).await.unwrap().unwrap();
        detail
            .cards
            .into_iter()
            .map(|c| (c.card.get("name").to_string(), c.position))
            .collect()
    }

    fn pairs(list: &[(&str, i64)]) -> Vec<(String, i64)> {
        list.iter().map(|(n, p)| (n.to_string(), *p)).collect()
    }

    #[tokio::test]
    async fn bulk_delete_closes_the_gaps() {
        let (_dir, store, me) = store().await;
        let (set, ids) = filled(&store, me, "Dusk", &["A", "B", "C", "D", "E"]).await;
        let deleted = store.delete_cards(me, vec![ids[1], ids[3]]).await.unwrap();
        assert_eq!(deleted, Some(2));
        assert_eq!(
            order(&store, me, set.id).await,
            pairs(&[("A", 1), ("C", 2), ("E", 3)])
        );
        assert_eq!(
            store.get(me, set.id).await.unwrap().unwrap().set.card_count,
            3
        );
    }

    #[tokio::test]
    async fn bulk_delete_is_all_or_nothing() {
        let (_dir, store, me) = store().await;
        let (set, ids) = filled(&store, me, "Dusk", &["A", "B", "C"]).await;
        // One id that is not a card at all: nothing is deleted, not even the real ones.
        assert_eq!(
            store
                .delete_cards(me, vec![ids[0], Uuid::new_v4()])
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            order(&store, me, set.id).await,
            pairs(&[("A", 1), ("B", 2), ("C", 3)])
        );
        // The same card named twice is one card.
        assert_eq!(
            store.delete_cards(me, vec![ids[0], ids[0]]).await.unwrap(),
            Some(1)
        );
    }

    #[tokio::test]
    async fn moving_appends_in_order_and_closes_the_gap_behind() {
        let (_dir, store, me) = store().await;
        let (from, ids) = filled(&store, me, "From", &["A", "B", "C", "D"]).await;
        let (to, _) = filled(&store, me, "To", &["X"]).await;
        // Named out of order: they still arrive in the order they had.
        let done = store
            .transfer_cards(
                me,
                vec![ids[2], ids[0]],
                Destination::Existing(to.id),
                false,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(done.count, 2);
        assert_eq!(done.set.id, to.id);
        assert_eq!(done.set.card_count, 3);
        assert_eq!(
            order(&store, me, to.id).await,
            pairs(&[("X", 1), ("A", 2), ("C", 3)])
        );
        assert_eq!(
            order(&store, me, from.id).await,
            pairs(&[("B", 1), ("D", 2)])
        );
    }

    #[tokio::test]
    async fn copying_leaves_the_originals_and_makes_new_cards() {
        let (_dir, store, me) = store().await;
        let (from, ids) = filled(&store, me, "From", &["A", "B"]).await;
        let (to, _) = filled(&store, me, "To", &[]).await;
        let done = store
            .transfer_cards(me, ids.clone(), Destination::Existing(to.id), true)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(done.count, 2);
        assert_eq!(
            order(&store, me, from.id).await,
            pairs(&[("A", 1), ("B", 2)])
        );
        assert_eq!(order(&store, me, to.id).await, pairs(&[("A", 1), ("B", 2)]));
        let copied = store.get(me, to.id).await.unwrap().unwrap().cards;
        assert!(
            copied.iter().all(|c| !ids.contains(&c.id)),
            "a copy is a new card, not the same one"
        );
    }

    #[tokio::test]
    async fn a_new_destination_is_made_in_the_same_step() {
        let (_dir, store, me) = store().await;
        let (from, ids) = filled(&store, me, "From", &["A", "B", "C"]).await;
        let dest = Destination::New {
            name: "Sideboard".into(),
            code: "SID".into(),
        };
        let done = store
            .transfer_cards(me, vec![ids[1]], dest, false)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (done.set.name.as_str(), done.set.code.as_str(), done.count),
            ("Sideboard", "SID", 1)
        );
        assert_eq!(order(&store, me, done.set.id).await, pairs(&[("B", 1)]));
        assert_eq!(
            order(&store, me, from.id).await,
            pairs(&[("A", 1), ("C", 2)])
        );
        assert!(
            store
                .list(me)
                .await
                .unwrap()
                .iter()
                .any(|s| s.id == done.set.id)
        );
    }

    #[tokio::test]
    async fn moving_a_card_to_the_set_it_is_in_does_nothing() {
        let (_dir, store, me) = store().await;
        let (set, ids) = filled(&store, me, "Dusk", &["A", "B"]).await;
        let moved = store
            .transfer_cards(me, vec![ids[0]], Destination::Existing(set.id), false)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(moved.count, 0);
        assert_eq!(
            order(&store, me, set.id).await,
            pairs(&[("A", 1), ("B", 2)])
        );
        // Copying it there is a duplicate at the end.
        let copied = store
            .transfer_cards(me, vec![ids[0]], Destination::Existing(set.id), true)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(copied.count, 1);
        assert_eq!(
            order(&store, me, set.id).await,
            pairs(&[("A", 1), ("B", 2), ("A", 3)])
        );
    }

    #[tokio::test]
    async fn a_failed_transfer_changes_nothing_not_even_the_new_set() {
        let (_dir, store, me) = store().await;
        let (set, ids) = filled(&store, me, "Dusk", &["A"]).await;
        let before = store.list(me).await.unwrap().len();
        let dest = Destination::New {
            name: "Ghost".into(),
            code: "GST".into(),
        };
        assert!(
            store
                .transfer_cards(me, vec![ids[0], Uuid::new_v4()], dest, false)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store.list(me).await.unwrap().len(),
            before,
            "the new set must not survive a failed transfer"
        );
        assert_eq!(order(&store, me, set.id).await, pairs(&[("A", 1)]));
    }

    #[tokio::test]
    async fn another_account_can_do_nothing_in_bulk() {
        let (_dir, store, me) = store().await;
        let you = account(&store.db, "bob").await;
        let (mine, my_ids) = filled(&store, me, "Mine", &["A", "B"]).await;
        let (yours, your_ids) = filled(&store, you, "Yours", &["Z"]).await;

        assert_eq!(store.delete_cards(you, my_ids.clone()).await.unwrap(), None);
        // Neither moving my cards out, nor putting yours into my set.
        assert!(
            store
                .transfer_cards(you, my_ids.clone(), Destination::Existing(yours.id), false)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .transfer_cards(you, your_ids.clone(), Destination::Existing(mine.id), false)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .transfer_cards(you, my_ids, Destination::Existing(yours.id), true)
                .await
                .unwrap()
                .is_none()
        );

        assert_eq!(
            order(&store, me, mine.id).await,
            pairs(&[("A", 1), ("B", 2)])
        );
        assert_eq!(order(&store, you, yours.id).await, pairs(&[("Z", 1)]));
    }
}
