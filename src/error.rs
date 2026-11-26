use crate::render;
use axum::{
    Json,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

impl AppError {
    pub fn bad_request(x: impl Into<String>) -> Self {
        Self::BadRequest(x.into())
    }
    pub fn not_found(x: impl Into<String>) -> Self {
        Self::NotFound(x.into())
    }
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    pub fn detail(&self) -> &str {
        match self {
            Self::BadRequest(x) | Self::NotFound(x) | Self::Internal(x) => x,
        }
    }
    fn title(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "Bad request",
            Self::NotFound(_) => "Not found",
            Self::Internal(_) => "Something went wrong",
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.title(), self.detail())
    }
}
impl std::error::Error for AppError {}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        // The detail is for the log, not the caller: a SQLite message has no
        // business in a response body.
        tracing::error!(%error, "internal error");
        Self::Internal("The server could not finish that.".into())
    }
}

/// A page that failed renders as a page.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            self.status_code(),
            Html(render::error_page(self.title(), self.detail())),
        )
            .into_response()
    }
}

/// An `/api` call that failed answers in JSON, because its caller is script.
pub struct ApiError(pub AppError);

impl<E: Into<AppError>> From<E> for ApiError {
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.0.status_code(),
            Json(json!({ "error": self.0.detail() })),
        )
            .into_response()
    }
}
