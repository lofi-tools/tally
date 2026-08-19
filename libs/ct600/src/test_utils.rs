#![cfg(test)]
//! Test utilities for the ct600 crate.
//!
//! [`TestData::sample_tax`] is the shared sample FRS 105 tax computation
//! (the company profile it builds on lives in the
//! [`companies_house::test_utils`] module of the client crate; the company
//! number and accounts metadata come from the shared `test_utils` crate),
//! [`sample_values`] derives the CT600 form values from it, and [`REPO`]
//! (from the shared `test_utils` crate) resolves the repository root (the
//! live-tests cache lives under `.cache/api_responses`).

use crate::ct600_return::{
    CompanyInformation, Ct600Return, Declaration, EnvelopeConfig, FinancialYear,
    ReturnInfoSummary,
};
use crate::form::Ct600FormValues;
use chrono::NaiveDate;

/// The CT600 form values derived from the shared sample tax computation
/// ([`TestData::sample_tax`]).
pub fn sample_values() -> Ct600FormValues {
    Ct600FormValues::from_tax(&TestData::sample_tax())
}

/// The repository root and `.cache` helpers (from the shared `test_utils`
/// crate).
pub use test_utils::{REPO, cache_dir, cache_path};

/// Hardcoded test data: the shared sample tax computation.
pub struct TestData;

impl TestData {
    /// A sample FRS 105 tax computation for a fictional company (Acme Ltd,
    /// company number `9876543`, period 2025), including the numeric facts
    /// (profits, tax rates, allowances, R&D) so the derived form values are
    /// fully populated.
    pub fn sample_tax() -> reports::reports::uk_frs105_corp_tax::Frs105CorpTax {
        let mut facts = reports::ixbrl_fmt::ParsedIxBrlFacts::default();
        facts
            .non_numeric
            .insert("ct-comp:CompanyName".to_string(), "Acme Ltd".to_string());
        facts
            .non_numeric
            .insert("ct-comp:TaxReference".to_string(), "1234567890".to_string());
        facts.non_numeric.insert(
            "ct-comp:FinancialYear1CoveredByTheReturn".to_string(),
            "2025".to_string(),
        );
        facts.non_numeric.insert(
            "ct-comp:FinancialYear2CoveredByTheReturn".to_string(),
            "2026".to_string(),
        );
        facts.non_numeric.insert(
            "ct-comp:PeriodOfAccountStartDate".to_string(),
            "1 January 2026".to_string(),
        );
        facts.non_numeric.insert(
            "ct-comp:PeriodOfAccountEndDate".to_string(),
            "31 December 2026".to_string(),
        );
        for (name, ctx, v) in [
            (
                "ct-comp:AdjustedTradingProfitOfThisPeriod",
                "ctxt-3",
                12345.0,
            ),
            ("ct-comp:NetTradingProfits", "ctxt-3", 12345.0),
            (
                "ct-comp:FY1AmountOfProfitChargeableAtFirstRate",
                "ctxt-3",
                6000.0,
            ),
            (
                "ct-comp:FY2AmountOfProfitChargeableAtFirstRate",
                "ctxt-3",
                6345.0,
            ),
            ("ct-comp:FY1FirstRateOfTax", "ctxt-1", 19.0),
            ("ct-comp:FY2FirstRateOfTax", "ctxt-1", 19.0),
            ("ct-comp:FY1TaxAtFirstRate", "ctxt-3", 1140.0),
            ("ct-comp:FY2TaxAtFirstRate", "ctxt-3", 1205.55),
            ("ct-comp:CorporationTaxChargeable", "ctxt-3", 2345.55),
            ("ct-comp:TaxChargeable", "ctxt-3", 2345.55),
            ("ct-comp:TaxPayable", "ctxt-3", 2345.55),
            (
                "ct-comp:MainPoolAnnualInvestmentAllowance",
                "ctxt-2",
                1000.0,
            ),
            (
                "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
                "ctxt-4",
                5000.0,
            ),
        ] {
            facts
                .numeric_by_ctx
                .insert((name.to_string(), ctx.to_string()), v);
        }

        // The company is built from the client crate's sample company so the
        // profile served for it (by `tax.company_number()`) can never
        // disagree with the tax computation.
        let sample = companies_house::test_utils::TestData::sample_company();
        let mut company = reports::company::Company::new(
            &sample.company_name,
            "1234567890",
            &sample.company_number,
        );
        company.registration_date = chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        reports::reports::uk_frs105_corp_tax::Frs105CorpTax::from_parsed_facts(
            &facts,
            &company,
            &test_utils::Fixtures::sample_accounts_meta(),
        )
    }

