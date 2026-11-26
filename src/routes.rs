use crate::{
    app::AppState,
    art,
    card::Card,
    error::{ApiError, AppError},
    render,
    session::{ApiUser, MaybeUser},
    sets::{self, SetDetail, SetSummary, StoredCard},
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

pub async fn get_set(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Path(id): Path<Uuid>,
) -> Api<SetDetail> {
    s.sets.get(me, id).await?.map(Json).ok_or_else(no_set)
}

#[derive(Deserialize)]
pub struct SetPatch {
    name: Option<String>,
    code: Option<String>,
}

pub async fn patch_set(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SetPatch>,
) -> Api<SetSummary> {
    let name = body.name.as_deref().map(sets::clean_name).transpose()?;
    let code = body.code.as_deref().map(sets::clean_code).transpose()?;
    s.sets
        .rename(me, id, name, code)
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
    let id = s.art.save(me, content_type, &body).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn get_art(
    State(s): State<AppState>,
    ApiUser(me): ApiUser,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let (content_type, bytes) = s
        .art
        .read(me, id)
        .await?
        .ok_or_else(|| AppError::not_found("There is no such image."))?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            // An id names one upload forever, so the browser may keep it; only
            // the owner's browser, because the response depends on the cookie.
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable".to_string(),
            ),
        ],
        bytes,
    )
        .into_response())
}
