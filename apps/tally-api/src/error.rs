//! The API error contract (spec §11).
//!
//! Every non-2xx response is the envelope
//! `{ "error": { "code", "message", "details"? } }`.  The `code` strings are
//! stable, machine-readable identifiers (clients branch on them); `message`
//! is UI-safe; `details` is a free-form object, omitted when empty.
//!
//! `AppError` is a **snafu** enum, like the libs' own error types
//! (`GnucashError`, `CompaniesHouseError`, …).  `impl IntoResponse for
//! AppError` is the single place where status/code mapping lives.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
// `pub` so modules that derive `Snafu` on their own enums can import it
// through here (`auth.rs` does `use crate::error::Snafu as _`).
pub use snafu::Snafu;

// Per-request id, scoped by middleware into the request task; 5xx
// `details.request_id` echoes it (see `app.rs`).
tokio::task_local! {
    pub static REQUEST_ID: String;
}

/// One field-level validation issue (`details.fields` of `Validation`).
#[derive(Debug, Clone, Serialize)]
pub struct FieldIssue {
    pub field: String,
    pub reason: String,
}

/// The API error type.  Every variant maps to exactly one HTTP status and
/// one `code` (spec §11.2).
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum AppError {
    // -- auth ----------------------------------------------------------------
    #[snafu(display("missing bearer token"))]
    AuthHeaderMissing,
    #[snafu(display("invalid bearer token"))]
    AuthTokenInvalid,
    #[snafu(display("session has expired"))]
    AuthTokenExpired,
    #[snafu(display("an account with email '{email}' already exists"))]
    EmailTaken { email: String },
    #[snafu(display("invalid email or password"))]
    InvalidCredentials,
    #[snafu(display("missing or blank X-Guest-Id header"))]
    GuestIdRequired,
    #[snafu(display("no guest workspace exists for this guest id"))]
    GuestNotFound,
    #[snafu(display("this guest workspace has already been adopted by an account"))]
    GuestAlreadyAdopted,

    // -- request -------------------------------------------------------------
    #[snafu(display("invalid JSON body: {message}"))]
    InvalidJson { message: String, line: Option<u64>, column: Option<u64> },
    #[snafu(display("validation failed"))]
    Validation { fields: Vec<FieldIssue> },
    #[snafu(display("unsupported file type"))]
    UnsupportedFileType { expected: &'static str, got: String },
    #[snafu(display("file too large"))]
    FileTooLarge { limit_bytes: u64 },
    #[snafu(display("multipart error: {message}"))]
    Multipart { message: String },

    // -- resources -----------------------------------------------------------
    #[snafu(display("{resource} '{id}' not found"))]
    NotFound { resource: &'static str, id: String },
    #[snafu(display("ledger '{ledger_id}' does not belong to company '{company_id}'"))]
    CompanyLedgerMismatch { company_id: String, ledger_id: String },
    #[snafu(display("company '{company_number}' already exists"))]
    DuplicateCompany { company_number: String },
    #[snafu(display("could not determine the accounting period"))]
    PeriodNotDetermined { hint: String },
    #[snafu(display("no company number is configured for the Companies House lookup"))]
    MissingCompanyNumber,

    // -- external services ---------------------------------------------------
    #[snafu(display("no Companies House API key is configured"))]
    CompaniesHouseKeyMissing { hint: String },
    #[snafu(display("company '{company_number}' was not found at Companies House"))]
    CompaniesHouseNotFound { company_number: String },
    #[snafu(display("Companies House rate limited the request"))]
    CompaniesHouseRateLimited { retry_after: Option<String> },
    #[snafu(display("Companies House request failed"))]
    CompaniesHouseUpstream { url: String, upstream_status: Option<u16> },

    // -- parse / storage -----------------------------------------------------
    #[snafu(display("failed to parse the GnuCash ledger"))]
    LedgerParse { source: ixbrl::GnucashError },
    #[snafu(display("failed to store the uploaded file"))]
    Storage { source: std::io::Error },

    // -- internal ------------------------------------------------------------
    #[snafu(display("database error: {source}"))]
    Db { source: toasty::Error },
    #[snafu(display("{message}"))]
    Internal { message: String },
}

impl AppError {
    /// `(status, code, message, details)` for the variant (the unit-test
    /// table in `tests` asserts this mapping end to end).
    pub fn parts(&self) -> (StatusCode, &'static str, String, Option<serde_json::Value>) {
        use serde_json::json;
        match self {
            Self::AuthHeaderMissing => (StatusCode::UNAUTHORIZED, "auth_missing", self.to_string(), None),
            Self::AuthTokenInvalid => (StatusCode::UNAUTHORIZED, "auth_invalid", self.to_string(), None),
            Self::AuthTokenExpired => (StatusCode::UNAUTHORIZED, "auth_expired", self.to_string(), None),
            Self::EmailTaken { email } => (
                StatusCode::CONFLICT,
                "email_taken",
                self.to_string(),
                Some(json!({ "email": email })),
            ),
            Self::InvalidCredentials => (StatusCode::UNAUTHORIZED, "invalid_credentials", self.to_string(), None),
            Self::GuestIdRequired => (StatusCode::BAD_REQUEST, "guest_id_required", self.to_string(), None),
            Self::GuestNotFound => (StatusCode::NOT_FOUND, "guest_not_found", self.to_string(), None),
            Self::GuestAlreadyAdopted => (StatusCode::BAD_REQUEST, "guest_already_adopted", self.to_string(), None),
            Self::InvalidJson { message, line, column } => {
                let mut details = json!({ "message": message });
                if let (Some(line), Some(column)) = (line, column) {
                    details["line"] = json!(line);
                    details["column"] = json!(column);
                }
                (StatusCode::BAD_REQUEST, "invalid_json", self.to_string(), Some(details))
            }
            Self::Validation { fields } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                self.to_string(),
                Some(json!({ "fields": fields })),
            ),
            Self::UnsupportedFileType { expected, got } => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_file_type",
                self.to_string(),
                Some(json!({ "expected": expected, "got": got })),
            ),
            Self::FileTooLarge { limit_bytes } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "file_too_large",
                self.to_string(),
                Some(json!({ "limit_bytes": limit_bytes })),
            ),
            Self::Multipart { message } => (
                StatusCode::BAD_REQUEST,
                "multipart_error",
                self.to_string(),
                Some(json!({ "message": message })),
            ),
            Self::NotFound { resource, id } => (
                StatusCode::NOT_FOUND,
                "not_found",
                self.to_string(),
                Some(json!({ "resource": resource, "id": id })),
            ),
            Self::CompanyLedgerMismatch { company_id, ledger_id } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "ledger_not_in_company",
                self.to_string(),
                Some(json!({ "company_id": company_id, "ledger_id": ledger_id })),
            ),
            Self::DuplicateCompany { company_number } => (
                StatusCode::CONFLICT,
                "duplicate_company",
                self.to_string(),
                Some(json!({ "company_number": company_number })),
            ),
            Self::PeriodNotDetermined { hint } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "period_not_determined",
                self.to_string(),
                Some(json!({ "hint": hint })),
            ),
            Self::MissingCompanyNumber => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "missing_company_number",
                self.to_string(),
                None,
            ),
            Self::CompaniesHouseKeyMissing { hint } => (
                StatusCode::BAD_REQUEST,
                "companies_house_key_missing",
                self.to_string(),
                Some(json!({ "hint": hint })),
            ),
            Self::CompaniesHouseNotFound { company_number } => (
                StatusCode::NOT_FOUND,
                "company_not_found",
                self.to_string(),
                Some(json!({ "company_number": company_number })),
            ),
            Self::CompaniesHouseRateLimited { retry_after } => (
                StatusCode::TOO_MANY_REQUESTS,
                "companies_house_rate_limited",
                self.to_string(),
                Some(json!({ "retry_after": retry_after })),
            ),
            Self::CompaniesHouseUpstream { url, upstream_status } => (
                StatusCode::BAD_GATEWAY,
                "companies_house_upstream",
                self.to_string(),
                Some(json!({ "url": url, "upstream_status": upstream_status })),
            ),
            Self::LedgerParse { .. } => (StatusCode::UNPROCESSABLE_ENTITY, "ledger_parse_failed", self.to_string(), None),
            Self::Storage { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "storage_error", self.to_string(), None),
            Self::Db { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal server error".into(), None),
            // The detail stays server-side: log it (the caller has already
            // done so with `tracing::error!`), never echo it to the client.
            Self::Internal { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal server error".into(), None),
        }
    }

    /// A single-field `Validation` error.
    pub fn field(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Validation {
            fields: vec![FieldIssue {
                field: field.into(),
                reason: reason.into(),
            }],
        }
    }

    /// An `invalid_json` error from an axum rejection's body text.  axum 0.8
    /// rejections only expose `body_text()` (no inner serde error), and the
    /// message embeds the position as "… at line X column Y" — parse that.
    pub fn invalid_json(message: String) -> Self {
        let (line, column) = parse_serde_position(&message);
        Self::InvalidJson { message, line, column }
    }
}

