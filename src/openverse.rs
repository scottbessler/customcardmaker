//! Searching Creative Commons art on Openverse, and bringing a pick home.
//!
//! The browser never talks to Openverse's API and the server never fetches a URL
//! that came out of a search result: a record's own image URL could point
//! anywhere, including at this machine's network. Openverse serves any image
//! through its own host (`/v1/images/<id>/thumb/?full_size=true`), so the only
//! host the server ever contacts is the one it was configured with, and an id
//! that is not a UUID never reaches a URL at all.
//!
//! Licences: CC0, the public domain mark, BY and BY-SA are offered; the
//! non-commercial ones only when asked for. NoDerivatives is never offered,
//! because a card crops its art, which is a derivative.

use crate::{art, error::AppError};
use reqwest::{Client, StatusCode, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub const DEFAULT_BASE: &str = "https://api.openverse.org";
pub const MAX_QUERY: usize = 100;
pub const MAX_PAGE: u32 = 20;

const PAGE_SIZE: u32 = 20;
const CACHE_TTL: Duration = Duration::from_secs(60);
const CACHE_MAX: usize = 200;
const SEARCHES_PER_WINDOW: usize = 30;
const WINDOW: Duration = Duration::from_secs(600);
const LICENSES: &str = "cc0,pdm,by,by-sa";
const LICENSES_NC: &str = "cc0,pdm,by,by-sa,by-nc,by-nc-sa";
/// What the card can use. GIF and SVG are not images this app stores.
const EXTENSIONS: &str = "jpg,png";
const FIELD_MAX: usize = 200;

/// Who made an image and under what terms: kept with it, shown with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtCredit {
    pub title: String,
    pub creator: String,
    /// Openverse's code: `cc0`, `pdm`, `by`, `by-sa`, `by-nc`, `by-nc-sa`.
    pub license: String,
    /// What to print: `CC BY 2.0`.
    pub license_label: String,
    pub license_url: String,
    /// Where the image lives, for a reader who wants more of it.
    pub source_url: String,
}

