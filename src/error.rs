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
    Unauthorized(String),
    NotFound(String),
    Conflict(String),
    TooManyRequests(String),
    BadGateway(String),
    Internal(String),
}

impl AppError {
    pub fn bad_request(x: impl Into<String>) -> Self {
        Self::BadRequest(x.into())
    }
    pub fn unauthorized(x: impl Into<String>) -> Self {
        Self::Unauthorized(x.into())
    }
    pub fn conflict(x: impl Into<String>) -> Self {
        Self::Conflict(x.into())
    }
    pub fn too_many(x: impl Into<String>) -> Self {
        Self::TooManyRequests(x.into())
    }
    pub fn bad_gateway(x: impl Into<String>) -> Self {
        Self::BadGateway(x.into())
    }
    pub fn not_found(x: impl Into<String>) -> Self {
        Self::NotFound(x.into())
    }
    pub fn internal_text(x: impl Into<String>) -> Self {
        Self::Internal(x.into())
    }
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::BadGateway(_) => StatusCode::BAD_GATEWAY,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    pub fn detail(&self) -> &str {
        match self {
            Self::BadRequest(x)
            | Self::Unauthorized(x)
            | Self::NotFound(x)
            | Self::Conflict(x)
            | Self::TooManyRequests(x)
            | Self::BadGateway(x)
            | Self::Internal(x) => x,
        }
    }
    fn title(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "Bad request",
            Self::Unauthorized(_) => "Sign in required",
            Self::NotFound(_) => "Not found",
            Self::Conflict(_) => "Already exists",
            Self::TooManyRequests(_) => "Slow down",
            Self::BadGateway(_) => "Service unavailable",
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
