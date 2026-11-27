use crate::{
    app::AppState,
    art,
    card::{self, Card},
    cockatrice,
    error::{ApiError, AppError},
    openverse::{ArtCredit, SearchPage},
    render,
    session::{ApiUser, MaybeUser},
    sets::{self, Destination, SetDetail, SetSummary, StoredCard, Transferred},
    users::User,
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::{sync::Semaphore, task::JoinSet};
use uuid::Uuid;

const SERVICE_WORKER_JS: &str = include_str!("../public/sw.js");

type Api<T> = Result<Json<T>, ApiError>;

pub async fn healthcheck() -> &'static str {
    "OK"
}

pub async fn service_worker() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        SERVICE_WORKER_JS,
    )
}

/// The account behind the session cookie, if it still exists. A cookie for an
/// account that was deleted is a signed-out visitor, not an error.
async fn account(s: &AppState, MaybeUser(id): MaybeUser) -> Result<Option<User>, AppError> {
    match id {
        Some(id) => Ok(s.users.get(id).await?),
        None => Ok(None),
    }
}

pub async fn frames() -> Html<String> {
    Html(render::frames_page())
}

pub async fn index(State(s): State<AppState>, user: MaybeUser) -> Result<Html<String>, AppError> {
    Ok(Html(match account(&s, user).await? {
        Some(user) => render::sets_page(&user.display_name),
        None => render::welcome_page(),
    }))
}

pub async fn set_page(
    State(s): State<AppState>,
    user: MaybeUser,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let Some(user) = account(&s, user).await? else {
        return Ok(Redirect::to("/").into_response());
    };
    let detail = s
        .sets
        .get(user.id, id)
        .await?
        .ok_or_else(|| AppError::not_found("There is no such set."))?;
    Ok(Html(render::set_page(id, &detail.set.name, &user.display_name)).into_response())
}

fn no_set() -> ApiError {
    AppError::not_found("There is no such set.").into()
}

fn no_card() -> ApiError {
    AppError::not_found("There is no such card.").into()
}

pub async fn list_sets(State(s): State<AppState>, ApiUser(me): ApiUser) -> Api<Vec<SetSummary>> {
    Ok(Json(s.sets.list(me).await?))
}

#[derive(Deserialize)]
pub struct NewSet {
    name: String,
    code: String,
}

pub async fn create_set(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Json(body): Json<NewSet>,
) -> Result<(StatusCode, Json<SetSummary>), ApiError> {
    let name = sets::clean_name(&body.name)?;
    let code = sets::clean_code(&body.code)?;
    Ok((
        StatusCode::CREATED,
        Json(s.sets.create(me, name, code).await?),
    ))
}

/// A set as its owner sees it, plus who made each piece of borrowed art.
#[derive(serde::Serialize)]
pub struct SetView {
    #[serde(flatten)]
    detail: SetDetail,
    credits: std::collections::HashMap<Uuid, ArtCredit>,
}

pub async fn get_set(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Path(id): Path<Uuid>,
) -> Api<SetView> {
    let detail = s.sets.get(me, id).await?.ok_or_else(no_set)?;
    let credits = s
        .art
        .credits(detail.cards.iter().filter_map(|c| c.card.art).collect())
        .await?;
    Ok(Json(SetView { detail, credits }))
}

#[derive(Deserialize)]
pub struct SetPatch {
    name: Option<String>,
    code: Option<String>,
    /// Publish to the gallery, or take it back.
    public: Option<bool>,
    /// The template a card with none of its own wears in this set.
    default_template: Option<String>,
}

pub async fn patch_set(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SetPatch>,
) -> Api<SetSummary> {
    let name = body.name.as_deref().map(sets::clean_name).transpose()?;
    let code = body.code.as_deref().map(sets::clean_code).transpose()?;
    if let Some(template) = body.default_template.as_deref()
        && !card::is_template(template)
    {
        return Err(AppError::bad_request("That is not a card template.").into());
    }
    let changes = sets::SetChanges {
        name,
        code,
        public: body.public,
        default_template: body.default_template,
    };
    s.sets
        .rename(me, id, changes)
        .await?
        .map(Json)
        .ok_or_else(no_set)
}

