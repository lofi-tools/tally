//! Unaudited micro-entity accounts (FRS 105).
//!
//! Maps a [`GnucashBook`] (the ledger) plus company details (a [`Company`],
//! a [`CompanyProfile`] and an [`AccountsMeta`]) to the
//! "Unaudited Micro-Entity Accounts" iXBRL document: a title page, a
//! company-information page, the statement of financial position and the
//! notes to the accounts.
//!
//! The computations (balance-sheet lines, e.g. fixed assets, debtors, bank,
//! creditors) follow the reference `ixbrl-reporter` semantics:
//!
//! * a `line` computation sums the splits on an account (and its children)
//!   up to the balance-sheet date, negating the sum for debit-side account
//!   types (`INCOME`, `EQUITY`, `EXPENSE` — matching GnuCash's sign
//!   convention where income and equity are stored negative);
//! * `sum` / `group` computations add their inputs;
//! * values are rounded to whole pounds (decimals = 0) at render time.
//!
//! For the example company (`example_data/basic-1/input.gnucash`) the
//! rendered output matches the reference fixture
//! `example_data/basic-1/output-accounts.html` byte for byte (after
//! stripping the reference's random element ids).

use std::collections::HashMap;

use chrono::Datelike;

use companies_house::FiledBalanceSheet;
use core_model::{AccountingPeriod, AccountsMeta, Company, CompanyProfile};
use ixbrl_ir::ixbrl_fmt::*;
use snafu::Snafu;
use crate::{AdjustmentTransaction, GnucashBook};

/// The report title written into the generated document (the title page and
/// the hidden `uk-bus:ReportTitle` fact).  Auto-generated here — the config
/// file no longer carries a `report_title`.
const REPORT_TITLE: &str = "Unaudited Micro-Entity Accounts";

/// The bundled default director's signature (base64 PNG), embedded on the
/// statement of financial position when no `signature_b64` is supplied —
/// the same asset the example company's config carries, kept as a file
/// under `example_data/`.  Override with [`Frs105Accounts::with_signature`].
const DEFAULT_SIGNATURE_B64: &str = include_str!("../../../../example_data/default_signature.b64");

/// `value` when non-empty, else `default` — used to fill the JFCVC-mandatory
/// taxonomy facts that this report always emits even when the profile leaves
/// the corresponding field blank.
fn non_empty_or<'a>(value: &'a str, default: &'a str) -> &'a str {
    if value.is_empty() {
        default
    } else {
        value
    }
}

/// The fallback text for `DescriptionPrincipalActivities` when the profile
/// leaves it blank — the same placeholder the real 14510633 filing uses
/// (Companies House's validator requires actual text on this fact, an empty
/// tag is reported as "missing").  Parsed back as blank (`None`).
const DEFAULT_PRINCIPAL_ACTIVITIES: &str = "No description of principal activity";

/// Previous-period balance-sheet figures — the comparative column of the
/// statement of financial position when the ledger doesn't cover the
/// previous period (e.g. sourced from the company's last filed accounts at
/// Companies House).  Mirrors the `Frs105Accounts` balance-sheet fields;
/// values in whole pounds with the iXBRL sign convention (creditor lines
/// negative).
pub use core_model::PreviousPeriodFigures;

/// The unaudited micro-entity accounts (FRS 105) statement of financial
/// position.
///
/// Each balance-sheet value is stored as `[current_period, previous_period]`
/// in whole-pence precision; rounding to whole pounds happens at render time.
#[derive(Debug, Clone)]
pub struct Frs105Accounts {
    /// The company the accounts are prepared for.
    pub company: Company,
    /// The company's descriptive profile (directors, contacts, accountant/
    /// auditor, ...) from the config's `company.*` sub-object.
    pub profile: CompanyProfile,
    /// The set of accounts: the return period, the financial-year tax
    /// parameters and the report metadata (resolved before the report is
    /// built).
    pub accounts: AccountsMeta,
    /// The current-period book the report was computed from, retained so
    /// [`Self::with_prev_period_data`] can recompute the current column as
    /// the previous period's closing figures plus this period's activity.
    /// `None` for reports parsed from iXBRL ([`Self::from_ixbrl_node`]),
    /// which have no ledger.
    book: Option<GnucashBook>,
    /// The previous period's chart of accounts, retained (cloned) by
    /// [`Self::with_prev_period_data`] so [`Self::with_previous_year_adjustments`]
    /// can recompute both columns with start-balance adjustments merged
    /// into the previous-period book.  `None` when no previous-period data
    /// was supplied (or the report was parsed from iXBRL).
    prev_book: Option<GnucashBook>,
    /// The filing deadlines the default signing date is capped by, when no
    /// explicit `authorised_date` is supplied: the earliest of the accounts
    /// (Companies House) and CT600 (HMRC) deadlines.  Defaults to the
    /// period-end schedule (9 / 12 months after the period end); override
    /// with [`Self::with_signing_deadlines`] when the real deadlines are
    /// known (e.g. the company profile's `next_accounts`).
    filing_deadlines: FilingDeadlines,
    /// Tangible / fixed assets.
    pub fixed_assets: [f64; 2],
    /// Called-up share capital not paid — line A of the FRS 105
    /// balance-sheet format, shown above fixed assets: the amount called
    /// up on allotted shares but not yet received as at the balance-sheet
    /// date.  Micro-entities need not disclose it separately, so it
    /// defaults to zero and the row is omitted unless the setter supplies
    /// a nonzero amount ([`Self::with_called_up_share_capital_not_paid`]).
    pub called_up_share_capital_not_paid: [f64; 2],
    /// Current assets (debtors + VAT refund due + bank).
    pub current_assets: [f64; 2],
    /// Prepayments and accrued income — an asset-side line, computed from
    /// `Assets:Prepayments and Accrued Income`.
    pub prepayments_and_accrued_income: [f64; 2],
    /// Creditors: amounts falling due within one year.
    pub creditors_within_1_year: [f64; 2],
    /// Net current assets / (liabilities).
    pub net_current_assets: [f64; 2],
    /// Total assets less current liabilities.
    pub total_assets_less_liabilities: [f64; 2],
    /// Creditors: amounts falling due after one year — a negative line
    /// (creditor), computed from `Liabilities:Creditors After 1 Year`.
    pub creditors_after_1_year: [f64; 2],
    /// Provisions for liabilities — a positive magnitude (the filed
    /// presentation), computed from `Liabilities:Provisions` and *deducted*
    /// from net assets.
    pub provisions_for_liabilities: [f64; 2],
    /// Accrued liabilities and deferred income — a positive magnitude (the
    /// filed presentation), computed from `Liabilities:Accruals and
    /// Deferred Income` and *deducted* from net assets.
    pub accruals_and_deferred_income: [f64; 2],
    /// Net assets.
    pub net_assets: [f64; 2],
    /// Capital and reserves (share capital + profit/loss + dividends +
    /// corporation tax).
    pub capital_and_reserves: [f64; 2],
}

/// The filing deadlines that cap the default signing date: the deadline to
/// file the accounts with Companies House and the deadline to file the
/// CT600 corporation-tax return with HMRC.  [`Frs105Accounts::signing_date`]
/// defaults to one day before the **earliest** of the two, capped at today.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilingDeadlines {
    /// Deadline to file the accounts with Companies House (the profile's
    /// `next_accounts.due_on`, else 9 months after the period end).
    pub companies_house_accounts: chrono::NaiveDate,
    /// Deadline to file the CT600 corporation-tax return with HMRC (12
    /// months after the period end).
    pub hmrc_ct600: chrono::NaiveDate,
}

impl FilingDeadlines {
    /// The default deadlines from the period end: the accounts are due 9
    /// months after the period end, the CT600 12 months after.
    pub fn from_period_end(period_end: chrono::NaiveDate) -> Self {
        Self {
            companies_house_accounts: period_end + chrono::Months::new(9),
            hmrc_ct600: period_end + chrono::Months::new(12),
        }
    }

    /// The earliest of the two deadlines — the binding one for the signing
    /// date.
    pub fn earliest(&self) -> chrono::NaiveDate {
        self.companies_house_accounts.min(self.hmrc_ct600)
    }
}

/// The default signing date: one day before `earliest_deadline`, capped at
/// `today` — the financial statements cannot be authorised in the future,
/// and must be signed before the last day to file.
pub fn default_signing_date(
    today: chrono::NaiveDate,
    earliest_deadline: chrono::NaiveDate,
) -> chrono::NaiveDate {
    (earliest_deadline - chrono::Duration::days(1)).min(today)
}

/// The previous period's input to the accounts builder: the full chart of
/// accounts of the previous period, plus an optional filed balance sheet
/// used only to check that the book matches what was filed.
///
/// The previous-period comparative column is **computed from the chart of
/// accounts** — a filed balance sheet alone cannot rebuild the CoA (the
/// filing carries only the aggregate lines, not the accounts or
/// transactions).  The previous CoA is an *optional* input: without it the
/// comparative column defaults to zeros, and it is supplied afterwards via
/// [`Frs105Accounts::with_prev_period_data`].  Supplying it also **seeds**
/// the current column — it starts from the previous period's closing
/// figures and adds this period's activity.  When a filed balance sheet is
/// also available it is not used for the computation:
/// [`Frs105Accounts::check_previous_period_matches_filing`] compares the
/// computed comparative column against it.
pub struct PreviousPeriodData<'a> {
    /// The previous period's chart of accounts (accounts + transactions).
    pub book: &'a GnucashBook,
    /// The previous period's filed balance sheet, when one is on record —
    /// check-only, see [`Frs105Accounts::check_previous_period_matches_filing`].
    pub filing: Option<&'a FiledBalanceSheet>,
}

impl<'a> PreviousPeriodData<'a> {
    /// The previous period's chart of accounts is the same book as the
    /// current one — the book covers both periods, so the comparative
    /// column is computed from its own history.  No filed balance sheet is
    /// attached.
    pub fn same_book(book: &'a GnucashBook) -> Self {
        PreviousPeriodData { book, filing: None }
    }
}

/// The account paths each FRS 105 balance-sheet line's computation reads
/// (a path covers its child accounts too).  Shared by
/// [`Frs105Accounts::compute_lines`] (the leaf lines sum these paths) and
/// the previous-period check's error messages ([`Frs105CheckError`]), so
/// the errors always name the accounts the computation actually used.
const LINE_ACCOUNTS: [(&str, &[&str]); 8] = [
    ("fixed assets", &["Assets:Capital Equipment"]),
    (
        "current assets",
        &[
            "Accounts Receivable",
            "Assets:Owed To Us",
            "VAT:Input",
            "VAT:Settlement:Input",
            "Assets:VAT Repayments Due",
            "Bank Accounts",
        ],
    ),
    ("prepayments and accrued income", &["Assets:Prepayments and Accrued Income"]),
    (
        "creditors within one year",
        &[
            "Accounts Payable",
            "VAT:Output",
            "VAT:Settlement:Output",
            "Liabilities:Credit Cards",
            "Liabilities:Owed Corporation Tax",
        ],
    ),
    ("creditors after one year", &["Liabilities:Creditors After 1 Year"]),
    ("provisions for liabilities", &["Liabilities:Provisions"]),
    ("accruals and deferred income", &["Liabilities:Accruals and Deferred Income"]),
    (
        "capital and reserves",
        &[
            "Equity:Shareholdings",
            "Income",
            "Expenses",
            "Equity:Dividends",
            "Equity:Corporation Tax",
        ],
    ),
];

/// One mismatch between the previous-period accounts computed from the
/// previous CoA and the previous period's filed balance sheet: a differing
/// FRS 105 line (one variant per line, naming the accounts or derivation
/// the computation read and the computed vs filed values) or the
/// period-dates mismatch.
#[derive(Debug, PartialEq, Snafu)]
pub enum Frs105CheckError {
    #[snafu(display("fixed assets: {accounts} computed {computed}, the filing has {filed}"))]
    FixedAssetsMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display(
        "called up share capital not paid: {accounts} computed {computed}, the filing has {filed}"
    ))]
    CalledUpShareCapitalNotPaidMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display("current assets: {accounts} computed {computed}, the filing has {filed}"))]
    CurrentAssetsMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display(
        "prepayments and accrued income: {accounts} computed {computed}, the filing has {filed}"
    ))]
    PrepaymentsAndAccruedIncomeMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display(
        "creditors within one year: {accounts} computed {computed}, the filing has {filed}"
    ))]
    CreditorsWithinOneYearMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display("net current assets: {accounts} computed {computed}, the filing has {filed}"))]
    NetCurrentAssetsMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display(
        "total assets less liabilities: {accounts} computed {computed}, the filing has {filed}"
    ))]
    TotalAssetsLessLiabilitiesMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display(
        "creditors after one year: {accounts} computed {computed}, the filing has {filed}"
    ))]
    CreditorsAfterOneYearMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display(
        "provisions for liabilities: {accounts} computed {computed}, the filing has {filed}"
    ))]
    ProvisionsForLiabilitiesMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display(
        "accruals and deferred income: {accounts} computed {computed}, the filing has {filed}"
    ))]
    AccrualsAndDeferredIncomeMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display("net assets: {accounts} computed {computed}, the filing has {filed}"))]
    NetAssetsMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display("capital and reserves: {accounts} computed {computed}, the filing has {filed}"))]
    CapitalAndReservesMismatch {
        computed: f64,
        filed: f64,
        accounts: String,
    },
    #[snafu(display(
        "period dates: the filing's period ends {filed_end}, the previous period ends {prev_end}"
    ))]
    PeriodDatesMismatch {
        filed_end: chrono::NaiveDate,
        prev_end: chrono::NaiveDate,
    },
}

/// All the mismatches found by
/// [`Frs105Accounts::check_previous_period_matches_filing`]: every
/// differing line and/or the period-dates mismatch, collected so the caller
/// sees all of them at once instead of failing at the first.
#[derive(Debug, Default, PartialEq)]
pub struct CheckErrors {
    pub errors: Vec<Frs105CheckError>,
}

impl std::fmt::Display for CheckErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for error in &self.errors {
            writeln!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CheckErrors {}

/// The computation inputs a line reads, for the check's error messages:
/// the account paths for the leaf lines ([`LINE_ACCOUNTS`]), the derivation
/// for the derived lines, and the note for line A (no account path).
fn line_accounts(label: &'static str) -> String {
    match label {
        "called up share capital not paid" => {
            "no account path (line A of the FRS 105 format, supplied via \
             with_called_up_share_capital_not_paid)"
                .into()
        }
        "net current assets" => {
            "derived: current assets + prepayments and accrued income + creditors within one year"
                .into()
        }
        "total assets less liabilities" => {
            "derived: fixed assets + current assets + prepayments and accrued income \
             + creditors within one year"
                .into()
        }
        "net assets" => {
            "derived: total assets less liabilities − creditors after one year − provisions \
             for liabilities − accruals and deferred income"
                .into()
        }
        other => LINE_ACCOUNTS
            .iter()
            .find(|(l, _)| *l == other)
            .map(|(_, paths)| paths.join(", "))
            .expect("line label in LINE_ACCOUNTS"),
    }
}

/// The [`Frs105CheckError`] variant for one differing line, carrying the
/// computation's account inputs and the computed vs filed values.
fn line_mismatch_error(label: &'static str, computed: f64, filed: f64) -> Frs105CheckError {
    let accounts = line_accounts(label);
    match label {
        "fixed assets" => Frs105CheckError::FixedAssetsMismatch { computed, filed, accounts },
        "called up share capital not paid" => {
            Frs105CheckError::CalledUpShareCapitalNotPaidMismatch { computed, filed, accounts }
        }
        "current assets" => Frs105CheckError::CurrentAssetsMismatch { computed, filed, accounts },
        "prepayments and accrued income" => {
            Frs105CheckError::PrepaymentsAndAccruedIncomeMismatch { computed, filed, accounts }
        }
        "creditors within one year" => {
            Frs105CheckError::CreditorsWithinOneYearMismatch { computed, filed, accounts }
        }
        "net current assets" => {
            Frs105CheckError::NetCurrentAssetsMismatch { computed, filed, accounts }
        }
        "total assets less liabilities" => {
            Frs105CheckError::TotalAssetsLessLiabilitiesMismatch { computed, filed, accounts }
        }
        "creditors after one year" => {
            Frs105CheckError::CreditorsAfterOneYearMismatch { computed, filed, accounts }
        }
        "provisions for liabilities" => {
            Frs105CheckError::ProvisionsForLiabilitiesMismatch { computed, filed, accounts }
        }
        "accruals and deferred income" => {
            Frs105CheckError::AccrualsAndDeferredIncomeMismatch { computed, filed, accounts }
        }
        "net assets" => Frs105CheckError::NetAssetsMismatch { computed, filed, accounts },
        "capital and reserves" => {
            Frs105CheckError::CapitalAndReservesMismatch { computed, filed, accounts }
        }
        other => unreachable!("unknown line label {other}"),
    }
}

/// One violated balance-sheet identity in a computed [`Frs105Accounts`]
/// column: a line that doesn't tie to the lines it is derived from, or a
/// sheet that doesn't balance (net assets ≠ capital and reserves).
#[derive(Debug, PartialEq, Snafu)]
#[snafu(context(suffix(Validate)))]
pub enum BalanceSheetCheckError {
    #[snafu(display(
        "column {column}: net current assets {computed}, but current assets + prepayments \
         and accrued income + creditors within one year = {derived}"
    ))]
    NetCurrentAssetsMismatch {
        column: &'static str,
        computed: f64,
        derived: f64,
    },
    #[snafu(display(
        "column {column}: total assets less liabilities {computed}, but fixed assets + net \
         current assets + called up share capital not paid = {derived}"
    ))]
    TotalAssetsLessLiabilitiesMismatch {
        column: &'static str,
        computed: f64,
        derived: f64,
    },
    #[snafu(display(
        "column {column}: net assets {computed}, but total assets less liabilities + \
         creditors after one year − provisions for liabilities − accruals and deferred \
         income = {derived}"
    ))]
    NetAssetsMismatch {
        column: &'static str,
        computed: f64,
        derived: f64,
    },
    #[snafu(display(
        "column {column}: the balance sheet does not balance — net assets {net_assets}, \
         capital and reserves + called up share capital not paid {capital_and_reserves}"
    ))]
    BalanceSheetDoesNotBalance {
        column: &'static str,
        net_assets: f64,
        capital_and_reserves: f64,
    },
}

/// All the balance-sheet identities violated by a computed
/// [`Frs105Accounts`] ([`Frs105Accounts::validate`]): every broken tie in
/// both columns, collected so the caller sees all of them at once instead
/// of failing at the first.
#[derive(Debug, Default, PartialEq)]
pub struct BalanceSheetCheckErrors {
    pub errors: Vec<BalanceSheetCheckError>,
}

impl std::fmt::Display for BalanceSheetCheckErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for error in &self.errors {
            writeln!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for BalanceSheetCheckErrors {}

/// The 12 balance-sheet lines computed from one book up to an end date —
/// the current-period column from the current book, and the previous-period
/// column from the previous-period book when one is supplied (see
/// [`Frs105Accounts::with_prev_period_data`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct ComputedLines {
    fixed_assets: f64,
    current_assets: f64,
    prepayments_and_accrued_income: f64,
    creditors_within_1_year: f64,
    net_current_assets: f64,
    total_assets_less_liabilities: f64,
    creditors_after_1_year: f64,
    provisions_for_liabilities: f64,
    accruals_and_deferred_income: f64,
    net_assets: f64,
    capital_and_reserves: f64,
}