    /// A minimal but complete `Ct600Return` for message-building tests.
    pub fn sample_return() -> Ct600Return {
        Ct600Return {
            envelope: EnvelopeConfig::default(),
            contact: Default::default(),
            sender: "Company".to_string(),
            company: CompanyInformation {
                company_name: "Acme Ltd".to_string(),
                registration_number: "12345678".to_string(),
                reference: "8596148860".to_string(),
                company_type: 1,
                period_start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                period_end: NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            },
            return_info: ReturnInfoSummary {
                this_period_accounts: true,
                this_period_computations: true,
            },
            turnover: 11218.0,
            trading_profits: 748.0,
            trading_losses_brought_forward: None,
            net_trading_profits: 748.0,
            profits_before_other_deductions: 748.0,
            profits_before_charges_and_group_relief: 748.0,
            chargeable_profits: 748.0,
            fy1: FinancialYear {
                year: 2024,
                profit: 186.0,
                tax_rate: 19.0,
                tax: 35.34,
            },
            fy2: FinancialYear {
                year: 2025,
                profit: 562.0,
                tax_rate: 19.0,
                tax: 106.78,
            },
            corporation_tax: 142.12,
            net_corporation_tax_chargeable: 142.12,
            net_corporation_tax_liability: 142.12,
            tax_chargeable: 142.12,
            tax_payable: 142.12,
            tax_payable_including_restitution_tax: 142.12,
            sme_claim: true,
            rnd_enhanced_expenditure: Some(465.0),
            rnd_and_creative_enhanced_expenditure: Some(465.0),
            aia_capital_allowances: 591.0,
            payment_address_lines: vec!["1 High Street".to_string()],
            payment_recipient: "Acme Ltd".to_string(),
            payment_nominee_reference: "8596148860".to_string(),
            declaration: Declaration {
                name: Some("Jane Doe".to_string()),
                date: Some(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()),
                status: Some("Director".to_string()),
            },
            computation_document: Some("<html/>".to_string()),
            accounts_document: Some("<html/>".to_string()),
        }
    }
}