/// Extract the trailing `at line X column Y` from a serde failure message.
fn parse_serde_position(message: &str) -> (Option<u64>, Option<u64>) {
    let Some((_, tail)) = message.rsplit_once(" at line ") else {
        return (None, None);
    };
    let Some((line, column)) = tail.split_once(" column ") else {
        return (None, None);
    };
    (line.trim().parse().ok(), column.trim().parse().ok())
}

/// The response envelope (§11.1).
#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // `Internal { message }` never reaches the client; log it so the
        // detail isn't lost.
        if let Self::Internal { message } = &self {
            tracing::error!(message = %message, "internal error");
        }
        let (status, code, message, mut details) = self.parts();

        // 5xx responses echo the request id; nothing internal ever leaks.
        if status.is_server_error() {
            let request_id = REQUEST_ID.try_with(|id| id.clone()).unwrap_or_default();
            let d = details.get_or_insert_with(|| serde_json::json!({}));
            if let Some(obj) = d.as_object_mut() {
                obj.insert("request_id".into(), serde_json::Value::String(request_id));
            }
        }

        let mut response = (status, Json(ErrorEnvelope { error: ErrorBody { code, message, details } })).into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert("WWW-Authenticate", axum::http::HeaderValue::from_static("Bearer"));
        }
        response
    }
}

