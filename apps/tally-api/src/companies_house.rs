//! Thin adapter over `ct600::CompaniesHouseClient` (spec §8).
//!
//! - A key is **required** for every CH-backed endpoint (`COMPANIES_HOUSE_API_KEY`
//!   for live, `COMPANIES_HOUSE_SANDBOX_API_KEY` for sandbox; live wins).
//!   Without one the endpoints return 400 `companies_house_key_missing` —
//!   no fixture fallback in the API.
//! - Caching goes through the client's `get_company_profile_cached`, using
//!   `CT600_CACHE_DIR` when set (exactly like the CLI).
//! - The lib has no company *search* method, so [`ChApi::search`] calls the
//!   CH `/search/companies` endpoint directly (HTTP basic auth, the key as
//!   the username — the same scheme the lib's client uses).
//! - All lib errors map into the §11.2 `companies_house_*` variants.

use std::path::PathBuf;

use ct600::companies_house::OfficerList;
use ct600::{CompaniesHouseClient, CompaniesHouseError, CompanyProfile};
use serde::Deserialize;

use crate::error::AppError;

/// A configured CH API handle. Construct via [`ChApi::from_env`]; `None`
/// when no key is configured.
pub struct ChApi {
    client: CompaniesHouseClient,
    key: String,
    base_url: String,
    http: reqwest::Client,
}

/// The human hint for the key-missing error.
pub fn key_missing_hint() -> &'static str {
    "set COMPANIES_HOUSE_API_KEY (live) or COMPANIES_HOUSE_SANDBOX_API_KEY (sandbox)"
}

impl ChApi {
    /// Build from the environment; `None` when no key is set (never fails —
    /// a missing key is a per-request 400, not a boot error).
    pub fn from_env() -> Option<Self> {
        let live = non_empty_env("COMPANIES_HOUSE_API_KEY");
        let sandbox = non_empty_env("COMPANIES_HOUSE_SANDBOX_API_KEY");
        let (key, sandbox) = match (live, sandbox) {
            (Some(k), _) => (k, false),
            (None, Some(k)) => (k, true),
            (None, None) => return None,
        };
        let cache_dir = non_empty_env("CT600_CACHE_DIR").map(PathBuf::from);
        let mut config = ct600::Config::default().with_api(key.clone(), sandbox);
        if let Some(dir) = cache_dir {
            config = config.with_cache_dir(dir);
        }
        let client = CompaniesHouseClient::new(config);
        Some(Self {
            base_url: client.config().base_url().to_string(),
            client,
            key,
            http: reqwest::Client::new(),
        })
    }

    /// Fetch (cache-first) the company profile.
    pub async fn profile(&self, company_number: &str) -> Result<CompanyProfile, AppError> {
        self.client
            .get_company_profile_cached(company_number)
            .await
            .map_err(|e| map_ch_error(e, company_number, self.base_url.clone()))
    }

    /// Fetch the officers (used by enrich for the directors).
    pub async fn officers(&self, company_number: &str) -> Result<OfficerList, AppError> {
        self.client
            .get_officers(company_number)
            .await
            .map_err(|e| map_ch_error(e, company_number, self.base_url.clone()))
    }

    /// Fetch a company's complete filing history (all pages merged, newest
    /// first). Unused cache — the filings sync persists its own copy.
    pub async fn filing_history_all(
        &self,
        company_number: &str,
    ) -> Result<ct600::companies_house::FilingHistory, AppError> {
        self.client
            .get_filing_history_all(company_number)
            .await
            .map_err(|e| map_ch_error(e, company_number, self.base_url.clone()))
    }

    /// Download a filed document (resolving the metadata link to its
    /// content URL on the document API host). Returns the raw bytes; the
    /// caller interprets them (HTML iXBRL, zipped iXBRL, PDF, ...).
    pub async fn filing_document(&self, document_metadata_url: &str) -> Result<Vec<u8>, AppError> {
        self.client
            .get_filing_document(document_metadata_url)
            .await
            .map_err(|e| map_ch_error(e, "", self.base_url.clone()))
    }

    /// Search companies by name/number. The lib has no search method, so
    /// this calls `GET {base}/search/companies?q=…` directly.
    pub async fn search(&self, query: &str) -> Result<Vec<SearchItem>, AppError> {
        let url = format!("{}/search/companies?q={}", self.base_url, urlencode(query));
        let response = self
            .http
            .get(&url)
            .basic_auth(&self.key, Some(""))
            .send()
            .await
            .map_err(|_| AppError::CompaniesHouseUpstream {
                url: url.clone(),
                upstream_status: None,
            })?;
        match response.status().as_u16() {
            200 => {
                let body: SearchResponse = response
                    .json()
                    .await
                    .map_err(|_| AppError::CompaniesHouseUpstream { url, upstream_status: Some(200) })?;
                Ok(body.items)
            }
            404 => Err(AppError::CompaniesHouseNotFound { company_number: query.to_string() }),
            429 => Err(AppError::CompaniesHouseRateLimited {
                retry_after: response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string),
            }),
            status => Err(AppError::CompaniesHouseUpstream { url, upstream_status: Some(status) }),
        }
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Map a lib `CompaniesHouseError` to the §11.2 variants.
fn map_ch_error(e: CompaniesHouseError, company_number: &str, url: String) -> AppError {
    match e {
        CompaniesHouseError::HttpStatus { status, .. } => match status.as_u16() {
            404 => AppError::CompaniesHouseNotFound { company_number: company_number.to_string() },
            429 => AppError::CompaniesHouseRateLimited { retry_after: None },
            other => AppError::CompaniesHouseUpstream { url, upstream_status: Some(other) },
        },
        CompaniesHouseError::MissingCompanyNumber => AppError::MissingCompanyNumber,
        _ => AppError::CompaniesHouseUpstream { url, upstream_status: None },
    }
}

/// The CH search response (only the fields the API surfaces).
#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    items: Vec<SearchItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CH search endpoint puts the company name in `title` (not
    /// `company_name`) — a regression test so decoding a real response
    /// can't silently break again (the whole search then 502s).
    #[test]
    fn search_item_decodes_from_real_ch_shape() {
        let json = r#"{
            "items": [{
                "company_number": "00445790",
                "company_status": "active",
                "company_type": "plc",
                "date_of_creation": "1947-11-27",
                "address_snippet": "Tesco House, Shire Park, Kestrel Way, Welwyn Garden City",
                "title": "TESCO PLC",
                "description": "00445790 - Incorporated on 27 November 1947"
            }]
        }"#;
        let parsed: SearchResponse = serde_json::from_str(json).expect("real CH search response decodes");
        assert_eq!(parsed.items.len(), 1);
        let item = &parsed.items[0];
        assert_eq!(item.company_name, "TESCO PLC");
        assert_eq!(item.company_number, "00445790");
    }
}

/// A CH search result.
///
/// Note: the search endpoint puts the company name in `title` (not
/// `company_name`), so `company_name` maps to it.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct SearchItem {
    #[serde(rename = "company_number")]
    pub company_number: String,
    /// CH's search endpoint calls the name `title`; the API contract
    /// serializes it as `company_name` (alias only affects decoding).
    #[serde(alias = "title")]
    pub company_name: String,
    #[serde(default, rename = "company_status")]
    pub company_status: Option<String>,
    #[serde(default, rename = "date_of_creation")]
    pub date_of_creation: Option<String>,
    #[serde(default, rename = "address_snippet")]
    pub address_snippet: Option<String>,
    #[serde(default, rename = "company_type")]
    pub company_type: Option<String>,
    #[serde(default, rename = "description")]
    pub description: Option<String>,
}
