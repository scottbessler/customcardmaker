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

#[derive(Debug, Clone, Serialize)]
pub struct SetSummary {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub card_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredCard {
    pub id: Uuid,
    pub position: i64,
    pub card: Card,
    pub updated_at: String,
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
    })
}

const SUMMARY: &str = "SELECT s.id, s.name, s.code, s.updated_at,
        (SELECT COUNT(*) FROM cards c WHERE c.set_id = s.id)
    FROM sets s";

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
        name: Option<String>,
        code: Option<String>,
    ) -> Result<Option<SetSummary>> {
        self.db
            .call(move |c| {
                let id = id.to_string();
                let changed = c.execute(
                    "UPDATE sets SET name = COALESCE(?2, name), code = COALESCE(?3, code), updated_at = ?4 WHERE id = ?1 AND owner_id = ?5",
                    params![id, name, code, now(), owner.to_string()],
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
        Card {
            name: name.into(),
            ..Card::default()
        }
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
            .map(|c| (c.card.name.as_str(), c.position))
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
        let saved = Card {
            name: "Final".into(),
            rules_text: "Flying".into(),
            ..Card::default()
        };
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
                .rename(you, set.id, Some("Stolen".into()), None)
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
        assert_eq!(detail.cards[0].card.name, "Mine");
    }
}
