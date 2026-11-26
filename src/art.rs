//! Uploaded card art.
//!
//! The bytes live in a file under `DATA_PATH/art/`; the `art` table says whose
//! it is. The browser shrinks and re-encodes an image before sending it, so
//! what arrives is small, but the server does not trust that: it checks the
//! file's own signature rather than the claimed type, and caps the size.

use crate::db::Db;
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use std::path::PathBuf;
use uuid::Uuid;

/// The largest upload accepted. A shrunk image is a few hundred KB.
pub const MAX_BYTES: usize = 5 * 1024 * 1024;

/// The type an image really is, from its first bytes, or `None` if it is not
/// one we serve. A client's `Content-Type` is never consulted.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[derive(Clone)]
pub struct ArtStore {
    db: Db,
    dir: PathBuf,
}

impl ArtStore {
    /// The directory sits beside the database, so one volume holds both.
    pub async fn open(db: Db) -> Result<Self> {
        let dir = db
            .path()
            .parent()
            .context("database has no parent directory")?
            .join("art");
        tokio::fs::create_dir_all(&dir)
            .await
            .with_context(|| format!("failed to create {}", dir.display()))?;
        Ok(Self { db, dir })
    }

    /// Writes the file first and the row second, so a crash between them leaves
    /// an unreferenced file rather than a row with nothing behind it.
    pub async fn save(
        &self,
        owner: Uuid,
        content_type: &'static str,
        bytes: &[u8],
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let path = self.dir.join(id.to_string());
        tokio::fs::write(&path, bytes)
            .await
            .with_context(|| format!("failed to write {}", path.display()))?;
        let length = bytes.len() as i64;
        let row = self
            .db
            .call(move |c| {
                c.execute(
                    "INSERT INTO art (id, owner_id, content_type, bytes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id.to_string(), owner.to_string(), content_type, length, Utc::now().to_rfc3339()],
                )?;
                Ok(())
            })
            .await;
        if let Err(error) = row {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(error);
        }
        Ok(id)
    }

    pub async fn owned_by(&self, owner: Uuid, id: Uuid) -> Result<bool> {
        self.db
            .call(move |c| {
                Ok(c.query_row(
                    "SELECT 1 FROM art WHERE id = ?1 AND owner_id = ?2",
                    [id.to_string(), owner.to_string()],
                    |_| Ok(()),
                )
                .optional()?
                .is_some())
            })
            .await
    }

    /// An image its owner may always read. Anyone else may read it only while a
    /// card in a *public* set uses it: publishing a set shows its art, and
    /// nothing else of the owner's. `None` otherwise, which is also what a
    /// stranger gets for an image that does not exist.
    pub async fn read(&self, viewer: Option<Uuid>, id: Uuid) -> Result<Option<(String, Vec<u8>)>> {
        let viewer = viewer.map(|v| v.to_string()).unwrap_or_default();
        let content_type: Option<String> = self
            .db
            .call(move |c| {
                Ok(c.query_row(
                    "SELECT a.content_type FROM art a
                     WHERE a.id = ?1
                       AND (a.owner_id = ?2
                            OR EXISTS (SELECT 1 FROM cards c JOIN sets s ON s.id = c.set_id
                                       WHERE s.is_public = 1 AND json_extract(c.doc, '$.art') = a.id))",
                    [id.to_string(), viewer],
                    |row| row.get(0),
                )
                .optional()?)
            })
            .await?;
        let Some(content_type) = content_type else {
            return Ok(None);
        };
        let bytes = tokio::fs::read(self.dir.join(id.to_string()))
            .await
            .with_context(|| format!("art {id} has a row but no file"))?;
        Ok(Some((content_type, bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signature_decides_the_type_not_the_claim() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n....."), Some("image/png"));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]), Some("image/jpeg"));
        assert_eq!(sniff(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"<svg xmlns='http://www.w3.org/2000/svg'/>"), None);
        assert_eq!(sniff(b"GIF89a"), None);
        assert_eq!(sniff(b"<html><script>alert(1)</script>"), None);
        assert_eq!(sniff(b""), None);
    }
}