/// One search result, cleaned for the page.
#[derive(Debug, Clone, Serialize)]
pub struct Found {
    pub id: String,
    pub title: String,
    pub creator: String,
    pub license: String,
    pub license_label: String,
    pub license_url: String,
    pub thumbnail: String,
    pub landing_url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchPage {
    pub results: Vec<Found>,
    pub page: u32,
    pub page_count: u32,
    pub total: u64,
}

/// An image fetched for a card, and its credit.
pub struct Fetched {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
    pub credit: ArtCredit,
}

#[derive(Deserialize)]
struct RawPage {
    #[serde(default)]
    results: Vec<RawImage>,
    #[serde(default)]
    page_count: u32,
    #[serde(default)]
    result_count: u64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawImage {
    id: String,
    title: Option<String>,
    creator: Option<String>,
    license: Option<String>,
    license_version: Option<String>,
    license_url: Option<String>,
    thumbnail: Option<String>,
    foreign_landing_url: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    mature: bool,
}

pub struct Openverse {
    client: Client,
    base: String,
    host: String,
    cache: Mutex<HashMap<String, (Instant, SearchPage)>>,
    recent: Mutex<HashMap<Uuid, Vec<Instant>>>,
}

/// `CC BY 2.0`, `CC0 1.0`, `Public Domain Mark 1.0`.
pub fn license_label(code: &str, version: &str) -> String {
    let name = match code {
        "cc0" => "CC0".to_string(),
        "pdm" => "Public Domain Mark".to_string(),
        other => format!("CC {}", other.to_uppercase()),
    };
    if version.is_empty() {
        name
    } else {
        format!("{name} {version}")
    }
}

/// The terms' own page, if the record did not give a usable one.
fn license_url(code: &str, version: &str) -> String {
    match code {
        "cc0" => "https://creativecommons.org/publicdomain/zero/1.0/".into(),
        "pdm" => "https://creativecommons.org/publicdomain/mark/1.0/".into(),
        other => format!(
            "https://creativecommons.org/licenses/{other}/{}/",
            if version.is_empty() { "4.0" } else { version }
        ),
    }
}

/// Text from a stranger's record: no control characters, bounded.
fn clean(text: Option<&str>, fallback: &str) -> String {
    let cleaned: String = text
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(FIELD_MAX)
        .collect();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

/// A link a page may put in an `href`: web only, never `javascript:`.
fn web_url(text: Option<&str>) -> Option<String> {
    let url = Url::parse(text?).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

fn allowed(code: &str, non_commercial: bool) -> bool {
    (if non_commercial {
        LICENSES_NC
    } else {
        LICENSES
    })
    .split(',')
    .any(|c| c == code)
}

impl Openverse {
    pub fn new(base: &str) -> anyhow::Result<Self> {
        let base = base.trim_end_matches('/').to_string();
        let parsed = Url::parse(&base)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("OPENVERSE_URL has no host"))?
            .to_string();
        let follow = host.clone();
        let client = Client::builder()
            .user_agent(concat!(
                "customcardmaker/",
                env!("CARGO_PKG_VERSION"),
                " (personal fan tool)"
            ))
            .timeout(Duration::from_secs(20))
            // A redirect may stay on the configured host and nowhere else.
            .redirect(Policy::custom(move |attempt| {
                if attempt.previous().len() < 3 && attempt.url().host_str() == Some(follow.as_str())
                {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()?;
        Ok(Self {
            client,
            base,
            host,
            cache: Mutex::default(),
            recent: Mutex::default(),
        })
    }

    /// Per-account ceiling on searches, so one person cannot spend the shared
    /// (anonymous) quota Openverse allows this server.
    fn within_limit(&self, who: Uuid) -> bool {
        let mut recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let times = recent.entry(who).or_default();
        times.retain(|t| now.duration_since(*t) < WINDOW);
        if times.len() >= SEARCHES_PER_WINDOW {
            return false;
        }
        times.push(now);
        true
    }

    fn thumbnail(&self, text: Option<&str>) -> Option<String> {
        // Only Openverse's own host: the page never loads from anywhere else.
        let url = Url::parse(text?).ok()?;
        (url.host_str() == Some(self.host.as_str()) && matches!(url.scheme(), "http" | "https"))
            .then(|| url.to_string())
    }

    fn found(&self, raw: RawImage, non_commercial: bool) -> Option<Found> {
        let code = raw.license.as_deref()?.to_lowercase();
        if raw.mature || !allowed(&code, non_commercial) || Uuid::parse_str(&raw.id).is_err() {
            return None;
        }
        let version = raw.license_version.clone().unwrap_or_default();
        Some(Found {
            thumbnail: self.thumbnail(raw.thumbnail.as_deref())?,
            title: clean(raw.title.as_deref(), "Untitled"),
            creator: clean(raw.creator.as_deref(), "Unknown"),
            license_label: license_label(&code, &version),
            license_url: web_url(raw.license_url.as_deref())
                .unwrap_or_else(|| license_url(&code, &version)),
            landing_url: web_url(raw.foreign_landing_url.as_deref()).unwrap_or_default(),
            license: code,
            id: raw.id,
            width: raw.width,
            height: raw.height,
        })
    }

    pub async fn search(
        &self,
        who: Uuid,
        query: &str,
        page: u32,
        non_commercial: bool,
    ) -> Result<SearchPage, AppError> {
        let query = query.trim();
        if query.is_empty()
            || query.chars().count() > MAX_QUERY
            || query.chars().any(char::is_control)
        {
            return Err(AppError::bad_request(format!(
                "Search for 1 to {MAX_QUERY} characters."
            )));
        }
        if !(1..=MAX_PAGE).contains(&page) {
            return Err(AppError::bad_request(
                "That page of results does not exist.",
            ));
        }
        let key = format!("{}|{page}|{non_commercial}", query.to_lowercase());
        if let Some((at, hit)) = self
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            && at.elapsed() < CACHE_TTL
        {
            return Ok(hit.clone());
        }
        if !self.within_limit(who) {
            return Err(AppError::too_many(
                "That is a lot of searches. Wait a few minutes and try again.",
            ));
        }
        let response = self
            .client
            .get(format!("{}/v1/images/", self.base))
            .query(&[
                ("q", query),
                (
                    "license",
                    if non_commercial {
                        LICENSES_NC
                    } else {
                        LICENSES
                    },
                ),
                ("extension", EXTENSIONS),
                ("mature", "false"),
                ("page", &page.to_string()),
                ("page_size", &PAGE_SIZE.to_string()),
            ])
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "openverse search failed");
                unavailable()
            })?;
        let raw: RawPage = match response.status() {
            StatusCode::OK => response.json().await.map_err(|e| {
                tracing::warn!(error = %e, "openverse search answered with something unreadable");
                unavailable()
            })?,
            // Asking past the last page is not an error to a reader.
            StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND => RawPage {
                results: vec![],
                page_count: 0,
                result_count: 0,
            },
            StatusCode::TOO_MANY_REQUESTS => {
                return Err(AppError::too_many(
                    "Image search is busy right now. Try again in a minute.",
                ));
            }
            other => {
                tracing::warn!(status = %other, "openverse search refused");
                return Err(unavailable());
            }
        };
        let out = SearchPage {
            results: raw
                .results
                .into_iter()
                .filter_map(|r| self.found(r, non_commercial))
                .collect(),
            page,
            page_count: raw.page_count,
            total: raw.result_count,
        };
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.len() >= CACHE_MAX {
            cache.retain(|_, (at, _)| at.elapsed() < CACHE_TTL);
            if cache.len() >= CACHE_MAX {
                cache.clear();
            }
        }
        cache.insert(key, (Instant::now(), out.clone()));
        Ok(out)
    }

    /// The image for a search result's id, and its credit. Refuses what a card
    /// may not use: a mature or NoDerivatives work, or anything that is not a
    /// PNG or JPEG, or is larger than an upload may be.
    pub async fn fetch(&self, id: &str) -> Result<Fetched, AppError> {
        let id = Uuid::parse_str(id.trim())
            .map_err(|_| AppError::bad_request("That is not an image id."))?;
        let detail = self
            .client
            .get(format!("{}/v1/images/{id}/", self.base))
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "openverse detail failed");
                unavailable()
            })?;
        let raw: RawImage = match detail.status() {
            StatusCode::OK => detail.json().await.map_err(|_| unavailable())?,
            StatusCode::NOT_FOUND => {
                return Err(AppError::not_found("That image is no longer on Openverse."));
            }
            StatusCode::TOO_MANY_REQUESTS => {
                return Err(AppError::too_many(
                    "Image search is busy right now. Try again in a minute.",
                ));
            }
            _ => return Err(unavailable()),
        };
        let code = raw.license.as_deref().unwrap_or("").to_lowercase();
        if raw.mature || !allowed(&code, true) {
            return Err(AppError::bad_request(
                "That image's licence does not let it be used on a card.",
            ));
        }
        let version = raw.license_version.clone().unwrap_or_default();
        let credit = ArtCredit {
            title: clean(raw.title.as_deref(), "Untitled"),
            creator: clean(raw.creator.as_deref(), "Unknown"),
            license_label: license_label(&code, &version),
            license_url: web_url(raw.license_url.as_deref())
                .unwrap_or_else(|| license_url(&code, &version)),
            source_url: web_url(raw.foreign_landing_url.as_deref()).unwrap_or_default(),
            license: code,
        };
        let mut response = self
            .client
            .get(format!("{}/v1/images/{id}/thumb/", self.base))
            .query(&[("full_size", "true")])
            .send()
            .await
            .map_err(|_| unavailable())?;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::too_many(
                "Image search is busy right now. Try again in a minute.",
            ));
        }
        if !response.status().is_success() {
            return Err(AppError::bad_gateway(
                "Openverse could not give us that image.",
            ));
        }
        if response
            .content_length()
            .is_some_and(|n| n > art::MAX_BYTES as u64)
        {
            return Err(AppError::bad_request("That image is too large."));
        }
        // Read it in pieces so a body that lies about its length still stops at the cap.
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > art::MAX_BYTES {
                return Err(AppError::bad_request("That image is too large."));
            }
        }
        let content_type = art::sniff(&bytes)
            .ok_or_else(|| AppError::bad_gateway("That was not a PNG, JPEG or WebP image."))?;
        Ok(Fetched {
            bytes,
            content_type,
            credit,
        })
    }
}

