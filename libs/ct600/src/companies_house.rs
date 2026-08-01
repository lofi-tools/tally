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

use serde::{Deserialize, Serialize};

use crate::form::CompanyFormValues;
use ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax;

/// Errors returned by the Companies House API client.
#[derive(Debug, thiserror::Error)]
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

    /// A client pointed at an unreachable address with a placeholder API
    /// key, for tests that must never reach the network.
    #[cfg(test)]
    pub(crate) fn offline() -> Self {
        Self {
            base_url: "http://127.0.0.1:1",
            http: reqwest::Client::new(),
            api_key: "unused".to_string(),
        }
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

    /// Fetch the company profile for the tax computation's company number and
    /// combine it with the tax-derived values into the company header boxes
    /// (1, 2, 3, 4, 30 and 35).
    ///
    /// Companies House supplies boxes 1 (name), 2 (registration number) and 4
    /// (type of company, mapped from the register's company type); the tax
    /// reference (3) and the return period (30/35) come from the tax
    /// computation.
    pub async fn company_form_values(&self, tax: &Frs105CorpTax) -> ApiResult<CompanyFormValues> {
        let profile = self.get_company_profile(tax.company_number()).await?;
        Ok(CompanyFormValues::from_profile_and_tax(&profile, tax))
    }
}

impl CompanyFormValues {
    /// Build the CT600 company header boxes from a Companies House profile
    /// and the tax computation.
    pub(crate) fn from_profile_and_tax(
        profile: &CompanyProfile,
        tax: &Frs105CorpTax,
    ) -> Self {
        Self {
            company_name: profile.company_name.clone(),
            company_number: profile.company_number.clone(),
            tax_reference: tax.tax_reference().to_string(),
            type_of_company: profile
                .company_type
                .as_deref()
                .and_then(CompanyType::parse_str)
                .map(CompanyType::code)
                .unwrap_or(0),
            start: tax.start(),
            end: tax.end(),
        }
    }
}

/// The CT600 box 4 "Type of company" options, as defined in the HMRC CT600
/// guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanyType {
    PrivateCompanyLimitedByShares,
    PrivateCompanyLimitedByGuarantee,
    PrivateUnlimitedCompany,
    PublicLimitedCompany,
    OldPublicCompany,
    PrivateCompanyLimitedBySharesExempt,
    LimitedLiabilityPartnership,
}

impl CompanyType {
    /// Parse a Companies House company type string into a [`CompanyType`].
    ///
    /// Returns `None` for types not recognised.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "ltd" => Some(Self::PrivateCompanyLimitedByShares),
            "private-limited-guarant-nsc" => Some(Self::PrivateCompanyLimitedByGuarantee),
            "private-unlimited" | "unltd" => Some(Self::PrivateUnlimitedCompany),
            "plc" => Some(Self::PublicLimitedCompany),
            "old-public-company" => Some(Self::OldPublicCompany),
            "private-limited-shares-section-30-exemption" => {
                Some(Self::PrivateCompanyLimitedBySharesExempt)
            }
            "llp" => Some(Self::LimitedLiabilityPartnership),
            _ => None,
        }
    }

    /// The CT600 box 4 code for this company type.
    pub fn code(self) -> u8 {
        match self {
            Self::PrivateCompanyLimitedByShares => 1,
            Self::PrivateCompanyLimitedByGuarantee => 2,
            Self::PrivateUnlimitedCompany => 3,
            Self::PublicLimitedCompany => 4,
            Self::OldPublicCompany => 5,
            Self::PrivateCompanyLimitedBySharesExempt => 6,
            Self::LimitedLiabilityPartnership => 9,
        }
    }
}

/// Company profile returned by `GET /company/{companyNumber}`.
///
/// Only the commonly used fields are modelled; all optional fields are
/// tolerated when absent from the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Accounts {
    #[serde(default)]
    pub next_accounts: Option<NextAccounts>,
    #[serde(default)]
    pub next_due: Option<String>,
    #[serde(default)]
    pub overdue: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextAccounts {
    #[serde(default)]
    pub period_end_on: Option<String>,
    #[serde(default)]
    pub period_start_on: Option<String>,
    #[serde(default)]
    pub due_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyLinks {
    #[serde(default)]
    pub filing_history: Option<String>,
    #[serde(default)]
    pub officers: Option<String>,
    #[serde(default)]
    pub self_link: Option<String>,
}