impl ComputedLines {
    /// Round every line to whole pence — kills the f64 representation noise
    /// that appears when two rounded line sets are added or subtracted
    /// (e.g. seeded current = previous figures + in-period activity).
    fn rounded(self) -> ComputedLines {
        let round2 = |v: f64| (v * 100.0).round() / 100.0;
        ComputedLines {
            fixed_assets: round2(self.fixed_assets),
            current_assets: round2(self.current_assets),
            prepayments_and_accrued_income: round2(self.prepayments_and_accrued_income),
            creditors_within_1_year: round2(self.creditors_within_1_year),
            net_current_assets: round2(self.net_current_assets),
            total_assets_less_liabilities: round2(self.total_assets_less_liabilities),
            creditors_after_1_year: round2(self.creditors_after_1_year),
            provisions_for_liabilities: round2(self.provisions_for_liabilities),
            accruals_and_deferred_income: round2(self.accruals_and_deferred_income),
            net_assets: round2(self.net_assets),
            capital_and_reserves: round2(self.capital_and_reserves),
        }
    }
}

impl std::ops::Add for ComputedLines {
    type Output = ComputedLines;

    fn add(self, rhs: ComputedLines) -> ComputedLines {
        ComputedLines {
            fixed_assets: self.fixed_assets + rhs.fixed_assets,
            current_assets: self.current_assets + rhs.current_assets,
            prepayments_and_accrued_income: self.prepayments_and_accrued_income
                + rhs.prepayments_and_accrued_income,
            creditors_within_1_year: self.creditors_within_1_year + rhs.creditors_within_1_year,
            net_current_assets: self.net_current_assets + rhs.net_current_assets,
            total_assets_less_liabilities: self.total_assets_less_liabilities
                + rhs.total_assets_less_liabilities,
            creditors_after_1_year: self.creditors_after_1_year + rhs.creditors_after_1_year,
            provisions_for_liabilities: self.provisions_for_liabilities
                + rhs.provisions_for_liabilities,
            accruals_and_deferred_income: self.accruals_and_deferred_income
                + rhs.accruals_and_deferred_income,
            net_assets: self.net_assets + rhs.net_assets,
            capital_and_reserves: self.capital_and_reserves + rhs.capital_and_reserves,
        }
    }
}

impl std::ops::Sub for ComputedLines {
    type Output = ComputedLines;

    fn sub(self, rhs: ComputedLines) -> ComputedLines {
        ComputedLines {
            fixed_assets: self.fixed_assets - rhs.fixed_assets,
            current_assets: self.current_assets - rhs.current_assets,
            prepayments_and_accrued_income: self.prepayments_and_accrued_income
                - rhs.prepayments_and_accrued_income,
            creditors_within_1_year: self.creditors_within_1_year - rhs.creditors_within_1_year,
            net_current_assets: self.net_current_assets - rhs.net_current_assets,
            total_assets_less_liabilities: self.total_assets_less_liabilities
                - rhs.total_assets_less_liabilities,
            creditors_after_1_year: self.creditors_after_1_year - rhs.creditors_after_1_year,
            provisions_for_liabilities: self.provisions_for_liabilities
                - rhs.provisions_for_liabilities,
            accruals_and_deferred_income: self.accruals_and_deferred_income
                - rhs.accruals_and_deferred_income,
            net_assets: self.net_assets - rhs.net_assets,
            capital_and_reserves: self.capital_and_reserves - rhs.capital_and_reserves,
        }
    }
}

impl Frs105Accounts {
    /// Compute the statement of financial position from the ledgers and the
    /// company details.
    ///
    /// The current-period column is computed from the current-period book
    /// (the `gnucash` ledger, up to the period end).  The previous-period
    /// comparative column defaults to **zeros** — a company whose ledger
    /// doesn't cover the previous period, or one that hasn't supplied
    /// previous-period data yet, renders a blank comparative column.
    /// Supply the previous period afterwards with
    /// [`Self::with_prev_period_data`] (a previous-period chart of accounts;
    /// a filed balance sheet alone cannot rebuild the previous CoA).  Doing
    /// so **seeds** the current column: it starts from the previous period's
    /// closing figures (an override — the current book's own pre-period
    /// transactions are ignored) and adds this period's activity, instead
    /// of computing the current column from the book alone.
    pub fn new(
        gnucash: &GnucashBook,
        company: &Company,
        profile: &CompanyProfile,
        accounts_meta: &AccountsMeta,
    ) -> Self {
        let period = accounts_meta.period();
        let current = Self::compute_lines(gnucash, period.end);
        let col = |current: f64| [current, 0.0];

        Frs105Accounts {
            company: company.clone(),
            profile: profile.clone(),
            accounts: accounts_meta.clone(),
            book: Some(gnucash.clone()),
            prev_book: None,
            filing_deadlines: FilingDeadlines::from_period_end(period.end),
            fixed_assets: col(current.fixed_assets),
            // Line A (called-up share capital not paid) has no account path:
            // it defaults to zero and is supplied via
            // `with_called_up_share_capital_not_paid`.
            called_up_share_capital_not_paid: [0.0; 2],
            current_assets: col(current.current_assets),
            prepayments_and_accrued_income: col(current.prepayments_and_accrued_income),
            creditors_within_1_year: col(current.creditors_within_1_year),
            net_current_assets: col(current.net_current_assets),
            total_assets_less_liabilities: col(current.total_assets_less_liabilities),
            creditors_after_1_year: col(current.creditors_after_1_year),
            provisions_for_liabilities: col(current.provisions_for_liabilities),
            accruals_and_deferred_income: col(current.accruals_and_deferred_income),
            net_assets: col(current.net_assets),
            capital_and_reserves: col(current.capital_and_reserves),
        }
    }

    /// The date the financial statements were authorised for issue (the
    /// signing date): the explicitly-supplied `accounts.authorised_date`,
    /// else the default — one day before the earliest filing deadline
    /// ([`FilingDeadlines::earliest`]), capped at today (the statements
    /// cannot be authorised in the future).  Override the deadlines with
    /// [`Self::with_signing_deadlines`].
    pub fn signing_date(&self) -> chrono::NaiveDate {
        self.accounts.authorised_date.unwrap_or_else(|| {
            default_signing_date(chrono::Utc::now().date_naive(), self.filing_deadlines.earliest())
        })
    }

    /// Override the filing deadlines the default signing date is capped by
    /// ([`Self::signing_date`]) with the real ones — e.g. from the company
    /// profile's `next_accounts.due_on` and the CT600 deadline (see
    /// `companies_house::next_accounting_period_from`).  Only affects the
    /// default: an explicitly-supplied `accounts.authorised_date` wins.
    pub fn with_signing_deadlines(mut self, deadlines: FilingDeadlines) -> Self {
        self.filing_deadlines = deadlines;
        self
    }

    /// The base64-encoded director's signature embedded on the statement of
    /// financial position: the explicitly-supplied `accounts.signature_b64`,
    /// else the bundled default ([`DEFAULT_SIGNATURE_B64`]).  Override with
    /// [`Self::with_signature`].
    pub fn signature_b64(&self) -> &str {
        self.accounts
            .signature_b64
            .as_deref()
            .unwrap_or(DEFAULT_SIGNATURE_B64)
    }

    /// Override the director's signature embedded on the statement of
    /// financial position with a user-provided one (base64 PNG).
    pub fn with_signature(mut self, signature_b64: String) -> Self {
        self.accounts.signature_b64 = Some(signature_b64);
        self
    }

    /// Supply the previous period's chart of accounts, recomputing both
    /// columns from it:
    ///
    /// - the previous-period comparative column is computed from
    ///   [`PreviousPeriodData::book`] up to the day before the current
    ///   period starts;
    /// - the current-period column is **seeded**: it starts from those
    ///   previous-period closing figures (the caller's figures act as an
    ///   **override** — the current book's own pre-period transactions are
    ///   ignored entirely) and adds this period's activity, i.e. the
    ///   current book's splits dated within the period, computed with the
    ///   same per-line computations as a full-history book but on the
    ///   filtered transactions only.
    ///
    /// For a book that spans both periods ([`PreviousPeriodData::same_book`])
    /// the seeded current column equals the full-history computation, so the
    /// two are indistinguishable; for a current-period-only book the seeding
    /// carries the opening balances forward instead of starting from zero
    /// (a pending period with an empty book renders the balances unchanged,
    /// current == previous).  To keep the current column computed purely
    /// from the current book, use [`Self::with_prev_period_data_no_seed`].
    ///
    /// The optional filed balance sheet ([`PreviousPeriodData::filing`]) is
    /// not used here; verify the book against it with
    /// [`Self::check_previous_period_matches_filing`].
    pub fn with_prev_period_data(mut self, prev: &PreviousPeriodData) -> Self {
        let period = self.accounts.period();
        let prev_end = period.start - chrono::Duration::days(1);
        let previous = Self::compute_lines(prev.book, prev_end);

        // Retained (cloned) so [`Self::with_previous_year_adjustments`] can
        // later merge start-balance adjustments into the comparative
        // column.
        self.prev_book = Some(prev.book.clone());

        // This period's activity: the current book's splits dated within
        // the period, filtered explicitly (everything outside the period is
        // ignored — the previous period is represented entirely by the
        // caller's override).
        let current = Self::seeded_current(&self.book, previous, period);
        self.set_computed(current, previous);
        self
    }

    /// The opt-out variant of [`Self::with_prev_period_data`]: fills the
    /// previous-period comparative column from [`PreviousPeriodData::book`]
    /// (up to the day before the current period starts) but leaves the
    /// current-period column as computed in [`Self::new`] — purely from the
    /// current book, with no seeding from the previous period's figures.
    pub fn with_prev_period_data_no_seed(mut self, prev: &PreviousPeriodData) -> Self {
        let period = self.accounts.period();
        let previous = Self::compute_lines(prev.book, period.start - chrono::Duration::days(1));
        self.fixed_assets[1] = previous.fixed_assets;
        self.current_assets[1] = previous.current_assets;
        self.prepayments_and_accrued_income[1] = previous.prepayments_and_accrued_income;
        self.creditors_within_1_year[1] = previous.creditors_within_1_year;
        self.net_current_assets[1] = previous.net_current_assets;
        self.total_assets_less_liabilities[1] = previous.total_assets_less_liabilities;
        self.creditors_after_1_year[1] = previous.creditors_after_1_year;
        self.provisions_for_liabilities[1] = previous.provisions_for_liabilities;
        self.accruals_and_deferred_income[1] = previous.accruals_and_deferred_income;
        self.net_assets[1] = previous.net_assets;
        self.capital_and_reserves[1] = previous.capital_and_reserves;
        self
    }

    /// Merge previous-period start-balance adjustments into the comparative
    /// column: a list of transactions (splits referencing accounts by
    /// path, resolved against the previous period's chart of accounts
    /// supplied via [`Self::with_prev_period_data`]) dated within the
    /// previous period — e.g. correcting a prior-period error the
    /// comparative column must reflect, such as restoring a liability the
    /// filed balance sheet omitted.  Both columns are recomputed exactly as
    /// [`Self::with_prev_period_data`] does, with the adjustments merged
    /// into the previous-period book:
    ///
    /// - the previous-period comparative column is computed from the
    ///   adjusted previous-period book up to the day before the current
    ///   period starts;
    /// - the current-period column is re-seeded: the adjusted
    ///   previous-period closing figures plus this period's activity — so a
    ///   current-period transaction that settles the restored balance (e.g.
    ///   paying the restored tax liability out of the bank) nets it out of
    ///   the current column, leaving the correction visible in the
    ///   comparative column only.
    ///
    /// The adjustment splits' account paths must exist in the previous
    /// book and fall on the balance-sheet lines' account paths
    /// ([`LINE_ACCOUNTS`]) — a split on an account outside them is silently
    /// ignored by the line computations (like any ledger posting off the
    /// lines).  Requires the seeding variant [`Self::with_prev_period_data`]
    /// (the `_no_seed` opt-out does not retain the previous book); the
    /// transactions must balance and their dates must fall within the
    /// previous period.
    pub fn with_previous_year_adjustments(
        mut self,
        adjustments: &[AdjustmentTransaction],
    ) -> Self {
        let period = self.accounts.period();
        let prev_period = period.previous();
        let prev_end = prev_period.end;
        let prev_book = self.prev_book.as_ref().expect(
            "with_previous_year_adjustments requires previous-period data: \
             call with_prev_period_data (the seeding variant) first",
        );

        // Resolve each adjustment split's account path against the previous
        // book's accounts, and merge the transactions into the previous
        // book's raw parts (fresh transaction GUIDs, existing accounts).
        let accounts = prev_book.raw_accounts();
        let known: Vec<String> = accounts.iter().map(|a| Self::account_path(accounts, a)).collect();
        let merged_accounts = accounts.to_vec();
        let mut merged_txns = prev_book.raw_transactions().to_vec();
        let mut merged_splits = prev_book.raw_splits().to_vec();
        for (ti, txn) in adjustments.iter().enumerate() {
            let date = txn.post_datetime.date();
            assert!(
                prev_period.contains(date),
                "with_previous_year_adjustments: transaction '{}' dated {date} falls \
                 outside the previous period ({} .. {})",
                txn.description,
                prev_period.start,
                prev_period.end,
            );
            let txn_guid = format!("adj-txn-{ti}");
            merged_txns.push(crate::RawTransaction {
                guid: txn_guid.clone(),
                post_datetime: txn.post_datetime,
                description: txn.description.clone(),
            });
            for (si, split) in txn.splits.iter().enumerate() {
                let acc = accounts
                    .iter()
                    .find(|a| Self::account_path(accounts, a) == split.account)
                    .unwrap_or_else(|| {
                        panic!(
                            "with_previous_year_adjustments: split {} of transaction '{}' \
                             references unknown account path '{}' (known: {})",
                            si,
                            txn.description,
                            split.account,
                            known.join(", "),
                        )
                    });
                merged_splits.push(crate::RawSplit {
                    tx_guid: txn_guid.clone(),
                    account_guid: acc.guid.clone(),
                    value: split.value,
                });
            }
        }
        let adjusted_prev =
            GnucashBook::from_raw_parts(merged_accounts, merged_txns, merged_splits);

        let previous = Self::compute_lines(&adjusted_prev, prev_end);
        let current = Self::seeded_current(&self.book, previous, period);
        self.set_computed(current, previous);
        self
    }

    /// The seeded current column: the previous-period figures plus this
    /// period's activity (the current book's splits dated within the
    /// period), rounded per line (the sum of two rounded sets carries f64
    /// noise).  `None` book (a report parsed from iXBRL) contributes zero
    /// activity.
    fn seeded_current(
        book: &Option<GnucashBook>,
        previous: ComputedLines,
        period: AccountingPeriod,
    ) -> ComputedLines {
        let activity = match book {
            Some(book) => {
                Self::compute_lines_between(book, Some(period.start), Some(period.end))
            }
            None => ComputedLines::default(),
        };
        (previous + activity).rounded()
    }

    /// Set both columns of the statement of financial position from a
    /// computed current and previous period.
    fn set_computed(&mut self, current: ComputedLines, previous: ComputedLines) {
        self.fixed_assets = [current.fixed_assets, previous.fixed_assets];
        self.current_assets = [current.current_assets, previous.current_assets];
        self.prepayments_and_accrued_income = [
            current.prepayments_and_accrued_income,
            previous.prepayments_and_accrued_income,
        ];
        self.creditors_within_1_year = [
            current.creditors_within_1_year,
            previous.creditors_within_1_year,
        ];
        self.net_current_assets = [current.net_current_assets, previous.net_current_assets];
        self.total_assets_less_liabilities = [
            current.total_assets_less_liabilities,
            previous.total_assets_less_liabilities,
        ];
        self.creditors_after_1_year = [
            current.creditors_after_1_year,
            previous.creditors_after_1_year,
        ];
        self.provisions_for_liabilities = [
            current.provisions_for_liabilities,
            previous.provisions_for_liabilities,
        ];
        self.accruals_and_deferred_income = [
            current.accruals_and_deferred_income,
            previous.accruals_and_deferred_income,
        ];
        self.net_assets = [current.net_assets, previous.net_assets];
        self.capital_and_reserves = [current.capital_and_reserves, previous.capital_and_reserves];
    }

    /// Compute the balance-sheet lines of one book up to `end` (a balance
    /// sheet date): the account-path → type map, the splits collected as
    /// (date, path, value) and the "line" / derived computations.
    ///
    /// Called by [`Self::new`] (with the current book up to the period end)
    /// and by [`Self::with_prev_period_data`] (with the previous-period
    /// book up to the day before the current period starts).
    fn compute_lines(book: &GnucashBook, end: chrono::NaiveDate) -> ComputedLines {
        Self::compute_lines_between(book, None, Some(end))
    }

    /// Like [`Self::compute_lines`], but only splits dated within
    /// `start..=end` contribute (`None` = unbounded on that side).  Used to
    /// isolate a period's activity from a book that spans more than one
    /// period.
    fn compute_lines_between(
        book: &GnucashBook,
        start: Option<chrono::NaiveDate>,
        end: Option<chrono::NaiveDate>,
    ) -> ComputedLines {
        let accounts = book.raw_accounts();

        // Map account path -> GnuCash account type, for the debit flip.
        let mut account_types: HashMap<String, String> = HashMap::new();
        for acc in accounts {
            if acc.r#type == "ROOT" || acc.r#type == "TEMPLATE" {
                continue;
            }
            account_types.insert(Self::account_path(accounts, acc), acc.r#type.clone());
        }

        // Collect (date, path, value) for every split, skipping the ROOT and
        // TEMPLATE accounts.
        let mut splits: Vec<(chrono::NaiveDate, String, f64)> = Vec::new();
        for split in book.raw_splits() {
            let tx = match book.raw_transactions().iter().find(|t| t.guid == split.tx_guid) {
                Some(t) => t,
                None => continue,
            };
            let acc = match accounts.iter().find(|a| a.guid == split.account_guid) {
                Some(a) => a,
                None => continue,
            };
            if acc.r#type == "ROOT" || acc.r#type == "TEMPLATE" {
                continue;
            }
            let path = Self::account_path(accounts, acc);
            let val = split.value.to_string().parse::<f64>().unwrap_or(0.0);
            splits.push((tx.post_datetime.date(), path, val));
        }

        // A "line" computation: sum the splits recorded against an account
        // (and any child accounts) up to and including the balance-sheet
        // date, negating the total for debit-side account types.
        let line = |acct: &str| -> f64 {
            let mut total = 0.0;
            for (date, path, val) in &splits {
                if end.is_some_and(|e| *date > e) || start.is_some_and(|s| *date < s) {
                    continue;
                }
                if path == acct || path.starts_with(&format!("{acct}:")) {
                    let mut amount = *val;
                    if matches!(
                        account_types.get(acct).map(String::as_str),
                        Some("INCOME") | Some("EQUITY") | Some("EXPENSE")
                    ) {
                        amount = -amount;
                    }
                    total += amount;
                }
            }
            total
        };

        // Round to whole pence so computed values match the reference exactly.
        let round2 = |v: f64| (v * 100.0).round() / 100.0;