pub async fn delete_set(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if s.sets.delete(me, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(no_set())
    }
}

/// A body is optional: no body, or `{}`, is a blank card.
pub async fn add_card(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Path(set_id): Path<Uuid>,
    body: Option<Json<Card>>,
) -> Result<(StatusCode, Json<StoredCard>), ApiError> {
    let card = body.map(|Json(card)| card).unwrap_or_default();
    card.validate()?;
    own_art(&s, me, &card).await?;
    let stored = s
        .sets
        .add_card(me, set_id, card)
        .await?
        .ok_or_else(no_set)?;
    Ok((StatusCode::CREATED, Json(stored)))
}

pub async fn save_card(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Path(id): Path<Uuid>,
    Json(card): Json<Card>,
) -> Api<StoredCard> {
    card.validate()?;
    own_art(&s, me, &card).await?;
    s.sets
        .save_card(me, id, card)
        .await?
        .map(Json)
        .ok_or_else(no_card)
}

pub async fn delete_card(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if s.sets.delete_card(me, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(no_card())
    }
}

/// A card may only point at art its owner uploaded.
async fn own_art(s: &AppState, me: Uuid, card: &Card) -> Result<(), ApiError> {
    match card.art {
        Some(id) if !s.art.owned_by(me, id).await? => {
            Err(AppError::bad_request("That image is not in your uploads.").into())
        }
        _ => Ok(()),
    }
}

/// The body is the image itself, not a form: the browser has already shrunk it.
pub async fn upload_art(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    body: Bytes,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let content_type = art::sniff(&body)
        .ok_or_else(|| AppError::bad_request("That is not a PNG, JPEG or WebP image."))?;
    let id = s.art.save(me, content_type, &body, None).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn get_art(
    State(s): State<AppState>,
    MaybeUser(viewer): MaybeUser,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let (content_type, bytes) = s
        .art
        .read(viewer, id)
        .await?
        .ok_or_else(|| AppError::not_found("There is no such image."))?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            // An id names one upload forever, so the browser may keep it; only
            // that browser, because who may see it depends on the cookie.
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable".to_string(),
            ),
        ],
        bytes,
    )
        .into_response())
}

/// Import cards from a Cockatrice XML database into a new set owned by the
/// signed-in account. The set is created and filled in one transaction.
pub async fn import_cockatrice(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Json(body): Json<ImportCockatriceForm>,
) -> Result<(StatusCode, Json<ImportCockatriceResult>), ApiError> {
    let database = cockatrice::parse(&body.xml)?;

    let code = match body.set_code.as_deref().map(str::trim) {
        Some(code) if !code.is_empty() => sets::clean_code(code)?,
        _ => sets::clean_code(database.set_code.as_deref().ok_or_else(|| {
            AppError::bad_request("That file does not name a set; enter a set code.")
        })?)?,
    };
    let name = match body.set_name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => sets::clean_name(name)?,
        _ => sets::clean_name(database.set_name.as_deref().unwrap_or(&code))?,
    };

    let cards_imported = database.cards.len();
    let (cards, images_imported) = if body.images {
        attach_pictures(&s, me, database.cards, body.whole_cards).await
    } else {
        (detach(database.cards), 0)
    };
    let set = s.sets.create_with_cards(me, name, code, cards).await?;
    Ok((
        StatusCode::CREATED,
        Json(ImportCockatriceResult {
            set_id: set.id,
            set_code: set.code,
            set_name: set.name,
            cards_imported,
            images_imported,
        }),
    ))
}

/// The largest picture downloaded from a card database. A card face is a few
/// hundred KB.
const MAX_PICTURE_BYTES: usize = art::MAX_BYTES;
/// How many pictures are downloaded at once. Enough to keep an import quick
/// without hammering the host the file points at.
const PICTURE_FETCHES: usize = 4;
const PICTURE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

fn detach(cards: Vec<cockatrice::ImportedCard>) -> Vec<Card> {
    cards.into_iter().map(|item| item.card).collect()
}

