use crate::{
    app::AppState,
    card::Card,
    error::{ApiError, AppError},
    render,
    sets::{self, SetDetail, SetSummary, StoredCard},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse},
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

pub async fn index() -> Html<String> {
    Html(render::sets_page())
}

pub async fn set_page(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, AppError> {
    let detail = s
        .sets
        .get(id)
        .await?
        .ok_or_else(|| AppError::not_found("There is no such set."))?;
    Ok(Html(render::set_page(id, &detail.set.name)))
}

pub async fn list_sets(State(s): State<AppState>) -> Api<Vec<SetSummary>> {
    Ok(Json(s.sets.list().await?))
}

#[derive(Deserialize)]
pub struct NewSet {
    name: String,
    code: String,
}

pub async fn create_set(
    State(s): State<AppState>,
    Json(body): Json<NewSet>,
) -> Result<(StatusCode, Json<SetSummary>), ApiError> {
    let name = sets::clean_name(&body.name)?;
    let code = sets::clean_code(&body.code)?;
    Ok((StatusCode::CREATED, Json(s.sets.create(name, code).await?)))
}

pub async fn get_set(State(s): State<AppState>, Path(id): Path<Uuid>) -> Api<SetDetail> {
    s.sets
        .get(id)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::not_found("There is no such set.").into())
}

#[derive(Deserialize)]
pub struct SetPatch {
    name: Option<String>,
    code: Option<String>,
}

pub async fn patch_set(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<SetPatch>,
) -> Api<SetSummary> {
    let name = body.name.as_deref().map(sets::clean_name).transpose()?;
    let code = body.code.as_deref().map(sets::clean_code).transpose()?;
    s.sets
        .rename(id, name, code)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::not_found("There is no such set.").into())
}

pub async fn delete_set(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if s.sets.delete(id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("There is no such set.").into())
    }
}

/// A body is optional: no body, or `{}`, is a blank card.
pub async fn add_card(
    State(s): State<AppState>,
    Path(set_id): Path<Uuid>,
    body: Option<Json<Card>>,
) -> Result<(StatusCode, Json<StoredCard>), ApiError> {
    let card = body.map(|Json(card)| card).unwrap_or_default();
    card.validate()?;
    let stored = s
        .sets
        .add_card(set_id, card)
        .await?
        .ok_or_else(|| AppError::not_found("There is no such set."))?;
    Ok((StatusCode::CREATED, Json(stored)))
}

pub async fn save_card(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
    Json(card): Json<Card>,
) -> Api<StoredCard> {
    card.validate()?;
    s.sets
        .save_card(id, card)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::not_found("There is no such card.").into())
}

pub async fn delete_card(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if s.sets.delete_card(id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("There is no such card.").into())
    }
}