        // Leaf lines: each sums its [`LINE_ACCOUNTS`] paths (a path covers
        // its child accounts) and rounds once — the same paths the previous-
        // period check's error messages name.
        let leaf = |label: &str| -> f64 {
            let paths = LINE_ACCOUNTS
                .iter()
                .find(|(l, _)| *l == label)
                .expect("line label in LINE_ACCOUNTS")
                .1;
            round2(paths.iter().map(|path| line(path)).sum())
        };

        let fixed_assets = leaf("fixed assets");
        let current_assets = leaf("current assets");
        let prepayments_and_accrued_income = leaf("prepayments and accrued income");
        let creditors_within_1_year = leaf("creditors within one year");

        let net_current_assets = round2(
            current_assets + prepayments_and_accrued_income + creditors_within_1_year,
        );
        let total_assets_less_liabilities = round2(
            fixed_assets + current_assets + prepayments_and_accrued_income
                + creditors_within_1_year,
        );

        let creditors_after_1_year = leaf("creditors after one year");
        // The ledger stores provisions and accruals as negative credit
        // balances (like every other liability); the report renders them
        // as positive magnitudes (the filed presentation), so the leaves
        // are negated here.
        let provisions_for_liabilities = -leaf("provisions for liabilities");
        let accruals_and_deferred_income = -leaf("accruals and deferred income");

        // Net assets deduct the after-one-year creditors and the provisions
        // / accruals — the lines render as positive magnitudes (matching the
        // filed presentation), so the deduction happens here.
        let net_assets = round2(
            total_assets_less_liabilities
                + creditors_after_1_year
                - provisions_for_liabilities
                - accruals_and_deferred_income,
        );

        let capital_and_reserves = leaf("capital and reserves");

