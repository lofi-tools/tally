//! `AppError`-shaped extractor wrappers.
//!
//! axum 0.8's handler machinery renders an extractor's **own** rejection via
//! `Rejection::into_response()` — it never converts the rejection into the
//! handler's error type.  A bare `Json<T>` rejection would therefore produce
//! axum's default text/plain 400 instead of the §11 JSON envelope, and the
//! `From<…Rejection> for AppError` impls in `error.rs` would be dead code.
//!
//! These wrappers use `AppError` as their rejection type, so every rejection
//! flows through [`AppError::into_response`] and the spec's
//! `{ error: { code, message, details } }` envelope.  Handlers destructure
//! them exactly like the plain extractors (`AppJson(input)`,
//! `AppPath(id)`, `AppQuery(q)`, `AppMultipart(mut mp)`).

use axum::extract::{FromRequest, FromRequestParts, Multipart, Path, Query};
use axum::http::request::Parts;
use axum::Json;
use serde::de::DeserializeOwned;

use crate::error::AppError;

/// `Json<T>` whose rejection is [`AppError`].
pub struct AppJson<T>(pub T);

impl<T, S> FromRequest<S> for AppJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(req, state)
            .await
            .map_err(|e| AppError::from(e))
            .map(|w| Self(w.0))
    }
}

/// `Path<T>` whose rejection is [`AppError`].
pub struct AppPath<T>(pub T);

impl<T, S> FromRequestParts<S> for AppPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(|e| AppError::from(e))
            .map(|w| Self(w.0))
    }
}

/// `Query<T>` whose rejection is [`AppError`].
pub struct AppQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for AppQuery<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|e| AppError::from(e))
            .map(|w| Self(w.0))
    }
}

/// `Multipart` whose rejection is [`AppError`].
pub struct AppMultipart(pub Multipart);

impl<S> FromRequest<S> for AppMultipart
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        Multipart::from_request(req, state)
            .await
            .map_err(|e| AppError::from(e))
            .map(Self)
    }
}
