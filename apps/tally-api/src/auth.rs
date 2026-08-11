//! Authentication (spec §10): register / login / logout / me, plus the
//! bearer-token [`AuthUser`] extractor that every non-auth handler runs
//! through.
//!
//! Sessions: an opaque 32-byte random token is issued once (returned
//! plaintext); only its sha256 is stored (on `Session.token_hash`, unique).
//! Passwords are hashed with argon2 (PHC strings).

use std::sync::Arc;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::app::AppState;
use crate::error::{AppError, FieldIssue};
use crate::extract::AppJson;
use crate::models::{Session, User};

/// 30 days, per spec §10.
const SESSION_TTL_DAYS: i64 = 30;
/// Minimum password length (semantic validation, 422).
const MIN_PASSWORD_LEN: usize = 8;

// ---------------------------------------------------------------------------
// Request / response bodies
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterBody {
    pub display_name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /auth/register` → `{ token, user }`.
pub async fn register(
    State(state): State<Arc<AppState>>,
    AppJson(body): AppJson<RegisterBody>,
) -> Result<Json<AuthResponse>, AppError> {
    let email = body.email.trim().to_lowercase();
    let mut fields = Vec::new();
    if body.display_name.trim().is_empty() {
        fields.push(FieldIssue { field: "display_name".into(), reason: "required".into() });
    }
    if !valid_email(&email) {
        fields.push(FieldIssue { field: "email".into(), reason: "must be a valid email address".into() });
    }
    if body.password.len() < MIN_PASSWORD_LEN {
        fields.push(FieldIssue {
            field: "password".into(),
            reason: format!("must be at least {MIN_PASSWORD_LEN} characters"),
        });
    }
    if !fields.is_empty() {
        return Err(AppError::Validation { fields });
    }

    let mut db = state.db.clone();
    if User::filter_by_email(&email).first().exec(&mut db).await?.is_some() {
        return Err(AppError::EmailTaken { email });
    }

    let password_hash = hash_password(&body.password)?;
    let user = toasty::create!(User {
        email,
        password_hash,
        display_name: body.display_name.trim().to_string(),
        created_at: now_rfc3339(),
    })
    .exec(&mut db)
    .await?;

    let (plain, token_hash) = new_session_token();
    let expires_at = (Utc::now() + Duration::days(SESSION_TTL_DAYS)).to_rfc3339();
    toasty::create!(Session {
        user_id: user.id,
        token_hash,
        created_at: now_rfc3339(),
        expires_at,
    })
    .exec(&mut db)
    .await?;

    Ok(Json(AuthResponse { token: plain, user }))
}

/// `POST /auth/login` → `{ token, user }`.
pub async fn login(
    State(state): State<Arc<AppState>>,
    AppJson(body): AppJson<LoginBody>,
) -> Result<Json<AuthResponse>, AppError> {
    let email = body.email.trim().to_lowercase();
    let mut db = state.db.clone();
    let user = match User::filter_by_email(&email).first().exec(&mut db).await? {
        Some(user) => user,
        // Verify against a fixed dummy hash so unknown emails cost the same
        // as a wrong password (no user-enumeration timing oracle).
        None => {
            let _ = verify_password(DUMMY_HASH, &body.password);
            return Err(AppError::InvalidCredentials);
        }
    };
    if !verify_password(&user.password_hash, &body.password) {
        return Err(AppError::InvalidCredentials);
    }

    let (plain, token_hash) = new_session_token();
    let expires_at = (Utc::now() + Duration::days(SESSION_TTL_DAYS)).to_rfc3339();
    toasty::create!(Session {
        user_id: user.id,
        token_hash,
        created_at: now_rfc3339(),
        expires_at,
    })
    .exec(&mut db)
    .await?;

    Ok(Json(AuthResponse { token: plain, user }))
}

/// `POST /auth/logout` → 204 (revokes the presented session).
pub async fn logout(State(state): State<Arc<AppState>>, AuthUser { session, .. }: AuthUser) -> Result<StatusCode, AppError> {
    let mut db = state.db.clone();
    Session::filter_by_id(session.id).delete().exec(&mut db).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /auth/me` → the current user.
pub async fn me(AuthUser { user, .. }: AuthUser) -> Json<User> {
    Json(user)
}

// ---------------------------------------------------------------------------
// Bearer extractor
// ---------------------------------------------------------------------------

/// The authenticated user + its session, resolved from the bearer token.
pub struct AuthUser {
    pub user: User,
    pub session: Session,
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::AuthHeaderMissing)?;
        let token = header
            .strip_prefix("Bearer ")
            .filter(|t| !t.is_empty())
            .ok_or(AppError::AuthHeaderMissing)?;

        let token_hash = sha256_hex(token.as_bytes());
        let mut db = state.db.clone();
        let session = Session::filter_by_token_hash(&token_hash)
            .first()
            .exec(&mut db)
            .await?
            .ok_or(AppError::AuthTokenInvalid)?;

        if expired(&session.expires_at) {
            return Err(AppError::AuthTokenExpired);
        }
        let user = User::filter_by_id(session.user_id)
            .first()
            .exec(&mut db)
            .await?
            .ok_or(AppError::AuthTokenInvalid)?;
        Ok(Self { user, session })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A fixed argon2 hash (of a random password) for the timing-equalisation
/// branch of [`login`].
const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHlzYWx0c2FsdHk$9WQ3HGlX0l0kP6JbYx8bM1mNf3gK4qWc";

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal { message: format!("failed to hash password: {e}") })
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

/// sha256 hex of `data` (used for token hashing).
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// A fresh session: returns `(plaintext, sha256-hex)`.
fn new_session_token() -> (String, String) {
    let bytes: [u8; 32] = rand::random();
    let plain = hex::encode(&bytes);
    let hash = sha256_hex(plain.as_bytes());
    (plain, hash)
}

/// RFC 3339 UTC now.
fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn expired(rfc3339: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| t < Utc::now())
        .unwrap_or(true)
}

fn valid_email(email: &str) -> bool {
    email.contains('@') && email.len() >= 3 && !email.starts_with('@') && !email.ends_with('@')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn password_hash_roundtrip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password(&hash, "correct horse battery staple"));
        assert!(!verify_password(&hash, "wrong password"));
    }

    #[test]
    fn token_is_opaque_and_hashed() {
        let (plain, hash) = new_session_token();
        assert_eq!(plain.len(), 64); // 32 bytes, hex
        assert_ne!(plain, hash);
        assert_eq!(sha256_hex(plain.as_bytes()), hash);
    }

    #[test]
    fn email_validation() {
        assert!(valid_email("a@b.co"));
        assert!(!valid_email("not-an-email"));
        assert!(!valid_email("@b.co"));
        assert!(!valid_email("a@"));
    }

    #[test]
    fn expiry_checks() {
        let past = (Utc::now() - Duration::days(1)).to_rfc3339();
        let future = (Utc::now() + Duration::days(1)).to_rfc3339();
        assert!(expired(&past));
        assert!(!expired(&future));
    }
}