/// Downloads each card's picture and stores it as the importer's art, so the
/// imported cards come with the images the file named. A picture that fails to
/// download (or is not an image) is skipped: the card still imports, without
/// art.
async fn attach_pictures(
    s: &AppState,
    me: Uuid,
    cards: Vec<cockatrice::ImportedCard>,
    whole_cards: bool,
) -> (Vec<Card>, usize) {
    let permits = Arc::new(Semaphore::new(PICTURE_FETCHES));
    let mut set = JoinSet::new();
    let mut arts: Vec<Option<Uuid>> = vec![None; cards.len()];

    for (index, item) in cards.iter().enumerate() {
        let Some(url) = item.image_url.clone() else {
            continue;
        };
        let client = s.http.clone();
        let store = s.art.clone();
        let permits = Arc::clone(&permits);
        set.spawn(async move {
            let _permit = permits.acquire_owned().await.ok()?;
            let (content_type, bytes) = fetch_picture(&client, &url).await?;
            let id = store.save(me, content_type, &bytes, None).await.ok()?;
            Some((index, id))
        });
    }

    let mut imported = 0;
    while let Some(joined) = set.join_next().await {
        if let Ok(Some((index, id))) = joined {
            arts[index] = Some(id);
            imported += 1;
        }
    }

    let cards = cards
        .into_iter()
        .zip(arts)
        .map(|(item, art)| Card {
            art_is_card: whole_cards && art.is_some(),
            art,
            ..item.card
        })
        .collect();
    (cards, imported)
}

/// `None` unless the URL answers with a small image of a kind we serve.
async fn fetch_picture(client: &reqwest::Client, url: &str) -> Option<(&'static str, Vec<u8>)> {
    let mut response = client.get(url).timeout(PICTURE_TIMEOUT).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PICTURE_BYTES as u64)
    {
        return None;
    }
    let mut bytes: Vec<u8> = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        if bytes.len() + chunk.len() > MAX_PICTURE_BYTES {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    art::sniff(&bytes).map(|content_type| (content_type, bytes))
}

/// Form data for Cockatrice import
#[derive(Deserialize)]
pub struct ImportCockatriceForm {
    /// Cockatrice XML data
    pub xml: String,
    /// Set code (3-5 chars, upper case); falls back to the code in the file
    pub set_code: Option<String>,
    /// Set name; falls back to the file's long name
    pub set_name: Option<String>,
    /// Download each card's picture and use it as the card's art
    #[serde(default)]
    pub images: bool,
    /// The pictures are whole printed cards (what MTG Nexus exports), not just
    /// art: each card is marked so it shows the picture edge to edge.
    #[serde(default)]
    pub whole_cards: bool,
}

/// Result of Cockatrice import
#[derive(serde::Serialize)]
pub struct ImportCockatriceResult {
    /// Set ID
    pub set_id: Uuid,
    /// Set code
    pub set_code: String,
    /// Set name
    pub set_name: String,
    /// Number of cards imported
    pub cards_imported: usize,
    /// Number of card pictures downloaded and attached
    pub images_imported: usize,
}

/// Show the Cockatrice import UI page. It needs an account to import into.
pub async fn import_cockatrice_ui(
    State(s): State<AppState>,
    user: MaybeUser,
) -> Result<Response, AppError> {
    let Some(user) = account(&s, user).await? else {
        return Ok(Redirect::to("/").into_response());
    };
    Ok(Html(render::import_page(&user.display_name)).into_response())
}

#[derive(Deserialize)]
pub struct CardIds {
    ids: Vec<Uuid>,
}

/// A bulk request names at least one card and no more than `MAX_BULK`.
fn checked(ids: &[Uuid]) -> Result<(), AppError> {
    if ids.is_empty() {
        return Err(AppError::bad_request("Choose at least one card."));
    }
    if ids.len() > sets::MAX_BULK {
        return Err(AppError::bad_request(format!(
            "Choose at most {} cards at a time.",
            sets::MAX_BULK
        )));
    }
    Ok(())
}