        ComputedLines {
            fixed_assets,
            current_assets,
            prepayments_and_accrued_income,
            creditors_within_1_year,
            net_current_assets,
            total_assets_less_liabilities,
            creditors_after_1_year,
            provisions_for_liabilities,
            accruals_and_deferred_income,
            net_assets,
            capital_and_reserves,
        }
    }

    /// The full ":"-separated account path for an account, excluding the
    /// ROOT account.
    fn account_path(accounts: &[crate::RawAccount], acc: &crate::RawAccount) -> String {
        let mut parts = Vec::new();
        let mut current = Some(acc);
        while let Some(a) = current {
            if a.r#type == "ROOT" {
                break;
            }
            parts.push(a.name.clone());
            current = accounts.iter().find(|p| p.guid == a.parent_guid);
        }
        parts.reverse();
        parts.join(":")
    }

    /// Render the accounts as an iXBRL HTML document.
    pub fn to_ixbrl(&self) -> String {
        let company = &self.company;
        let profile = &self.profile;
        let period = self.accounts.period();
        let period_start = period.start;
        let period_end = period.end;
        // The previous period is the calendar year before the current one
        // (e.g. 2019-01-01..2019-12-31), matching the reference metadata.
        let prev_end = period_start - chrono::Duration::days(1);
        let prev_start = chrono::NaiveDate::from_ymd_opt(prev_end.year(), 1, 1).unwrap();
        let current_year = period_end.year().to_string();
        let prev_year = prev_end.year().to_string();

        // JFCVC.3312 (Arelle's validate/UK plugin) makes these concepts
        // mandatory for accounts filings; when the profile / accounts meta
        // leaves them blank, fall back to the fixed values this report uses
        // (matching the API/CLI defaults).
        let accounting_standards = non_empty_or(
            &self.accounts.accounting_standards_dimension,
            "uk-bus:Micro-entities",
        );
        let accounts_type = non_empty_or(
            &self.accounts.accounts_type_dimension,
            "uk-bus:AbridgedAccounts",
        );
        let accounts_status = non_empty_or(
            &self.accounts.accounts_status_dimension,
            "uk-bus:AuditExempt-NoAccountantsReport",
        );
        let legal_form = non_empty_or(
            &profile.legal_form_dimension,
            "uk-bus:PrivateLimitedCompanyLtd",
        );

        // -- ix:header ------------------------------------------------------

        let mut hidden_children = vec![
            non_numeric("uk-bus:ReportTitle", "ctxt-0", REPORT_TITLE),
            non_numeric_fmt(
                "uk-bus:BusinessReportPublicationDate",
                "ctxt-1",
                &format_date(&self.accounts.report_date),
                "ixt2:datedaymonthyearen",
            ),
            non_numeric_fmt(
                "uk-core:DateAuthorisationFinancialStatementsForIssue",
                "ctxt-2",
                &format_date(&self.signing_date()),
                "ixt2:datedaymonthyearen",
            ),
            // JFCVC.3312 (Arelle's validate/UK plugin) requires both period
            // facts to sit on an *instant* context whose date equals the
            // period end (EndDateForPeriodCoveredByReport) — so the start
            // date is tagged on the same period-end instant context
            // (ctxt-2), matching how Companies House filings render it.
            non_numeric_fmt(
                "uk-bus:StartDateForPeriodCoveredByReport",
                "ctxt-2",
                &format_date(&period_start),
                "ixt2:datedaymonthyearen",
            ),
            non_numeric_fmt(
                "uk-bus:EndDateForPeriodCoveredByReport",
                "ctxt-2",
                &format_date(&period_end),
                "ixt2:datedaymonthyearen",
            ),
            non_numeric(
                "uk-bus:EntityCurrentLegalOrRegisteredName",
                "ctxt-0",
                &company.name,
            ),
            non_numeric(
                "uk-bus:UKCompaniesHouseRegisteredNumber",
                "ctxt-0",
                &company.company_number,
            ),
        ];
        // The VAT registration number is a voluntary fact: it is omitted
        // when the profile leaves it blank.  Its position here (between the
        // company number and NameProductionSoftware) must match the
        // reference fixture's fact order.
        if let Some(vat) = profile
            .vat_registration
            .as_deref()
            .filter(|v| !v.is_empty())
        {
            hidden_children.push(non_numeric("uk-bus:VATRegistrationNumber", "ctxt-0", vat));
        }
        hidden_children.extend(vec![
            non_numeric("uk-bus:NameProductionSoftware", "ctxt-0", "ixbrl-reporter"),
            // Must match the version of the flake-pinned reference
            // (`ixbrl-reporter` in flake.nix); the fixture is generated with
            // that version, so keep them in sync.
            non_numeric("uk-bus:VersionProductionSoftware", "ctxt-0", "1.2.1"),
            non_numeric_fmt(
                "uk-bus:BalanceSheetDate",
                "ctxt-2",
                &format_date(&period_end),
                "ixt2:datedaymonthyearen",
            ),
        ]);
        // JFCVC.3312 makes the principal-activities description mandatory;
        // when the profile leaves it blank the fact is emitted with the
        // placeholder text (an empty tag is reported as "missing" by
        // Companies House's validator).  Its position here (after the
        // balance-sheet date, before the SIC codes) must match the
        // reference fixture's fact order.
        hidden_children.push(non_numeric(
            "uk-bus:DescriptionPrincipalActivities",
            "ctxt-0",
            profile
                .activities
                .as_deref()
                .unwrap_or(DEFAULT_PRINCIPAL_ACTIVITIES),
        ));
        hidden_children.extend(vec![
            non_numeric(
                "uk-bus:SICCodeRecordedUKCompaniesHouse1",
                "ctxt-0",
                profile.sic_codes.first().map(String::as_str).unwrap_or(""),
            ),
            non_numeric(
                "uk-bus:SICCodeRecordedUKCompaniesHouse2",
                "ctxt-0",
                profile.sic_codes.get(1).map(String::as_str).unwrap_or(""),
            ),
        ]);
        // Dimensioned taxonomy facts: the JFCVC-mandatory ones (ctxt-4..7)
        // are always emitted with the fallback values above; the voluntary
        // ones (ctxt-3, ctxt-8) only when the profile supplies a non-empty
        // dimension value, matching the corresponding xbrli:context.
        if !profile.industry_sector_dimension.is_empty() {
            hidden_children.push(non_numeric(
                "uk-bus:MainIndustrySector",
                "ctxt-3",
                &profile.industry_sector_dimension,
            ));
        }
        hidden_children.extend(vec![
            non_numeric("uk-bus:EntityDormantTruefalse", "ctxt-0", "false"),
            non_numeric("uk-bus:EntityTradingStatus", "ctxt-0", ""),
        ]);
        // These concepts are `types:fixedItemType` in the FRC taxonomy: the
        // fact is an empty marker — the meaning lives in the context's
        // dimension member (e.g. AccountingStandardsDimension) — exactly how
        // Companies House filings render them.  The `accounting_standards` /
        // `accounts_type` / `accounts_status` / `legal_form` fallbacks above
        // still choose the dimension members for the contexts (ctxt-4..7).
        hidden_children.push(non_numeric(
            "uk-bus:AccountingStandardsApplied",
            "ctxt-4",
            "",
        ));
        hidden_children.push(non_numeric("uk-bus:AccountsType", "ctxt-5", ""));
        hidden_children.push(non_numeric(
            "uk-bus:AccountsStatusAuditedOrUnaudited",
            "ctxt-6",
            "",
        ));
        hidden_children.push(non_numeric("uk-bus:LegalFormEntity", "ctxt-7", ""));
        if !profile.country_dimension.is_empty() {
            hidden_children.push(non_numeric(
                "uk-bus:CountryFormationOrIncorporation",
                "ctxt-8",
                &profile.country_dimension,
            ));
        }
        hidden_children.extend(vec![
            non_numeric_fmt(
                "uk-bus:DateFormationOrIncorporation",
                "ctxt-1",
                &format_date(&self.accounts.incorporation_date),
                "ixt2:datedaymonthyearen",
            ),
            employees_non_fraction(
                "ctxt-0",
                &self.accounts.average_employees_for(period_end.year()).to_string(),
            ),
            employees_non_fraction(
                "ctxt-9",
                &self.accounts.average_employees_for(prev_end.year()).to_string(),
            ),
        ]);
        {
            let signing_idx = profile
                .directors
                .iter()
                .position(|d| d == &self.accounts.signed_by)
                .unwrap_or(0);
            hidden_children.push(non_numeric(
                "uk-core:DirectorSigningFinancialStatements",
                &format!("ctxt-{}", 10 + signing_idx),
                "",
            ));
        }
        hidden_children.extend(vec![
            non_numeric(
                "uk-bus:NameContactDepartmentOrPerson",
                "ctxt-13",
                &profile.contact_name,
            ),
            non_numeric(
                "uk-bus:AddressLine1",
                "ctxt-13",
                profile.address_lines.first().map(String::as_str).unwrap_or(""),
            ),
            non_numeric(
                "uk-bus:AddressLine2",
                "ctxt-13",
                profile.address_lines.get(1).map(String::as_str).unwrap_or(""),
            ),
            non_numeric(
                "uk-bus:PrincipalLocation-CityOrTown",
                "ctxt-13",
                &profile.location,
            ),
        ]);
        // The registered-office county is a voluntary fact: it is omitted
        // when the profile leaves it blank.
        if let Some(county) = profile.county.as_deref().filter(|v| !v.is_empty()) {
            hidden_children.push(non_numeric("uk-bus:CountyRegion", "ctxt-13", county));
        }
        // The postal code and the remaining voluntary contact facts
        // (e-mail, phone, website) follow in the reference order.
        hidden_children.push(non_numeric(
            "uk-bus:PostalCodeZip",
            "ctxt-13",
            &profile.postcode,
        ));
        for (name, ctx, value) in [
            ("uk-bus:E-mailAddress", "ctxt-13", profile.email.as_deref()),
            (
                "uk-bus:CountryCode",
                "ctxt-14",
                profile.phone_country.as_deref(),
            ),
            ("uk-bus:AreaCode", "ctxt-14", profile.phone_area.as_deref()),
            (
                "uk-bus:LocalNumber",
                "ctxt-14",
                profile.phone_number.as_deref(),
            ),
            (
                "uk-bus:WebsiteMainPageURL",
                "ctxt-13",
                profile.website_url.as_deref(),
            ),
            (
                "uk-bus:DescriptionOrOtherInformationOnWebsite",
                "ctxt-13",
                profile.website_description.as_deref(),
            ),
        ] {
            if let Some(value) = value.filter(|v| !v.is_empty()) {
                hidden_children.push(non_numeric(name, ctx, value));
            }
        }
        let hidden = elt("ix:hidden", &[]).children(hidden_children);

        let refs = elt("ix:references", &[]).children(vec![elt_text(
            "link:schemaRef",
            &[
                ("xlink:type", "simple"),
                (
                    "xlink:href",
                    "https://xbrl.frc.org.uk/FRS-102/2023-01-01/FRS-102-2023-01-01.xsd",
                ),
            ],
            "",
        )]);

        let mut resource_children = vec![
            context_duration(
                "ctxt-0",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
            ),
            context_instant(
                "ctxt-1",
                &company.company_number,
                &self.accounts.report_date,
                None,
                None,
            ),
            context_instant(
                "ctxt-2",
                &company.company_number,
                &period_end,
                None,
                None,
            ),
        ];
        // Dimensioned contexts: the JFCVC-mandatory ones (ctxt-4..7) are
        // always emitted (matching the hidden-header facts); the voluntary
        // ones (ctxt-3, ctxt-8) only when the profile supplies a non-empty
        // dimension value.
        if !profile.industry_sector_dimension.is_empty() {
            resource_children.push(context_duration_full(
                "ctxt-3",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-bus:MainIndustrySectorDimension",
                    &profile.industry_sector_dimension,
                )],
            ));
        }
        resource_children.push(context_duration_full(
            "ctxt-4",
            &company.company_number,
            &period_start,
            &period_end,
            None,
            None,
            &[("uk-bus:AccountingStandardsDimension", accounting_standards)],
        ));
        resource_children.push(context_duration_full(
            "ctxt-5",
            &company.company_number,
            &period_start,
            &period_end,
            None,
            None,
            &[("uk-bus:AccountsTypeDimension", accounts_type)],
        ));
        resource_children.push(context_duration_full(
            "ctxt-6",
            &company.company_number,
            &period_start,
            &period_end,
            None,
            None,
            &[("uk-bus:AccountsStatusDimension", accounts_status)],
        ));
        resource_children.push(context_duration_full(
            "ctxt-7",
            &company.company_number,
            &period_start,
            &period_end,
            None,
            None,
            &[("uk-bus:LegalFormEntityDimension", legal_form)],
        ));
        if !profile.country_dimension.is_empty() {
            resource_children.push(context_duration_full(
                "ctxt-8",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-geo:CountriesRegionsDimension",
                    &profile.country_dimension,
                )],
            ));
        }
        resource_children.extend(vec![context_duration(
                "ctxt-9",
                &company.company_number,
                &prev_start,
                &prev_end,
                None,
                None,
            ),
            context_duration_full(
                "ctxt-10",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[("uk-bus:EntityOfficersDimension", "uk-bus:Director1")],
            ),
        ]);
        for (i, dim_val) in ["uk-bus:Director2", "uk-bus:Director3"]
            .iter()
            .enumerate()
        {
            if profile.directors.len() > i + 1 {
                resource_children.push(context_duration_full(
                    &format!("ctxt-{}", 11 + i),
                    &company.company_number,
                    &period_start,
                    &period_end,
                    None,
                    None,
                    &[("uk-bus:EntityOfficersDimension", dim_val)],
                ));
            }
        }
        resource_children.extend(vec![
            context_duration_full(
                "ctxt-13",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-geo:CountriesRegionsDimension",
                    &profile.contact_country_dimension,
                )],
            ),
            context_duration_full(
                "ctxt-14",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-bus:PhoneNumberTypeDimension",
                    &profile.phone_type_dimension,
                )],
            ),
            context_instant("ctxt-15", &company.company_number, &period_end, None, None),
            context_instant("ctxt-16", &company.company_number, &prev_end, None, None),
            context_instant(
                "ctxt-17",
                &company.company_number,
                &period_end,
                Some("uk-core:MaturitiesOrExpirationPeriodsDimension"),
                Some("uk-core:WithinOneYear"),
            ),
            context_instant(
                "ctxt-18",
                &company.company_number,
                &prev_end,
                Some("uk-core:MaturitiesOrExpirationPeriodsDimension"),
                Some("uk-core:WithinOneYear"),
            ),
            context_instant(
                "ctxt-19",
                &company.company_number,
                &period_end,
                Some("uk-core:MaturitiesOrExpirationPeriodsDimension"),
                Some("uk-core:AfterOneYear"),
            ),
            context_instant(
                "ctxt-20",
                &company.company_number,
                &prev_end,
                Some("uk-core:MaturitiesOrExpirationPeriodsDimension"),
                Some("uk-core:AfterOneYear"),
            ),
            context_duration(
                "ctxt-21",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
            ),
            elt("xbrli:unit", &[("id", "GBP")])
                .child(elt_text("xbrli:measure", &[], "iso4217:GBP")),
            elt("xbrli:unit", &[("id", "pure")])
                .child(elt_text("xbrli:measure", &[], "xbrli:pure")),
        ]);
        let resources = elt("ix:resources", &[]).children(resource_children);

        let header = elt("ix:header", &[]).children(vec![hidden, refs, resources]);

        // -- Report pages ----------------------------------------------------

        let report_pages = vec![
            self.build_title_page(),
            self.build_company_info_page(),
            self.build_balance_sheet_page(&current_year, &prev_year),
            // Revision page placeholder (not revised: renders an empty div,
            // matching the reference output).
            el("div"),
            self.build_notes_page(&current_year, &prev_year),
        ];

        // -- Assemble the full document --------------------------------------

        let doc = elt("html", ACCTS_HTML_ATTRS).children(vec![
            elt("head", &[]).children(vec![
                elt_text("title", &[], REPORT_TITLE),
                elt_text(
                    "style",
                    &[("type", "text/css")],
                    include_str!("uk_frs105_accounts.css"),
                ),
            ]),
            elt("body", &[]).children(vec![
                // Inline `display:none`: Arelle's `headerDisplayNone` rule only
                // inspects inline style attributes, never stylesheet classes.
                elt("div", &[("style", "display:none")]).child(header),
                elt("div", &[("id", "report"), ("class", "report")]).children(report_pages),
            ]),
        ]);

        let body = doc.to_xml_string();
        // The reference serialises with lxml using HTML semantics: non-ASCII
        // punctuation is written as ASCII entities and empty spans keep
        // explicit open/close tags.  Reproduce both here.
        let body = body
            .replace('\u{00A0}', "&#160;")
            .replace('\u{00A3}', "&#163;")
            // lxml (the reference) writes apostrophes raw in attributes.
            .replace("&apos;", "'")
            .replace("<span/>", "<span></span>");
        format!("<?xml version='1.0' encoding='ASCII'?>\n{}", body)
    }

    /// Deserialise a [`Frs105Accounts`] from the [`XmlNode`] intermediate
    /// representation (step 2 of the round trip: XML string -> `XmlNode` ->
    /// `Frs105Accounts`).
    ///
    /// The `company` parameter supplies fields that are not serialised to
    /// iXBRL (`tax_reference`, `registration_date`); `accounts` supplies the
    /// financial-year tax parameters (also not serialised).  Fields that
    /// *are* serialised (name, company number, accounting-period dates) are
    /// recovered from the document and override the supplied values.
    /// Similarly, profile/report fields that have no iXBRL fact
    /// (`jurisdiction`, `signed_by`) come back empty.
    ///
    /// Balance-sheet values are rendered at whole pounds (`decimals = 0`),
    /// so the round trip preserves them to the nearest pound; the sign is
    /// recovered from the enclosing cell's `negative` class.
    pub fn from_ixbrl_node(
        node: &XmlNode,
        company: &Company,
        accounts: &AccountsMeta,
    ) -> Frs105Accounts {
        let facts = ParsedIxBrlFacts::from_node(node);
        let dims = xbrl_context_dimensions(node);

        // Numeric / non-numeric fact lookups.
        let num = |name: &str, ctx: &str| -> f64 {
            facts
                .numeric_by_ctx
                .get(&(name.to_string(), ctx.to_string()))
                .copied()
                .unwrap_or(0.0)
        };
        let text = |name: &str| -> String {
            facts.non_numeric.get(name).cloned().unwrap_or_default()
        };
        // The voluntary contact facts are omitted when blank, so their
        // fields come back `None` unless the document carries a value.
        let opt_text = |name: &str| -> Option<String> {
            facts
                .non_numeric
                .get(name)
                .cloned()
                .filter(|v| !v.is_empty())
        };
        let fallback_start = accounts.period().start;
        let parse_date = |raw: &str| -> chrono::NaiveDate {
            let cleaned = raw.replace('\u{00A0}', " ");
            chrono::NaiveDate::parse_from_str(&cleaned, "%d %B %Y")
                .unwrap_or(fallback_start)
        };

        // -- company / accounts --------------------------------------------------

        let period_start = parse_date(&text("uk-bus:StartDateForPeriodCoveredByReport"));
        let period_end = parse_date(&text("uk-bus:EndDateForPeriodCoveredByReport"));
        let prev_year = (period_start - chrono::Duration::days(1)).year();

        let company = Company {
            name: text("uk-bus:EntityCurrentLegalOrRegisteredName"),
            tax_reference: company.tax_reference.clone(), // not serialised
            company_number: text("uk-bus:UKCompaniesHouseRegisteredNumber"),
            registration_date: company.registration_date,
        };

        // The return period is serialised and recovered from the document;
        // the financial-year parameters come from the supplied `accounts`.
        let accounts = AccountsMeta {
            period: Some(AccountingPeriod {
                start: period_start,
                end: period_end,
            }),
            ..accounts.clone()
        };

        // -- balance-sheet values (signed, whole pounds) ----------------------

        let mut bs: HashMap<(String, String), f64> = HashMap::new();
        signed_non_fractions(node, false, &mut bs);
        let fact = |name: &str, ctx: &str| -> f64 {
            bs.get(&(name.to_string(), ctx.to_string()))
                .copied()
                .unwrap_or(0.0)
        };

        // -- embedded images ---------------------------------------------------

        let mut imgs: HashMap<String, String> = HashMap::new();
        img_src_by_alt(node, &mut imgs);

        // -- metadata -----------------------------------------------------------

        let dim = |ctx: &str, dimension: &str| -> String {
            dims.get(ctx)
                .and_then(|m| m.get(dimension))
                .cloned()
                .unwrap_or_default()
        };

        let directors: Vec<String> = ["ctxt-10", "ctxt-11", "ctxt-12"]
            .iter()
            .map(|c| {
                facts
                    .non_numeric_by_ctx
                    .get(&("uk-bus:NameEntityOfficer".to_string(), c.to_string()))
                    .cloned()
                    .unwrap_or_default()
            })
            .filter(|d| !d.is_empty())
            .collect();

        let address_lines: Vec<String> = [
            "uk-bus:AddressLine1",
            "uk-bus:AddressLine2",
            "uk-bus:AddressLine3",
        ]
        .iter()
        .map(|n| text(n))
        .filter(|a| !a.is_empty())
        .collect();

        let sic_codes: Vec<String> = [
            "uk-bus:SICCodeRecordedUKCompaniesHouse1",
            "uk-bus:SICCodeRecordedUKCompaniesHouse2",
            "uk-bus:SICCodeRecordedUKCompaniesHouse3",
            "uk-bus:SICCodeRecordedUKCompaniesHouse4",
        ]
        .iter()
        .map(|n| text(n))
        .filter(|s| !s.is_empty())
        .collect();

        let profile = CompanyProfile {
            directors,
            contact_name: text("uk-bus:NameContactDepartmentOrPerson"),
            address_lines,
            county: opt_text("uk-bus:CountyRegion"),
            location: text("uk-bus:PrincipalLocation-CityOrTown"),
            postcode: text("uk-bus:PostalCodeZip"),
            email: opt_text("uk-bus:E-mailAddress"),
            phone_country: opt_text("uk-bus:CountryCode"),
            phone_area: opt_text("uk-bus:AreaCode"),
            phone_number: opt_text("uk-bus:LocalNumber"),
            website_url: opt_text("uk-bus:WebsiteMainPageURL"),
            website_description: opt_text("uk-bus:DescriptionOrOtherInformationOnWebsite"),
            vat_registration: opt_text("uk-bus:VATRegistrationNumber"),
            sic_codes,
            // The placeholder counts as "no description" on the way back in
            // (a blank profile round-trips to `None`).
            activities: opt_text("uk-bus:DescriptionPrincipalActivities")
                .filter(|v| v != DEFAULT_PRINCIPAL_ACTIVITIES),
            jurisdiction: String::new(), // not serialised to iXBRL
            accountant_name: text("uk-accrep:NameAccountantResponsible"),
            accountant_business: text("uk-bus:NameEntityAccountants"),
            accountant_address: text("uk-accrep:NameOrLocationAccountantsOffice"),
            auditor_name: text("uk-aurep:NameIndividualAuditor"),
            auditor_business: text("uk-bus:NameEntityAuditors"),
            auditor_address: text("uk-aurep:NameOrLocationOfficePerformingAudit"),
            industry_sector_dimension: dim("ctxt-3", "uk-bus:MainIndustrySectorDimension"),
            legal_form_dimension: dim("ctxt-7", "uk-bus:LegalFormEntityDimension"),
            country_dimension: dim("ctxt-8", "uk-geo:CountriesRegionsDimension"),
            contact_country_dimension: dim("ctxt-13", "uk-geo:CountriesRegionsDimension"),
            phone_type_dimension: dim("ctxt-14", "uk-bus:PhoneNumberTypeDimension"),
            logo_b64: imgs.get("Company logo").cloned(),
        };

        // The report metadata (dates, signatory, employee counts and the
        // accounts-related dimensions) is serialised to iXBRL and recovered
        // from the document; the period and fy parameters come from the
        // earlier `accounts` binding.
        let accounts = AccountsMeta {
            report_date: parse_date(&text("uk-bus:BusinessReportPublicationDate")),
            authorised_date: Some(parse_date(
                &text("uk-core:DateAuthorisationFinancialStatementsForIssue"),
            )),
            incorporation_date: parse_date(&text("uk-bus:DateFormationOrIncorporation")),
            signed_by: String::new(), // not serialised to iXBRL
            average_employees: HashMap::from([
                (period_end.year().to_string(), num("uk-core:AverageNumberEmployeesDuringPeriod", "ctxt-0") as u32),
                (prev_year.to_string(), num("uk-core:AverageNumberEmployeesDuringPeriod", "ctxt-9") as u32),
            ]),
            accounting_standards_dimension: dim("ctxt-4", "uk-bus:AccountingStandardsDimension"),
            accounts_type_dimension: dim("ctxt-5", "uk-bus:AccountsTypeDimension"),
            accounts_status_dimension: dim("ctxt-6", "uk-bus:AccountsStatusDimension"),
            signature_b64: imgs.get("Director's signature").cloned(),
            ..accounts
        };

        Frs105Accounts {
            company,
            profile,
            accounts,
            // Parsed from iXBRL: no ledger, so the seeding step in
            // `with_prev_period_data` has no book to read activity from.
            book: None,
            prev_book: None,
            // The signing date is recovered from the document (explicit);
            // the deadlines only back the default when it is absent.
            filing_deadlines: FilingDeadlines::from_period_end(period_end),
            fixed_assets: [
                fact("uk-core:FixedAssets", "ctxt-15"),
                fact("uk-core:FixedAssets", "ctxt-16"),
            ],
            called_up_share_capital_not_paid: [
                fact(
                    "uk-core:CalledUpShareCapitalNotPaidNotExpressedAsCurrentAsset",
                    "ctxt-15",
                ),
                fact(
                    "uk-core:CalledUpShareCapitalNotPaidNotExpressedAsCurrentAsset",
                    "ctxt-16",
                ),
            ],
            current_assets: [
                fact("uk-core:CurrentAssets", "ctxt-15"),
                fact("uk-core:CurrentAssets", "ctxt-16"),
            ],
            prepayments_and_accrued_income: [
                fact(
                    "uk-core:PrepaymentsAccruedIncomeNotExpressedWithinCurrentAssetSubtotal",
                    "ctxt-15",
                ),
                fact(
                    "uk-core:PrepaymentsAccruedIncomeNotExpressedWithinCurrentAssetSubtotal",
                    "ctxt-16",
                ),
            ],
            creditors_within_1_year: [
                fact("uk-core:Creditors", "ctxt-17"),
                fact("uk-core:Creditors", "ctxt-18"),
            ],
            net_current_assets: [
                fact("uk-core:NetCurrentAssetsLiabilities", "ctxt-15"),
                fact("uk-core:NetCurrentAssetsLiabilities", "ctxt-16"),
            ],
            total_assets_less_liabilities: [
                fact("uk-core:TotalAssetsLessCurrentLiabilities", "ctxt-15"),
                fact("uk-core:TotalAssetsLessCurrentLiabilities", "ctxt-16"),
            ],
            creditors_after_1_year: [
                fact("uk-core:Creditors", "ctxt-19"),
                fact("uk-core:Creditors", "ctxt-20"),
            ],
            provisions_for_liabilities: [
                fact("uk-core:ProvisionsForLiabilitiesBalanceSheetSubtotal", "ctxt-15"),
                fact("uk-core:ProvisionsForLiabilitiesBalanceSheetSubtotal", "ctxt-16"),
            ],
            accruals_and_deferred_income: [
                fact("uk-core:AccruedLiabilitiesDeferredIncome", "ctxt-15"),
                fact("uk-core:AccruedLiabilitiesDeferredIncome", "ctxt-16"),
            ],
            net_assets: [
                fact("uk-core:NetAssetsLiabilities", "ctxt-15"),
                fact("uk-core:NetAssetsLiabilities", "ctxt-16"),
            ],
            capital_and_reserves: [
                fact("uk-core:Equity", "ctxt-15"),
                fact("uk-core:Equity", "ctxt-16"),
            ],
        }
    }

    /// Verify the previous-period comparative column against a filed balance
    /// sheet: the comparative column was computed from the previous-period
    /// chart of accounts ([`Self::with_prev_period_data`]'s `prev.book`),
    /// and each of the 12 lines is compared with the filing's figures.  The
    /// filing's period end must also align with the previous period's end
    /// (the day before the current period starts).
    ///
    /// Returns `Ok(())` when the book reconciles with the filing;
    /// otherwise every mismatch is collected into a [`CheckErrors`] — one
    /// [`Frs105CheckError`] per differing line (naming the accounts or
    /// derivation the computation read) and/or the period-dates mismatch —
    /// rather than failing at the first.
    pub fn check_previous_period_matches_filing(
        &self,
        filing: &FiledBalanceSheet,
    ) -> Result<(), CheckErrors> {
        let prev_end = self.accounts.period().start - chrono::Duration::days(1);
        let mut errors: Vec<Frs105CheckError> = Vec::new();
        let mut check = |label: &'static str, computed: f64, filed: f64| {
            if (computed - filed).abs() > 0.005 {
                errors.push(line_mismatch_error(label, computed, filed));
            }
        };
        check("fixed assets", self.fixed_assets[1], filing.figures.fixed_assets);
        check(
            "called up share capital not paid",
            self.called_up_share_capital_not_paid[1],
            filing.figures.called_up_share_capital_not_paid,
        );
        check("current assets", self.current_assets[1], filing.figures.current_assets);
        check(
            "prepayments and accrued income",
            self.prepayments_and_accrued_income[1],
            filing.figures.prepayments_and_accrued_income,
        );
        check(
            "creditors within one year",
            self.creditors_within_1_year[1],
            filing.figures.creditors_within_1_year,
        );
        check(
            "net current assets",
            self.net_current_assets[1],
            filing.figures.net_current_assets,
        );
        check(
            "total assets less liabilities",
            self.total_assets_less_liabilities[1],
            filing.figures.total_assets_less_liabilities,
        );
        check(
            "creditors after one year",
            self.creditors_after_1_year[1],
            filing.figures.creditors_after_1_year,
        );
        check(
            "provisions for liabilities",
            self.provisions_for_liabilities[1],
            filing.figures.provisions_for_liabilities,
        );
        check(
            "accruals and deferred income",
            self.accruals_and_deferred_income[1],
            filing.figures.accruals_and_deferred_income,
        );
        check("net assets", self.net_assets[1], filing.figures.net_assets);
        check(
            "capital and reserves",
            self.capital_and_reserves[1],
            filing.figures.capital_and_reserves,
        );
        if filing.period_end != prev_end {
            errors.push(Frs105CheckError::PeriodDatesMismatch {
                filed_end: filing.period_end,
                prev_end,
            });
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(CheckErrors { errors })
        }
    }

    /// Verify the balance-sheet identities hold in **both** columns, with
    /// the reports' sign conventions (creditor lines negative, provisions
    /// and accruals positive magnitudes):
    ///
    /// 1. `net current assets == current assets + prepayments and accrued
    ///    income + creditors within one year`
    /// 2. `total assets less liabilities == fixed assets + net current
    ///    assets + called up share capital not paid`
    /// 3. `net assets == total assets less liabilities + creditors after
    ///    one year − provisions for liabilities − accruals and deferred
    ///    income`
    /// 4. `net assets == capital and reserves + called up share capital
    ///    not paid` — the fundamental A − L = E tie.
    ///
    /// Identities 1–3 hold by construction for the computed report (they
    /// *are* the computation); identity 4 is the genuine check — it fails
    /// when the underlying book is unbalanced.  Called-up share capital
    /// not paid (line A) appears in 2 and 4 because the builder adds it to
    /// the totals but not to `net_current_assets` or `capital_and_reserves`.
    ///
    /// Returns `Ok(())` when both columns tie; otherwise every violation
    /// is collected into a [`BalanceSheetCheckErrors`] rather than failing
    /// at the first.
    pub fn validate(&self) -> Result<(), BalanceSheetCheckErrors> {
        let mut errors = Vec::new();
        for (i, column) in [(0usize, "current"), (1usize, "previous")] {
            let net_current = self.current_assets[i]
                + self.prepayments_and_accrued_income[i]
                + self.creditors_within_1_year[i];
            if (self.net_current_assets[i] - net_current).abs() > 0.005 {
                errors.push(BalanceSheetCheckError::NetCurrentAssetsMismatch {
                    column,
                    computed: self.net_current_assets[i],
                    derived: net_current,
                });
            }
            let total = self.fixed_assets[i]
                + self.net_current_assets[i]
                + self.called_up_share_capital_not_paid[i];
            if (self.total_assets_less_liabilities[i] - total).abs() > 0.005 {
                errors.push(BalanceSheetCheckError::TotalAssetsLessLiabilitiesMismatch {
                    column,
                    computed: self.total_assets_less_liabilities[i],
                    derived: total,
                });
            }
            let net_assets = self.total_assets_less_liabilities[i]
                + self.creditors_after_1_year[i]
                - self.provisions_for_liabilities[i]
                - self.accruals_and_deferred_income[i];
            if (self.net_assets[i] - net_assets).abs() > 0.005 {
                errors.push(BalanceSheetCheckError::NetAssetsMismatch {
                    column,
                    computed: self.net_assets[i],
                    derived: net_assets,
                });
            }
            let capital = self.capital_and_reserves[i] + self.called_up_share_capital_not_paid[i];
            if (self.net_assets[i] - capital).abs() > 0.005 {
                errors.push(BalanceSheetCheckError::BalanceSheetDoesNotBalance {
                    column,
                    net_assets: self.net_assets[i],
                    capital_and_reserves: capital,
                });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(BalanceSheetCheckErrors { errors })
        }
    }

    /// Supply the called-up share capital not paid (line A of the FRS 105
    /// balance-sheet format, above fixed assets): the amount called up on
    /// allotted shares but not yet received as at the balance-sheet date.
    /// Defaults to zero — micro-entities need not disclose it separately,
    /// and the iXBRL row is rendered only when a nonzero amount is
    /// supplied.  Line A is part of the total-assets-less-current-
    /// liabilities and net-assets totals, so the amount is added to both
    /// columns of those fields; `net_current_assets` is left unchanged.
    pub fn with_called_up_share_capital_not_paid(mut self, amount: [f64; 2]) -> Self {
        self.called_up_share_capital_not_paid = amount;
        for (i, &a) in amount.iter().enumerate() {
            self.total_assets_less_liabilities[i] += a;
            self.net_assets[i] += a;
        }
        self
    }

    /// Deserialise a [`Frs105Accounts`] from its serialised iXBRL HTML, in
    /// two steps: first into the [`XmlNode`] intermediate representation,
    /// then into the struct.
    ///
    /// The supplied `accounts` must carry a return period; only its
    /// financial-year parameters are preserved (the period is recovered from
    /// the document).
    pub fn from_ixbrl(
        html: &str,
        company: &Company,
        accounts: &AccountsMeta,
    ) -> Result<Frs105Accounts, String> {
        let node = XmlNode::from_xml_string(html)?;
        Ok(Self::from_ixbrl_node(&node, company, accounts))
    }

    // -- Page builders --------------------------------------------------------

    /// Title page: company number, logo, company name, report title and the
    /// period end date.
    fn build_title_page(&self) -> XmlNode {
        let company = &self.company;
        let profile = &self.profile;
        let mut title_children = vec![div("company-number", vec![span(vec![
            span_text(" Company registration no. "),
            span(vec![non_numeric(
                "uk-bus:UKCompaniesHouseRegisteredNumber",
                "ctxt-0",
                &company.company_number,
            )]),
            span_text(" ("),
            span(vec![span_text(&profile.jurisdiction)]),
            span_text(")"),
        ])])];
        // The logo is optional: omit the image (and the empty data URI)
        // when none is supplied.
        if let Some(logo) = profile.logo_b64.as_deref().filter(|logo| !logo.is_empty()) {
            title_children.push(elt(
                "img",
                &[
                    ("alt", "Company logo"),
                    ("src", &format!("data:image/png;base64,{logo}")),
                ],
            ));
        }
        title_children.push(div("company-name", vec![span(vec![span(vec![non_numeric(
            "uk-bus:EntityCurrentLegalOrRegisteredName",
            "ctxt-0",
            &company.name,
        )])])]));
        title_children.push(div("title", vec![span(vec![span(vec![non_numeric(
            "uk-bus:ReportTitle",
            "ctxt-0",
            REPORT_TITLE,
        )])])]));
        title_children.push(div("subtitle", vec![span(vec![
            span_text("For the year ended "),
            span(vec![date_fact(
                "uk-bus:EndDateForPeriodCoveredByReport",
                "ctxt-1",
                &self.accounts.period().end,
            )]),
        ])]));
        page(vec![div("titlepage", title_children)])
    }

    /// Company-information page: header, then a table of directors, company
    /// number, registered office, accountant and auditor.
    fn build_company_info_page(&self) -> XmlNode {
        let company = &self.company;
        let profile = &self.profile;

        let directors_cell = td_no_class(
            profile
                .directors
                .iter()
                .enumerate()
                .flat_map(|(i, director)| {
                    vec![elt("div", &[]).children(vec![
                        span(vec![span(vec![non_numeric(
                            "uk-bus:NameEntityOfficer",
                            &format!("ctxt-{}", 10 + i),
                            director,
                        )])]),
                        el("br"),
                    ])]
                })
                .collect(),
        );

        let company_number_cell = td_no_class(vec![span(vec![
            span_text(" "),
            span(vec![non_numeric(
                "uk-bus:UKCompaniesHouseRegisteredNumber",
                "ctxt-0",
                &company.company_number,
            )]),
            span_text(", registered in "),
            span(vec![span_text(&profile.jurisdiction)]),
        ])]);

        let office_children: Vec<XmlNode> = profile
            .address_lines
            .iter()
            .enumerate()
            .flat_map(|(i, line)| {
                let fact = match i {
                    0 => "uk-bus:AddressLine1",
                    1 => "uk-bus:AddressLine2",
                    _ => "uk-bus:AddressLine3",
                };
                vec![elt("div", &[]).children(vec![
                    span(vec![span(vec![non_numeric(fact, "ctxt-13", line), span_text(", ")])]),
                    el("br"),
                ])]
            })
            .chain(std::iter::once(span(vec![
                span(vec![
                    non_numeric(
                        "uk-bus:PrincipalLocation-CityOrTown",
                        "ctxt-13",
                        &profile.location,
                    ),
                    span_text(", "),
                ]),
                span_text(" "),
                span(vec![non_numeric(
                    "uk-bus:PostalCodeZip",
                    "ctxt-13",
                    &profile.postcode,
                )]),
            ])))
            .collect();
        let registered_office_cell = td_no_class(office_children);

        let accountant_cell = td_no_class(vec![
            span(vec![span(vec![non_numeric(
                "uk-accrep:NameAccountantResponsible",
                "ctxt-0",
                &profile.accountant_name,
            )])]),
            el("br"),
            span(vec![span(vec![non_numeric(
                "uk-bus:NameEntityAccountants",
                "ctxt-0",
                &profile.accountant_business,
            )])]),
            el("br"),
            span(vec![span(vec![non_numeric(
                "uk-accrep:NameOrLocationAccountantsOffice",
                "ctxt-0",
                &profile.accountant_address,
            )])]),
        ]);

        let auditor_cell = td_no_class(vec![
            span(vec![span(vec![non_numeric(
                "uk-aurep:NameIndividualAuditor",
                "ctxt-0",
                &profile.auditor_name,
            )])]),
            el("br"),
            span(vec![span(vec![non_numeric(
                "uk-bus:NameEntityAuditors",
                "ctxt-0",
                &profile.auditor_business,
            )])]),
            el("br"),
            span(vec![span(vec![non_numeric(
                "uk-aurep:NameOrLocationOfficePerformingAudit",
                "ctxt-0",
                &profile.auditor_address,
            )])]),
        ]);

        let table = elt("table", &[("class", "company-info")]).children(vec![
            elt("tr", &[]).children(vec![
                elt("td", &[("class", "tag")]).child(span_text("Directors")),
                directors_cell,
            ]),
            elt("tr", &[]).children(vec![
                elt("td", &[("class", "tag")]).child(span_text("Company number")),
                company_number_cell,
            ]),
            elt("tr", &[]).children(vec![
                elt("td", &[("class", "tag")]).child(span_text("Registered office")),
                registered_office_cell,
            ]),
            elt("tr", &[]).children(vec![
                elt("td", &[("class", "tag")]).child(span_text("Accountant")),
                accountant_cell,
            ]),
            elt("tr", &[]).children(vec![
                elt("td", &[("class", "tag")]).child(span_text("Auditor")),
                auditor_cell,
            ]),
        ]);

        page(vec![elt("div", &[]).children(vec![
            self.page_header("Company information", HeaderSubtitle::ForYearEnded),
            table,
        ])])
    }

    /// Statement of financial position: header, the balance-sheet worksheet,
    /// the statutory notes paragraphs and the approval / signature block.
    fn build_balance_sheet_page(&self, current_year: &str, prev_year: &str) -> XmlNode {
        let a = &self;
        let mut rows = vec![
            worksheet_header_row_accts(current_year, prev_year),
            worksheet_currency_row_accts(),
            spacer_row(),
        ];
        // Called-up share capital not paid is line A of the FRS 105
        // balance-sheet format, shown above fixed assets.  Micro-entities
        // need not disclose it separately, so the row is omitted unless
        // the setter supplied a nonzero amount.
        if a.called_up_share_capital_not_paid[0] != 0.0
            || a.called_up_share_capital_not_paid[1] != 0.0
        {
            rows.push(bs_row(
                "Called up share capital not paid",
                "uk-core:CalledUpShareCapitalNotPaidNotExpressedAsCurrentAsset",
                "ctxt-15",
                "ctxt-16",
                a.called_up_share_capital_not_paid[0],
                a.called_up_share_capital_not_paid[1],
            ));
            rows.push(spacer_row());
        }
        rows.extend(vec![
            bs_row(
                "Fixed Assets",
                "uk-core:FixedAssets",
                "ctxt-15",
                "ctxt-16",
                a.fixed_assets[0],
                a.fixed_assets[1],
            ),
            spacer_row(),
            bs_row(
                "Current Assets",
                "uk-core:CurrentAssets",
                "ctxt-15",
                "ctxt-16",
                a.current_assets[0],
                a.current_assets[1],
            ),
            spacer_row(),
            bs_row(
                "Prepayments and Accrued Income",
                "uk-core:PrepaymentsAccruedIncomeNotExpressedWithinCurrentAssetSubtotal",
                "ctxt-15",
                "ctxt-16",
                a.prepayments_and_accrued_income[0],
                a.prepayments_and_accrued_income[1],
            ),
            spacer_row(),
            bs_row(
                "Creditors: falling due within one year",
                "uk-core:Creditors",
                "ctxt-17",
                "ctxt-18",
                a.creditors_within_1_year[0],
                a.creditors_within_1_year[1],
            ),
            spacer_row(),
            bs_row(
                "Net Current Assets",
                "uk-core:NetCurrentAssetsLiabilities",
                "ctxt-15",
                "ctxt-16",
                a.net_current_assets[0],
                a.net_current_assets[1],
            ),
            spacer_row(),
            bs_row(
                "Total Assets Less Liabilities",
                "uk-core:TotalAssetsLessCurrentLiabilities",
                "ctxt-15",
                "ctxt-16",
                a.total_assets_less_liabilities[0],
                a.total_assets_less_liabilities[1],
            ),
            spacer_row(),
            bs_row(
                "Creditors: falling due after one year",
                "uk-core:Creditors",
                "ctxt-19",
                "ctxt-20",
                a.creditors_after_1_year[0],
                a.creditors_after_1_year[1],
            ),
            spacer_row(),
            bs_row(
                "Provisions For Liabilities",
                "uk-core:ProvisionsForLiabilitiesBalanceSheetSubtotal",
                "ctxt-15",
                "ctxt-16",
                a.provisions_for_liabilities[0],
                a.provisions_for_liabilities[1],
            ),
            spacer_row(),
            bs_row(
                "Accrued liabilities and deferred income",
                "uk-core:AccruedLiabilitiesDeferredIncome",
                "ctxt-15",
                "ctxt-16",
                a.accruals_and_deferred_income[0],
                a.accruals_and_deferred_income[1],
            ),
            spacer_row(),
            bs_row(
                "Net Assets",
                "uk-core:NetAssetsLiabilities",
                "ctxt-15",
                "ctxt-16",
                a.net_assets[0],
                a.net_assets[1],
            ),
            spacer_row(),
            bs_row(
                "Capital and Reserves",
                "uk-core:Equity",
                "ctxt-15",
                "ctxt-16",
                a.capital_and_reserves[0],
                a.capital_and_reserves[1],
            ),
        ]);

        let notes = elt("div", &[("class", "notes")]).child(elt("div", &[]).children(vec![
            statement_p(
                "uk-direp:StatementThatAccountsHaveBeenPreparedInAccordanceWithProvisionsSmallCompaniesRegime",
                vec![span_text(
                    "These financial statements have been prepared in accordance with the micro-entity provisions and delivered in accordance with the provisions applicable under the small companies regime.",
                )],
            ),
            statement_p(
                "uk-direp:StatementThatCompanyEntitledToExemptionFromAuditUnderSection477CompaniesAct2006RelatingToSmallCompanies",
                vec![
                    span_text("For the accounting period ending "),
                    span(vec![date_fact(
                        "uk-bus:EndDateForPeriodCoveredByReport",
                        "ctxt-1",
                        &self.accounts.period().end,
                    )]),
                    span_text(
                        " the company was entitled to exemption from audit under section 477 of the Companies Act 2006 relating to small companies.",
                    ),
                ],
            ),
            statement_p(
                "uk-direp:StatementThatMembersHaveNotRequiredCompanyToObtainAnAudit",
                vec![span_text(
                    "The members have not required the company to obtain an audit of its financial statements for the accounting period in accordance with section 476.",
                )],
            ),
            statement_p(
                "uk-direp:StatementThatDirectorsAcknowledgeTheirResponsibilitiesUnderCompaniesAct",
                vec![span_text(
                    "The directors acknowledge their responsibilities for complying with the requirements of the Act with respect to accounting records and the preparation of financial statements.",
                )],
            ),
        ]));

        let mut approval_children = vec![
            elt("p", &[]).child(span(vec![
                span_text("Approved by the board of directors and authorised for publication on "),
                span(vec![date_fact(
                    "uk-core:DateAuthorisationFinancialStatementsForIssue",
                    "ctxt-2",
                    &self.signing_date(),
                )]),
                span_text("."),
            ])),
            elt("p", &[]).child(span(vec![
                span_text("Signed on behalf of the board, by "),
                span(vec![span_text(&self.accounts.signed_by)]),
                span_text("."),
            ])),
        ];
        // The signature image is always embedded: the supplied one, or the
        // bundled default ([`Self::signature_b64`]).
        approval_children.push(elt(
            "img",
            &[
                ("alt", "Director's signature"),
                ("src", &format!("data:image/png;base64,{}", self.signature_b64())),
            ],
        ));
        let approval = elt("div", &[]).child(elt("div", &[]).children(approval_children));

        page(vec![elt("div", &[("class", "sheet-page")]).children(vec![
            self.page_header("Statement of financial position", HeaderSubtitle::AsAt),
            worksheet(vec![table("sheet table", rows)]),
            notes,
            approval,
        ])])
    }

    /// Notes to the accounts: company-information note and employees note.
    fn build_notes_page(&self, current_year: &str, prev_year: &str) -> XmlNode {
        let company = &self.company;
        let profile = &self.profile;
        // Employee figures are indexed by calendar year in the metadata.
        let employees_cur_year = self.accounts.period().end.year();
        let employees_prev_year =
            (self.accounts.period().start - chrono::Duration::days(1)).year();

        let company_note = elt("div", &[]).children(vec![elt("div", &[]).children(vec![
            elt("div", &[]).child(elt_text(
                "h3",
                &[("class", "noteheading")],
                "1. Company information",
            )),
            elt("p", &[]).child(span(vec![
                span_text(
                    "The company is a private company limited by shares and is registered in England and Wales number ",
                ),
                span(vec![non_numeric(
                    "uk-bus:UKCompaniesHouseRegisteredNumber",
                    "ctxt-0",
                    &company.company_number,
                )]),
                span_text(". The registered address is: "),
                span(vec![
                    non_numeric(
                        "uk-bus:AddressLine1",
                        "ctxt-13",
                        profile.address_lines.first().map(String::as_str).unwrap_or(""),
                    ),
                    span_text(", "),
                ]),
                span_text(" "),
                span(vec![
                    non_numeric(
                        "uk-bus:AddressLine2",
                        "ctxt-13",
                        profile.address_lines.get(1).map(String::as_str).unwrap_or(""),
                    ),
                    span_text(", "),
                ]),
                span_text(" "),
                span(vec![]),
                span_text(" "),
                span(vec![
                    non_numeric(
                        "uk-bus:PrincipalLocation-CityOrTown",
                        "ctxt-13",
                        &profile.location,
                    ),
                    span_text(" "),
                ]),
                span_text(" "),
                span(vec![non_numeric(
                    "uk-bus:PostalCodeZip",
                    "ctxt-13",
                    &profile.postcode,
                )]),
                span_text("."),
            ])),
        ])]);

        let employees_note = elt("div", &[]).children(vec![elt("div", &[]).children(vec![
            elt("div", &[]).child(elt_text(
                "h3",
                &[("class", "noteheading")],
                "2. Employees",
            )),
            elt("p", &[]).child(span_text(
                "The average monthly number of persons employed by the company (including directors) during the period was as follows:",
            )),
            elt("table", &[("class", "sheet table")]).children(vec![
                elt("tr", &[]).children(vec![
                    elt("td", &[("class", "label")]).child(span(vec![])),
                    elt("td", &[("class", "column header")])
                        .child(span(vec![span(vec![span_text(current_year)])])),
                    elt("td", &[("class", "column header")])
                        .child(span(vec![span(vec![span_text(prev_year)])])),
                ]),
                elt("tr", &[]).children(vec![
                    elt("td", &[("class", "label heading")]).child(span_text("Employees")),
                    elt("td", &[("class", "data value")]).child(span(vec![span(vec![
                        employees_non_fraction(
                            "ctxt-0",
                            &self.accounts.average_employees_for(employees_cur_year).to_string(),
                        ),
                    ])])),
                    elt("td", &[("class", "data value")]).child(span(vec![span(vec![
                        employees_non_fraction(
                            "ctxt-9",
                            &self.accounts.average_employees_for(employees_prev_year).to_string(),
                        ),
                    ])])),
                ]),
            ]),
        ])]);

        page(vec![elt("div", &[]).children(vec![
            self.page_header("Notes to the accounts", HeaderSubtitle::ForYearEnded),
            company_note,
            employees_note,
        ])])
    }

    /// The page header block: company name, page title and a subtitle date.
    fn page_header(&self, title: &str, subtitle: HeaderSubtitle) -> XmlNode {
        let company = &self.company;
        let subtitle_span = match subtitle {
            HeaderSubtitle::ForYearEnded => span(vec![
                span_text("For the year ended "),
                span(vec![date_fact(
                    "uk-bus:EndDateForPeriodCoveredByReport",
                    "ctxt-1",
                    &self.accounts.period().end,
                )]),
            ]),
            HeaderSubtitle::AsAt => span(vec![
                span_text("As at "),
                span(vec![date_fact(
                    "uk-bus:BalanceSheetDate",
                    "ctxt-2",
                    &self.accounts.period().end,
                )]),
            ]),
        };
        elt("div", &[("class", "header")]).children(vec![
            elt("div", &[]).child(span(vec![span(vec![non_numeric(
                "uk-bus:EntityCurrentLegalOrRegisteredName",
                "ctxt-0",
                &company.name,
            )])])),
            elt("div", &[]).child(span_text(title)),
            elt("div", &[]).child(subtitle_span),
            el("hr"),
        ])
    }
}