fn unavailable() -> AppError {
    AppError::bad_gateway("Image search is not available right now.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn licences_are_labelled_the_way_they_are_printed() {
        assert_eq!(license_label("by", "2.0"), "CC BY 2.0");
        assert_eq!(license_label("by-sa", "4.0"), "CC BY-SA 4.0");
        assert_eq!(license_label("cc0", "1.0"), "CC0 1.0");
        assert_eq!(license_label("pdm", "1.0"), "Public Domain Mark 1.0");
    }

    #[test]
    fn no_derivatives_is_never_allowed_and_non_commercial_only_on_request() {
        for code in ["cc0", "pdm", "by", "by-sa"] {
            assert!(allowed(code, false), "{code}");
        }
        assert!(!allowed("by-nc", false));
        assert!(allowed("by-nc", true) && allowed("by-nc-sa", true));
        for code in ["by-nd", "by-nc-nd", "", "all-rights-reserved"] {
            assert!(!allowed(code, true), "{code} must never be offered");
        }
    }

    #[test]
    fn only_web_links_survive() {
        assert!(web_url(Some("https://example.org/a")).is_some());
        assert!(web_url(Some("javascript:alert(1)")).is_none());
        assert!(web_url(Some("data:text/html,x")).is_none());
        assert!(web_url(Some("not a url")).is_none());
        assert!(web_url(None).is_none());
    }

    #[test]
    fn a_records_text_is_cleaned_and_bounded() {
        assert_eq!(clean(Some("  A\u{0}B\nC  "), "x"), "ABC");
        assert_eq!(clean(Some("   "), "Untitled"), "Untitled");
        assert_eq!(
            clean(Some(&"y".repeat(500)), "x").chars().count(),
            FIELD_MAX
        );
    }

    #[test]
    fn only_the_configured_host_may_supply_a_thumbnail() {
        let ov = Openverse::new("https://api.example.org").unwrap();
        assert!(
            ov.thumbnail(Some("https://api.example.org/v1/images/x/thumb/"))
                .is_some()
        );
        assert!(
            ov.thumbnail(Some("https://evil.example.net/x.jpg"))
                .is_none()
        );
        assert!(ov.thumbnail(Some("javascript:alert(1)")).is_none());
    }

    #[test]
    fn one_account_gets_a_bounded_number_of_searches() {
        let ov = Openverse::new("https://api.example.org").unwrap();
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        assert!((0..SEARCHES_PER_WINDOW).all(|_| ov.within_limit(a)));
        assert!(!ov.within_limit(a), "the next one is refused");
        assert!(ov.within_limit(b), "and someone else is unaffected");
    }
}
