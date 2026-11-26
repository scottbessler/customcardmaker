//! Who is asking. The session is a signed cookie holding the user's id; it is
//! only a claim to *be* that user, so a handler that needs more than a name
//! looks the account up.

use crate::error::{ApiError, AppError};
use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use axum_extra::extract::cookie::{Cookie, Key, SameSite, SignedCookieJar};
use std::convert::Infallible;
use uuid::Uuid;

pub const SESSION_COOKIE: &str = "sid";

pub fn session_cookie(id: Uuid) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, id.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .permanent()
        .build()
}

fn read(jar: &SignedCookieJar) -> Option<Uuid> {
    jar.get(SESSION_COOKIE)
        .and_then(|cookie| cookie.value().parse().ok())
}

/// For a page, which shows something either way.
#[derive(Clone, Copy, Debug)]
pub struct MaybeUser(pub Option<Uuid>);

/// For an `/api` call, which has nothing to say to a stranger.
#[derive(Clone, Copy, Debug)]
pub struct ApiUser(pub Uuid);

impl<S> FromRequestParts<S> for MaybeUser
where
    S: Send + Sync,
    Key: FromRef<S>,
{
    type Rejection = Infallible;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Ok(jar) = SignedCookieJar::<Key>::from_request_parts(parts, state).await;
        Ok(Self(read(&jar)))
    }
}

impl<S> FromRequestParts<S> for ApiUser
where
    S: Send + Sync,
    Key: FromRef<S>,
{
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Ok(jar) = SignedCookieJar::<Key>::from_request_parts(parts, state).await;
        read(&jar)
            .map(Self)
            .ok_or_else(|| AppError::unauthorized("Sign in to do that.").into())
    }
}
