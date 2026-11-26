//! Accounts. A user is a name and a list of passkeys; there are no passwords.

use crate::{db::Db, error::AppError};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;
use webauthn_rs::prelude::Passkey;

pub const MIN_USERNAME: usize = 2;
pub const MAX_USERNAME: usize = 32;
pub const MAX_DISPLAY_NAME: usize = 48;

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub credentials: Vec<Passkey>,
    pub created_at: DateTime<Utc>,
}

/// Usernames are unique case-insensitively: `Ada` and `ada` are one person.
pub fn username_key(name: &str) -> String {
    name.trim().to_lowercase()
}

pub fn clean_username(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    let length = name.chars().count();
    if !(MIN_USERNAME..=MAX_USERNAME).contains(&length) || name.chars().any(char::is_control) {
        return Err(AppError::bad_request(format!(
            "Pick a username of {MIN_USERNAME} to {MAX_USERNAME} characters."
        )));
    }
    Ok(name.to_string())
}

/// Falls back to the username when none was given.
pub fn clean_display_name(display: Option<&str>, username: &str) -> Result<String, AppError> {
    let display = display
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .unwrap_or(username);
    if display.chars().count() > MAX_DISPLAY_NAME || display.chars().any(char::is_control) {
        return Err(AppError::bad_request(format!(
            "A display name is at most {MAX_DISPLAY_NAME} characters."
        )));
    }
    Ok(display.to_string())
}

fn user(row: &rusqlite::Row<'_>) -> Result<User> {
    let id: String = row.get(0)?;
    let credentials: String = row.get(3)?;
    let created: String = row.get(4)?;
    Ok(User {
        id: id.parse().context("user id is not a uuid")?,
        username: row.get(1)?,
        display_name: row.get(2)?,
        credentials: serde_json::from_str(&credentials).context("credentials are not valid")?,
        created_at: created.parse().context("created_at is not a timestamp")?,
    })
}

const COLUMNS: &str = "SELECT id, username, display_name, credentials, created_at FROM users";

#[derive(Clone)]
pub struct UserStore {
    db: Db,
}

impl UserStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<User>> {
        self.db
            .call(move |c| {
                let mut stmt = c.prepare(&format!("{COLUMNS} WHERE id = ?1"))?;
                let mut rows = stmt.query([id.to_string()])?;
                rows.next()?.map(user).transpose()
            })
            .await
    }

    pub async fn get_by_username(&self, name: &str) -> Result<Option<User>> {
        let key = username_key(name);
        self.db
            .call(move |c| {
                let mut stmt = c.prepare(&format!("{COLUMNS} WHERE username_key = ?1"))?;
                let mut rows = stmt.query([key])?;
                rows.next()?.map(user).transpose()
            })
            .await
    }

    pub async fn username_taken(&self, name: &str) -> Result<bool> {
        let key = username_key(name);
        self.db
            .call(move |c| {
                Ok(
                    c.query_row("SELECT 1 FROM users WHERE username_key = ?1", [key], |_| {
                        Ok(())
                    })
                    .optional()?
                    .is_some(),
                )
            })
            .await
    }

    /// `false` when the name was taken between the check and the write: the
    /// UNIQUE index is the arbiter, not the earlier read.
    pub async fn insert(&self, new: User) -> Result<bool> {
        self.db
            .call(move |c| {
                let changed = c.execute(
                    "INSERT OR IGNORE INTO users (id, username, username_key, display_name, credentials, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        new.id.to_string(),
                        new.username,
                        username_key(&new.username),
                        new.display_name,
                        serde_json::to_string(&new.credentials)?,
                        new.created_at.to_rfc3339(),
                    ],
                )?;
                Ok(changed > 0)
            })
            .await
    }

    pub async fn update_credentials(&self, id: Uuid, credentials: Vec<Passkey>) -> Result<()> {
        self.db
            .call(move |c| {
                c.execute(
                    "UPDATE users SET credentials = ?2 WHERE id = ?1",
                    params![id.to_string(), serde_json::to_string(&credentials)?],
                )?;
                Ok(())
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::tempdir;

    pub fn new_user(name: &str) -> User {
        User {
            id: Uuid::new_v4(),
            username: name.into(),
            display_name: name.into(),
            credentials: vec![],
            created_at: Utc::now(),
        }
    }

    #[test]
    fn usernames_are_trimmed_and_bounded() {
        assert_eq!(clean_username("  Ada ").unwrap(), "Ada");
        assert!(clean_username("a").is_err());
        assert!(clean_username(&"x".repeat(MAX_USERNAME + 1)).is_err());
        assert!(clean_username("tab\there").is_err());
    }

    #[test]
    fn display_name_defaults_to_the_username() {
        assert_eq!(clean_display_name(None, "ada").unwrap(), "ada");
        assert_eq!(clean_display_name(Some("  "), "ada").unwrap(), "ada");
        assert_eq!(clean_display_name(Some("Ada L."), "ada").unwrap(), "Ada L.");
    }

    #[tokio::test]
    async fn a_name_is_taken_regardless_of_case() {
        let dir = tempdir::Dir::new();
        let users = UserStore::new(Db::open(dir.path()).await.unwrap());
        assert!(users.insert(new_user("Ada")).await.unwrap());
        assert!(!users.insert(new_user("ada")).await.unwrap());
        assert!(users.username_taken("ADA").await.unwrap());
        assert_eq!(
            users
                .get_by_username("aDa")
                .await
                .unwrap()
                .unwrap()
                .username,
            "Ada"
        );
    }
}
