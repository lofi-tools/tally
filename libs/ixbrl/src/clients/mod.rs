//! Company data clients.
//!
//! [`companies_house`] provides the Companies House API client, its layered
//! configuration ([`Config`]) and the company-resolution chain used to fill
//! in absent company details: a full company override first, then the cached
//! response for the configured company number, then a live API fetch.
//!
//! [`test_utils`] holds the offline test client ([`test_utils::TestClient`])
//! and the hardcoded company fixtures ([`test_utils::TestData`]) shared by
//! the crates that consume company data.

pub mod companies_house;
pub use companies_house::{
    ApiResult, CompaniesHouseClient, CompaniesHouseClientType, CompaniesHouseError, CompanyProfile,
    CompanyType, Config,
};

/// Test fixtures and offline test clients for the company clients.
pub mod test_utils;
