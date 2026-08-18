#![cfg(test)]
//! Test utilities for the ct600 crate.
//!
//! [`TestData::sample_tax`] is the shared sample FRS 105 tax computation
//! (the company fixtures it builds on live in the
//! [`companies_house::test_utils`] module of the client crate), [`sample_values`]
//! derives the CT600 form values from it, and [`REPO`] resolves the
//! repository root (the live-tests cache lives under `.cache/api_responses`).

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

/// The repository root directory.
pub static REPO: std::sync::LazyLock<std::path::PathBuf> = std::sync::LazyLock::new(|| {
    let path_bytes = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .unwrap()
        .stdout;
    let path_str = std::str::from_utf8(&path_bytes).unwrap().trim();
    std::path::PathBuf::from(path_str)
});

/// Hardcoded test data: the shared sample tax computation.
pub struct TestData;

impl TestData {
    /// A sample FRS 105 tax computation for a fictional company (Acme Ltd,
    /// company number `9876543`, period 2025), including the numeric facts
    /// (profits, tax rates, allowances, R&D) so the derived form values are
    /// fully populated.
    pub fn sample_tax() -> ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax {
        let mut facts = ixbrl::ixbrl_fmt::ParsedIxBrlFacts::default();
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
        let mut company = ixbrl::company::Company::new(
            &sample.company_name,
            "1234567890",
            &sample.company_number,
        );
        company.registration_date = chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax::from_parsed_facts(
            &facts,
            &company,
            &companies_house::test_utils::TestData::sample_accounts_meta(),
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
