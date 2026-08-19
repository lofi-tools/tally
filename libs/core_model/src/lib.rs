//! Core domain model shared by the reporting and client crates.
//!
//! [`company`] holds the company identity and the accounting-period model
//! ([`company::Company`], [`company::CompanyProfile`],
//! [`company::AccountingPeriod`], [`company::AccountsMeta`]) — the domain
//! types the accounts/tax reports and the Companies House
//! next-accounting-period chain are built on.  [`PreviousPeriodFigures`] is
//! the filed-balance-sheet shape: produced by the Companies House client's
//! filing parse and consumed by the accounts report's comparative column.
//!
//! This crate is a leaf: no dependency on the reporting or client crates.

pub mod company;
pub use company::{
    AccountingPeriod, AccountsMeta, Company, CompanyProfile, EmployeePeriod, Employees,
};

/// A filed balance sheet's figures for one period: the previous-period
/// comparative column of the micro-entity accounts (FRS 105) report.
///
/// Produced by the Companies House client's `parse_filed_accounts` (the
/// `figures` of a filed accounts document) and consumed by the accounts
/// report's previous-period comparative column (and its
/// `check_previous_period_matches_filing` validation).  Values in whole
/// pounds with the iXBRL sign convention (creditor lines negative).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PreviousPeriodFigures {
    /// Tangible / fixed assets.
    pub fixed_assets: f64,
    /// Called-up share capital not paid — line A of the FRS 105
    /// balance-sheet format (above fixed assets), when the filed accounts
    /// disclosed it separately.
    pub called_up_share_capital_not_paid: f64,
    /// Current assets (debtors + VAT refund due + bank).
    pub current_assets: f64,
    /// Prepayments and accrued income.
    pub prepayments_and_accrued_income: f64,
    /// Creditors: amounts falling due within one year.
    pub creditors_within_1_year: f64,
    /// Net current assets / (liabilities).
    pub net_current_assets: f64,
    /// Total assets less current liabilities.
    pub total_assets_less_liabilities: f64,
    /// Creditors: amounts falling due after one year.
    pub creditors_after_1_year: f64,
    /// Provisions for liabilities.
    pub provisions_for_liabilities: f64,
    /// Accrued liabilities and deferred income.
    pub accruals_and_deferred_income: f64,
    /// Net assets.
    pub net_assets: f64,
    /// Capital and reserves.
    pub capital_and_reserves: f64,
}
