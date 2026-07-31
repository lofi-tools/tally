//! Companies House public API client.
//!
//! A minimal client for the Companies House API
//! (<https://developer.company-information.service.gov.uk/>), authenticated
//! with HTTP basic access authentication.
//!
//! The API key is sent as the basic-auth *username* and the password is left
//! blank, e.g. for an API key `my_api_key`:
//!
//! ```text
//! Authorization: Basic bXlfYXBpX2tleTo=
//! ```

use std::{env::VarError, result::Result};

use serde::Deserialize;
use thiserror::Error;

/// Errors returned by the Companies House API client.
#[derive(Debug, Error)]
pub enum CompaniesHouseError {
    /// The HTTP request could not be sent.
    #[error("request failed: {0}")]
    RequestFailed(reqwest::Error),

    /// The API returned a non-success status code.
    #[error("GET {url} returned HTTP {status}")]
    HttpStatus {
        url: String,
        status: reqwest::StatusCode,
    },

    /// The response body could not be decoded as JSON.
    #[error("failed to decode response: {0}")]
    DecodeFailed(reqwest::Error),
}

pub type ApiResult<T> = Result<T, CompaniesHouseError>;

/// A client for the Companies House public API.
///
/// All requests are authenticated with the API key using HTTP basic
/// authentication (username = API key, empty password).
#[derive(Debug, Clone)]
pub struct CompaniesHouseClient {
    base_url: &'static str,
    http: reqwest::Client,
    api_key: String,
}

impl CompaniesHouseClient {
    /// Create a new client using the given API key.
    pub fn test_client_from_env() -> Result<Self, VarError> {
        const API_BASE_URL_TEST: &str = "https://api-sandbox.company-information.service.gov.uk";
        Ok(Self {
            base_url: API_BASE_URL_TEST,
            http: reqwest::Client::new(),
            api_key: std::env::var("COMPANIES_HOUSE_API_KEY_TEST")?,
        })
    }
    pub fn live_from_env() -> Result<Self, VarError> {
        /// Base URL of the Companies House public API.
        const API_BASE_URL: &str = "https://api.company-information.service.gov.uk";

        Ok(Self {
            base_url: API_BASE_URL,
            http: reqwest::Client::new(),
            api_key: std::env::var("COMPANIES_HOUSE_API_KEY")?,
        })
    }

    /// Fetch the company profile for the given company number.
    ///
    /// `GET /company/{companyNumber}`
    pub async fn get_company_profile(&self, company_number: &str) -> ApiResult<CompanyProfile> {
        let url = format!("{}/company/{company_number}", self.base_url);
        let response = self
            .http
            .get(&url)
            .basic_auth(&self.api_key, Some("")) //  Companies House API takes the username as the API key
            .send()
            .await
            .map_err(CompaniesHouseError::RequestFailed)?;

        let status = response.status();
        if !status.is_success() {
            return Err(CompaniesHouseError::HttpStatus { url, status });
        }

        response
            .json::<CompanyProfile>()
            .await
            .map_err(CompaniesHouseError::DecodeFailed)
    }
}

/// Company profile returned by `GET /company/{companyNumber}`.
///
/// Only the commonly used fields are modelled; all optional fields are
/// tolerated when absent from the response.
#[derive(Debug, Clone, Deserialize)]
pub struct CompanyProfile {
    pub company_number: String,
    pub company_name: String,
    #[serde(default)]
    pub company_status: Option<String>,
    #[serde(default)]
    pub company_status_detail: Option<String>,
    #[serde(default)]
    pub date_of_creation: Option<String>,
    #[serde(default)]
    pub date_of_dissolution: Option<String>,
    #[serde(rename = "type", default)]
    pub company_type: Option<String>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub registered_office_address: Option<RegisteredOfficeAddress>,
    #[serde(default)]
    pub accounts: Option<Accounts>,
    #[serde(default)]
    pub confirmation_statement: Option<ConfirmationStatement>,
    #[serde(default)]
    pub sic_codes: Option<Vec<String>>,
    #[serde(default)]
    pub undeliverable_registered_office_address: Option<bool>,
    #[serde(default)]
    pub links: Option<CompanyLinks>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisteredOfficeAddress {
    #[serde(default)]
    pub address_line_1: Option<String>,
    #[serde(default)]
    pub address_line_2: Option<String>,
    #[serde(default)]
    pub care_of: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub locality: Option<String>,
    #[serde(default)]
    pub po_box: Option<String>,
    #[serde(default)]
    pub postal_code: Option<String>,
    #[serde(default)]
    pub premises: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Accounts {
    #[serde(default)]
    pub next_accounts: Option<NextAccounts>,
    #[serde(default)]
    pub next_due: Option<String>,
    #[serde(default)]
    pub overdue: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NextAccounts {
    #[serde(default)]
    pub period_end_on: Option<String>,
    #[serde(default)]
    pub period_start_on: Option<String>,
    #[serde(default)]
    pub due_on: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfirmationStatement {
    #[serde(default)]
    pub last_made_up_to: Option<String>,
    #[serde(default)]
    pub next_due: Option<String>,
    #[serde(default)]
    pub next_made_up_to: Option<String>,
    #[serde(default)]
    pub overdue: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompanyLinks {
    #[serde(default)]
    pub filing_history: Option<String>,
    #[serde(default)]
    pub officers: Option<String>,
    #[serde(default)]
    pub self_link: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EXT SOFTWARE SERVICES LTD, an active private limited company
    /// incorporated on 28 November 2022.
    const TEST_COMPANY_NUMBER: &str = "14510633";

    /// Live integration test against the Companies House API.
    ///
    /// Requires a valid API key in the `COMPANIES_HOUSE_API_KEY_TEST`
    /// environment variable and network access, so it is ignored by default:
    ///
    /// ```text
    /// COMPANIES_HOUSE_API_KEY_TEST=<key> cargo test --package ct600 -- --ignored
    /// ```
    #[tokio::test]
    // #[ignore = "requires the COMPANIES_HOUSE_API_KEY_TEST env var and network access"]
    async fn get_company_profile_14510633() {
        let client = CompaniesHouseClient::live_from_env()
            .expect("set COMPANIES_HOUSE_API_KEY_TEST to run this test");

        let profile = client
            .get_company_profile(TEST_COMPANY_NUMBER)
            .await
            .expect("fetching the company profile should succeed");

        assert_eq!(profile.company_number, "14510633");
        assert_eq!(profile.company_name, "EXT SOFTWARE SERVICES LTD");
        assert_eq!(profile.company_status.as_deref(), Some("active"));
        assert_eq!(profile.company_type.as_deref(), Some("ltd"));
        assert_eq!(profile.date_of_creation.as_deref(), Some("2022-11-28"));
    }
}