/// Build a plausible previous-period chart of accounts from a filed balance
/// sheet: the accounts the reports read, each aggregate line distributed
/// across its contributing accounts (whole pounds, remainder to the last),
/// with the reports' sign convention (equity / income / expense stored
/// negative so the reports' debit flip reproduces the filed magnitudes).
/// One transaction per account, dated at the filing's period end, balanced
/// against an invisible "Opening Balances" account.
///
/// Used by the live full-year test: a filed balance sheet alone cannot
/// rebuild a company's CoA (the filing carries only the aggregates), so the
/// test feeds the builder a CoA that is *plausible* — it reproduces the
/// filed figures exactly when run through the reports' computations, so
/// `check_previous_period_matches_filing` passes.
///
/// Lines the reports cannot represent are not generated: called-up share
/// capital not paid (setter-only, no account path) is left zero, so a
/// nonzero filed amount on it would be reported by the check.
pub fn plausible_previous_book(
    filing: &companies_house::FiledBalanceSheet,
) -> reports::GnucashBook {
    use reports::{RawAccount, RawSplit, RawTransaction};

    /// Push an account and return its index (parents precede children).
    fn add_account(
        accounts: &mut Vec<RawAccount>,
        name: &str,
        r#type: &str,
        parent: Option<usize>,
    ) -> usize {
        let id = accounts.len();
        accounts.push(RawAccount {
            guid: format!("guid-{id}"),
            name: name.to_string(),
            r#type: r#type.to_string(),
            parent_guid: parent.map(|p| format!("guid-{p}")).unwrap_or_default(),
        });
        id
    }

    let mut accounts: Vec<RawAccount> = Vec::new();
    let root = add_account(&mut accounts, "Root Account", "ROOT", None);
    let assets = add_account(&mut accounts, "Assets", "ASSET", Some(root));
    let capital_equipment = add_account(&mut accounts, "Capital Equipment", "ASSET", Some(assets));
    let owed_to_us = add_account(&mut accounts, "Owed To Us", "ASSET", Some(assets));
    let vat_repayments = add_account(&mut accounts, "VAT Repayments Due", "ASSET", Some(assets));
    let prepayments = add_account(&mut accounts, "Prepayments and Accrued Income", "ASSET", Some(assets));
    let vat = add_account(&mut accounts, "VAT", "ASSET", Some(root));
    let vat_input = add_account(&mut accounts, "Input", "ASSET", Some(vat));
    let vat_output = add_account(&mut accounts, "Output", "LIABILITY", Some(vat));
    let vat_settlement = add_account(&mut accounts, "Settlement", "ASSET", Some(vat));
    let vat_settlement_input = add_account(&mut accounts, "Input", "ASSET", Some(vat_settlement));
    let vat_settlement_output = add_account(&mut accounts, "Output", "LIABILITY", Some(vat_settlement));
    let liabilities = add_account(&mut accounts, "Liabilities", "LIABILITY", Some(root));
    let credit_cards = add_account(&mut accounts, "Credit Cards", "LIABILITY", Some(liabilities));
    let owed_corp_tax = add_account(&mut accounts, "Owed Corporation Tax", "LIABILITY", Some(liabilities));
    let creditors_after_1yr = add_account(&mut accounts, "Creditors After 1 Year", "LIABILITY", Some(liabilities));
    let provisions = add_account(&mut accounts, "Provisions", "LIABILITY", Some(liabilities));
    let accruals = add_account(&mut accounts, "Accruals and Deferred Income", "LIABILITY", Some(liabilities));
    let equity = add_account(&mut accounts, "Equity", "EQUITY", Some(root));
    let shareholdings = add_account(&mut accounts, "Shareholdings", "EQUITY", Some(equity));
    let dividends = add_account(&mut accounts, "Dividends", "EQUITY", Some(equity));
    let corp_tax_equity = add_account(&mut accounts, "Corporation Tax", "EQUITY", Some(equity));
    let opening_balances = add_account(&mut accounts, "Opening Balances", "EQUITY", Some(equity));
    let bank = add_account(&mut accounts, "Bank Accounts", "BANK", Some(root));
    let accounts_receivable = add_account(&mut accounts, "Accounts Receivable", "RECEIVABLE", Some(root));
    let accounts_payable = add_account(&mut accounts, "Accounts Payable", "PAYABLE", Some(root));
    let income = add_account(&mut accounts, "Income", "INCOME", Some(root));
    let expenses = add_account(&mut accounts, "Expenses", "EXPENSE", Some(root));

    // Distribute each filed aggregate across its contributing accounts:
    // whole pounds, remainder to the last.  `total` carries the filed sign
    // (creditor lines negative); `negate` flips for the debit-side account
    // types (equity / income / expense), which the reports negate again.
    let f = &filing.figures;
    let mut postings: Vec<(usize, i64)> = Vec::new();
    let mut distribute = |targets: &[usize], total: f64, negate: bool| {
        let magnitude = total.abs().round() as i64;
        if magnitude == 0 {
            return;
        }
        let n = targets.len() as i64;
        let base = magnitude / n;
        let rem = magnitude % n;
        for (i, t) in targets.iter().enumerate() {
            let mut share = base + i64::from((i as i64) < rem);
            if total < 0.0 {
                share = -share;
            }
            if negate {
                share = -share;
            }
            postings.push((*t, share));
        }
    };
    distribute(&[capital_equipment], f.fixed_assets, false);
    distribute(
        &[
            accounts_receivable,
            owed_to_us,
            vat_input,
            vat_settlement_input,
            vat_repayments,
            bank,
        ],
        f.current_assets,
        false,
    );
    distribute(&[prepayments], f.prepayments_and_accrued_income, false);
    distribute(
        &[
            accounts_payable,
            vat_output,
            vat_settlement_output,
            credit_cards,
            owed_corp_tax,
        ],
        f.creditors_within_1_year,
        false,
    );
    distribute(&[creditors_after_1yr], f.creditors_after_1_year, false);
    distribute(&[provisions], f.provisions_for_liabilities, false);
    distribute(&[accruals], f.accruals_and_deferred_income, false);
    distribute(
        &[shareholdings, income, expenses, dividends, corp_tax_equity],
        f.capital_and_reserves,
        true,
    );

    // One balanced transaction per posting, dated at the filing's period
    // end (the previous period's balance-sheet date); the counter-split on
    // the invisible "Opening Balances" account keeps the book balanced.
    let date = filing.period_end;
    let raw_txns: Vec<RawTransaction> = postings
        .iter()
        .enumerate()
        .map(|(i, _)| RawTransaction {
            guid: format!("txn-{i}"),
            post_datetime: date.and_hms_opt(12, 0, 0).unwrap(),
            description: String::new(),
        })
        .collect();
    let raw_splits: Vec<RawSplit> = postings
        .iter()
        .enumerate()
        .flat_map(|(i, (acc, value))| {
            [
                RawSplit {
                    tx_guid: format!("txn-{i}"),
                    account_guid: format!("guid-{acc}"),
                    value: rucash::Num::from(*value),
                },
                RawSplit {
                    tx_guid: format!("txn-{i}"),
                    account_guid: format!("guid-{opening_balances}"),
                    value: rucash::Num::from(-*value),
                },
            ]
        })
        .collect();
    reports::GnucashBook::from_raw_parts(accounts, raw_txns, raw_splits)
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::NaiveDate;
    use companies_house::FiledBalanceSheet;
    use reports::company::{AccountingPeriod, AccountsMeta, Company, CompanyProfile};
    use reports::reports::uk_frs105_accounts::{Frs105Accounts, PreviousPeriodData};

    /// The generator reproduces the filed figures exactly: a plausible
    /// previous-period CoA built from a filing, run through the accounts
    /// builder, yields a previous-period column equal to the filing, and
    /// `check_previous_period_matches_filing` passes.
    #[test]
    fn plausible_previous_book_reproduces_the_filing() {
        let period_start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let period_end = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let filing = FiledBalanceSheet {
            period_start,
            period_end,
            figures: reports::reports::uk_frs105_accounts::PreviousPeriodFigures {
                fixed_assets: 1000.0,
                current_assets: 5000.0,
                prepayments_and_accrued_income: 200.0,
                creditors_within_1_year: -300.0,
                net_current_assets: 4900.0,
                total_assets_less_liabilities: 5900.0,
                creditors_after_1_year: -400.0,
                provisions_for_liabilities: 540.0,
                accruals_and_deferred_income: 60.0,
                // 5900 − 400 − 540 − 60: provisions and accruals are
                // deductions (the filed presentation), and net assets
                // equals capital and reserves.
                net_assets: 4900.0,
                capital_and_reserves: 4900.0,
                ..Default::default()
            },
        };

        let book = plausible_previous_book(&filing);
        let empty_current =
            reports::GnucashBook::from_raw_parts(Vec::new(), Vec::new(), Vec::new());
        let company = Company::new("Acme Ltd", "1234567890", "12345678");
        let meta = AccountsMeta {
            period: Some(AccountingPeriod {
                start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            }),
            ..AccountsMeta::default()
        };
        let accounts = Frs105Accounts::new(
            &empty_current,
            &company,
            &CompanyProfile::default(),
            &meta,
        )
        .with_prev_period_data(&PreviousPeriodData {
            book: &book,
            filing: Some(&filing),
        });

        // Every line of the previous-period column equals the filing.
        assert_eq!(accounts.fixed_assets[1], 1000.0);
        assert_eq!(accounts.current_assets[1], 5000.0);
        assert_eq!(accounts.prepayments_and_accrued_income[1], 200.0);
        assert_eq!(accounts.creditors_within_1_year[1], -300.0);
        assert_eq!(accounts.net_current_assets[1], 4900.0);
        assert_eq!(accounts.total_assets_less_liabilities[1], 5900.0);
        assert_eq!(accounts.creditors_after_1_year[1], -400.0);
        assert_eq!(accounts.provisions_for_liabilities[1], 540.0);
        assert_eq!(accounts.accruals_and_deferred_income[1], 60.0);
        assert_eq!(accounts.net_assets[1], 4900.0);
        assert_eq!(accounts.capital_and_reserves[1], 4900.0);

        // And the check reconciles.
        accounts
            .check_previous_period_matches_filing(&filing)
            .expect("the plausible book must reconcile with the filing");
    }
}
