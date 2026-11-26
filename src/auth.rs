//! Passkey (WebAuthn) registration and sign-in. There are no passwords: an
//! account is a username and the passkeys registered to it.
//!
//! `PASSKEY_DISABLED=1` skips the ceremony and signs in by username alone, for
//! local development and tests, where no authenticator can be driven.

use crate::{
    app::AppState,
    error::{ApiError, AppError},
    session::{SESSION_COOKIE, session_cookie},
    users::{User, clean_display_name, clean_username},
};
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, SameSite, SignedCookieJar};
use chrono::Utc;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use uuid::Uuid;
use webauthn_rs::prelude::{
    PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential,
};

const REG_COOKIE: &str = "reg_state";
const AUTH_COOKIE: &str = "auth_state";

#[derive(Deserialize)]
pub struct RegisterBegin {
    username: String,
    display_name: Option<String>,
}

/// Held in a signed cookie between the two halves of the ceremony, so the
/// server keeps no state for an account that was never finished.
#[derive(Serialize, Deserialize)]
struct PendingRegistration {
    user_id: Uuid,
    username: String,
    display_name: String,
    state: PasskeyRegistration,
}

#[derive(Deserialize)]
pub struct LoginBegin {
    username: String,
}

#[derive(Serialize, Deserialize)]
struct PendingLogin {
    user_id: Uuid,
    state: PasskeyAuthentication,
}

fn taken() -> AppError {
    AppError::conflict("That username is already taken.")
}

async fn create_account(s: &AppState, user: User) -> Result<(), AppError> {
    if s.users.insert(user).await? {
        Ok(())
    } else {
        Err(taken())
    }
}

pub async fn register_begin(
    State(s): State<AppState>,
    jar: SignedCookieJar,
    Json(body): Json<RegisterBegin>,
) -> Result<Response, ApiError> {
    let username = clean_username(&body.username)?;
    let display_name = clean_display_name(body.display_name.as_deref(), &username)?;
    if s.users.username_taken(&username).await? {
        return Err(taken().into());
    }
    let id = Uuid::new_v4();
    if s.passkey_disabled {
        let user = User {
            id,
            username,
            display_name: display_name.clone(),
            credentials: vec![],
            created_at: Utc::now(),
        };
        create_account(&s, user).await?;
        return Ok((
            jar.add(session_cookie(id)),
            Json(json!({ "display_name": display_name })),
        )
            .into_response());
    }
    let (challenge, state) = s
        .webauthn
        .start_passkey_registration(id, &username, &display_name, None)
        .map_err(|e| AppError::bad_request(e.to_string()))?;
    let pending = PendingRegistration {
        user_id: id,
        username,
        display_name,
        state,
    };
    Ok((
        jar.add(state_cookie(REG_COOKIE, &pending)?),
        Json(challenge),
    )
        .into_response())
}

pub async fn register_finish(
    State(s): State<AppState>,
    jar: SignedCookieJar,
    Json(credential): Json<RegisterPublicKeyCredential>,
) -> Result<(SignedCookieJar, Json<serde_json::Value>), ApiError> {
    let pending: PendingRegistration = take(&jar, REG_COOKIE)
        .ok_or_else(|| AppError::bad_request("Registration expired. Start again."))?;
    let passkey = s
        .webauthn
        .finish_passkey_registration(&credential, &pending.state)
        .map_err(|e| AppError::bad_request(e.to_string()))?;
    let user = User {
        id: pending.user_id,
        username: pending.username,
        display_name: pending.display_name.clone(),
        credentials: vec![passkey],
        created_at: Utc::now(),
    };
    create_account(&s, user).await?;
    Ok((
        jar.remove(Cookie::build((REG_COOKIE, "")).path("/").build())
            .add(session_cookie(pending.user_id)),
        Json(json!({ "display_name": pending.display_name })),
    ))
}

pub async fn login_begin(
    State(s): State<AppState>,
    jar: SignedCookieJar,
    Json(body): Json<LoginBegin>,
) -> Result<Response, ApiError> {
    let user = s
        .users
        .get_by_username(&body.username)
        .await?
        .ok_or_else(|| AppError::not_found("No account has that username."))?;
    if s.passkey_disabled {
        return Ok((
            jar.add(session_cookie(user.id)),
            Json(json!({ "display_name": user.display_name })),
        )
            .into_response());
    }
    let (challenge, state) = s
        .webauthn
        .start_passkey_authentication(&user.credentials)
        .map_err(|e| AppError::bad_request(e.to_string()))?;
    let pending = PendingLogin {
        user_id: user.id,
        state,
    };
    Ok((
        jar.add(state_cookie(AUTH_COOKIE, &pending)?),
        Json(challenge),
    )
        .into_response())
}

pub async fn login_finish(
    State(s): State<AppState>,
    jar: SignedCookieJar,
    Json(credential): Json<PublicKeyCredential>,
) -> Result<(SignedCookieJar, Json<serde_json::Value>), ApiError> {
    let pending: PendingLogin = take(&jar, AUTH_COOKIE)
        .ok_or_else(|| AppError::bad_request("Sign-in expired. Start again."))?;
    let result = s
        .webauthn
        .finish_passkey_authentication(&credential, &pending.state)
        .map_err(|e| AppError::bad_request(e.to_string()))?;
    // The signature counter moves on every use; keeping it current is what
    // lets a cloned authenticator be noticed.
    if result.needs_update()
        && let Some(user) = s.users.get(pending.user_id).await?
    {
        let mut credentials = user.credentials;
        for passkey in &mut credentials {
            passkey.update_credential(&result);
        }
        s.users.update_credentials(user.id, credentials).await?;
    }
    Ok((
        jar.remove(Cookie::build((AUTH_COOKIE, "")).path("/").build())
            .add(session_cookie(pending.user_id)),
        Json(json!({ "ok": true })),
    ))
}

pub async fn logout(jar: SignedCookieJar) -> (SignedCookieJar, Redirect) {
    (
        jar.remove(Cookie::build((SESSION_COOKIE, "")).path("/").build()),
        Redirect::to("/"),
    )
}

fn state_cookie<T: Serialize>(name: &'static str, value: &T) -> Result<Cookie<'static>, AppError> {
    let json = serde_json::to_string(value).map_err(|e| AppError::internal_text(e.to_string()))?;
    Ok(Cookie::build((name, json))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .build())
}

fn take<T: DeserializeOwned>(jar: &SignedCookieJar, name: &str) -> Option<T> {
    jar.get(name)
        .and_then(|cookie| serde_json::from_str(cookie.value()).ok())
}