/// Which subtitle a page header shows.
enum HeaderSubtitle {
    /// "For the year ended <period end>".
    ForYearEnded,
    /// "As at <balance-sheet date>".
    AsAt,
}

// ============================================================================
// Rendering helpers
// ============================================================================

/// A `td` without a class attribute.
fn td_no_class(children: Vec<XmlNode>) -> XmlNode {
    elt("td", &[]).children(children)
}

/// A `<p><span><ix:nonNumeric ...>...</ix:nonNumeric></span></p>` statement
/// used in the balance-sheet notes, tagged with the given fact name.
fn statement_p(name: &str, content: Vec<XmlNode>) -> XmlNode {
    elt("p", &[]).child(span(vec![elt(
        "ix:nonNumeric",
        &[("name", name), ("contextRef", "ctxt-21")],
    )
    .children(content)]))
}

/// A balance-sheet money fact: decimals 0, scale 0, GBP unit, with the
/// reference's attribute order.
fn accts_non_fraction(name: &str, ctx: &str, value: &str) -> XmlNode {
    elt_text(
        "ix:nonFraction",
        &[
            ("name", name),
            ("contextRef", ctx),
            ("format", "ixt2:numdotdecimal"),
            ("unitRef", "GBP"),
            ("decimals", "0"),
            ("scale", "0"),
        ],
        value,
    )
}

/// An average-employees fact: pure unit, decimals 0, no format/scale.
fn employees_non_fraction(ctx: &str, value: &str) -> XmlNode {
    elt_text(
        "ix:nonFraction",
        &[
            ("name", "uk-core:AverageNumberEmployeesDuringPeriod"),
            ("contextRef", ctx),
            ("unitRef", "pure"),
            ("decimals", "0"),
        ],
        value,
    )
}

/// A dated non-numeric fact with the reference's date format.
fn date_fact(name: &str, ctx: &str, date: &chrono::NaiveDate) -> XmlNode {
    non_numeric_fmt(name, ctx, &format_date(date), "ixt2:datedaymonthyearen")
}

/// A balance-sheet worksheet header row (colspan 1 on each column header).
fn worksheet_header_row_accts(current_year: &str, prev_year: &str) -> XmlNode {
    tr(
        Some("row"),
        vec![
            td("label cell", vec![nbsp()]),
            elt_text(
                "td",
                &[("class", "column header cell"), ("colspan", "1")],
                current_year,
            ),
            elt_text(
                "td",
                &[("class", "column header cell"), ("colspan", "1")],
                prev_year,
            ),
        ],
    )
}

/// A balance-sheet currency row (colspan 1 on each currency cell).
fn worksheet_currency_row_accts() -> XmlNode {
    tr(
        Some("row"),
        vec![
            td("label cell", vec![nbsp()]),
            elt_text(
                "td",
                &[("class", "column currency cell"), ("colspan", "1")],
                "\u{00A3}",
            ),
            elt_text(
                "td",
                &[("class", "column currency cell"), ("colspan", "1")],
                "\u{00A3}",
            ),
        ],
    )
}

/// A balance-sheet total row: label + current/previous value cells.
fn bs_row(
    label: &str,
    name: &str,
    ctx_cur: &str,
    ctx_prev: &str,
    val_cur: f64,
    val_prev: f64,
) -> XmlNode {
    tr(
        Some("row"),
        vec![
            td("label heading total cell", vec![span_text(label)]),
            bs_cell(name, ctx_cur, val_cur),
            bs_cell(name, ctx_prev, val_prev),
        ],
    )
}

/// A balance-sheet value cell: negative values in parens, zero as a nil
/// cell, positive as a plain total cell.
fn bs_cell(name: &str, ctx: &str, value: f64) -> XmlNode {
    let formatted = format_f64_0(value.abs());
    if value < 0.0 {
        td(
            "data value total negative cell",
            vec![span(vec![
                span_text("( "),
                accts_non_fraction(name, ctx, &formatted),
                span_text(" )"),
            ])],
        )
    } else if value == 0.0 {
        td(
            "data value total nil cell",
            vec![span(vec![
                el("span"),
                accts_non_fraction(name, ctx, "0"),
                span_space2(),
            ])],
        )
    } else {
        td(
            "data value total cell",
            vec![span(vec![
                el("span"),
                accts_non_fraction(name, ctx, &formatted),
                span_space2(),
            ])],
        )
    }
}