// ---------------------------------------------------------------------------
// Axum extractor rejections → AppError.  axum 0.8 renders an extractor's
// own rejection (never converting to the handler error type), so these
// impls are consumed by the wrapper extractors in `extract.rs`, whose
// rejection type is `AppError` — every rejection then renders this
// envelope instead of axum's default text/plain response.
// ---------------------------------------------------------------------------

impl From<axum::extract::rejection::JsonRejection> for AppError {
    fn from(rejection: axum::extract::rejection::JsonRejection) -> Self {
        use axum::extract::rejection::JsonRejection;
        match rejection {
            // Real serde parse failures carry the position (400 invalid_json).
            JsonRejection::JsonDataError(e) => Self::invalid_json(e.body_text()),
            JsonRejection::JsonSyntaxError(e) => Self::invalid_json(e.body_text()),
            // Missing content-type / body too large: a field-level 422.
            other => Self::Validation {
                fields: vec![FieldIssue { field: "body".into(), reason: other.body_text() }],
            },
        }
    }
}

impl From<axum::extract::rejection::QueryRejection> for AppError {
    fn from(rejection: axum::extract::rejection::QueryRejection) -> Self {
        Self::Validation {
            fields: vec![FieldIssue { field: "query".into(), reason: rejection.body_text() }],
        }
    }
}

impl From<axum::extract::rejection::PathRejection> for AppError {
    fn from(rejection: axum::extract::rejection::PathRejection) -> Self {
        Self::Validation {
            fields: vec![FieldIssue { field: "path".into(), reason: rejection.body_text() }],
        }
    }
}

impl From<axum::extract::multipart::MultipartRejection> for AppError {
    fn from(rejection: axum::extract::multipart::MultipartRejection) -> Self {
        Self::Multipart { message: rejection.body_text() }
    }
}

impl From<toasty::Error> for AppError {
    fn from(source: toasty::Error) -> Self {
        Self::Db { source }
    }
}