fn some_missing() -> ApiError {
    AppError::not_found("Some of those cards, or that set, no longer exist.").into()
}

pub async fn delete_cards(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Json(body): Json<CardIds>,
) -> Api<serde_json::Value> {
    checked(&body.ids)?;
    let deleted = s
        .sets
        .delete_cards(me, body.ids)
        .await?
        .ok_or_else(some_missing)?;
    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Existing { set: Uuid },
    New { name: String, code: String },
}

#[derive(Deserialize)]
pub struct Transfer {
    ids: Vec<Uuid>,
    to: Target,
    /// Copy instead of move.
    #[serde(default)]
    copy: bool,
}

pub async fn transfer_cards(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Json(body): Json<Transfer>,
) -> Api<Transferred> {
    checked(&body.ids)?;
    let destination = match body.to {
        Target::Existing { set } => Destination::Existing(set),
        Target::New { name, code } => Destination::New {
            name: sets::clean_name(&name)?,
            code: sets::clean_code(&code)?,
        },
    };
    let done = s
        .sets
        .transfer_cards(me, body.ids, destination, body.copy)
        .await?
        .ok_or_else(some_missing)?;
    if done.count == 0 {
        return Err(AppError::bad_request("Those cards are already in that set.").into());
    }
    Ok(Json(done))
}

/// The gallery's front page: every public set, for anyone.
pub async fn gallery_page(
    State(s): State<AppState>,
    user: MaybeUser,
) -> Result<Html<String>, AppError> {
    let who = account(&s, user).await?;
    Ok(Html(render::gallery_page(
        who.as_ref().map(|u| u.display_name.as_str()),
    )))
}

pub async fn gallery_set_page(
    State(s): State<AppState>,
    user: MaybeUser,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, AppError> {
    let who = account(&s, user).await?;
    let set = s
        .sets
        .public_set(id)
        .await?
        .ok_or_else(|| AppError::not_found("There is no such public set."))?;
    Ok(Html(render::gallery_set_page(
        id,
        &set.name,
        who.as_ref().map(|u| u.display_name.as_str()),
    )))
}

pub async fn gallery_list(State(s): State<AppState>) -> Api<Vec<sets::GalleryEntry>> {
    Ok(Json(s.sets.gallery().await?))
}

/// A public set as anyone sees it, plus who made each piece of borrowed art.
#[derive(serde::Serialize)]
pub struct GalleryView {
    #[serde(flatten)]
    detail: sets::GalleryDetail,
    credits: std::collections::HashMap<Uuid, ArtCredit>,
}

pub async fn gallery_set(State(s): State<AppState>, Path(id): Path<Uuid>) -> Api<GalleryView> {
    let detail = s
        .sets
        .public_set(id)
        .await?
        .ok_or_else(|| ApiError::from(AppError::not_found("There is no such public set.")))?;
    let credits = s
        .art
        .credits(detail.cards.iter().filter_map(|c| c.card.art).collect())
        .await?;
    Ok(Json(GalleryView { detail, credits }))
}

#[derive(Deserialize)]
pub struct ArtSearch {
    q: String,
    #[serde(default = "first_page")]
    page: u32,
    /// Include the non-commercial Creative Commons licences.
    #[serde(default)]
    nc: bool,
}

fn first_page() -> u32 {
    1
}

pub async fn search_art(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    axum::extract::Query(query): axum::extract::Query<ArtSearch>,
) -> Api<SearchPage> {
    Ok(Json(
        s.openverse
            .search(me, &query.q, query.page, query.nc)
            .await?,
    ))
}

#[derive(Deserialize)]
pub struct ArtPick {
    /// An Openverse result's id, from a search. Nothing else: the server never
    /// fetches a URL it was handed.
    id: String,
}

pub async fn import_art(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Json(pick): Json<ArtPick>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let got = s.openverse.fetch(&pick.id).await?;
    let id = s
        .art
        .save(me, got.content_type, &got.bytes, Some(&got.credit))
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": id, "credit": got.credit })),
    ))
}