/// Collect every `ix:nonFraction` fact with the sign recovered from the
/// enclosing `td` cell: a cell whose class contains `negative` renders the
/// value in parentheses, so the fact is stored negated.
fn signed_non_fractions(
    node: &XmlNode,
    in_negative_cell: bool,
    out: &mut HashMap<(String, String), f64>,
) {
    if let XmlNode::Elem {
        name,
        attributes,
        children,
    } = node
    {
        let is_negative_cell = name == "td"
            && attributes
                .iter()
                .any(|(k, v)| k == "class" && v.contains("negative"));
        let negative = in_negative_cell || is_negative_cell;

        if name == "ix:nonFraction" {
            let fact_name = attributes
                .iter()
                .find(|(k, _)| k == "name")
                .map(|(_, v)| v.clone());
            let ctx = attributes
                .iter()
                .find(|(k, _)| k == "contextRef")
                .map(|(_, v)| v.clone());
            let value: String = children
                .iter()
                .filter_map(|c| match c {
                    XmlNode::Text(t) => Some(t.trim().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ");
            let cleaned = value.replace(',', "");
            if let (Some(fact_name), Some(ctx), Ok(v)) =
                (fact_name, ctx, cleaned.parse::<f64>())
            {
                let v = if negative { -v } else { v };
                out.insert((fact_name, ctx), v);
            }
        }
        for child in children {
            signed_non_fractions(child, negative, out);
        }
    }
}

/// Collect the base64 payload of every `<img>` by its `alt` text.
fn img_src_by_alt(node: &XmlNode, out: &mut HashMap<String, String>) {
    if let XmlNode::Elem {
        name,
        attributes,
        children,
    } = node
    {
        if name == "img" {
            let alt = attributes
                .iter()
                .find(|(k, _)| k == "alt")
                .map(|(_, v)| v.clone());
            let src = attributes
                .iter()
                .find(|(k, _)| k == "src")
                .map(|(_, v)| v.clone());
            if let (Some(alt), Some(src)) = (alt, src) {
                let b64 = src
                    .strip_prefix("data:image/png;base64,")
                    .unwrap_or(&src)
                    .to_string();
                out.insert(alt, b64);
            }
        }
        for child in children {
            img_src_by_alt(child, out);
        }
    }
}

/// Format a value as whole pounds with thousands separators and no decimals.
fn format_f64_0(v: f64) -> String {
    let n = v.round() as i64;
    let neg = n < 0;
    let abs = n.abs().to_string();
    let bytes = abs.as_bytes();
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    if neg {
        out.insert(0, '-');
    }
    out
}

/// Format a date with non-breaking spaces: `31 December 2020`.  The day is
/// zero-padded (`01 March 2021`), matching the reference output.
fn format_date(d: &chrono::NaiveDate) -> String {
    let day = d.format("%d").to_string();
    let month = d.format("%B").to_string();
    let year = d.format("%Y").to_string();
    format!("{}\u{00A0}{}\u{00A0}{}", day, month, year)
}

/// XML namespace declarations for the accounts document.
#[rustfmt::skip]
pub const ACCTS_HTML_ATTRS: &[(&str, &str)] = &[
    ("xmlns", "http://www.w3.org/1999/xhtml"),
    ("xmlns:ix", "http://www.xbrl.org/2013/inlineXBRL"),
    ("xmlns:link", "http://www.xbrl.org/2003/linkbase"),
    ("xmlns:xlink", "http://www.w3.org/1999/xlink"),
    ("xmlns:xbrli", "http://www.xbrl.org/2003/instance"),
    ("xmlns:xbrldi", "http://xbrl.org/2006/xbrldi"),
    ("xmlns:ixt2", "http://www.xbrl.org/inlineXBRL/transformation/2011-07-31"),
    ("xmlns:iso4217", "http://www.xbrl.org/2003/iso4217"),
    ("xmlns:uk-accrep", "http://xbrl.frc.org.uk/reports/2023-01-01/accrep"),
    ("xmlns:uk-aurep", "http://xbrl.frc.org.uk/reports/2023-01-01/aurep"),
    ("xmlns:uk-bus", "http://xbrl.frc.org.uk/cd/2023-01-01/business"),
    ("xmlns:uk-core", "http://xbrl.frc.org.uk/fr/2023-01-01/core"),
    ("xmlns:uk-direp", "http://xbrl.frc.org.uk/reports/2023-01-01/direp"),
    ("xmlns:uk-geo", "http://xbrl.frc.org.uk/cd/2023-01-01/countries"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{TestData, cache_dir, cache_path, repo_path};
    use crate::AdjustmentSplit;

    async fn load_example() -> (Company, GnucashBook) {
        let company = example_company();
        let gnucash = GnucashBook::try_from_gnucash_file(
            &TestData::accounts_path(&company.company_number)
                .expect("example company accounts path"),
        )
        .await
        .expect("open gnucash");
        (company, gnucash)
    }

    /// A [`PreviousPeriodData`] pointing at the same book as the current
    /// one — the example book covers both periods, so the comparative
    /// column is computed from its own history (the historical behaviour).
    fn prev_from_same_book(book: &GnucashBook) -> PreviousPeriodData<'_> {
        PreviousPeriodData { book, filing: None }
    }

    /// A calendar date, for the tests' readability.
    fn date(y: i32, m: u32, d: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    /// A book with one transaction dated `date`: a "Bank Accounts" split of
    /// `amount` balanced against an invisible "Opening Balances" account —
    /// the reports read only their known account paths, so the counter
    /// split never affects a computation.
    fn bank_book(amount: i64, date: chrono::NaiveDate) -> GnucashBook {
        let raw_accounts = vec![
            crate::RawAccount {
                guid: "root".into(),
                name: "Root Account".into(),
                r#type: "ROOT".into(),
                parent_guid: String::new(),
            },
            crate::RawAccount {
                guid: "bank".into(),
                name: "Bank Accounts".into(),
                r#type: "BANK".into(),
                parent_guid: "root".into(),
            },
            crate::RawAccount {
                guid: "opening".into(),
                name: "Opening Balances".into(),
                r#type: "EQUITY".into(),
                parent_guid: "root".into(),
            },
        ];
        let raw_txns = vec![crate::RawTransaction {
            guid: "txn".into(),
            post_datetime: date.and_hms_opt(12, 0, 0).unwrap(),
            description: String::new(),
        }];
        let raw_splits = vec![
            crate::RawSplit {
                tx_guid: "txn".into(),
                account_guid: "bank".into(),
                value: rucash::Num::from(amount),
            },
            crate::RawSplit {
                tx_guid: "txn".into(),
                account_guid: "opening".into(),
                value: rucash::Num::from(-amount),
            },
        ];
        GnucashBook::from_raw_parts(raw_accounts, raw_txns, raw_splits)
    }

    /// Like [`bank_book`], but with two bank transactions: `pre_amount`
    /// dated `pre_date` and `in_amount` dated `in_date`, each balanced
    /// against the invisible "Opening Balances" account.
    fn two_bank_book(
        pre_amount: i64,
        pre_date: chrono::NaiveDate,
        in_amount: i64,
        in_date: chrono::NaiveDate,
    ) -> GnucashBook {
        let raw_accounts = vec![
            crate::RawAccount {
                guid: "root".into(),
                name: "Root Account".into(),
                r#type: "ROOT".into(),
                parent_guid: String::new(),
            },
            crate::RawAccount {
                guid: "bank".into(),
                name: "Bank Accounts".into(),
                r#type: "BANK".into(),
                parent_guid: "root".into(),
            },
            crate::RawAccount {
                guid: "opening".into(),
                name: "Opening Balances".into(),
                r#type: "EQUITY".into(),
                parent_guid: "root".into(),
            },
        ];
        let raw_txns = vec![
            crate::RawTransaction {
                guid: "txn-pre".into(),
                post_datetime: pre_date.and_hms_opt(12, 0, 0).unwrap(),
                description: String::new(),
            },
            crate::RawTransaction {
                guid: "txn-in".into(),
                post_datetime: in_date.and_hms_opt(12, 0, 0).unwrap(),
                description: String::new(),
            },
        ];
        let raw_splits = vec![
            crate::RawSplit {
                tx_guid: "txn-pre".into(),
                account_guid: "bank".into(),
                value: rucash::Num::from(pre_amount),
            },
            crate::RawSplit {
                tx_guid: "txn-pre".into(),
                account_guid: "opening".into(),
                value: rucash::Num::from(-pre_amount),
            },
            crate::RawSplit {
                tx_guid: "txn-in".into(),
                account_guid: "bank".into(),
                value: rucash::Num::from(in_amount),
            },
            crate::RawSplit {
                tx_guid: "txn-in".into(),
                account_guid: "opening".into(),
                value: rucash::Num::from(-in_amount),
            },
        ];
        GnucashBook::from_raw_parts(raw_accounts, raw_txns, raw_splits)
    }

    /// The chart the previous-year-adjustment tests use: the bank, the
    /// corporation-tax liability (`Liabilities:Owed Corporation Tax`) and
    /// the CT equity reserve (`Equity:Corporation Tax`) the adjustment
    /// posts to, plus the shareholdings reserve (so a previous book can be
    /// balanced) and the invisible "Opening Balances" counter account.
    fn ct_chart() -> Vec<crate::RawAccount> {
        vec![
            crate::RawAccount {
                guid: "root".into(),
                name: "Root Account".into(),
                r#type: "ROOT".into(),
                parent_guid: String::new(),
            },
            crate::RawAccount {
                guid: "bank".into(),
                name: "Bank Accounts".into(),
                r#type: "BANK".into(),
                parent_guid: "root".into(),
            },
            crate::RawAccount {
                guid: "liab".into(),
                name: "Liabilities".into(),
                r#type: "LIABILITY".into(),
                parent_guid: "root".into(),
            },
            crate::RawAccount {
                guid: "owed-ct".into(),
                name: "Owed Corporation Tax".into(),
                r#type: "LIABILITY".into(),
                parent_guid: "liab".into(),
            },
            crate::RawAccount {
                guid: "equity".into(),
                name: "Equity".into(),
                r#type: "EQUITY".into(),
                parent_guid: "root".into(),
            },
            crate::RawAccount {
                guid: "shareholdings".into(),
                name: "Shareholdings".into(),
                r#type: "EQUITY".into(),
                parent_guid: "equity".into(),
            },
            crate::RawAccount {
                guid: "equity-ct".into(),
                name: "Corporation Tax".into(),
                r#type: "EQUITY".into(),
                parent_guid: "equity".into(),
            },
            crate::RawAccount {
                guid: "opening".into(),
                name: "Opening Balances".into(),
                r#type: "EQUITY".into(),
                parent_guid: "root".into(),
            },
        ]
    }

    /// A balanced previous-period book on the [`ct_chart`] chart: `bank`
    /// posted to the bank and `equity` to the shareholdings reserve on
    /// `date` (the equity is stored negative, the GnuCash convention the
    /// reports negate for the capital-and-reserves line).
    fn ct_book(bank: i64, equity: i64, date: chrono::NaiveDate) -> GnucashBook {
        let raw_txns = vec![crate::RawTransaction {
            guid: "txn".into(),
            post_datetime: date.and_hms_opt(12, 0, 0).unwrap(),
            description: String::new(),
        }];
        let raw_splits = vec![
            crate::RawSplit {
                tx_guid: "txn".into(),
                account_guid: "bank".into(),
                value: rucash::Num::from(bank),
            },
            crate::RawSplit {
                tx_guid: "txn".into(),
                account_guid: "shareholdings".into(),
                value: rucash::Num::from(equity),
            },
        ];
        GnucashBook::from_raw_parts(ct_chart(), raw_txns, raw_splits)
    }

    /// A current-period book on the [`ct_chart`] chart with a single
    /// transaction dated `date`: paying `payment` off the corporation-tax
    /// liability (Dr `Liabilities:Owed Corporation Tax`, Cr the bank) — the
    /// current-period side of a restored prior-year CT liability.
    fn ct_payment_book(payment: &str, date: chrono::NaiveDate) -> GnucashBook {
        let value: rucash::Num = payment.parse().unwrap();
        let raw_txns = vec![crate::RawTransaction {
            guid: "txn-payment".into(),
            post_datetime: date.and_hms_opt(12, 0, 0).unwrap(),
            description: "corporation tax paid".into(),
        }];
        let raw_splits = vec![
            crate::RawSplit {
                tx_guid: "txn-payment".into(),
                account_guid: "owed-ct".into(),
                value,
            },
            crate::RawSplit {
                tx_guid: "txn-payment".into(),
                account_guid: "bank".into(),
                value: -value,
            },
        ];
        GnucashBook::from_raw_parts(ct_chart(), raw_txns, raw_splits)
    }

    /// The example company's identity fields from the JSON; the remaining
    /// `company` keys are the flattened [`CompanyProfile`] fields, and the
    /// top-level `accounts` sub-object holds the period and report metadata.
    #[derive(serde::Deserialize)]
    struct CompanyData {
        name: String,
        tax_reference: String,
        company_number: String,
        #[serde(flatten)]
        profile: CompanyProfile,
    }

    /// The top-level shape of `input_config.jsonc`: a nested `company`
    /// identity + profile block and an `accounts` sub-object (period + report
    /// metadata, incl. the signature asset).
    #[derive(serde::Deserialize)]
    struct ExampleCompanyData {
        company: CompanyData,
        #[serde(default)]
        accounts: AccountsMeta,
    }

    /// Load the example company's data file — the single source of truth for
    /// the company identity + profile, the report metadata and the
    /// logo/signature assets.
    fn load_example_data() -> ExampleCompanyData {
        let json = std::fs::read_to_string(repo_path("example_data/basic-1/input_config.jsonc"))
            .expect("read example company data file");
        // Lenient parse (JSONC: comments / trailing commas allowed).
        serde_json_lenient::from_str(&json).expect("parse example company data file")
    }

    /// The example [`Company`] (identity only) from the JSON.
    fn example_company() -> Company {
        let data = load_example_data().company;
        Company::new(data.name, data.tax_reference, data.company_number)
    }

    /// The example company's set of accounts (return period + financial-year
    /// parameters + report metadata) from the JSON.
    fn example_accounts_meta() -> AccountsMeta {
        load_example_data().accounts
    }

    /// The example company profile (directors, contacts, accountant/auditor,
    /// logo, ...) from the JSON.
    fn example_profile() -> CompanyProfile {
        load_example_data().company.profile
    }

    #[test]
    fn test_example_company_data_from_json() {
        // Company identity round-trips from the JSON.
        let company = example_company();
        assert_eq!(company.name, "Example Biz Ltd.");
        assert_eq!(company.company_number, "12345678");
        assert_eq!(company.tax_reference, "8596148860");

        // The accounts sub-object round-trips from the same file.
        let accounts = example_accounts_meta();
        assert_eq!(
            accounts.period().start,
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()
        );
        assert_eq!(
            accounts.period().end,
            chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()
        );

        // Must stay in sync with the hardcoded TestData company and accounts
        // used by the corp-tax tests.
        let t = TestData::default_company();
        assert_eq!(company.name, t.name);
        assert_eq!(company.company_number, t.company_number);
        assert_eq!(company.tax_reference, t.tax_reference);
        assert_eq!(accounts.period(), TestData::default_accounts_meta().period());

        // The company profile round-trips from the same file.
        let p = example_profile();
        assert_eq!(p.directors, vec!["A Bloggs", "B Smith", "C Jones"]);
        assert_eq!(p.sic_codes, vec!["62020", "62021"]);
        assert!(!p.logo_b64.as_deref().unwrap_or("").is_empty());

        // The report metadata round-trips from the same file.
        assert_eq!(
            accounts.average_employees,
            HashMap::from([("2020".to_string(), 2), ("2019".to_string(), 1)])
        );
        assert_eq!(
            accounts.report_date,
            chrono::NaiveDate::from_ymd_opt(2021, 3, 1).unwrap()
        );
        assert_eq!(
            accounts.incorporation_date,
            chrono::NaiveDate::from_ymd_opt(2017, 4, 5).unwrap()
        );
        assert!(accounts.signature_b64.is_some());
    }

    #[tokio::test]
    async fn test_accounts_from_basic_1() {
        let (company, gnucash) = load_example().await;
        let accounts = Frs105Accounts::new(
            &gnucash,
            &company,
            &example_profile(),
            &example_accounts_meta()
        )
        .with_prev_period_data(&prev_from_same_book(&gnucash));
        assert_eq!(accounts.fixed_assets, [932.74, 633.10]);
        assert_eq!(accounts.current_assets, [14923.66, 10333.27]);
        assert_eq!(accounts.creditors_within_1_year, [-3712.32, -3020.37]);
        assert_eq!(accounts.net_current_assets, [11211.34, 7312.90]);
        assert_eq!(accounts.total_assets_less_liabilities, [12144.08, 7946.00]);
        assert_eq!(accounts.net_assets, [12144.08, 7946.00]);
        assert_eq!(accounts.capital_and_reserves, [12144.08, 7946.00]);

        // Rounded (whole-pound) rendering.
        let out = accounts.to_ixbrl();
        assert!(out.contains(">933</ix:nonFraction>"));
        assert!(out.contains(">633</ix:nonFraction>"));
        assert!(out.contains(">14,924</ix:nonFraction>"));
        assert!(out.contains(">10,333</ix:nonFraction>"));
        assert!(out.contains(">3,712</ix:nonFraction>"));
        assert!(out.contains(">3,020</ix:nonFraction>"));
        assert!(out.contains(">11,211</ix:nonFraction>"));
        assert!(out.contains(">12,144</ix:nonFraction>"));
    }

    #[tokio::test]
    async fn test_multi_year_one_shot_matches_two_step() {
        // basic-1's book spans several years (transactions 2018-01-31 →
        // 2021-03-01; the configured period is calendar 2020).  Computing
        // the last period's balance sheet directly from origin of time
        // (one shot) must equal re-calculating it after first computing the
        // previous period (2019) and feeding it in as the previous-period
        // input — both columns, line for line.
        let (company, gnucash) = load_example().await;
        let profile = example_profile();

        // The earlier period (calendar 2019) and the last period (2020).
        let meta_2019 = AccountsMeta {
            period: Some(AccountingPeriod {
                start: chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                end: chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            }),
            fy1_year: 2018,
            fy2_year: 2019,
            ..example_accounts_meta()
        };
        let meta_2020 = example_accounts_meta();

        // One shot: the last-period balance sheet from origin of time —
        // both columns computed from the book's own history (current up to
        // 2020-12-31, comparative up to 2019-12-31).
        let one_shot = Frs105Accounts::new(&gnucash, &company, &profile, &meta_2020)
            .with_prev_period_data(&prev_from_same_book(&gnucash));

        // Two step: first the 2019 balance sheet, then re-calculate the
        // last period with that earlier period as the previous input.
        let earlier = Frs105Accounts::new(&gnucash, &company, &profile, &meta_2019)
            .with_prev_period_data(&prev_from_same_book(&gnucash));
        let two_step = Frs105Accounts::new(&gnucash, &company, &profile, &meta_2020)
            .with_prev_period_data(&PreviousPeriodData {
                book: &gnucash,
                filing: None,
            });

        // The earlier period's current column is the last period's
        // comparative column, and the two last-period balance sheets are
        // identical on every line.
        let lines: [([f64; 2], [f64; 2]); 12] = [
            (earlier.fixed_assets, one_shot.fixed_assets),
            (earlier.called_up_share_capital_not_paid, one_shot.called_up_share_capital_not_paid),
            (earlier.current_assets, one_shot.current_assets),
            (
                earlier.prepayments_and_accrued_income,
                one_shot.prepayments_and_accrued_income,
            ),
            (earlier.creditors_within_1_year, one_shot.creditors_within_1_year),
            (earlier.net_current_assets, one_shot.net_current_assets),
            (
                earlier.total_assets_less_liabilities,
                one_shot.total_assets_less_liabilities,
            ),
            (earlier.creditors_after_1_year, one_shot.creditors_after_1_year),
            (
                earlier.provisions_for_liabilities,
                one_shot.provisions_for_liabilities,
            ),
            (
                earlier.accruals_and_deferred_income,
                one_shot.accruals_and_deferred_income,
            ),
            (earlier.net_assets, one_shot.net_assets),
            (earlier.capital_and_reserves, one_shot.capital_and_reserves),
        ];
        for (earlier_line, last_line) in lines {
            assert_eq!(
                earlier_line[0], last_line[1],
                "earlier period's current column must equal the last period's comparative"
            );
        }
        for (a, b) in [
            (one_shot.fixed_assets, two_step.fixed_assets),
            (
                one_shot.called_up_share_capital_not_paid,
                two_step.called_up_share_capital_not_paid,
            ),
            (one_shot.current_assets, two_step.current_assets),
            (
                one_shot.prepayments_and_accrued_income,
                two_step.prepayments_and_accrued_income,
            ),
            (one_shot.creditors_within_1_year, two_step.creditors_within_1_year),
            (one_shot.net_current_assets, two_step.net_current_assets),
            (
                one_shot.total_assets_less_liabilities,
                two_step.total_assets_less_liabilities,
            ),
            (one_shot.creditors_after_1_year, two_step.creditors_after_1_year),
            (
                one_shot.provisions_for_liabilities,
                two_step.provisions_for_liabilities,
            ),
            (
                one_shot.accruals_and_deferred_income,
                two_step.accruals_and_deferred_income,
            ),
            (one_shot.net_assets, two_step.net_assets),
            (one_shot.capital_and_reserves, two_step.capital_and_reserves),
        ] {
            assert_eq!(a, b, "one-shot and two-step balance sheets must be equal");
        }
    }

    #[tokio::test]
    async fn test_accounts_output_matches_reference_fixture() {
        // Regenerate the fixture with:
        //   nix run .#racc-gnucash   # -> .cache/py-ixbrl-reporter/accts-micro-gnucash.html
        // then strip the reference's random `id="elt-*"` attributes, change the
        // header wrapper to `<div style="display:none">` (Arelle's
        // `headerDisplayNone` rule ignores stylesheet classes), and copy to
        // example_data/basic-1/output-accounts.html.  The Rust output below
        // must match it byte for byte.
        //
        // The fixture carries deliberate divergences from the reference
        // tool's output, both required for Arelle's validate/UK plugin to
        // pass:
        // * `StartDateForPeriodCoveredByReport` is tagged on the period-end
        //   instant context (ctxt-2) and the then-unused ctxt-22 (instant at
        //   the period start) is dropped — JFCVC.3312 requires both period
        //   facts on a context whose instant date equals
        //   `EndDateForPeriodCoveredByReport`;
        // * `AccountingStandardsApplied`, `AccountsType`,
        //   `AccountsStatusAuditedOrUnaudited` and `LegalFormEntity` are
        //   emitted as empty facts (the taxonomy types them
        //   `types:fixedItemType`, length 0) — the reference tool emits the
        //   dimension member as the fact text, which fails schema
        //   validation; the meaning lives in the context's dimension
        //   member.
        let (company, gnucash) = load_example().await;
        let accounts = Frs105Accounts::new(
            &gnucash,
            &company,
            &example_profile(),
            &example_accounts_meta()
        )
        .with_prev_period_data(&prev_from_same_book(&gnucash));
        let out = accounts.to_ixbrl();

        // Write the Rust output for external validation (arelle).
        std::fs::create_dir_all(cache_dir("ixbrl-rs-tests")).unwrap();
        std::fs::write(
            cache_path("ixbrl-rs-tests", "accts-micro-basic-1.html"),
            &out,
        )
        .unwrap();

        let expected = std::fs::read_to_string(repo_path(
            "example_data/basic-1/output-accounts.html",
        ))
        .expect("read reference fixture");
        assert_eq!(
            out, expected,
            "accounts output must match the reference fixture"
        );
    }

    #[tokio::test]
    async fn test_accounts_ixbrl_structure() {
        let (company, gnucash) = load_example().await;
        let out = Frs105Accounts::new(
            &gnucash,
            &company,
            &example_profile(),
            &example_accounts_meta()
        )
        .with_prev_period_data(&prev_from_same_book(&gnucash))
        .to_ixbrl();

        // Header structure
        assert!(out.contains("<div style=\"display:none\"><ix:header><ix:hidden>"));
        assert!(out.contains("<ix:references>"));
        assert!(out.contains("<ix:resources>"));
        assert!(out.contains(
            "xlink:href=\"https://xbrl.frc.org.uk/FRS-102/2023-01-01/FRS-102-2023-01-01.xsd\""
        ));

        // Contexts
        assert!(out.contains("xbrli:context id=\"ctxt-0\""));
        assert!(out.contains("xbrli:context id=\"ctxt-17\""));
        assert!(out.contains("uk-bus:M-ProfessionalScientificTechnicalActivities"));
        assert!(out.contains("uk-core:MaturitiesOrExpirationPeriodsDimension"));

        // Pages
        assert!(out.contains("<div class=\"titlepage\">"));
        assert!(out.contains("Company information"));
        assert!(out.contains("Statement of financial position"));
        assert!(out.contains("Notes to the accounts"));
        assert!(out.contains("1. Company information"));
        assert!(out.contains("2. Employees"));
        assert!(out.contains("AverageNumberEmployeesDuringPeriod"));
    }

    /// The voluntary facts (registered-office county, e-mail, phone,
    /// website) are omitted from the document when the profile leaves them
    /// blank, and come back `None` on the round trip.
    #[tokio::test]
    async fn test_blank_contact_facts_are_omitted() {
        let (company, gnucash) = load_example().await;
        let mut profile = example_profile();
        profile.county = None;
        profile.vat_registration = None;
        profile.activities = None;
        profile.email = None;
        profile.phone_country = None;
        profile.phone_area = None;
        profile.phone_number = None;
        profile.website_url = None;
        profile.website_description = None;
        let accounts =
            Frs105Accounts::new(
                &gnucash,
                &company,
                &profile,
                &example_accounts_meta()
            )
            .with_prev_period_data(&prev_from_same_book(&gnucash));
        let html = accounts.to_ixbrl();

        // No voluntary fact is tagged.
        for fact in [
            "uk-bus:CountyRegion",
            "uk-bus:VATRegistrationNumber",
            "uk-bus:E-mailAddress",
            "uk-bus:CountryCode",
            "uk-bus:AreaCode",
            "uk-bus:LocalNumber",
            "uk-bus:WebsiteMainPageURL",
            "uk-bus:DescriptionOrOtherInformationOnWebsite",
        ] {
            assert!(!html.contains(fact), "{fact} must be omitted when blank");
        }
        // ... while the required contact facts are still tagged.
        assert!(html.contains("uk-bus:NameContactDepartmentOrPerson"));
        assert!(html.contains("uk-bus:PostalCodeZip"));
        // JFCVC.3312 makes the principal-activities description mandatory:
        // it is still tagged when the profile leaves it blank, with the
        // placeholder text.
        assert!(html.contains("uk-bus:DescriptionPrincipalActivities"));
        assert!(html.contains(">No description of principal activity</ix:nonNumeric>"));

        // Round trip: the blank fields come back absent.
        let node = XmlNode::from_xml_string(&html).expect("parse ixbrl");
        let back = Frs105Accounts::from_ixbrl_node(&node, &company, &example_accounts_meta());
        assert_eq!(back.profile.county, None);
        assert_eq!(back.profile.vat_registration, None);
        assert_eq!(back.profile.activities, None);
        assert_eq!(back.profile.email, None);
        assert_eq!(back.profile.phone_country, None);
        assert_eq!(back.profile.phone_area, None);
        assert_eq!(back.profile.phone_number, None);
        assert_eq!(back.profile.website_url, None);
        assert_eq!(back.profile.website_description, None);
    }

    #[tokio::test]
    async fn test_accounts_ixbrl_round_trip() {
        // Serialise, write the output to .cache/ixbrl-rs-tests, then
        // deserialise in two steps (XML -> XmlNode -> Frs105Accounts) and
        // compare against the original.
        let (company, gnucash) = load_example().await;
        let accounts = Frs105Accounts::new(
            &gnucash,
            &company,
            &example_profile(),
            &example_accounts_meta()
        )
        .with_prev_period_data(&prev_from_same_book(&gnucash));
        let html = accounts.to_ixbrl();

        std::fs::create_dir_all(cache_dir("ixbrl-rs-tests")).unwrap();
        std::fs::write(
            cache_path("ixbrl-rs-tests", "accts-micro-roundtrip-basic-1.html"),
            &html,
        )
        .unwrap();

        let node = XmlNode::from_xml_string(&html).expect("parse ixbrl");
        let back = Frs105Accounts::from_ixbrl_node(&node, &company, &example_accounts_meta());

        // Balance-sheet values are rendered at whole pounds (decimals = 0),
        // so the round trip preserves them to the nearest pound (the sign
        // is recovered from the cell class).
        let round = |a: [f64; 2]| [a[0].round(), a[1].round()];
        assert_eq!(round(back.fixed_assets), round(accounts.fixed_assets));
        assert_eq!(round(back.current_assets), round(accounts.current_assets));
        assert_eq!(
            round(back.prepayments_and_accrued_income),
            round(accounts.prepayments_and_accrued_income)
        );
        assert_eq!(
            round(back.creditors_within_1_year),
            round(accounts.creditors_within_1_year)
        );
        assert_eq!(
            round(back.net_current_assets),
            round(accounts.net_current_assets)
        );
        assert_eq!(
            round(back.total_assets_less_liabilities),
            round(accounts.total_assets_less_liabilities)
        );
        assert_eq!(
            round(back.creditors_after_1_year),
            round(accounts.creditors_after_1_year)
        );
        assert_eq!(
            round(back.provisions_for_liabilities),
            round(accounts.provisions_for_liabilities)
        );
        assert_eq!(
            round(back.accruals_and_deferred_income),
            round(accounts.accruals_and_deferred_income)
        );
        assert_eq!(round(back.net_assets), round(accounts.net_assets));
        assert_eq!(
            round(back.capital_and_reserves),
            round(accounts.capital_and_reserves)
        );

        // Company identity round-trips.
        assert_eq!(back.company.name, accounts.company.name);
        assert_eq!(back.company.company_number, accounts.company.company_number);
        assert_eq!(back.company.tax_reference, accounts.company.tax_reference);
        assert_eq!(
            back.accounts.period().start,
            accounts.accounts.period().start
        );
        assert_eq!(
            back.accounts.period().end,
            accounts.accounts.period().end
        );

        // Metadata fields that are serialised to iXBRL round-trip.
        assert_eq!(back.profile.directors, accounts.profile.directors);
        assert_eq!(
            back.profile.contact_name,
            accounts.profile.contact_name
        );
        assert_eq!(
            back.profile.address_lines,
            accounts.profile.address_lines
        );
        assert_eq!(back.profile.county, accounts.profile.county);
        assert_eq!(back.profile.location, accounts.profile.location);
        assert_eq!(back.profile.postcode, accounts.profile.postcode);
        assert_eq!(back.profile.email, accounts.profile.email);
        assert_eq!(back.profile.phone_country, accounts.profile.phone_country);
        assert_eq!(back.profile.phone_area, accounts.profile.phone_area);
        assert_eq!(back.profile.phone_number, accounts.profile.phone_number);
        assert_eq!(back.profile.website_url, accounts.profile.website_url);
        assert_eq!(
            back.profile.website_description,
            accounts.profile.website_description
        );
        assert_eq!(
            back.profile.vat_registration,
            accounts.profile.vat_registration
        );
        assert_eq!(back.profile.sic_codes, accounts.profile.sic_codes);
        assert_eq!(back.profile.activities, accounts.profile.activities);
        assert_eq!(
            back.accounts.average_employees,
            accounts.accounts.average_employees
        );
        assert_eq!(
            back.profile.accountant_name,
            accounts.profile.accountant_name
        );
        assert_eq!(
            back.profile.accountant_business,
            accounts.profile.accountant_business
        );
        assert_eq!(
            back.profile.accountant_address,
            accounts.profile.accountant_address
        );
        assert_eq!(back.profile.auditor_name, accounts.profile.auditor_name);
        assert_eq!(
            back.profile.auditor_business,
            accounts.profile.auditor_business
        );
        assert_eq!(
            back.profile.auditor_address,
            accounts.profile.auditor_address
        );
        assert_eq!(back.accounts.report_date, accounts.accounts.report_date);
        assert_eq!(
            back.accounts.authorised_date,
            accounts.accounts.authorised_date
        );
        assert_eq!(
            back.accounts.incorporation_date,
            accounts.accounts.incorporation_date
        );
        assert_eq!(
            back.profile.industry_sector_dimension,
            accounts.profile.industry_sector_dimension
        );
        assert_eq!(
            back.accounts.accounting_standards_dimension,
            accounts.accounts.accounting_standards_dimension
        );
        assert_eq!(
            back.accounts.accounts_type_dimension,
            accounts.accounts.accounts_type_dimension
        );
        assert_eq!(
            back.accounts.accounts_status_dimension,
            accounts.accounts.accounts_status_dimension
        );
        assert_eq!(
            back.profile.legal_form_dimension,
            accounts.profile.legal_form_dimension
        );
        assert_eq!(
            back.profile.country_dimension,
            accounts.profile.country_dimension
        );
        assert_eq!(
            back.profile.contact_country_dimension,
            accounts.profile.contact_country_dimension
        );
        assert_eq!(
            back.profile.phone_type_dimension,
            accounts.profile.phone_type_dimension
        );
        assert_eq!(back.profile.logo_b64, accounts.profile.logo_b64);
        assert_eq!(
            back.accounts.signature_b64,
            accounts.accounts.signature_b64
        );
    }

    #[test]
    fn test_format_f64_0() {
        assert_eq!(format_f64_0(14924.0), "14,924");
        assert_eq!(format_f64_0(933.0), "933");
        assert_eq!(format_f64_0(0.0), "0");
        assert_eq!(format_f64_0(12144.08), "12,144");
    }

    #[test]
    fn test_comparative_column_comes_from_the_previous_book() {
        // Two books with different balances: the opt-out variant keeps the
        // current column computed purely from the current book, while the
        // comparative column comes from the previous-period book.
        let company = example_company();
        let profile = example_profile();
        let accounts = example_accounts_meta();
        let period = accounts.period();
        let current_book = bank_book(1000, period.end - chrono::Duration::days(30));
        let previous_book = bank_book(2000, period.start - chrono::Duration::days(1));

        let out = Frs105Accounts::new(
            &current_book,
            &company,
            &profile,
            &accounts,
        )
        .with_prev_period_data_no_seed(&PreviousPeriodData {
            book: &previous_book,
            filing: None,
        });

        // Current column from the current book; comparative from the
        // previous book.  A bank balance shows up in current assets (and
        // the derived totals), not in fixed assets or capital/reserves.
        assert_eq!(out.current_assets, [1000.0, 2000.0]);
        assert_eq!(out.net_current_assets, [1000.0, 2000.0]);
        assert_eq!(out.total_assets_less_liabilities, [1000.0, 2000.0]);
        assert_eq!(out.net_assets, [1000.0, 2000.0]);
        assert_eq!(out.fixed_assets, [0.0, 0.0]);
        assert_eq!(out.creditors_within_1_year, [0.0, 0.0]);
        assert_eq!(out.capital_and_reserves, [0.0, 0.0]);
        // Identity fields survive.
        assert_eq!(out.company.name, company.name);
        assert_eq!(out.company.company_number, company.company_number);
    }

    #[test]
    fn test_with_prev_period_data_seeds_current_from_previous_figures() {
        // The default (seeded) variant starts the current column from the
        // previous period's closing figures and adds this period's
        // activity: same two books as above, the current column reads
        // 2000 (previous closing) + 1000 (this period's bank transaction)
        // = 3000, while the comparative column stays at the previous
        // book's 2000.
        let company = example_company();
        let profile = example_profile();
        let accounts = example_accounts_meta();
        let period = accounts.period();
        let current_book = bank_book(1000, period.end - chrono::Duration::days(30));
        let previous_book = bank_book(2000, period.start - chrono::Duration::days(1));

        let out = Frs105Accounts::new(
            &current_book,
            &company,
            &profile,
            &accounts,
        )
        .with_prev_period_data(&PreviousPeriodData {
            book: &previous_book,
            filing: None,
        });

        assert_eq!(out.current_assets, [3000.0, 2000.0]);
        assert_eq!(out.net_current_assets, [3000.0, 2000.0]);
        assert_eq!(out.total_assets_less_liabilities, [3000.0, 2000.0]);
        assert_eq!(out.net_assets, [3000.0, 2000.0]);
        assert_eq!(out.fixed_assets, [0.0, 0.0]);
    }

    /// The seeded in-period activity — the current book's splits dated
    /// within the period — is equivalent to the full-history computation up
    /// to the period end minus the book's own pre-period portion: on a book
    /// that spans both periods (basic-1, transactions 2018 → 2021) the two
    /// agree line for line.  The implementation uses the filtered
    /// computation directly (see [`Frs105Accounts::with_prev_period_data`]);
    /// this test pins the equivalence to the `full − before` view.
    #[tokio::test]
    async fn test_in_period_activity_equals_full_minus_before() {
        let (_, gnucash) = load_example().await;
        let period = example_accounts_meta().period();
        let full = Frs105Accounts::compute_lines(&gnucash, period.end);
        let before = Frs105Accounts::compute_lines(
            &gnucash,
            period.start - chrono::Duration::days(1),
        );
        let filtered = Frs105Accounts::compute_lines_between(
            &gnucash,
            Some(period.start),
            Some(period.end),
        );
        assert_eq!((full - before).rounded(), filtered);
    }

    #[test]
    fn test_prev_period_data_ignores_current_book_pre_period_splits() {
        // The caller's previous-period figures are an override: the current
        // book's own pre-period transactions are ignored entirely, and only
        // its in-period splits add on top of the override.
        let company = example_company();
        let profile = example_profile();
        let accounts = example_accounts_meta();
        let period = accounts.period();
        // Current book: £500 before the period (ignored) + £1,000 in-period.
        let current_book = two_bank_book(
            500,
            period.start - chrono::Duration::days(5),
            1000,
            period.end - chrono::Duration::days(30),
        );
        let previous_book = bank_book(2000, period.start - chrono::Duration::days(1));

        // Seeded: 2000 (override) + 1000 (in-period) = 3000; the £500
        // pre-period split does not leak into the current column.
        let seeded = Frs105Accounts::new(&current_book, &company, &profile, &accounts)
            .with_prev_period_data(&PreviousPeriodData {
                book: &previous_book,
                filing: None,
            });
        assert_eq!(seeded.current_assets, [3000.0, 2000.0]);
        assert_eq!(seeded.net_current_assets, [3000.0, 2000.0]);
        assert_eq!(seeded.total_assets_less_liabilities, [3000.0, 2000.0]);
        assert_eq!(seeded.net_assets, [3000.0, 2000.0]);
        assert_eq!(seeded.fixed_assets, [0.0, 0.0]);

        // Opt-out: the current column stays the pure book computation
        // (500 + 1000), with the comparative from the previous book.
        let no_seed = Frs105Accounts::new(&current_book, &company, &profile, &accounts)
            .with_prev_period_data_no_seed(&PreviousPeriodData {
                book: &previous_book,
                filing: None,
            });
        assert_eq!(no_seed.current_assets, [1500.0, 2000.0]);
    }

    #[test]
    fn test_empty_year_current_matches_previous_via_seeding() {
        // A pending period with no transactions yet: the current book is
        // empty, so the seeded current column is the previous period's
        // closing figures plus zero activity — a year with no activity
        // leaves the balances unchanged (current == previous on every
        // line), with no separate carry-forward step needed.
        let company = example_company();
        let profile = example_profile();
        let accounts = example_accounts_meta();
        let empty_book = GnucashBook::from_raw_parts(Vec::new(), Vec::new(), Vec::new());
        let previous_book = bank_book(2222, accounts.period().start - chrono::Duration::days(1));

        let out = Frs105Accounts::new(
            &empty_book,
            &company,
            &profile,
            &accounts,
        )
        .with_prev_period_data(&PreviousPeriodData {
            book: &previous_book,
            filing: None,
        });

        // Every balance carried forward: current column == previous column.
        assert_eq!(out.fixed_assets, [0.0, 0.0]);
        assert_eq!(out.called_up_share_capital_not_paid, [0.0, 0.0]);
        assert_eq!(out.current_assets, [2222.0, 2222.0]);
        assert_eq!(out.prepayments_and_accrued_income, [0.0, 0.0]);
        assert_eq!(out.creditors_within_1_year, [0.0, 0.0]);
        assert_eq!(out.net_current_assets, [2222.0, 2222.0]);
        assert_eq!(out.total_assets_less_liabilities, [2222.0, 2222.0]);
        assert_eq!(out.creditors_after_1_year, [0.0, 0.0]);
        assert_eq!(out.provisions_for_liabilities, [0.0, 0.0]);
        assert_eq!(out.accruals_and_deferred_income, [0.0, 0.0]);
        assert_eq!(out.net_assets, [2222.0, 2222.0]);
        assert_eq!(out.capital_and_reserves, [0.0, 0.0]);
    }

    #[test]
    fn test_check_previous_period_matches_filing() {
        // A previous-period book whose figures reconcile with a filing
        // passes; one that differs reports exactly the differing lines.
        let company = example_company();
        let profile = example_profile();
        let accounts = example_accounts_meta();
        let period = accounts.period();
        let prev_end = period.start - chrono::Duration::days(1);
        let previous_book = bank_book(2000, prev_end);

        let out = Frs105Accounts::new(
            &bank_book(0, period.end),
            &company,
            &profile,
            &accounts,
        )
        .with_prev_period_data(&PreviousPeriodData {
            book: &previous_book,
            filing: None,
        });

        // Matching filing: period aligned, figures reconcile.
        let matching = FiledBalanceSheet {
            period_start: prev_end - chrono::Duration::days(364),
            period_end: prev_end,
            figures: PreviousPeriodFigures {
                current_assets: 2000.0,
                net_current_assets: 2000.0,
                total_assets_less_liabilities: 2000.0,
                net_assets: 2000.0,
                ..Default::default()
            },
        };
        assert_eq!(out.check_previous_period_matches_filing(&matching), Ok(()));

        // Conflicting filing: the differing lines are reported.
        let conflicting = FiledBalanceSheet {
            period_end: prev_end,
            figures: PreviousPeriodFigures {
                current_assets: 5000.0,
                ..Default::default()
            },
            ..matching
        };
        let err = out
            .check_previous_period_matches_filing(&conflicting)
            .expect_err("the conflicting filing must not reconcile");
        assert!(err.errors.iter().any(|e| matches!(
            e,
            Frs105CheckError::CurrentAssetsMismatch {
                computed,
                filed,
                accounts,
            } if *computed == 2000.0 && *filed == 5000.0
                && accounts.contains("Bank Accounts")
        )));
        // The collector displays every mismatch, naming the line, the
        // accounts the computation read and both values.
        let display = format!("{err}");
        assert!(display.contains("current assets"));
        assert!(display.contains("Bank Accounts"));
        assert!(display.contains("2000"));
        assert!(display.contains("5000"));

        // A filing for a different period is flagged too.
        let wrong_period = FiledBalanceSheet {
            period_end: prev_end - chrono::Duration::days(10),
            ..matching
        };
        let err = out
            .check_previous_period_matches_filing(&wrong_period)
            .expect_err("the wrong-period filing must not reconcile");
        assert!(err.errors.iter().any(|e| matches!(
            e,
            Frs105CheckError::PeriodDatesMismatch {
                filed_end,
                prev_end: reported_prev_end,
            } if *filed_end == prev_end - chrono::Duration::days(10)
                && *reported_prev_end == prev_end
        )));
    }

    #[test]
    fn called_up_share_capital_not_paid_renders_only_when_nonzero() {
        // Build a report with known figures (both columns from the ledger).
        let company = example_company();
        let profile = example_profile();
        let accounts = example_accounts_meta();
        let period_end = accounts.period().end;
        let base = Frs105Accounts {
            company,
            profile,
            accounts,
            book: None,
            prev_book: None,
            filing_deadlines: FilingDeadlines::from_period_end(period_end),
            fixed_assets: [100.0, 200.0],
            called_up_share_capital_not_paid: [0.0, 0.0],
            current_assets: [300.0, 400.0],
            prepayments_and_accrued_income: [0.0, 1.0],
            creditors_within_1_year: [-50.0, -60.0],
            net_current_assets: [250.0, 340.0],
            total_assets_less_liabilities: [350.0, 540.0],
            creditors_after_1_year: [0.0, 2.0],
            provisions_for_liabilities: [0.0, 3.0],
            accruals_and_deferred_income: [0.0, 4.0],
            net_assets: [350.0, 549.0],
            capital_and_reserves: [350.0, 549.0],
        };

        // Default (zero): the row and its fact are omitted from the iXBRL.
        let default = base.clone().to_ixbrl();
        assert!(!default.contains("uk-core:CalledUpShareCapitalNotPaidNotExpressedAsCurrentAsset"));
        assert!(!default.contains("Called up share capital not paid"));

        // Supplied: the amounts land in the field and are folded into the
        // total-assets-less-current-liabilities and net-assets totals
        // (line A is part of both), while net current assets is unchanged.
        let with_line = base
            .clone()
            .with_called_up_share_capital_not_paid([100.0, 50.0]);
        assert_eq!(with_line.called_up_share_capital_not_paid, [100.0, 50.0]);
        assert_eq!(with_line.total_assets_less_liabilities, [450.0, 590.0]);
        assert_eq!(with_line.net_assets, [450.0, 599.0]);
        assert_eq!(with_line.net_current_assets, [250.0, 340.0]);

        // The row renders, above the fixed-assets row.
        let html = with_line.to_ixbrl();
        assert!(html.contains("uk-core:CalledUpShareCapitalNotPaidNotExpressedAsCurrentAsset"));
        assert!(
            html.find("uk-core:CalledUpShareCapitalNotPaidNotExpressedAsCurrentAsset").unwrap()
                < html.find("uk-core:FixedAssets").unwrap()
        );
    }

    #[test]
    fn logo_omitted_and_signature_always_embedded() {
        // The logo is omitted when not supplied, but the signature is
        // always embedded — the bundled default when none is supplied (no
        // empty data URIs).
        let company = example_company();
        let empty_book = GnucashBook::from_raw_parts(Vec::new(), Vec::new(), Vec::new());
        let no_logo = Frs105Accounts::new(
            &empty_book,
            &company,
            &CompanyProfile::default(),
            &AccountsMeta {
                signature_b64: None,
                ..example_accounts_meta()
            },
        )
        .to_ixbrl();
        assert!(!no_logo.contains("alt=\"Company logo\""));
        assert!(no_logo.contains("alt=\"Director's signature\""));
        assert!(no_logo.contains("data:image/png;base64,"));

        // With the example's logo and signature both render.
        let with_assets = Frs105Accounts::new(
            &empty_book,
            &company,
            &example_profile(),
            &example_accounts_meta(),
        )
        .to_ixbrl();
        assert!(with_assets.contains("alt=\"Company logo\""));
        assert!(with_assets.contains("alt=\"Director's signature\""));
    }

    #[test]
    fn signature_defaults_to_bundled_and_with_signature_overrides() {
        let company = example_company();
        let profile = example_profile();
        let book = GnucashBook::from_raw_parts(Vec::new(), Vec::new(), Vec::new());

        // Not supplied: the bundled default is embedded.
        let accounts = Frs105Accounts::new(
            &book,
            &company,
            &profile,
            &AccountsMeta {
                signature_b64: None,
                ..example_accounts_meta()
            },
        );
        assert_eq!(accounts.signature_b64(), DEFAULT_SIGNATURE_B64);
        assert!(accounts
            .to_ixbrl()
            .contains(&format!("data:image/png;base64,{}", DEFAULT_SIGNATURE_B64)));

        // An explicit meta signature wins.
        let explicit = Frs105Accounts::new(
            &book,
            &company,
            &profile,
            &AccountsMeta {
                signature_b64: Some("custom-signature".to_string()),
                ..example_accounts_meta()
            },
        );
        assert_eq!(explicit.signature_b64(), "custom-signature");

        // The mut builder method overrides either.
        let overridden = accounts
            .with_signature("user-signature".to_string())
            .to_ixbrl();
        assert!(overridden.contains("data:image/png;base64,user-signature"));
        assert!(!overridden.contains(DEFAULT_SIGNATURE_B64));
    }



    #[test]
    fn default_signing_date_is_day_before_deadline_capped_at_today() {
        // The deadline is still ahead: the signing date is today.
        assert_eq!(
            default_signing_date(date(2025, 6, 1), date(2025, 9, 30)),
            date(2025, 6, 1)
        );
        // The deadline has passed: one day before the last day to file.
        assert_eq!(
            default_signing_date(date(2025, 10, 1), date(2025, 9, 30)),
            date(2025, 9, 29)
        );
    }

    #[test]
    fn filing_deadlines_from_period_end_and_earliest() {
        let deadlines = FilingDeadlines::from_period_end(date(2024, 11, 30));
        assert_eq!(deadlines.companies_house_accounts, date(2025, 8, 30));
        assert_eq!(deadlines.hmrc_ct600, date(2025, 11, 30));
        // The accounts deadline is the binding one.
        assert_eq!(deadlines.earliest(), date(2025, 8, 30));
    }

    #[test]
    fn signing_date_explicit_wins_and_deadlines_override_the_default() {
        let company = example_company();
        let profile = example_profile();
        let book = GnucashBook::from_raw_parts(Vec::new(), Vec::new(), Vec::new());
        let period = AccountingPeriod {
            start: date(2024, 1, 1),
            end: date(2024, 11, 30),
        };
        let meta = AccountsMeta {
            period: Some(period),
            authorised_date: None,
            ..example_accounts_meta()
        };

        // No explicit signing date: it defaults to one day before the
        // earliest deadline (accounts 9 months after the period end),
        // capped at today.
        let accounts = Frs105Accounts::new(&book, &company, &profile, &meta);
        let today = chrono::Utc::now().date_naive();
        assert_eq!(
            accounts.signing_date(),
            default_signing_date(today, date(2025, 8, 30))
        );

        // Overriding the deadlines re-defaults from the given ones.
        let overridden = accounts.clone().with_signing_deadlines(FilingDeadlines {
            companies_house_accounts: date(2025, 11, 29),
            hmrc_ct600: date(2025, 11, 30),
        });
        assert_eq!(
            overridden.signing_date(),
            default_signing_date(today, date(2025, 11, 29))
        );

        // An explicitly-supplied date wins over both the default and the
        // deadline override.
        let explicit = Frs105Accounts::new(
            &book,
            &company,
            &profile,
            &AccountsMeta {
                authorised_date: Some(date(2024, 2, 1)),
                ..meta.clone()
            },
        );
        assert_eq!(explicit.signing_date(), date(2024, 2, 1));
        let explicit = explicit.with_signing_deadlines(FilingDeadlines::from_period_end(
            period.end,
        ));
        assert_eq!(explicit.signing_date(), date(2024, 2, 1));
    }

    #[test]
    fn test_format_date() {
        let d = chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap();
        assert_eq!(format_date(&d), "31\u{00A0}December\u{00A0}2020");
    }

    #[test]
    fn previous_year_adjustments_restore_a_missing_liability() {
        // A prior-period error: the previous period's balance sheet omitted
        // the £2006.21 corporation-tax liability, and this period the tax is
        // paid (2020-06-15).  The previous-period book carries the position
        // as filed (bank £1000, no CT liability); the current book carries
        // the payment.
        let company = example_company();
        let profile = example_profile();
        let accounts = example_accounts_meta();
        let period = accounts.period();
        let previous_book = ct_book(1000, -1000, period.start - chrono::Duration::days(1));
        let current_book = ct_payment_book("2006.21", date(2020, 6, 15));
        let prev = PreviousPeriodData {
            book: &previous_book,
            filing: None,
        };
        let base = Frs105Accounts::new(&current_book, &company, &profile, &accounts)
            .with_prev_period_data(&prev);

        // Without the adjustment, the payment debits a liability that was
        // never restored: the CT account shows as an asset in the current
        // column (+2006.21 creditors) — the inconsistency the adjustment
        // fixes.
        let close = |a: f64, b: f64| (a - b).abs() < 0.005;
        assert!(
            close(base.creditors_within_1_year[0], 2006.21),
            "payment with no restored liability: {:?}",
            base.creditors_within_1_year
        );

        // The start-balance adjustment restores the omitted liability at
        // the start of the previous period: Dr the CT equity reserve, Cr
        // the CT liability (the comparative column's creditors fall by
        // 2006.21 and capital and reserves by the same).
        let adjustments = vec![AdjustmentTransaction {
            post_datetime: date(2019, 1, 1).and_hms_opt(12, 0, 0).unwrap(),
            description: "restore omitted prior-year corporation tax liability".into(),
            splits: vec![
                AdjustmentSplit {
                    account: "Liabilities:Owed Corporation Tax".into(),
                    value: "-2006.21".parse().unwrap(),
                },
                AdjustmentSplit {
                    account: "Equity:Corporation Tax".into(),
                    value: "2006.21".parse().unwrap(),
                },
            ],
        }];
        let adjusted = base.clone().with_previous_year_adjustments(&adjustments);

        // Comparative column: the liability restored (creditors −2006.21),
        // the CT reserve reduced — net assets down by 2006.21.
        assert!(close(adjusted.creditors_within_1_year[1], -2006.21));
        assert!(close(adjusted.capital_and_reserves[1], -1006.21));
        assert!(close(adjusted.net_assets[1], -1006.21));
        // Current column: the restored liability nets against the payment —
        // the liability clears (creditors 0), the bank is lower, and net
        // assets fall by the same amount in both columns.
        assert!(close(adjusted.creditors_within_1_year[0], 0.0));
        assert!(close(adjusted.current_assets[0], -1006.21));
        assert!(close(adjusted.net_assets[0], -1006.21));
        assert!(close(adjusted.capital_and_reserves[0], -1006.21));
        // The comparative column keeps the filed current assets.
        assert!(close(adjusted.current_assets[1], 1000.0));
    }

    #[test]
    #[should_panic(expected = "references unknown account path")]
    fn previous_year_adjustments_reject_unknown_account_paths() {
        let company = example_company();
        let profile = example_profile();
        let accounts = example_accounts_meta();
        let period = accounts.period();
        let previous_book = ct_book(1000, -1000, period.start - chrono::Duration::days(1));
        let current_book = ct_payment_book("2006.21", date(2020, 6, 15));
        let prev = PreviousPeriodData {
            book: &previous_book,
            filing: None,
        };
        let base = Frs105Accounts::new(&current_book, &company, &profile, &accounts)
            .with_prev_period_data(&prev);
        let adjustments = vec![AdjustmentTransaction {
            post_datetime: date(2019, 1, 1).and_hms_opt(12, 0, 0).unwrap(),
            description: "typo".into(),
            splits: vec![AdjustmentSplit {
                account: "Liabilities:Nonexistent".into(),
                value: "-1".parse().unwrap(),
            }],
        }];
        let _ = base.with_previous_year_adjustments(&adjustments);
    }

    #[tokio::test]
    async fn validate_passes_on_a_balanced_report() {
        // A report computed from a balanced book ties on every identity in
        // both columns.
        let (company, gnucash) = load_example().await;
        let accounts = Frs105Accounts::new(
            &gnucash,
            &company,
            &example_profile(),
            &example_accounts_meta(),
        )
        .with_prev_period_data(&prev_from_same_book(&gnucash));
        accounts.validate().expect("the balanced report ties on every identity");
    }

    #[tokio::test]
    async fn validate_reports_a_net_assets_mismatch() {
        // Breaking the net-assets tie reports the derived-line mismatch and
        // the balance-sheet-does-not-balance violation together.
        let (company, gnucash) = load_example().await;
        let mut accounts = Frs105Accounts::new(
            &gnucash,
            &company,
            &example_profile(),
            &example_accounts_meta(),
        )
        .with_prev_period_data(&prev_from_same_book(&gnucash));
        accounts.net_assets[0] += 1.0;

        let err = accounts.validate().expect_err("the broken tie must fail");
        assert!(
            err.errors
                .iter()
                .any(|e| matches!(e, BalanceSheetCheckError::NetAssetsMismatch { column: "current", .. })),
            "the derived net-assets mismatch is reported: {err}"
        );
        assert!(
            err.errors.iter().any(|e| matches!(
                e,
                BalanceSheetCheckError::BalanceSheetDoesNotBalance { column: "current", .. }
            )),
            "the sheet no longer balances: {err}"
        );
    }

    #[tokio::test]
    async fn validate_reports_a_net_current_assets_mismatch() {
        // Breaking the net-current-assets tie in the previous column.
        let (company, gnucash) = load_example().await;
        let mut accounts = Frs105Accounts::new(
            &gnucash,
            &company,
            &example_profile(),
            &example_accounts_meta(),
        )
        .with_prev_period_data(&prev_from_same_book(&gnucash));
        accounts.net_current_assets[1] += 0.5;

        let err = accounts.validate().expect_err("the broken tie must fail");
        assert!(err.errors.iter().any(|e| matches!(
            e,
            BalanceSheetCheckError::NetCurrentAssetsMismatch { column: "previous", .. }
        )));
    }
}