/// A raw framework-level error envelope (router fallback: `route_not_found`
/// / `method_not_allowed`, §11.3) — not an `AppError` variant.
pub fn plain_error(status: StatusCode, code: &'static str, message: &str) -> Response {
    let details = if status.is_server_error() {
        let request_id = REQUEST_ID.try_with(|id| id.clone()).unwrap_or_default();
        Some(serde_json::json!({ "request_id": request_id }))
    } else {
        None
    };
    (status, Json(ErrorEnvelope { error: ErrorBody { code, message: message.into(), details } })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The §11.2 table, minus `Db` (whose `toasty::Error` cannot be
    /// constructed offline; the integration suite covers it) and the
    /// source-bearing variants' details (covered by construction above).
    #[test]
    fn variant_status_and_code_table() {
        let cases: Vec<(AppError, StatusCode, &'static str)> = vec![
            (AppError::AuthHeaderMissing, StatusCode::UNAUTHORIZED, "auth_missing"),
            (AppError::AuthTokenInvalid, StatusCode::UNAUTHORIZED, "auth_invalid"),
            (AppError::AuthTokenExpired, StatusCode::UNAUTHORIZED, "auth_expired"),
            (AppError::EmailTaken { email: "a@b.c".into() }, StatusCode::CONFLICT, "email_taken"),
            (AppError::InvalidCredentials, StatusCode::UNAUTHORIZED, "invalid_credentials"),
            (AppError::GuestIdRequired, StatusCode::BAD_REQUEST, "guest_id_required"),
            (AppError::GuestNotFound, StatusCode::NOT_FOUND, "guest_not_found"),
            (AppError::GuestAlreadyAdopted, StatusCode::BAD_REQUEST, "guest_already_adopted"),
            (
                AppError::invalid_json("expected value at line 1 column 2".into()),
                StatusCode::BAD_REQUEST,
                "invalid_json",
            ),
            (AppError::Validation { fields: vec![] }, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed"),
            (
                AppError::UnsupportedFileType { expected: ".gnucash", got: "x.csv".into() },
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_file_type",
            ),
            (AppError::FileTooLarge { limit_bytes: 50 }, StatusCode::PAYLOAD_TOO_LARGE, "file_too_large"),
            (AppError::Multipart { message: "boom".into() }, StatusCode::BAD_REQUEST, "multipart_error"),
            (
                AppError::NotFound { resource: "company", id: "1".into() },
                StatusCode::NOT_FOUND,
                "not_found",
            ),
            (
                AppError::CompanyLedgerMismatch { company_id: "1".into(), ledger_id: "2".into() },
                StatusCode::UNPROCESSABLE_ENTITY,
                "ledger_not_in_company",
            ),
            (
                AppError::DuplicateCompany { company_number: "123".into() },
                StatusCode::CONFLICT,
                "duplicate_company",
            ),
            (
                AppError::PeriodNotDetermined { hint: "pass a period".into() },
                StatusCode::UNPROCESSABLE_ENTITY,
                "period_not_determined",
            ),
            (AppError::MissingCompanyNumber, StatusCode::UNPROCESSABLE_ENTITY, "missing_company_number"),
            (
                AppError::CompaniesHouseKeyMissing { hint: "set a key".into() },
                StatusCode::BAD_REQUEST,
                "companies_house_key_missing",
            ),
            (
                AppError::CompaniesHouseNotFound { company_number: "1".into() },
                StatusCode::NOT_FOUND,
                "company_not_found",
            ),
            (AppError::CompaniesHouseRateLimited { retry_after: None }, StatusCode::TOO_MANY_REQUESTS, "companies_house_rate_limited"),
            (
                AppError::CompaniesHouseUpstream { url: "https://x".into(), upstream_status: Some(500) },
                StatusCode::BAD_GATEWAY,
                "companies_house_upstream",
            ),
            (
                AppError::LedgerParse {
                    source: ixbrl::GnucashError::Io {
                        source: std::io::Error::new(std::io::ErrorKind::InvalidData, "nope"),
                    },
                },
                StatusCode::UNPROCESSABLE_ENTITY,
                "ledger_parse_failed",
            ),
            (
                AppError::Storage { source: std::io::Error::new(std::io::ErrorKind::Other, "disk") },
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage_error",
            ),
            (AppError::Internal { message: "boom".into() }, StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        ];
        for (err, status, code) in cases {
            let (got_status, got_code, _, _) = err.parts();
            assert_eq!(got_status, status, "status for {code}");
            assert_eq!(got_code, code, "code");
        }
    }

    /// 5xx responses carry a request id in `details.request_id`.
    #[tokio::test]
    async fn server_error_echoes_request_id() {
        let response = REQUEST_ID
            .scope("req-123".to_string(), async {
                // `Storage` hides its `std::io::Error` source in the message.
                AppError::Storage {
                    source: std::io::Error::new(std::io::ErrorKind::Other, "boom"),
                }
                .into_response()
            })
            .await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = axum::body::to_bytes(response.into_body(), 1024 * 16).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["details"]["request_id"], "req-123");
        // And nothing internal leaks (the io source stays hidden).
        assert_ne!(json["error"]["message"], "boom");
    }
}
