use std::collections::HashMap;

use crate::GnucashBook;
use crate::company::{AccountingPeriod, AccountsMeta, Company};
use crate::ixbrl_fmt::*;

#[derive(Debug, Clone)]
pub struct RdExpenditureItem {
    pub label: String,
    pub account_path: String,
    pub values_by_fy: HashMap<i32, f64>,
}

#[derive(Debug, Clone)]
pub struct RdProject {
    pub name: String,
    pub items: Vec<RdExpenditureItem>,
    pub enhanced_by_fy: HashMap<i32, f64>,
}

#[derive(Debug, Clone)]
pub struct Frs105CorpTax {
    pub company: Company,
    pub accounts: AccountsMeta,

    pub turnover: f64,
    pub total_costs: f64,
    pub gross_profit: f64,
    pub profit_before_tax: f64,
    pub tax_expense: f64,
    pub profit_after_tax: f64,

    pub annual_investment_allowance: f64,

    pub adjusted_trading_profit: f64,
    pub trading_losses_brought_forward: f64,
    pub net_trading_profits: f64,
    pub net_chargeable_gains: f64,
    pub profits_before_deductions: f64,
    pub profits_before_charges: f64,
    pub qualifying_donations: f64,
    pub group_relief: f64,
    pub group_relief_carried_forward: f64,
    pub profits_chargeable_to_corporation_tax: f64,

    pub fy1_profit: f64,
    pub fy2_profit: f64,
    pub fy1_tax: f64,
    pub fy2_tax: f64,
    pub corporation_tax_chargeable: f64,
    pub prev_fy1_profit: f64,
    pub prev_fy2_profit: f64,
    pub prev_fy1_tax: f64,
    pub prev_fy2_tax: f64,
    pub prev_corporation_tax_chargeable: f64,
    pub prev_profit_chargeable: f64,
    pub marginal_relief: f64,
    pub corporation_tax_chargeable_payable: f64,
    pub total_reliefs_deductions_tax: f64,
    pub net_corporation_tax_payable: f64,
    pub tax_chargeable: f64,
    pub tax_payable: f64,

    pub losses_of_trades_uk: f64,
    pub losses_of_trades_overseas: f64,
    pub uk_property_business_losses: f64,
    pub overseas_property_business_losses: f64,
    pub losses_from_miscellaneous: f64,
    pub capital_losses: f64,
    pub losses_on_intangible_fixed_assets: f64,

    pub rnd_qualifying_expenditure: f64,
    pub rnd_enhanced_expenditure: f64,
    pub creative_enhanced_expenditure: f64,
    pub rnd_creative_enhanced_total: f64,
    pub rnd_subcontracted_large: f64,
    pub is_sme: bool,
    pub is_large_company: bool,
    pub rnd_claim_notification: bool,
    pub rnd_additional_information: bool,
    pub partner_in_a_firm: bool,

    pub rd_projects: Vec<RdProject>,

    pub profit_per_accounts_by_fy: HashMap<i32, f64>,
    pub aia_by_fy: HashMap<i32, f64>,
    pub rnd_by_fy: HashMap<i32, f64>,
    pub turnover_by_fy: HashMap<i32, f64>,
    pub costs_by_fy: HashMap<i32, f64>,
    pub profit_before_tax_by_fy: HashMap<i32, f64>,
    pub tax_expense_by_fy: HashMap<i32, f64>,
    pub profit_after_tax_by_fy: HashMap<i32, f64>,
    pub wages_by_fy: HashMap<i32, f64>,
    pub pensions_by_fy: HashMap<i32, f64>,
    pub expenses_by_fy: HashMap<String, HashMap<i32, f64>>,
}

#[allow(clippy::type_complexity)]
#[derive(Debug, Clone)]
pub struct Frs105CorpTaxBuilder<'a> {
    company: &'a Company,
    accounts: &'a AccountsMeta,
    period_splits: Vec<(f64, String)>,
    prev_splits: Vec<(f64, String)>,
    rd_project_defs: Vec<(&'a str, Vec<(&'a str, &'a str)>, &'a str)>,
}

#[allow(clippy::type_complexity)]
impl<'a> Frs105CorpTaxBuilder<'a> {
    pub fn add_rd_project(
        mut self,
        name: &'a str,
        items: &[(&'a str, &'a str)],
        enhanced_path: &'a str,
    ) -> Self {
        self.rd_project_defs
            .push((name, items.to_vec(), enhanced_path));
        self
    }

    pub fn build(self) -> Frs105CorpTax {
        Frs105CorpTax::from_splits(
            self.company,
            self.accounts,
            &self.period_splits,
            &self.prev_splits,
            &self.rd_project_defs,
        )
    }
}

fn round_down(v: f64) -> f64 {
    v.floor()
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorporationTaxCalculation {
    pub taxable_profit: f64,
    pub tax_at_main_rate: f64,
    pub marginal_relief: f64,
    pub corporation_tax: f64,
    pub effective_rate: f64,
}

/// Calculate Corporation Tax for UK 2025/26 tax year using marginal relief formula.
///
/// Rates and thresholds:
/// - Up to £50,000: 19% (Small Profits Rate)
/// - £50,001 to £250,000: Marginal Relief (gradual transition from 19% to 25%)
/// - £250,000 and above: 25% (Main Rate)
///
/// Formula: Corporation Tax = (Profits × 25%) - Marginal Relief
/// Marginal Relief = (Upper Limit - Profits) × 3/200
pub fn calculate_corporation_tax_2025(taxable_profit: f64) -> CorporationTaxCalculation {
    const SMALL_PROFITS_LIMIT: f64 = 50_000.0;
    const UPPER_LIMIT: f64 = 250_000.0;
    const MAIN_RATE: f64 = 0.25;
    const MARGINAL_RELIEF_FRACTION: f64 = 3.0 / 200.0;

    let tax_at_main_rate = round2(taxable_profit * MAIN_RATE);

    let marginal_relief = if taxable_profit <= SMALL_PROFITS_LIMIT {
        // Below small profits limit - no marginal relief needed
        // Tax is simply profit × 19%
        0.0
    } else if taxable_profit <= UPPER_LIMIT {
        // In marginal relief band
        round2((UPPER_LIMIT - taxable_profit) * MARGINAL_RELIEF_FRACTION)
    } else {
        // Above upper limit - no marginal relief
        0.0
    };

    let corporation_tax = if taxable_profit <= SMALL_PROFITS_LIMIT {
        // Use small profits rate directly
        round2(taxable_profit * 0.19)
    } else {
        // Use main rate minus marginal relief
        round2(tax_at_main_rate - marginal_relief)
    };

    let effective_rate = if taxable_profit > 0.0 {
        round2((corporation_tax / taxable_profit) * 100.0)
    } else {
        0.0
    };

    CorporationTaxCalculation {
        taxable_profit,
        tax_at_main_rate,
        marginal_relief,
        corporation_tax,
        effective_rate,
    }
}

#[allow(clippy::type_complexity)]
impl Frs105CorpTax {
    pub fn builder<'a>(
        gnucash: &GnucashBook,
        company: &'a Company,
        accounts: &'a AccountsMeta,
    ) -> Frs105CorpTaxBuilder<'a> {
        let period = accounts.period();
        let accounts_raw = gnucash.raw_accounts();
        let txns = gnucash.raw_transactions();
        let splits = gnucash.raw_splits();

        let mut period_splits: Vec<(f64, String)> = Vec::new();
        let mut prev_splits: Vec<(f64, String)> = Vec::new();

        let prev_start = period.previous().start;
        let prev_end = period.previous().end;

        for split in splits {
            let tx = match txns.iter().find(|t| t.guid == split.tx_guid) {
                Some(t) => t,
                None => continue,
            };
            let tx_date = tx.post_datetime.date();
            let acc = match accounts_raw.iter().find(|a| a.guid == split.account_guid) {
                Some(a) => a,
                None => continue,
            };
            if acc.r#type == "ROOT" || acc.r#type == "TEMPLATE" {
                continue;
            }
            let path = Self::account_path(accounts_raw, acc);
            let val = split.value.to_string().parse::<f64>().unwrap_or(0.0);

            if tx_date >= period.start
                && tx_date <= period.end
            {
                period_splits.push((val, path.clone()));
            }
            if tx_date >= prev_start && tx_date <= prev_end {
                prev_splits.push((val, path.clone()));
            }
        }

        Frs105CorpTaxBuilder {
            company,
            accounts,
            period_splits,
            prev_splits,
            rd_project_defs: Vec::new(),
        }
    }

    fn from_splits(
        company: &Company,
        accounts: &AccountsMeta,
        period_splits: &[(f64, String)],
        prev_splits: &[(f64, String)],
        rd_project_defs: &[(&str, Vec<(&str, &str)>, &str)],
    ) -> Self {
        let period = accounts.period();
        let sum = |splits: &[(f64, String)], account: &str| -> f64 {
            splits
                .iter()
                .filter(|(_, p)| p == account)
                .map(|(v, _)| v)
                .sum()
        };

        let sum_abs =
            |splits: &[(f64, String)], account: &str| -> f64 { sum(splits, account).abs() };

        let sum_prefix = |splits: &[(f64, String)], prefix: &str| -> f64 {
            splits
                .iter()
                .filter(|(_, p)| p == prefix || p.starts_with(&format!("{}:", prefix)))
                .map(|(v, _)| v.abs())
                .sum()
        };

        let ct_turnover_current =
            sum_abs(period_splits, "Income:Sales:UK") + sum_abs(period_splits, "Income:Interest");

        let salaries_current = sum_prefix(period_splits, "Expenses:Emoluments:Employees");
        let pensions_current = sum_abs(
            period_splits,
            "Expenses:Emoluments:Employer Pension Contribution",
        );
        let accountancy_current = sum_abs(period_splits, "Expenses:VAT Purchases:Accountant");
        let bank_charges_current = sum_abs(period_splits, "Expenses:VAT Purchases:Bank Charges");
        let office_current = sum_abs(period_splits, "Expenses:VAT Purchases:Office");
        let software_current = sum_abs(period_splits, "Expenses:VAT Purchases:Software");
        let subscriptions_current = sum_abs(period_splits, "Expenses:VAT Purchases:Subscriptions");
        let sundries_current = sum_abs(period_splits, "Expenses:VAT Purchases:Sundries");
        let telecoms_current = sum_abs(period_splits, "Expenses:VAT Purchases:Telecoms");
        let travel_current = sum_abs(period_splits, "Expenses:VAT Purchases:Travel/Accom");

        let total_costs_current = salaries_current
            + pensions_current
            + accountancy_current
            + bank_charges_current
            + office_current
            + software_current
            + subscriptions_current
            + sundries_current
            + telecoms_current
            + travel_current;

        let gross_profit_current = round_down(ct_turnover_current);
        let profit_before_tax_current = gross_profit_current - total_costs_current;

        let aia_current = round_down(sum_abs(
            period_splits,
            "Assets:Capital Equipment:Computer Equipment",
        ));

        let rnd_enhanced_current = if !rd_project_defs.is_empty() {
            round_down(sum_abs(period_splits, rd_project_defs[0].2))
        } else {
            0.0
        };

        let rd_projects: Vec<RdProject> = rd_project_defs
            .iter()
            .map(|(name, items, enhanced_path)| {
                let items: Vec<RdExpenditureItem> = items
                    .iter()
                    .map(|(label, path)| RdExpenditureItem {
                        label: label.to_string(),
                        account_path: path.to_string(),
                        values_by_fy: HashMap::from([
                            (accounts.fy1_year, round_down(sum_abs(prev_splits, path))),
                            (accounts.fy2_year, round_down(sum_abs(period_splits, path))),
                        ]),
                    })
                    .collect();
                let enhanced: HashMap<i32, f64> = HashMap::from([
                    (
                        accounts.fy1_year,
                        round_down(sum_abs(prev_splits, enhanced_path)),
                    ),
                    (
                        accounts.fy2_year,
                        round_down(sum_abs(period_splits, enhanced_path)),
                    ),
                ]);
                RdProject {
                    name: name.to_string(),
                    items,
                    enhanced_by_fy: enhanced,
                }
            })
            .collect();

        let ct_trading_profits_raw = profit_before_tax_current - aia_current - rnd_enhanced_current;
        let ct_trading_profits = if ct_trading_profits_raw > 0.0 {
            round_down(ct_trading_profits_raw)
        } else {
            0.0
        };

        let trading_losses_brought_forward = 0.0;
        let net_trading_profits = ct_trading_profits + trading_losses_brought_forward;
        let profits_before_deductions = net_trading_profits;
        let profits_before_charges = profits_before_deductions;
        let profits_chargeable = profits_before_charges;

        let fy1_end = chrono::NaiveDate::from_ymd_opt(accounts.fy2_year, 3, 31).unwrap();
        let total_days =
            (period.end - period.start).num_days() + 1;
        let fy1_days = {
            let s = period.start;
            let e = fy1_end.min(period.end);
            if e >= s { (e - s).num_days() + 1 } else { 0 }
        };
        let fy2_days = total_days - fy1_days;

        let fy1_profit = (profits_chargeable * fy1_days as f64 / total_days as f64).round();
        let fy2_profit = (profits_chargeable * fy2_days as f64 / total_days as f64).round();
        let fy1_tax = round2(fy1_profit * accounts.fy1_rate / 100.0);
        let fy2_tax = round2(fy2_profit * accounts.fy2_rate / 100.0);
        let corporation_tax_chargeable = round2(fy1_tax + fy2_tax);

        let marginal_relief = 0.0;
        let corporation_tax_chargeable_payable =
            round2(corporation_tax_chargeable + marginal_relief);
        let total_reliefs_deductions_tax = 0.0;
        let net_corporation_tax_payable =
            round2(corporation_tax_chargeable_payable - total_reliefs_deductions_tax);
        let tax_chargeable = net_corporation_tax_payable;
        let tax_payable = tax_chargeable;

        let tax_expense_current = sum_abs(period_splits, "Equity:Corporation Tax:Corporation Tax");
        let profit_after_tax = round2(profit_before_tax_current - tax_expense_current);

        let ct_turnover_prev =
            sum_abs(prev_splits, "Income:Sales:UK") + sum_abs(prev_splits, "Income:Interest");
        let salaries_prev = sum_prefix(prev_splits, "Expenses:Emoluments:Employees");
        let pensions_prev = sum_abs(
            prev_splits,
            "Expenses:Emoluments:Employer Pension Contribution",
        );
        let accountancy_prev = sum_abs(prev_splits, "Expenses:VAT Purchases:Accountant");
        let bank_charges_prev = sum_abs(prev_splits, "Expenses:VAT Purchases:Bank Charges");
        let office_prev = sum_abs(prev_splits, "Expenses:VAT Purchases:Office");
        let software_prev = sum_abs(prev_splits, "Expenses:VAT Purchases:Software");
        let subscriptions_prev = sum_abs(prev_splits, "Expenses:VAT Purchases:Subscriptions");
        let sundries_prev = sum_abs(prev_splits, "Expenses:VAT Purchases:Sundries");
        let telecoms_prev = sum_abs(prev_splits, "Expenses:VAT Purchases:Telecoms");
        let travel_prev = sum_abs(prev_splits, "Expenses:VAT Purchases:Travel/Accom");

        let total_costs_prev = salaries_prev
            + pensions_prev
            + accountancy_prev
            + bank_charges_prev
            + office_prev
            + software_prev
            + subscriptions_prev
            + sundries_prev
            + telecoms_prev
            + travel_prev;

        let gross_profit_prev = round_down(ct_turnover_prev);
        let profit_before_tax_prev = gross_profit_prev - total_costs_prev;
        let aia_prev = round_down(sum_abs(
            prev_splits,
            "Assets:Capital Equipment:Computer Equipment",
        ));
        let rnd_enhanced_prev = if !rd_project_defs.is_empty() {
            round_down(sum_abs(prev_splits, rd_project_defs[0].2))
        } else {
            0.0
        };
        let tax_expense_prev = sum_abs(prev_splits, "Equity:Corporation Tax:Corporation Tax");

        let prev_ct_trading_profits_raw = profit_before_tax_prev - aia_prev - rnd_enhanced_prev;
        let prev_ct_trading_profits = if prev_ct_trading_profits_raw > 0.0 {
            round_down(prev_ct_trading_profits_raw)
        } else {
            0.0
        };
        let prev_profit_chargeable = prev_ct_trading_profits;
        let prev_period_start = period.previous().start;
        let prev_period_end = period.previous().end;
        let prev_total_days = (prev_period_end - prev_period_start).num_days() + 1;
        let prev_fy1_end = chrono::NaiveDate::from_ymd_opt(accounts.fy2_year - 1, 3, 31).unwrap();
        let prev_fy1_days = {
            let s = prev_period_start;
            let e = prev_fy1_end.min(prev_period_end);
            if e >= s { (e - s).num_days() + 1 } else { 0 }
        };
        let prev_fy2_days = prev_total_days - prev_fy1_days;
        let prev_fy1_profit =
            (prev_profit_chargeable * prev_fy1_days as f64 / prev_total_days as f64).round();
        let prev_fy2_profit =
            (prev_profit_chargeable * prev_fy2_days as f64 / prev_total_days as f64).round();
        let prev_fy1_tax = round2(prev_fy1_profit * accounts.fy1_rate / 100.0);
        let prev_fy2_tax = round2(prev_fy2_profit * accounts.fy2_rate / 100.0);
        let prev_corporation_tax_chargeable = round2(prev_fy1_tax + prev_fy2_tax);

        let mut expenses_by_fy: HashMap<String, HashMap<i32, f64>> = HashMap::new();
        let exp = [
            ("accountancy", accountancy_current, accountancy_prev),
            ("bank-charges", bank_charges_current, bank_charges_prev),
            ("office", office_current, office_prev),
            ("software-expenses", software_current, software_prev),
            (
                "subscriptions-costs",
                subscriptions_current,
                subscriptions_prev,
            ),
            ("sundries", sundries_current, sundries_prev),
            ("telecoms", telecoms_current, telecoms_prev),
            ("travel", travel_current, travel_prev),
        ];
        for (id, cur, prev) in exp {
            expenses_by_fy.insert(
                id.to_string(),
                HashMap::from([(accounts.fy2_year, cur), (accounts.fy1_year, prev)]),
            );
        }

        Frs105CorpTax {
            company: company.clone(),
            accounts: accounts.clone(),

            turnover: ct_turnover_current,
            total_costs: total_costs_current,
            gross_profit: gross_profit_current,
            profit_before_tax: profit_before_tax_current,
            tax_expense: tax_expense_current,
            profit_after_tax,

            annual_investment_allowance: aia_current,

            adjusted_trading_profit: ct_trading_profits,
            trading_losses_brought_forward,
            net_trading_profits,
            net_chargeable_gains: 0.0,
            profits_before_deductions,
            profits_before_charges,
            qualifying_donations: 0.0,
            group_relief: 0.0,
            group_relief_carried_forward: 0.0,
            profits_chargeable_to_corporation_tax: profits_chargeable,

            fy1_profit,
            fy2_profit,
            fy1_tax,
            fy2_tax,
            corporation_tax_chargeable,
            prev_fy1_profit,
            prev_fy2_profit,
            prev_fy1_tax,
            prev_fy2_tax,
            prev_corporation_tax_chargeable,
            prev_profit_chargeable,
            marginal_relief,
            corporation_tax_chargeable_payable,
            total_reliefs_deductions_tax,
            net_corporation_tax_payable,
            tax_chargeable,
            tax_payable,

            losses_of_trades_uk: 0.0,
            losses_of_trades_overseas: 0.0,
            uk_property_business_losses: 0.0,
            overseas_property_business_losses: 0.0,
            losses_from_miscellaneous: 0.0,
            capital_losses: 0.0,
            losses_on_intangible_fixed_assets: 0.0,

            rnd_qualifying_expenditure: 0.0,
            rnd_enhanced_expenditure: rnd_enhanced_current,
            creative_enhanced_expenditure: 0.0,
            rnd_creative_enhanced_total: rnd_enhanced_current,
            rnd_subcontracted_large: 0.0,
            is_sme: true,
            is_large_company: false,
            rnd_claim_notification: false,
            rnd_additional_information: false,
            partner_in_a_firm: false,

            rd_projects,

            profit_per_accounts_by_fy: HashMap::from([
                (accounts.fy1_year, profit_before_tax_prev),
                (accounts.fy2_year, profit_before_tax_current),
            ]),
            aia_by_fy: HashMap::from([
                (accounts.fy1_year, aia_prev),
                (accounts.fy2_year, aia_current),
            ]),
            rnd_by_fy: HashMap::from([
                (accounts.fy1_year, rnd_enhanced_prev),
                (accounts.fy2_year, rnd_enhanced_current),
            ]),
            turnover_by_fy: HashMap::from([
                (accounts.fy1_year, ct_turnover_prev),
                (accounts.fy2_year, ct_turnover_current),
            ]),
            costs_by_fy: HashMap::from([
                (accounts.fy1_year, total_costs_prev),
                (accounts.fy2_year, total_costs_current),
            ]),
            profit_before_tax_by_fy: HashMap::from([
                (accounts.fy1_year, profit_before_tax_prev),
                (accounts.fy2_year, profit_before_tax_current),
            ]),
            tax_expense_by_fy: HashMap::from([
                (accounts.fy1_year, tax_expense_prev),
                (accounts.fy2_year, tax_expense_current),
            ]),
            profit_after_tax_by_fy: HashMap::from([
                (
                    accounts.fy1_year,
                    round2(profit_before_tax_prev - tax_expense_prev),
                ),
                (accounts.fy2_year, profit_after_tax),
            ]),
            wages_by_fy: HashMap::from([
                (accounts.fy1_year, salaries_prev),
                (accounts.fy2_year, salaries_current),
            ]),
            pensions_by_fy: HashMap::from([
                (accounts.fy1_year, pensions_prev),
                (accounts.fy2_year, pensions_current),
            ]),
            expenses_by_fy,
        }
    }

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

    pub fn to_ixbrl(&self) -> String {
        // -- Build the ix:header section --------------------------------------

        let hidden = elt("ix:hidden", &[]).children(vec![
            non_numeric(
                "ct-comp:NameOfProductionSoftware",
                "ctxt-0",
                "ixbrl-reporter",
            ),
            non_numeric("ct-comp:VersionOfProductionSoftware", "ctxt-0", "1.2.1"),
            non_numeric("ct-comp:CompanyName", "ctxt-0", &self.company.name),
            non_numeric(
                "ct-comp:TaxReference",
                "ctxt-0",
                &self.company.tax_reference,
            ),
        ]);

        let refs = elt("ix:references", &[]).children(vec![
            elt_text(
                "link:schemaRef",
                &[
                    ("xlink:type", "simple"),
                    (
                        "xlink:href",
                        "http://www.hmrc.gov.uk/schemas/ct/comp/2023-01-01/ct-comp-2023.xsd",
                    ),
                ],
                "",
            ),
            elt_text(
                "link:schemaRef",
                &[
                    ("xlink:type", "simple"),
                    (
                        "xlink:href",
                        "https://xbrl.frc.org.uk/dpl/2023-01-01/dpl-2023-01-01.xsd",
                    ),
                ],
                "",
            ),
        ]);

        let resources = elt("ix:resources", &[]).children(vec![
            context_instant(
                "ctxt-0",
                &self.company.company_number,
                &self.accounts.period().end,
                Some("ct-comp:BusinessTypeDimension"),
                Some("ct-comp:Company"),
            ),
            context_duration(
                "ctxt-1",
                &self.company.company_number,
                &self.accounts.period().start,
                &self.accounts.period().end,
                Some("ct-comp:BusinessTypeDimension"),
                Some("ct-comp:Company"),
            ),
            context_duration(
                "ctxt-2",
                &self.company.company_number,
                &self.accounts.period().start,
                &self.accounts.period().end,
                Some("ct-comp:BusinessTypeDimension"),
                Some("ct-comp:ManagementExpenses"),
            ),
            context_duration(
                "ctxt-3",
                &self.company.company_number,
                &self.accounts.period().start,
                &self.accounts.period().end,
                Some("ct-comp:BusinessTypeDimension"),
                Some("ct-comp:Company"),
            ),
            context_duration_full(
                "ctxt-4",
                &self.company.company_number,
                &self.accounts.period().start,
                &self.accounts.period().end,
                Some("ct-comp:BusinessNameDimension"),
                Some(&self.company.name),
                &[
                    ("ct-comp:BusinessTypeDimension", "ct-comp:Trade"),
                    ("ct-comp:LossReformDimension", "ct-comp:Post-lossReform"),
                    ("ct-comp:TerritoryDimension", "ct-comp:UK"),
                ],
            ),
            context_duration_full(
                "ctxt-5",
                &self.company.company_number,
                &self.accounts.period().start,
                &self.accounts.period().end,
                Some("ct-comp:BusinessNameDimension"),
                Some(&self.company.name),
                &[
                    ("ct-comp:BusinessTypeDimension", "ct-comp:Trade"),
                    ("ct-comp:LossReformDimension", "ct-comp:Post-lossReform"),
                    ("ct-comp:TerritoryDimension", "ct-comp:UK"),
                ],
            ),
            context_duration_full(
                "ctxt-8",
                &self.company.company_number,
                &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
                Some("ct-comp:BusinessNameDimension"),
                Some(&self.company.name),
                &[
                    ("ct-comp:BusinessTypeDimension", "ct-comp:Trade"),
                    ("ct-comp:LossReformDimension", "ct-comp:Post-lossReform"),
                    ("ct-comp:TerritoryDimension", "ct-comp:UK"),
                ],
            ),
            context_duration_full(
                "ctxt-9",
                &self.company.company_number,
                &self.accounts.period().start,
                &self.accounts.period().end,
                None,
                None,
                &[
                    ("dpl:DetailedAnalysisDimension", "dpl:Item1"),
                    ("uk-geo:CountriesRegionsDimension", "uk-geo:UnitedKingdom"),
                ],
            ),
            context_duration_full(
                "ctxt-10",
                &self.company.company_number,
                &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
                None,
                None,
                &[
                    ("dpl:DetailedAnalysisDimension", "dpl:Item1"),
                    ("uk-geo:CountriesRegionsDimension", "uk-geo:UnitedKingdom"),
                ],
            ),
            context_duration_full(
                "ctxt-11",
                &self.company.company_number,
                &self.accounts.period().start,
                &self.accounts.period().end,
                None,
                None,
                &[],
            ),
            context_duration_full(
                "ctxt-12",
                &self.company.company_number,
                &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
                None,
                None,
                &[],
            ),
            context_duration(
                "ctxt-13",
                &self.company.company_number,
                &self.accounts.period().start,
                &self.accounts.period().end,
                Some("dpl:ExpenseTypeDimension"),
                Some("dpl:AdministrativeExpenses"),
            ),
            context_duration(
                "ctxt-14",
                &self.company.company_number,
                &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
                Some("dpl:ExpenseTypeDimension"),
                Some("dpl:AdministrativeExpenses"),
            ),
            context_duration(
                "ctxt-15",
                &self.company.company_number,
                &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
                Some("ct-comp:BusinessTypeDimension"),
                Some("ct-comp:ManagementExpenses"),
            ),
            context_duration(
                "ctxt-16",
                &self.company.company_number,
                &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
                Some("ct-comp:BusinessTypeDimension"),
                Some("ct-comp:Company"),
            ),
            unit("iso4217:GBP"),
        ]);

        let header = elt("ix:header", &[]).children(vec![hidden, refs, resources]);

        // -- Build report pages -----------------------------------------------

        let mut report_pages = vec![
            self.build_frs105_corp_tax_page(),
            self.build_capital_allowances_page(),
            self.build_profits_and_gains_page(),
            self.build_losses_page(),
            self.build_tax_chargeable_page(),
            self.build_rnd_page(),
        ];
        if !self.rd_projects.is_empty() {
            report_pages.push(self.build_rnd_worksheet_page());
        }
        report_pages.push(self.build_profit_and_loss_worksheet());
        report_pages.push(self.build_tax_calculation_worksheet());

        // -- Assemble full document -------------------------------------------

        let doc = elt("html", HTML_ATTRS).children(vec![
            elt("head", &[]).children(vec![
                elt_text("title", &[], "Corporation Tax Statement"),
                elt_text(
                    "style",
                    &[("type", "text/css")],
                    include_str!("uk_frs105_corp_tax.css"),
                ),
            ]),
            elt("body", &[]).children(vec![
                elt("div", &[("style", "display:none")]).child(header),
                elt("div", &[("id", "report"), ("class", "report")]).children(report_pages),
            ]),
        ]);

        let body = doc.to_xml_string();
        format!("<?xml version='1.0' encoding='UTF-8'?>\n{}", body)
    }

    /// Parse a [`ParsedIxBrlFacts`] into a [`Frs105CorpTax`].
    ///
    /// The `company` parameter supplies fields that are not represented in the
    /// iXBRL output (such as `company_number` and `registration_date`), and the
    /// `accounts` parameter supplies the default financial-year tax parameters.
    /// Any data that *is* present in the facts (name, tax reference, the
    /// accounting-period dates, financial-year numbers, tax rates) overrides the
    /// corresponding values on the supplied inputs.
    ///
    /// Numeric fields that have no corresponding iXBRL fact (e.g. `turnover`,
    /// `total_costs`, `gross_profit`, `tax_expense`, `profit_after_tax`) are set
    /// to `0.0`.  Similarly, the `rd_projects` vector and several per-year
    /// `HashMap`s that are not serialised to iXBRL are left empty.
    pub fn from_parsed_facts(
        facts: &ParsedIxBrlFacts,
        company: &Company,
        accounts: &AccountsMeta,
    ) -> Frs105CorpTax {
        let period = accounts.period();

        // -- helpers -----------------------------------------------------------

        // Look up a numeric fact by (name, context).
        let num = |name: &str, ctx: &str| -> f64 {
            facts
                .numeric_by_ctx
                .get(&(name.to_string(), ctx.to_string()))
                .copied()
                .unwrap_or(0.0)
        };

        // Look up a non-numeric fact by name (last value wins).
        let text =
            |name: &str| -> String { facts.non_numeric.get(name).cloned().unwrap_or_default() };

        // Parse a date from a non-numeric fact whose value is formatted as
        // `"1 January 2020"` (with non-breaking spaces after `unescape`).
        let parse_date = |name: &str| -> chrono::NaiveDate {
            let raw = text(name);
            let cleaned = raw.replace('\u{00A0}', " ");
            chrono::NaiveDate::parse_from_str(&cleaned, "%d %B %Y")
                .unwrap_or(period.start)
        };

        // -- company ----------------------------------------------------------

        let fy1_year = text("ct-comp:FinancialYear1CoveredByTheReturn")
            .parse::<i32>()
            .unwrap_or(accounts.fy1_year);
        let fy2_year = text("ct-comp:FinancialYear2CoveredByTheReturn")
            .parse::<i32>()
            .unwrap_or(accounts.fy2_year);

        let company = Company {
            name: text("ct-comp:CompanyName"),
            tax_reference: text("ct-comp:TaxReference"),
            company_number: company.company_number.clone(),
            registration_date: company.registration_date,
        };
        let accounts = AccountsMeta {
            period: Some(AccountingPeriod {
                start: parse_date("ct-comp:PeriodOfAccountStartDate"),
                end: parse_date("ct-comp:PeriodOfAccountEndDate"),
            }),
            accounts_made_up_to: None,
            fy1_year,
            fy2_year,
            fy1_rate: num("ct-comp:FY1FirstRateOfTax", "ctxt-1"),
            fy2_rate: num("ct-comp:FY2FirstRateOfTax", "ctxt-1"),
            ..AccountsMeta::default()
        };

        let fy1 = accounts.fy1_year;
        let fy2 = accounts.fy2_year;

        // -- per-year hash maps ----------------------------------------------

        let profit_per_accounts_by_fy: HashMap<i32, f64> = HashMap::from([
            (fy1, num("ct-comp:ProfitLossPerAccounts", "ctxt-8")),
            (fy2, num("ct-comp:ProfitLossPerAccounts", "ctxt-4")),
        ]);

        let aia_by_fy: HashMap<i32, f64> = HashMap::from([
            (
                fy1,
                num("ct-comp:MainPoolAnnualInvestmentAllowance", "ctxt-15"),
            ),
            (
                fy2,
                num("ct-comp:MainPoolAnnualInvestmentAllowance", "ctxt-2"),
            ),
        ]);

        let rnd_by_fy: HashMap<i32, f64> = HashMap::from([
            (
                fy1,
                num(
                    "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
                    "ctxt-8",
                ),
            ),
            (
                fy2,
                num(
                    "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
                    "ctxt-4",
                ),
            ),
        ]);

        // -- scalar fields ----------------------------------------------------

        let annual_investment_allowance =
            num("ct-comp:MainPoolAnnualInvestmentAllowance", "ctxt-2");
        let adjusted_trading_profit = num("ct-comp:AdjustedTradingProfitOfThisPeriod", "ctxt-3");
        let trading_losses_brought_forward = num("ct-comp:TradingLossesBroughtForward", "ctxt-3");
        let net_trading_profits = num("ct-comp:NetTradingProfits", "ctxt-3");
        let net_chargeable_gains = num("ct-comp:NetChargeableGains", "ctxt-1");
        let profits_before_deductions =
            num("ct-comp:ProfitsBeforeOtherDeductionsAndReliefs", "ctxt-3");
        let profits_before_charges = num("ct-comp:ProfitsBeforeChargesAndGroupRelief", "ctxt-3");
        let qualifying_donations = num("ct-comp:QualifyingDonations", "ctxt-1");
        let group_relief = num("ct-comp:GroupReliefClaimed", "ctxt-1");
        let group_relief_carried_forward = num(
            "ct-comp:GroupReliefClaimedForCarriedForwardLosses",
            "ctxt-1",
        );
        let profits_chargeable_to_corporation_tax =
            num("ct-comp:TotalProfitsChargeableToCorporationTax", "ctxt-3");

        let fy1_profit = num("ct-comp:FY1AmountOfProfitChargeableAtFirstRate", "ctxt-3");
        let fy2_profit = num("ct-comp:FY2AmountOfProfitChargeableAtFirstRate", "ctxt-3");
        let fy1_tax = num("ct-comp:FY1TaxAtFirstRate", "ctxt-3");
        let fy2_tax = num("ct-comp:FY2TaxAtFirstRate", "ctxt-3");
        let corporation_tax_chargeable = num("ct-comp:CorporationTaxChargeable", "ctxt-3");

        let prev_fy1_profit = num("ct-comp:FY1AmountOfProfitChargeableAtFirstRate", "ctxt-16");
        let prev_fy2_profit = num("ct-comp:FY2AmountOfProfitChargeableAtFirstRate", "ctxt-16");
        let prev_fy1_tax = num("ct-comp:FY1TaxAtFirstRate", "ctxt-16");
        let prev_fy2_tax = num("ct-comp:FY2TaxAtFirstRate", "ctxt-16");
        let prev_corporation_tax_chargeable = num("ct-comp:CorporationTaxChargeable", "ctxt-16");
        let prev_profit_chargeable = num("ct-comp:NetTradingProfits", "ctxt-16");

        let marginal_relief = num(
            "ct-comp:MarginalRateReliefForRingFenceTradesPayable",
            "ctxt-1",
        );
        let corporation_tax_chargeable_payable =
            num("ct-comp:CorporationTaxChargeablePayable", "ctxt-3");
        let total_reliefs_deductions_tax = num(
            "ct-comp:TotalReliefsAndDeductionsInTermsOfTaxPayable",
            "ctxt-1",
        );
        let net_corporation_tax_payable = num("ct-comp:NetCorporationTaxPayable", "ctxt-3");
        let tax_chargeable = num("ct-comp:TaxChargeable", "ctxt-3");
        let tax_payable = num("ct-comp:TaxPayable", "ctxt-3");

        let losses_of_trades_uk = num("ct-comp:TradingLossesOfThisOrLaterAP", "ctxt-3");
        let losses_from_miscellaneous =
            num("ct-comp:LossesFromMiscellaneousTransactions", "ctxt-1");

        let rnd_qualifying_expenditure = num(
            "ct-comp:SubsidisedQualifyingExpenditureOnIn-HouseDirectRD",
            "ctxt-4",
        );
        let rnd_enhanced_expenditure = num(
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
            "ctxt-4",
        );
        let creative_enhanced_expenditure = num(
            "ct-comp:AdjustmentsCreativeProductionCompanyAdjustment",
            "ctxt-5",
        );
        let rnd_creative_enhanced_total = num(
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
            "ctxt-4",
        );
        let rnd_subcontracted_large = num(
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
            "ctxt-8",
        );

        // `ct-comp:CompanyIsAPartnerInAFirm` is written three times with the
        // same context (ctxt-1) — for `partner_in_a_firm`, `is_sme`, and
        // `is_large_company`.  The parser only retains the last value, so we
        // cannot distinguish the three.  We use it for `partner_in_a_firm`
        // and default the SME / large-company flags.
        let partner_in_a_firm = text("ct-comp:CompanyIsAPartnerInAFirm")
            .parse::<bool>()
            .unwrap_or(false);

        // `profit_before_tax` is not serialised to iXBRL, but the invariant
        // `profit_per_accounts_by_fy[fy2] == profit_before_tax` must hold.
        let profit_before_tax = profit_per_accounts_by_fy.get(&fy2).copied().unwrap_or(0.0);

        Frs105CorpTax {
            company,
            accounts,

            // Not represented in the iXBRL output.
            turnover: 0.0,
            total_costs: 0.0,
            gross_profit: 0.0,
            profit_before_tax,
            tax_expense: 0.0,
            profit_after_tax: 0.0,

            annual_investment_allowance,

            adjusted_trading_profit,
            trading_losses_brought_forward,
            net_trading_profits,
            net_chargeable_gains,
            profits_before_deductions,
            profits_before_charges,
            qualifying_donations,
            group_relief,
            group_relief_carried_forward,
            profits_chargeable_to_corporation_tax,

            fy1_profit,
            fy2_profit,
            fy1_tax,
            fy2_tax,
            corporation_tax_chargeable,
            prev_fy1_profit,
            prev_fy2_profit,
            prev_fy1_tax,
            prev_fy2_tax,
            prev_corporation_tax_chargeable,
            prev_profit_chargeable,
            marginal_relief,
            corporation_tax_chargeable_payable,
            total_reliefs_deductions_tax,
            net_corporation_tax_payable,
            tax_chargeable,
            tax_payable,

            losses_of_trades_uk,
            losses_of_trades_overseas: 0.0,
            uk_property_business_losses: 0.0,
            overseas_property_business_losses: 0.0,
            losses_from_miscellaneous,
            capital_losses: 0.0,
            losses_on_intangible_fixed_assets: 0.0,

            rnd_qualifying_expenditure,
            rnd_enhanced_expenditure,
            creative_enhanced_expenditure,
            rnd_creative_enhanced_total,
            rnd_subcontracted_large,
            is_sme: true,
            is_large_company: false,
            rnd_claim_notification: false,
            rnd_additional_information: false,
            partner_in_a_firm,

            rd_projects: Vec::new(),

            profit_per_accounts_by_fy,
            aia_by_fy,
            rnd_by_fy,
            turnover_by_fy: HashMap::new(),
            costs_by_fy: HashMap::new(),
            profit_before_tax_by_fy: HashMap::new(),
            tax_expense_by_fy: HashMap::new(),
            profit_after_tax_by_fy: HashMap::new(),
            wages_by_fy: HashMap::new(),
            pensions_by_fy: HashMap::new(),
            expenses_by_fy: HashMap::new(),
        }
    }

    /// Deserialise a [`Frs105CorpTax`] from the [`XmlNode`] intermediate
    /// representation (step 2 of the round trip: XML string -> `XmlNode` ->
    /// `Frs105CorpTax`).
    ///
    /// See [`Self::from_parsed_facts`] for which fields are recoverable.
    pub fn from_ixbrl_node(node: &XmlNode, company: &Company, accounts: &AccountsMeta) -> Frs105CorpTax {
        let facts = ParsedIxBrlFacts::from_node(node);
        Self::from_parsed_facts(&facts, company, accounts)
    }

    /// Deserialise a [`Frs105CorpTax`] from its serialised iXBRL HTML, in
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
    ) -> Result<Frs105CorpTax, String> {
        let node = XmlNode::from_xml_string(html)?;
        Ok(Self::from_ixbrl_node(&node, company, accounts))
    }

    fn build_fact_text(&self, ref_num: &str, label: &str, value: &str) -> XmlNode {
        fact_wrapper(ref_num, label, span_text(value))
    }

    fn build_fact_numeric(
        &self,
        ref_num: &str,
        label: &str,
        name: &str,
        ctx: &str,
        value: f64,
    ) -> XmlNode {
        fact_wrapper(ref_num, label, non_fraction(name, ctx, &format_f64(value)))
    }

    fn build_fact_non_numeric(
        &self,
        ref_num: &str,
        label: &str,
        name: &str,
        ctx: &str,
        value: &str,
        format: Option<&str>,
    ) -> XmlNode {
        let fact = match format {
            Some(f) => non_numeric_fmt(name, ctx, value, f),
            None => non_numeric(name, ctx, value),
        };
        fact_wrapper(ref_num, label, fact)
    }

    fn page_facts(&self, title: &str, fact_items: Vec<XmlNode>) -> XmlNode {
        page(vec![crate::ixbrl_fmt::facts(
            vec![h2(title)].into_iter().chain(fact_items).collect(),
        )])
    }

    fn build_frs105_corp_tax_page(&self) -> XmlNode {
        self.page_facts(
            "Corporation Tax Return",
            vec![
                self.build_fact_non_numeric(
                    "1",
                    "Company name",
                    "ct-comp:CompanyName",
                    "ctxt-0",
                    &self.company.name,
                    None,
                ),
                self.build_fact_non_numeric(
                    "3",
                    "Tax reference",
                    "ct-comp:TaxReference",
                    "ctxt-0",
                    &self.company.tax_reference,
                    None,
                ),
                self.build_fact_text("-", "Company number", &self.company.company_number),
                self.build_fact_non_numeric(
                    "30",
                    "Return period start",
                    "ct-comp:StartOfPeriodCoveredByReturn",
                    "ctxt-0",
                    &format_date(&self.accounts.period().start),
                    Some("ixt2:datedaymonthyearen"),
                ),
                self.build_fact_non_numeric(
                    "35",
                    "Return period end",
                    "ct-comp:EndOfPeriodCoveredByReturn",
                    "ctxt-0",
                    &format_date(&self.accounts.period().end),
                    Some("ixt2:datedaymonthyearen"),
                ),
                self.build_fact_non_numeric(
                    "-",
                    "Period of account start",
                    "ct-comp:PeriodOfAccountStartDate",
                    "ctxt-0",
                    &format_date(&self.accounts.period().start),
                    Some("ixt2:datedaymonthyearen"),
                ),
                self.build_fact_non_numeric(
                    "-",
                    "Period of account end",
                    "ct-comp:PeriodOfAccountEndDate",
                    "ctxt-0",
                    &format_date(&self.accounts.period().end),
                    Some("ixt2:datedaymonthyearen"),
                ),
                self.build_fact_non_numeric(
                    "-",
                    "Partner in a firm",
                    "ct-comp:CompanyIsAPartnerInAFirm",
                    "ctxt-1",
                    &self.partner_in_a_firm.to_string(),
                    None,
                ),
            ],
        )
    }

    fn build_capital_allowances_page(&self) -> XmlNode {
        self.page_facts(
            "Capital allowances and balancing charges",
            vec![self.build_fact_numeric(
                "690",
                "Annual investment allowance",
                "ct-comp:MainPoolAnnualInvestmentAllowance",
                "ctxt-2",
                self.annual_investment_allowance,
            )],
        )
    }

    fn build_profits_and_gains_page(&self) -> XmlNode {
        let fields: &[(&str, &str, &str, &str, f64)] = &[
            (
                "155",
                "Trading profits",
                "ct-comp:AdjustedTradingProfitOfThisPeriod",
                "ctxt-3",
                self.adjusted_trading_profit,
            ),
            (
                "160",
                "Trading losses brought forward",
                "ct-comp:TradingLossesBroughtForward",
                "ctxt-3",
                self.trading_losses_brought_forward,
            ),
            (
                "165",
                "Net trading profits",
                "ct-comp:NetTradingProfits",
                "ctxt-3",
                self.net_trading_profits,
            ),
            (
                "220",
                "Net chargeable gains",
                "ct-comp:NetChargeableGains",
                "ctxt-1",
                self.net_chargeable_gains,
            ),
            (
                "235",
                "Profits before other deductions and reliefs",
                "ct-comp:ProfitsBeforeOtherDeductionsAndReliefs",
                "ctxt-3",
                self.profits_before_deductions,
            ),
            (
                "300",
                "Profits before donations and group relief",
                "ct-comp:ProfitsBeforeChargesAndGroupRelief",
                "ctxt-3",
                self.profits_before_charges,
            ),
            (
                "305",
                "Qualifying donations",
                "ct-comp:QualifyingDonations",
                "ctxt-1",
                self.qualifying_donations,
            ),
            (
                "310",
                "Group relief claimed",
                "ct-comp:GroupReliefClaimed",
                "ctxt-1",
                self.group_relief,
            ),
            (
                "320",
                "Group relief for carried forward losses",
                "ct-comp:GroupReliefClaimedForCarriedForwardLosses",
                "ctxt-1",
                self.group_relief_carried_forward,
            ),
            (
                "335",
                "Profits chargeable to Corporation Tax",
                "ct-comp:TotalProfitsChargeableToCorporationTax",
                "ctxt-3",
                self.profits_chargeable_to_corporation_tax,
            ),
        ];
        self.page_facts(
            "Profits and gains",
            fields
                .iter()
                .map(|(r, l, t, c, v)| self.build_fact_numeric(r, l, t, c, *v))
                .collect(),
        )
    }

    fn build_losses_page(&self) -> XmlNode {
        self.page_facts(
            "Losses",
            vec![
                self.build_fact_numeric(
                    "-",
                    "Trading losses of this or later AP",
                    "ct-comp:TradingLossesOfThisOrLaterAP",
                    "ctxt-3",
                    self.losses_of_trades_uk,
                ),
                self.build_fact_numeric(
                    "-",
                    "Losses from miscellaneous transactions",
                    "ct-comp:LossesFromMiscellaneousTransactions",
                    "ctxt-1",
                    self.losses_from_miscellaneous,
                ),
            ],
        )
    }

    fn build_tax_chargeable_page(&self) -> XmlNode {
        self.page_facts(
            "Tax chargeable",
            vec![
                self.build_fact_non_numeric(
                    "400",
                    "Financial year 1 covered by the return",
                    "ct-comp:FinancialYear1CoveredByTheReturn",
                    "ctxt-1",
                    &self.accounts.fy1_year.to_string(),
                    None,
                ),
                self.build_fact_non_numeric(
                    "405",
                    "Financial year 2 covered by the return",
                    "ct-comp:FinancialYear2CoveredByTheReturn",
                    "ctxt-1",
                    &self.accounts.fy2_year.to_string(),
                    None,
                ),
                self.build_fact_numeric(
                    "410",
                    "FY1 profit chargeable at first rate",
                    "ct-comp:FY1AmountOfProfitChargeableAtFirstRate",
                    "ctxt-3",
                    self.fy1_profit,
                ),
                self.build_fact_numeric(
                    "415",
                    "FY2 profit chargeable at first rate",
                    "ct-comp:FY2AmountOfProfitChargeableAtFirstRate",
                    "ctxt-3",
                    self.fy2_profit,
                ),
                self.build_fact_numeric(
                    "420",
                    "FY1 first rate of tax",
                    "ct-comp:FY1FirstRateOfTax",
                    "ctxt-1",
                    self.accounts.fy1_rate,
                ),
                self.build_fact_numeric(
                    "425",
                    "FY2 first rate of tax",
                    "ct-comp:FY2FirstRateOfTax",
                    "ctxt-1",
                    self.accounts.fy2_rate,
                ),
                self.build_fact_numeric(
                    "430",
                    "FY1 tax at first rate",
                    "ct-comp:FY1TaxAtFirstRate",
                    "ctxt-3",
                    self.fy1_tax,
                ),
                self.build_fact_numeric(
                    "435",
                    "FY2 tax at first rate",
                    "ct-comp:FY2TaxAtFirstRate",
                    "ctxt-3",
                    self.fy2_tax,
                ),
                self.build_fact_numeric(
                    "440",
                    "Corporation tax chargeable",
                    "ct-comp:CorporationTaxChargeable",
                    "ctxt-3",
                    self.corporation_tax_chargeable,
                ),
                self.build_fact_numeric(
                    "445",
                    "Marginal rate relief",
                    "ct-comp:MarginalRateReliefForRingFenceTradesPayable",
                    "ctxt-1",
                    self.marginal_relief,
                ),
                self.build_fact_numeric(
                    "450",
                    "Corporation tax chargeable payable",
                    "ct-comp:CorporationTaxChargeablePayable",
                    "ctxt-3",
                    self.corporation_tax_chargeable_payable,
                ),
                self.build_fact_numeric(
                    "455",
                    "Total reliefs and deductions",
                    "ct-comp:TotalReliefsAndDeductionsInTermsOfTaxPayable",
                    "ctxt-1",
                    self.total_reliefs_deductions_tax,
                ),
                self.build_fact_numeric(
                    "460",
                    "Net corporation tax payable",
                    "ct-comp:NetCorporationTaxPayable",
                    "ctxt-3",
                    self.net_corporation_tax_payable,
                ),
                self.build_fact_numeric(
                    "465",
                    "Tax chargeable",
                    "ct-comp:TaxChargeable",
                    "ctxt-3",
                    self.tax_chargeable,
                ),
                self.build_fact_numeric(
                    "470",
                    "Tax payable",
                    "ct-comp:TaxPayable",
                    "ctxt-3",
                    self.tax_payable,
                ),
            ],
        )
    }

    fn build_rnd_page(&self) -> XmlNode {
        self.page_facts(
            "R&D / Creative enhanced expenditure",
            vec![
                self.build_fact_non_numeric(
                    "560",
                    "SME company",
                    "ct-comp:CompanyIsAPartnerInAFirm",
                    "ctxt-1",
                    &self.is_sme.to_string(),
                    None,
                ),
                self.build_fact_non_numeric(
                    "565",
                    "Large company",
                    "ct-comp:CompanyIsAPartnerInAFirm",
                    "ctxt-1",
                    &self.is_large_company.to_string(),
                    None,
                ),
                self.build_fact_numeric(
                    "575",
                    "Qualifying expenditure",
                    "ct-comp:SubsidisedQualifyingExpenditureOnIn-HouseDirectRD",
                    "ctxt-4",
                    self.rnd_qualifying_expenditure,
                ),
                self.build_fact_numeric(
                    "580",
                    "Enhanced expenditure",
                    "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
                    "ctxt-4",
                    self.rnd_enhanced_expenditure,
                ),
                self.build_fact_numeric(
                    "585",
                    "Creative enhanced expenditure",
                    "ct-comp:AdjustmentsCreativeProductionCompanyAdjustment",
                    "ctxt-5",
                    self.creative_enhanced_expenditure,
                ),
                self.build_fact_numeric(
                    "590",
                    "R&D and creative total",
                    "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
                    "ctxt-4",
                    self.rnd_creative_enhanced_total,
                ),
                self.build_fact_numeric(
                    "-",
                    "Subcontracted large",
                    "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
                    "ctxt-8",
                    self.rnd_subcontracted_large,
                ),
            ],
        )
    }

    fn build_rnd_worksheet_page(&self) -> XmlNode {
        let fy1 = self.accounts.fy1_year;
        let fy2 = self.accounts.fy2_year;

        let mut rows = vec![worksheet_header_row(fy2, fy1), worksheet_currency_row()];

        for project in &self.rd_projects {
            rows.push(spacer_row());

            // Project heading
            rows.push(tr(
                Some("row"),
                vec![td(
                    "label breakdown heading cell",
                    vec![span_text(&project.name)],
                )],
            ));

            // Item rows
            for item in &project.items {
                let v2 = item.values_by_fy.get(&fy2).copied().unwrap_or(0.0);
                let v1 = item.values_by_fy.get(&fy1).copied().unwrap_or(0.0);
                rows.push(tr(
                    Some("row"),
                    vec![
                        td("label breakdown item cell", vec![span_text(&item.label)]),
                        data_cell(-v2),
                        data_cell(-v1),
                    ],
                ));
            }

            // Subtotal
            let total2: f64 = project
                .items
                .iter()
                .map(|i| i.values_by_fy.get(&fy2).copied().unwrap_or(0.0))
                .sum();
            let total1: f64 = project
                .items
                .iter()
                .map(|i| i.values_by_fy.get(&fy1).copied().unwrap_or(0.0))
                .sum();
            rows.push(table_row_total("Total", -total2, -total1));
            rows.push(spacer_row());

            // Enhanced heading
            rows.push(tr(
                Some("row"),
                vec![td(
                    "label breakdown heading cell",
                    vec![span_text("SME R&D tax relief (130%)")],
                )],
            ));

            // Enhanced project row
            let enh2 = project.enhanced_by_fy.get(&fy2).copied().unwrap_or(0.0);
            let enh1 = project.enhanced_by_fy.get(&fy1).copied().unwrap_or(0.0);
            rows.push(tr(
                Some("row"),
                vec![
                    td("label breakdown item cell", vec![span_text(&project.name)]),
                    data_cell(-enh2),
                    data_cell(-enh1),
                ],
            ));

            // Enhanced total with ix:nonFraction
            rows.push(tr(
                Some("row"),
                vec![
                    td_text("label breakdown total cell", "Total"),
                    data_cell_total_ix(
                        "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
                        "ctxt-4",
                        -enh2,
                    ),
                    data_cell_total_ix(
                        "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
                        "ctxt-8",
                        -enh1,
                    ),
                ],
            ));
        }

        page(vec![worksheet(vec![
            h2("SME R&D"),
            table("sheet table", rows),
        ])])
    }

    fn build_profit_and_loss_worksheet(&self) -> XmlNode {
        let fy1 = self.accounts.fy1_year;
        let fy2 = self.accounts.fy2_year;

        let turnover2 = *self.turnover_by_fy.get(&fy2).unwrap_or(&0.0);
        let turnover1 = *self.turnover_by_fy.get(&fy1).unwrap_or(&0.0);
        let costs2 = *self.costs_by_fy.get(&fy2).unwrap_or(&0.0);
        let costs1 = *self.costs_by_fy.get(&fy1).unwrap_or(&0.0);
        let pbt2 = *self.profit_before_tax_by_fy.get(&fy2).unwrap_or(&0.0);
        let pbt1 = *self.profit_before_tax_by_fy.get(&fy1).unwrap_or(&0.0);
        let tax2 = *self.tax_expense_by_fy.get(&fy2).unwrap_or(&0.0);
        let tax1 = *self.tax_expense_by_fy.get(&fy1).unwrap_or(&0.0);
        let pat2 = *self.profit_after_tax_by_fy.get(&fy2).unwrap_or(&0.0);
        let pat1 = *self.profit_after_tax_by_fy.get(&fy1).unwrap_or(&0.0);
        let wages2 = *self.wages_by_fy.get(&fy2).unwrap_or(&0.0);
        let wages1 = *self.wages_by_fy.get(&fy1).unwrap_or(&0.0);
        let pensions2 = *self.pensions_by_fy.get(&fy2).unwrap_or(&0.0);
        let pensions1 = *self.pensions_by_fy.get(&fy1).unwrap_or(&0.0);

        let gross_profit2 = round_down(turnover2);
        let gross_profit1 = round_down(turnover1);

        let get_exp = |key: &str, fy: i32| -> f64 {
            self.expenses_by_fy
                .get(key)
                .and_then(|m| m.get(&fy))
                .copied()
                .unwrap_or(0.0)
        };

        let expense_defs: &[(&str, &str, &str)] = &[
            (
                "Accountancy services",
                "dpl:AuditAccountancyCosts",
                "accountancy",
            ),
            ("Bank charges", "dpl:BankCharges", "bank-charges"),
            (
                "Office costs",
                "dpl:PrintingPostageStationeryCosts",
                "office",
            ),
            ("Software", "dpl:ITComputingCosts", "software-expenses"),
            (
                "Subscriptions",
                "dpl:SubscriptionsCosts",
                "subscriptions-costs",
            ),
            (
                "Sundries",
                "dpl:OtherOperationalAdministrationCosts",
                "sundries",
            ),
            ("Telecoms", "dpl:TelecommunicationsCosts", "telecoms"),
            ("Travel", "dpl:TravelSubsistenceCosts", "travel"),
        ];

        let mut rows = vec![worksheet_header_row_pl(fy2, fy1), worksheet_currency_row()];
        rows.push(spacer_row());

        // Turnover / revenue
        rows.push(tr(
            Some("row"),
            vec![td(
                "label breakdown heading cell",
                vec![span_text("Turnover / revenue")],
            )],
        ));

        // Income from main trade
        rows.push(tr(
            Some("row"),
            vec![
                td(
                    "label breakdown item cell",
                    vec![non_numeric(
                        "dpl:DescriptionActivity",
                        "ctxt-9",
                        "Income from main trade",
                    )],
                ),
                data_cell_ix(turnover2, "uk-core:TurnoverRevenue", "ctxt-9"),
                data_cell_ix(turnover1, "uk-core:TurnoverRevenue", "ctxt-10"),
            ],
        ));

        // Total (turnover)
        rows.push(table_total_row_ix(
            "Total",
            "uk-core:TurnoverRevenue",
            "ctxt-11",
            "ctxt-12",
            turnover2,
            turnover1,
        ));
        rows.push(spacer_row());

        // Gross profit
        rows.push(tr(
            Some("row"),
            vec![
                td("label heading total cell", vec![span_text("Gross profit")]),
                data_cell_total_n_ix("uk-core:GrossProfitLoss", "ctxt-11", gross_profit2),
                data_cell_total_n_ix("uk-core:GrossProfitLoss", "ctxt-12", gross_profit1),
            ],
        ));
        rows.push(spacer_row());

        // Total costs
        rows.push(tr(
            Some("row"),
            vec![td(
                "label breakdown heading cell",
                vec![span_text("Total costs")],
            )],
        ));

        // Salaries + Pensions
        rows.push(table_row_ix_neg(
            "Salaries",
            "uk-core:WagesSalaries",
            "ctxt-13",
            "ctxt-14",
            wages2,
            wages1,
        ));
        rows.push(table_row_ix_neg(
            "Pension contributions",
            "uk-core:PensionCostsDefinedContributionPlan",
            "ctxt-13",
            "ctxt-14",
            pensions2,
            pensions1,
        ));

        // Other expenses from expenses_by_fy
        for (label, ix_name, expense_key) in expense_defs {
            let v2 = get_exp(expense_key, fy2);
            let v1 = get_exp(expense_key, fy1);
            rows.push(table_row_ix_neg(
                label, ix_name, "ctxt-13", "ctxt-14", v2, v1,
            ));
        }

        // Total costs total
        rows.push(table_total_row_ix_neg(
            "Total",
            "dpl:TotalCosts",
            "ctxt-11",
            "ctxt-12",
            costs2,
            costs1,
        ));
        rows.push(spacer_row());

        // Net profit before tax
        rows.push(tr(
            Some("row"),
            vec![td(
                "label breakdown heading cell",
                vec![span_text("Net profit before tax")],
            )],
        ));

        rows.push(table_row_ix(
            "Gross profit",
            "uk-core:GrossProfitLoss",
            "ctxt-11",
            "ctxt-12",
            gross_profit2,
            gross_profit1,
        ));
        rows.push(table_row_ix_neg(
            "Total costs",
            "dpl:TotalCosts",
            "ctxt-11",
            "ctxt-12",
            costs2,
            costs1,
        ));
        rows.push(table_total_row_ix(
            "Total",
            "uk-core:ProfitLossBeforeTax",
            "ctxt-11",
            "ctxt-12",
            pbt2,
            pbt1,
        ));
        rows.push(spacer_row());

        // Corporation tax
        rows.push(tr(
            Some("row"),
            vec![
                td(
                    "label heading total cell",
                    vec![span_text("Corporation tax")],
                ),
                data_cell_total_neg_ix("uk-core:IncomeTaxExpenseCredit", "ctxt-11", tax2),
                data_cell_total_neg_ix("uk-core:IncomeTaxExpenseCredit", "ctxt-12", tax1),
            ],
        ));
        rows.push(spacer_row());

        // Profit (Loss) after tax
        rows.push(tr(
            Some("row"),
            vec![
                td(
                    "label heading total cell",
                    vec![span_text("Profit (Loss) after tax")],
                ),
                data_cell_total_n_ix("uk-core:ProfitLoss", "ctxt-11", pat2),
                data_cell_total_n_ix("uk-core:ProfitLoss", "ctxt-12", pat1),
            ],
        ));

        page(vec![worksheet(vec![
            h2("Detailed Profit-and-Loss"),
            table("sheet table", rows),
        ])])
    }

    fn build_tax_calculation_worksheet(&self) -> XmlNode {
        let fy1 = self.accounts.fy1_year;
        let fy2 = self.accounts.fy2_year;

        let ppa2 = *self.profit_per_accounts_by_fy.get(&fy2).unwrap_or(&0.0);
        let ppa1 = *self.profit_per_accounts_by_fy.get(&fy1).unwrap_or(&0.0);
        let aia2 = *self.aia_by_fy.get(&fy2).unwrap_or(&0.0);
        let aia1 = *self.aia_by_fy.get(&fy1).unwrap_or(&0.0);
        let rnd2 = *self.rnd_by_fy.get(&fy2).unwrap_or(&0.0);
        let rnd1 = *self.rnd_by_fy.get(&fy1).unwrap_or(&0.0);
        let total2 = ppa2 - aia2 - rnd2;
        let total1 = ppa1 - aia1 - rnd1;

        let mut rows = vec![
            worksheet_header_row(fy2, fy1),
            worksheet_currency_row(),
            spacer_row(),
        ];

        // Taxable profits
        rows.push(tr(
            Some("row"),
            vec![td(
                "label breakdown heading cell",
                vec![span_text("Taxable profits")],
            )],
        ));
        rows.push(table_row_ix(
            "Profit (loss) per accounts",
            "ct-comp:ProfitLossPerAccounts",
            "ctxt-4",
            "ctxt-8",
            ppa2,
            ppa1,
        ));
        rows.push(table_row_ix_neg(
            "Annual investment allowance",
            "ct-comp:MainPoolAnnualInvestmentAllowance",
            "ctxt-2",
            "ctxt-15",
            aia2,
            aia1,
        ));
        rows.push(table_row_ix_neg(
            "SME R&D tax relief (130%)",
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
            "ctxt-4",
            "ctxt-8",
            rnd2,
            rnd1,
        ));
        rows.push(table_row_total("Total", total2, total1));
        rows.push(spacer_row());

        // Trading losses brought forward
        rows.push(table_row_ix(
            "Trading losses brought forward",
            "ct-comp:TradingLossesBroughtForward",
            "ctxt-3",
            "ctxt-16",
            self.trading_losses_brought_forward,
            self.trading_losses_brought_forward,
        ));
        rows.push(spacer_row());

        // Profits chargeable
        rows.push(table_row_ix(
            "Profits chargeable to corporation tax",
            "ct-comp:NetTradingProfits",
            "ctxt-3",
            "ctxt-16",
            self.net_trading_profits,
            self.prev_profit_chargeable,
        ));
        rows.push(spacer_row());

        // Trading losses
        rows.push(table_row_ix(
            "Trading losses",
            "ct-comp:TradingLossesOfThisOrLaterAP",
            "ctxt-3",
            "ctxt-16",
            self.losses_of_trades_uk,
            self.losses_of_trades_uk,
        ));
        rows.push(spacer_row());

        // Profits, by financial year
        rows.push(tr(
            Some("row"),
            vec![td(
                "label breakdown heading cell",
                vec![span_text("Profits, by financial year")],
            )],
        ));
        rows.push(table_row_ix(
            "FY1",
            "ct-comp:FY1AmountOfProfitChargeableAtFirstRate",
            "ctxt-3",
            "ctxt-16",
            self.fy1_profit,
            self.prev_fy1_profit,
        ));
        rows.push(table_row_ix(
            "FY2",
            "ct-comp:FY2AmountOfProfitChargeableAtFirstRate",
            "ctxt-3",
            "ctxt-16",
            self.fy2_profit,
            self.prev_fy2_profit,
        ));
        rows.push(table_row_ix(
            "Total",
            "ct-comp:TotalProfitsChargeableToCorporationTax",
            "ctxt-3",
            "ctxt-16",
            self.profits_chargeable_to_corporation_tax,
            self.prev_profit_chargeable,
        ));
        rows.push(spacer_row());

        // Corporation tax chargeable
        rows.push(tr(
            Some("row"),
            vec![td(
                "label breakdown heading cell",
                vec![span_text("Corporation tax chargeable")],
            )],
        ));
        rows.push(table_row_ix_neg(
            "FY1 (19%)",
            "ct-comp:FY1TaxAtFirstRate",
            "ctxt-3",
            "ctxt-16",
            self.fy1_tax,
            self.prev_fy1_tax,
        ));
        rows.push(table_row_ix_neg(
            "FY2 (19%)",
            "ct-comp:FY2TaxAtFirstRate",
            "ctxt-3",
            "ctxt-16",
            self.fy2_tax,
            self.prev_fy2_tax,
        ));
        rows.push(table_row_ix_neg(
            "Total",
            "ct-comp:CorporationTaxChargeable",
            "ctxt-3",
            "ctxt-16",
            self.corporation_tax_chargeable,
            self.prev_corporation_tax_chargeable,
        ));

        page(vec![worksheet(vec![
            h2("Tax calculation"),
            table("sheet table", rows),
        ])])
    }
}

/*
============================================================================
CT600 form value accessors

The Python reference implementation re-parses the generated iXBRL to extract
these values.  Here we derive them directly from the computed
[`Frs105CorpTax`] fields instead.
============================================================================
*/

impl Frs105CorpTax {
    /// Box 1 — Company name.
    pub fn company_name(&self) -> &str {
        &self.company.name
    }

    /// Box 2 — Company registration number.
    pub fn company_number(&self) -> &str {
        &self.company.company_number
    }

    /// Box 3 — Tax reference (UTR).
    pub fn tax_reference(&self) -> &str {
        &self.company.tax_reference
    }

    /// Box 4 — Type of company (0 = company).
    pub fn type_of_company(&self) -> u8 {
        0
    }

    /// Box 30 — Start of the period covered by the return.
    pub fn start(&self) -> chrono::NaiveDate {
        self.accounts.period().start
    }

    /// Box 35 — End of the period covered by the return.
    pub fn end(&self) -> chrono::NaiveDate {
        self.accounts.period().end
    }

    /// Gross profit / loss per the accounts.
    pub fn gross_profit_loss(&self) -> f64 {
        self.gross_profit
    }

    /// Box 145 — Total turnover from trade.
    pub fn turnover_revenue(&self) -> f64 {
        self.turnover
    }

    /// Box 155 — Adjusted trading profit of this period.
    pub fn adjusted_trading_profit(&self) -> f64 {
        self.adjusted_trading_profit
    }

    /// Box 165 — Net trading profits.
    pub fn net_trading_profits(&self) -> f64 {
        self.net_trading_profits
    }

    /// Box 220 — Net chargeable gains.
    pub fn net_chargeable_gains(&self) -> f64 {
        self.net_chargeable_gains
    }

    /// Box 235 — Profits before other deductions and reliefs.
    pub fn profits_before_other_deductions_and_reliefs(&self) -> f64 {
        self.profits_before_deductions
    }

    /// Box 300 — Profits before qualifying donations and group relief.
    pub fn profits_before_charges_and_group_relief(&self) -> f64 {
        self.profits_before_charges
    }

    /// Box 315 — Profits chargeable to Corporation Tax.
    pub fn total_profits_chargeable_to_corporation_tax(&self) -> f64 {
        self.profits_chargeable_to_corporation_tax
    }

    /// Box 330 — Financial year 1 covered by the return.
    pub fn fy1(&self) -> i32 {
        self.accounts.fy1_year
    }

    /// Box 380 — Financial year 2 covered by the return.
    pub fn fy2(&self) -> i32 {
        self.accounts.fy2_year
    }

    /// Box 335 — FY1 profit chargeable at the first rate.
    pub fn fy1_profit(&self) -> f64 {
        self.fy1_profit
    }

    /// Box 385 — FY2 profit chargeable at the first rate.
    pub fn fy2_profit(&self) -> f64 {
        self.fy2_profit
    }

    /// Box 340 — FY1 first rate of tax.
    pub fn fy1_tax_rate(&self) -> f64 {
        self.accounts.fy1_rate
    }

    /// Box 390 — FY2 first rate of tax.
    pub fn fy2_tax_rate(&self) -> f64 {
        self.accounts.fy2_rate
    }

    /// Box 345 — FY1 tax at first rate.
    pub fn fy1_tax(&self) -> f64 {
        self.fy1_tax
    }

    /// Box 395 — FY2 tax at first rate.
    pub fn fy2_tax(&self) -> f64 {
        self.fy2_tax
    }

    /// Box 430 / 440 / 475 — Corporation Tax chargeable.
    pub fn corporation_tax_chargeable(&self) -> f64 {
        self.corporation_tax_chargeable
    }

    /// Box 510 — Tax chargeable.
    pub fn tax_chargeable(&self) -> f64 {
        self.tax_chargeable
    }

    /// Box 525 / 528 — Tax payable.
    pub fn tax_payable(&self) -> f64 {
        self.tax_payable
    }

    /// Boxes 660 / 670 — SME R&D enhanced expenditure, if any.
    pub fn sme_rnd_expenditure_deduction(&self) -> Option<f64> {
        (self.rnd_enhanced_expenditure > 0.0).then_some(self.rnd_enhanced_expenditure)
    }

    /// Box 690 — Annual investment allowance.
    pub fn investment_allowance(&self) -> f64 {
        self.annual_investment_allowance
    }

    /// Box 40 — Repayments this period.
    pub fn repayment(&self) -> bool {
        false
    }

    /// Box 70 — Compensating adjustment claimed (earlier period relief).
    pub fn claiming_earlier_period_relief(&self) -> bool {
        false
    }

    /// Box 50 — Making more than one return now.
    pub fn making_more_than_one_return(&self) -> bool {
        false
    }

    /// Box 55 — Estimated figures.
    pub fn estimated_figures(&self) -> bool {
        false
    }
}

/// Format a date for iXBRL output with non-breaking spaces.
fn format_date(d: &chrono::NaiveDate) -> String {
    let day = d.format("%d").to_string();
    let month = d.format("%B").to_string();
    let year = d.format("%Y").to_string();
    format!(
        "{}\u{00A0}{}\u{00A0}{}",
        day.trim_start_matches('0'),
        month,
        year
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ct_return_from_example2() {
        let company = crate::test_utils::TestData::default_company();
        let accounts = crate::test_utils::TestData::default_accounts_meta();
        let gnucash = crate::GnucashBook::try_from_gnucash_file(
            crate::test_utils::TestData::accounts_path(&company.company_number)
                .expect("example company accounts path"),
        )
        .await
        .expect("open gnucash");
        let ct = Frs105CorpTax::builder(&gnucash, &company, &accounts)
            .add_rd_project(
                "Project Iguana",
                &[
                    (
                        "Staffing Costs",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs",
                    ),
                    (
                        "Software/Consumables",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:Software/Consumables",
                    ),
                    (
                        "External Workers",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:External Workers",
                    ),
                ],
                "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs",
            )
            .build();

        assert_eq!(ct.company.name, company.name);
        assert_eq!(ct.company.tax_reference, company.tax_reference);
        assert_eq!(ct.company.company_number, company.company_number);
        assert_eq!(ct.accounts.fy1_year, accounts.fy1_year);
        assert_eq!(ct.accounts.fy2_year, accounts.fy2_year);

        assert_eq!(ct.net_trading_profits, 748.0);
        assert_eq!(ct.fy1_profit, 186.0);
        assert_eq!(ct.fy2_profit, 562.0);
        assert_eq!(ct.fy1_tax, 35.34);
        assert_eq!(ct.fy2_tax, 106.78);
        assert_eq!(ct.corporation_tax_chargeable, 142.12);
        assert_eq!(ct.net_corporation_tax_payable, 142.12);
        assert_eq!(ct.tax_payable, 142.12);
        assert_eq!(ct.annual_investment_allowance, 591.0);
        assert_eq!(ct.rnd_enhanced_expenditure, 465.0);
        assert!(ct.is_sme);

        let ixbrl = ct.to_ixbrl();
        assert!(ixbrl.contains("ct-comp:NetTradingProfits"));
        assert!(ixbrl.contains("ct-comp:TaxPayable"));
        assert!(ixbrl.contains(&company.name));
        assert!(ixbrl.contains(&company.tax_reference));

        std::fs::create_dir_all("../../.cache/ixbrl-rs-tests").unwrap();
        std::fs::write(
            "../../.cache/ixbrl-rs-tests/ct_return_example2.html",
            &ixbrl,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_try_from_gnucash_file_sql() {
        let company = crate::test_utils::TestData::default_company();
        let gnucash = crate::GnucashBook::try_from_gnucash_file(
            crate::test_utils::TestData::accounts_path(&company.company_number)
                .expect("example company accounts path"),
        )
        .await
        .expect("open sqlite gnucash");
        println!("{gnucash}");
    }

    #[tokio::test]
    async fn test_try_from_gnucash_file_xml() {
        let gnucash =
            crate::GnucashBook::try_from_gnucash_file("example_data/example1/example.gnucash")
                .await
                .expect("open xml gnucash");
        println!("{gnucash}");
    }

    #[test]
    fn test_format_f64() {
        assert_eq!(format_f64(142.12), "142.12");
        assert_eq!(format_f64(11218.12), "11,218.12");
        assert_eq!(format_f64(0.0), "0.00");
    }

    #[test]
    fn test_format_date() {
        let d = chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        assert_eq!(format_date(&d), "1\u{00A0}January\u{00A0}2020");
    }

    #[tokio::test]
    async fn test_rnd_worksheet_output() {
        let company = crate::test_utils::TestData::default_company();
        let accounts = crate::test_utils::TestData::default_accounts_meta();
        let gnucash = crate::GnucashBook::try_from_gnucash_file(
            crate::test_utils::TestData::accounts_path(&company.company_number)
                .expect("example company accounts path"),
        )
        .await
        .expect("open gnucash");
        let ct = Frs105CorpTax::builder(&gnucash, &company, &accounts)
            .add_rd_project(
                "Project Iguana",
                &[
                    (
                        "Staffing Costs",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs",
                    ),
                    (
                        "Software/Consumables",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:Software/Consumables",
                    ),
                    (
                        "External Workers",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:External Workers",
                    ),
                ],
                "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs",
            )
            .build();

        assert!(!ct.rd_projects.is_empty());
        let p = &ct.rd_projects[0];
        assert_eq!(p.name, "Project Iguana");
        assert_eq!(p.items.len(), 3);
        assert_eq!(p.items[0].label, "Staffing Costs");
        assert_eq!(p.items[0].values_by_fy[&accounts.fy2_year], 465.0);
        assert_eq!(p.items[1].label, "Software/Consumables");
        assert_eq!(p.items[1].values_by_fy[&accounts.fy2_year], 0.0);
        assert_eq!(p.items[2].label, "External Workers");
        assert_eq!(p.items[2].values_by_fy[&accounts.fy2_year], 0.0);

        let ixbrl = ct.to_ixbrl();
        assert!(ixbrl.contains("SME R&amp;D</h2>"));
        assert!(ixbrl.contains("Staffing Costs"));
        assert!(ixbrl.contains("Software/Consumables"));
        assert!(ixbrl.contains("External Workers"));
        assert!(ixbrl.contains("SME R&amp;D tax relief (130%)"));
        assert!(ixbrl.contains("Project Iguana"));
        assert!(
            ixbrl.contains("ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME")
        );
        assert!(ixbrl.contains("sheet table"));
    }

    #[tokio::test]
    async fn test_ixbrl_tag_structure_matches_reference() {
        let company = crate::test_utils::TestData::default_company();
        let accounts = crate::test_utils::TestData::default_accounts_meta();
        let gnucash = crate::GnucashBook::try_from_gnucash_file(
            crate::test_utils::TestData::accounts_path(&company.company_number)
                .expect("example company accounts path"),
        )
        .await
        .expect("open gnucash");
        let ct = Frs105CorpTax::builder(&gnucash, &company, &accounts)
            .add_rd_project(
                "Project Iguana",
                &[
                    (
                        "Staffing Costs",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs",
                    ),
                    (
                        "Software/Consumables",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:Software/Consumables",
                    ),
                    (
                        "External Workers",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:External Workers",
                    ),
                ],
                "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs",
            )
            .build();
        let ixbrl = ct.to_ixbrl();

        // Header structure
        assert!(ixbrl.contains("<ix:header>"));
        assert!(ixbrl.contains("<ix:hidden>"));
        assert!(ixbrl.contains("<ix:references>"));
        assert!(ixbrl.contains("<ix:resources>"));

        // Context structure
        assert!(ixbrl.contains("xbrli:context id=\"ctxt-0\""));
        assert!(ixbrl.contains("<xbrli:instant>"));
        assert!(ixbrl.contains("<xbrli:startDate>"));
        assert!(ixbrl.contains("<xbrli:endDate>"));

        // Report wrapper
        assert!(ixbrl.contains("id=\"report\" class=\"report\""));

        // Page structure - each page has div.page > div.facts > h2
        assert!(ixbrl.contains("<h2>Corporation Tax Return</h2>"));
        assert!(ixbrl.contains("<h2>Capital allowances and balancing charges</h2>"));
        assert!(ixbrl.contains("<h2>Profits and gains</h2>"));
        assert!(ixbrl.contains("<h2>Losses</h2>"));
        assert!(ixbrl.contains("<h2>Tax chargeable</h2>"));
        assert!(ixbrl.contains("<h2>R&amp;D / Creative enhanced expenditure</h2>"));

        // Fact structure
        assert!(ixbrl.contains("class=\"fact\""));
        assert!(ixbrl.contains("class=\"ref\""));
        assert!(ixbrl.contains("class=\"description\""));
        assert!(ixbrl.contains("class=\"factvalue\""));

        // XBRL tags with correct attributes
        assert!(ixbrl.contains("ix:nonNumeric name=\"ct-comp:CompanyName\""));
        assert!(ixbrl.contains("ix:nonFraction name=\"ct-comp:NetTradingProfits\""));
        assert!(ixbrl.contains("unitRef=\"U-GBP\""));
        assert!(ixbrl.contains("format=\"ixt2:numdotdecimal\""));
        assert!(ixbrl.contains("scale=\"0\""));
        assert!(ixbrl.contains("format=\"ixt2:datedaymonthyearen\""));

        // SME R&D worksheet
        assert!(ixbrl.contains("<h2>SME R&amp;D</h2>"));
        assert!(ixbrl.contains("class=\"sheet table\""));
        assert!(ixbrl.contains("class=\"column header cell\""));
        assert!(ixbrl.contains("breakdown heading cell"));
        assert!(ixbrl.contains("breakdown item cell"));
        assert!(ixbrl.contains("breakdown total cell"));
    }

    #[test]
    fn test_corporation_tax_2025_example_from_spec() {
        let calc = calculate_corporation_tax_2025(150_000.0);
        assert_eq!(calc.taxable_profit, 150_000.0);
        assert_eq!(calc.tax_at_main_rate, 37_500.0);
        assert_eq!(calc.marginal_relief, 1_500.0);
        assert_eq!(calc.corporation_tax, 36_000.0);
        assert_eq!(calc.effective_rate, 24.0);
    }

    #[test]
    fn test_corporation_tax_2025_below_small_profits_limit() {
        let calc = calculate_corporation_tax_2025(50_000.0);
        assert_eq!(calc.taxable_profit, 50_000.0);
        assert_eq!(calc.tax_at_main_rate, 12_500.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 9_500.0);
        assert_eq!(calc.effective_rate, 19.0);
    }

    #[test]
    fn test_corporation_tax_2025_just_above_small_profits_limit() {
        let calc = calculate_corporation_tax_2025(50_001.0);
        assert_eq!(calc.taxable_profit, 50_001.0);
        assert_eq!(calc.tax_at_main_rate, 12_500.25);
        assert_eq!(calc.marginal_relief, 2_999.98);
        assert_eq!(calc.corporation_tax, 9_500.27);
    }

    #[test]
    fn test_corporation_tax_2025_at_upper_limit() {
        let calc = calculate_corporation_tax_2025(250_000.0);
        assert_eq!(calc.taxable_profit, 250_000.0);
        assert_eq!(calc.tax_at_main_rate, 62_500.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 62_500.0);
        assert_eq!(calc.effective_rate, 25.0);
    }

    #[test]
    fn test_corporation_tax_2025_just_above_upper_limit() {
        let calc = calculate_corporation_tax_2025(250_001.0);
        assert_eq!(calc.taxable_profit, 250_001.0);
        assert_eq!(calc.tax_at_main_rate, 62_500.25);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 62_500.25);
        assert_eq!(calc.effective_rate, 25.0);
    }

    #[test]
    fn test_corporation_tax_2025_zero_profit() {
        let calc = calculate_corporation_tax_2025(0.0);
        assert_eq!(calc.taxable_profit, 0.0);
        assert_eq!(calc.tax_at_main_rate, 0.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 0.0);
        assert_eq!(calc.effective_rate, 0.0);
    }

    #[test]
    fn test_corporation_tax_2025_small_profit() {
        let calc = calculate_corporation_tax_2025(10_000.0);
        assert_eq!(calc.taxable_profit, 10_000.0);
        assert_eq!(calc.tax_at_main_rate, 2_500.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 1_900.0);
        assert_eq!(calc.effective_rate, 19.0);
    }

    #[test]
    fn test_corporation_tax_2025_mid_marginal_band() {
        let calc = calculate_corporation_tax_2025(100_000.0);
        assert_eq!(calc.taxable_profit, 100_000.0);
        assert_eq!(calc.tax_at_main_rate, 25_000.0);
        assert_eq!(calc.marginal_relief, 2_250.0);
        assert_eq!(calc.corporation_tax, 22_750.0);
        assert_eq!(calc.effective_rate, 22.75);
    }

    #[test]
    fn test_corporation_tax_2025_large_profit() {
        let calc = calculate_corporation_tax_2025(1_000_000.0);
        assert_eq!(calc.taxable_profit, 1_000_000.0);
        assert_eq!(calc.tax_at_main_rate, 250_000.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 250_000.0);
        assert_eq!(calc.effective_rate, 25.0);
    }

    #[test]
    fn test_corporation_tax_2025_midpoint_of_marginal_band() {
        let calc = calculate_corporation_tax_2025(150_000.0);
        assert_eq!(calc.taxable_profit, 150_000.0);
        assert_eq!(calc.tax_at_main_rate, 37_500.0);
        assert_eq!(calc.marginal_relief, 1_500.0);
        assert_eq!(calc.corporation_tax, 36_000.0);
        assert_eq!(calc.effective_rate, 24.0);
    }

    #[test]
    fn test_corporation_tax_2025_near_small_profits_limit() {
        let calc = calculate_corporation_tax_2025(49_999.0);
        assert_eq!(calc.taxable_profit, 49_999.0);
        assert_eq!(calc.tax_at_main_rate, 12_499.75);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 9_499.81);
        assert_eq!(calc.effective_rate, 19.0);
    }

    #[test]
    fn test_corporation_tax_2025_near_upper_limit() {
        let calc = calculate_corporation_tax_2025(249_999.0);
        assert_eq!(calc.taxable_profit, 249_999.0);
        assert_eq!(calc.tax_at_main_rate, 62_499.75);
        assert_eq!(calc.marginal_relief, 0.02);
        assert_eq!(calc.corporation_tax, 62_499.73);
        assert_eq!(calc.effective_rate, 25.0);
    }

    async fn build_example2_ct() -> Frs105CorpTax {
        let company = crate::test_utils::TestData::default_company();
        let accounts = crate::test_utils::TestData::default_accounts_meta();
        let gnucash = crate::GnucashBook::try_from_gnucash_file(
            crate::test_utils::TestData::accounts_path(&company.company_number)
                .expect("example company accounts path"),
        )
        .await
        .expect("open gnucash");
        Frs105CorpTax::builder(&gnucash, &company, &accounts)
            .add_rd_project(
                "Project Iguana",
                &[
                    (
                        "Staffing Costs",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs",
                    ),
                    (
                        "Software/Consumables",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:Software/Consumables",
                    ),
                    (
                        "External Workers",
                        "R&D Enhanced Expenditure:Expenditure:Project Iguana:External Workers",
                    ),
                ],
                "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs",
            )
            .build()
    }

    #[tokio::test]
    async fn test_invariant_gross_profit() {
        let ct = build_example2_ct().await;
        assert_eq!(ct.gross_profit, (ct.turnover).floor());
        assert_eq!(ct.profit_before_tax, ct.gross_profit - ct.total_costs);
    }

    #[tokio::test]
    async fn test_invariant_net_trading_profits() {
        let ct = build_example2_ct().await;
        assert_eq!(
            ct.net_trading_profits,
            ct.adjusted_trading_profit + ct.trading_losses_brought_forward
        );
        assert_eq!(ct.profits_before_deductions, ct.net_trading_profits);
        assert_eq!(ct.profits_before_charges, ct.profits_before_deductions);
    }

    #[tokio::test]
    async fn test_invariant_profit_chargeable() {
        let ct = build_example2_ct().await;
        let expected = ct.profits_before_charges
            - ct.qualifying_donations
            - ct.group_relief
            - ct.group_relief_carried_forward;
        assert_eq!(ct.profits_chargeable_to_corporation_tax, expected);
    }

    #[tokio::test]
    async fn test_invariant_tax_chargeable() {
        let ct = build_example2_ct().await;
        let expected_tax =
            (ct.fy1_tax * 100.0).round() / 100.0 + (ct.fy2_tax * 100.0).round() / 100.0;
        assert_eq!(ct.corporation_tax_chargeable, expected_tax);
        assert_eq!(
            ct.corporation_tax_chargeable_payable,
            ct.corporation_tax_chargeable + ct.marginal_relief
        );
    }

    #[tokio::test]
    async fn test_invariant_net_payable() {
        let ct = build_example2_ct().await;
        assert_eq!(
            ct.net_corporation_tax_payable,
            ct.corporation_tax_chargeable_payable - ct.total_reliefs_deductions_tax
        );
        assert_eq!(ct.tax_chargeable, ct.net_corporation_tax_payable);
        assert_eq!(ct.tax_payable, ct.tax_chargeable);
    }

    #[tokio::test]
    async fn test_invariant_profit_split() {
        let ct = build_example2_ct().await;
        let total = ct.fy1_profit + ct.fy2_profit;
        assert_eq!(total, ct.profits_chargeable_to_corporation_tax);
    }

    #[tokio::test]
    async fn test_invariant_rnd_totals() {
        let ct = build_example2_ct().await;
        assert_eq!(
            ct.rnd_creative_enhanced_total,
            ct.rnd_enhanced_expenditure + ct.creative_enhanced_expenditure
        );
    }

    #[tokio::test]
    async fn test_invariant_prev_tax_chargeable() {
        let ct = build_example2_ct().await;
        let expected_prev_tax =
            (ct.prev_fy1_tax * 100.0).round() / 100.0 + (ct.prev_fy2_tax * 100.0).round() / 100.0;
        assert_eq!(ct.prev_corporation_tax_chargeable, expected_prev_tax);
    }

    #[tokio::test]
    async fn test_invariant_profit_after_tax() {
        let ct = build_example2_ct().await;
        assert_eq!(
            ct.profit_after_tax,
            round2(ct.profit_before_tax - ct.tax_expense)
        );
    }

    #[tokio::test]
    async fn test_invariant_by_fy_consistency() {
        let ct = build_example2_ct().await;
        let fy2 = ct.accounts.fy2_year;
        assert_eq!(*ct.turnover_by_fy.get(&fy2).unwrap_or(&0.0), ct.turnover);
        assert_eq!(*ct.costs_by_fy.get(&fy2).unwrap_or(&0.0), ct.total_costs);
        assert_eq!(
            *ct.profit_per_accounts_by_fy.get(&fy2).unwrap_or(&0.0),
            ct.profit_before_tax
        );
    }

    #[tokio::test]
    async fn test_corp_tax_full_round_trip() {
        // Serialise, write the output to .cache/ixbrl-rs-tests, then
        // deserialise in two steps (XML -> XmlNode -> Frs105CorpTax) and
        // compare against the original for every field that is serialised to
        // iXBRL.
        let ct = build_example2_ct().await;
        let html = ct.to_ixbrl();
        std::fs::create_dir_all("../../.cache/ixbrl-rs-tests").unwrap();
        std::fs::write(
            "../../.cache/ixbrl-rs-tests/ct_roundtrip_example2.html",
            &html,
        )
        .unwrap();

        let node = XmlNode::from_xml_string(&html).expect("parse ixbrl");
        let back = Frs105CorpTax::from_ixbrl_node(&node, &ct.company, &ct.accounts);

        // -- company ---------------------------------------------------------

        assert_eq!(back.company.name, ct.company.name);
        assert_eq!(back.company.tax_reference, ct.company.tax_reference);
        assert_eq!(back.company.company_number, ct.company.company_number);
        assert_eq!(back.accounts.fy1_year, ct.accounts.fy1_year);
        assert_eq!(back.accounts.fy2_year, ct.accounts.fy2_year);
        assert_eq!(back.accounts.fy1_rate, ct.accounts.fy1_rate);
        assert_eq!(back.accounts.fy2_rate, ct.accounts.fy2_rate);
        assert_eq!(
            back.accounts.period().start,
            ct.accounts.period().start
        );
        assert_eq!(
            back.accounts.period().end,
            ct.accounts.period().end
        );

        // -- profits & gains page -------------------------------------------

        assert_eq!(back.annual_investment_allowance, ct.annual_investment_allowance);
        assert_eq!(back.adjusted_trading_profit, ct.adjusted_trading_profit);
        assert_eq!(
            back.trading_losses_brought_forward,
            ct.trading_losses_brought_forward
        );
        assert_eq!(back.net_trading_profits, ct.net_trading_profits);
        assert_eq!(back.net_chargeable_gains, ct.net_chargeable_gains);
        assert_eq!(back.profits_before_deductions, ct.profits_before_deductions);
        assert_eq!(back.profits_before_charges, ct.profits_before_charges);
        assert_eq!(back.qualifying_donations, ct.qualifying_donations);
        assert_eq!(back.group_relief, ct.group_relief);
        assert_eq!(
            back.group_relief_carried_forward,
            ct.group_relief_carried_forward
        );
        assert_eq!(
            back.profits_chargeable_to_corporation_tax,
            ct.profits_chargeable_to_corporation_tax
        );

        // -- tax chargeable page --------------------------------------------

        assert_eq!(back.fy1_profit, ct.fy1_profit);
        assert_eq!(back.fy2_profit, ct.fy2_profit);
        assert_eq!(back.fy1_tax, ct.fy1_tax);
        assert_eq!(back.fy2_tax, ct.fy2_tax);
        assert_eq!(
            back.corporation_tax_chargeable,
            ct.corporation_tax_chargeable
        );
        assert_eq!(back.prev_fy1_profit, ct.prev_fy1_profit);
        assert_eq!(back.prev_fy2_profit, ct.prev_fy2_profit);
        assert_eq!(back.prev_fy1_tax, ct.prev_fy1_tax);
        assert_eq!(back.prev_fy2_tax, ct.prev_fy2_tax);
        assert_eq!(
            back.prev_corporation_tax_chargeable,
            ct.prev_corporation_tax_chargeable
        );
        assert_eq!(back.prev_profit_chargeable, ct.prev_profit_chargeable);
        assert_eq!(back.marginal_relief, ct.marginal_relief);
        assert_eq!(
            back.corporation_tax_chargeable_payable,
            ct.corporation_tax_chargeable_payable
        );
        assert_eq!(
            back.total_reliefs_deductions_tax,
            ct.total_reliefs_deductions_tax
        );
        assert_eq!(back.net_corporation_tax_payable, ct.net_corporation_tax_payable);
        assert_eq!(back.tax_chargeable, ct.tax_chargeable);
        assert_eq!(back.tax_payable, ct.tax_payable);

        // -- losses & R&D ---------------------------------------------------

        assert_eq!(back.losses_of_trades_uk, ct.losses_of_trades_uk);
        assert_eq!(
            back.losses_from_miscellaneous,
            ct.losses_from_miscellaneous
        );
        assert_eq!(
            back.rnd_qualifying_expenditure,
            ct.rnd_qualifying_expenditure
        );
        assert_eq!(back.rnd_enhanced_expenditure, ct.rnd_enhanced_expenditure);
        assert_eq!(
            back.creative_enhanced_expenditure,
            ct.creative_enhanced_expenditure
        );
        assert_eq!(
            back.rnd_creative_enhanced_total,
            ct.rnd_creative_enhanced_total
        );
        assert_eq!(back.rnd_subcontracted_large, ct.rnd_subcontracted_large);
        assert_eq!(back.partner_in_a_firm, ct.partner_in_a_firm);

        // -- per-year maps that are serialised to iXBRL ----------------------

        // The round trip goes through the formatted (2-decimal) facts, so
        // compare each value in its rendered form.
        let map_eq = |a: &HashMap<i32, f64>, b: &HashMap<i32, f64>| -> bool {
            a.len() == b.len()
                && a.iter().all(|(fy, v)| {
                    format_f64(*v) == format_f64(b.get(fy).copied().unwrap_or(0.0))
                })
        };
        assert!(map_eq(&back.profit_per_accounts_by_fy, &ct.profit_per_accounts_by_fy));
        assert!(map_eq(&back.aia_by_fy, &ct.aia_by_fy));
        assert!(map_eq(&back.rnd_by_fy, &ct.rnd_by_fy));
    }

    #[tokio::test]
    async fn test_from_ixbrl_round_trip() {
        // Ensure the cache file exists (test may run in parallel)
        let ct = build_example2_ct().await;
        let html = ct.to_ixbrl();
        std::fs::create_dir_all("../../.cache/ixbrl-rs-tests").unwrap();
        std::fs::write("../../.cache/ixbrl-rs-tests/ct_return_example2.html", &html).unwrap();
        let facts = ParsedIxBrlFacts::from_html(&html);

        assert_eq!(
            facts.non_numeric.get("ct-comp:CompanyName").unwrap(),
            &ct.company.name
        );
        assert_eq!(
            facts.non_numeric.get("ct-comp:TaxReference").unwrap(),
            &ct.company.tax_reference
        );

        assert_eq!(
            facts
                .numeric_by_ctx
                .get(&("ct-comp:NetTradingProfits".into(), "ctxt-3".into())),
            Some(&748.0)
        );
        assert_eq!(
            facts
                .numeric_by_ctx
                .get(&("ct-comp:CorporationTaxChargeable".into(), "ctxt-3".into())),
            Some(&142.12)
        );
        assert_eq!(
            facts
                .numeric_by_ctx
                .get(&("ct-comp:NetCorporationTaxPayable".into(), "ctxt-3".into())),
            Some(&142.12)
        );
        assert_eq!(
            facts
                .numeric_by_ctx
                .get(&("ct-comp:TaxPayable".into(), "ctxt-3".into())),
            Some(&142.12)
        );
        assert_eq!(
            facts.numeric_by_ctx.get(&(
                "ct-comp:MainPoolAnnualInvestmentAllowance".into(),
                "ctxt-2".into()
            )),
            Some(&591.0)
        );
        assert_eq!(
            facts.numeric_by_ctx.get(&(
                "ct-comp:AdjustedTradingProfitOfThisPeriod".into(),
                "ctxt-3".into()
            )),
            Some(&748.0)
        );
    }

    #[tokio::test]
    async fn test_from_ixbrl_worksheet_fy_split() {
        let ct = build_example2_ct().await;
        let html = ct.to_ixbrl();
        std::fs::create_dir_all("../../.cache/ixbrl-rs-tests").unwrap();
        std::fs::write("../../.cache/ixbrl-rs-tests/ct_return_example2.html", &html).unwrap();
        let facts = ParsedIxBrlFacts::from_html(&html);

        let fy1_cur = facts.numeric_by_ctx.get(&(
            "ct-comp:FY1AmountOfProfitChargeableAtFirstRate".into(),
            "ctxt-3".into(),
        ));
        let fy1_prev = facts.numeric_by_ctx.get(&(
            "ct-comp:FY1AmountOfProfitChargeableAtFirstRate".into(),
            "ctxt-16".into(),
        ));
        assert_eq!(fy1_cur, Some(&186.0));
        assert!(fy1_prev.is_some());
    }

    #[tokio::test]
    async fn test_from_parsed_facts() {
        let ct = build_example2_ct().await;
        let html = ct.to_ixbrl();
        std::fs::create_dir_all("../../.cache/ixbrl-rs-tests").unwrap();
        std::fs::write("../../.cache/ixbrl-rs-tests/ct_return_example2.html", &html).unwrap();
        let facts = ParsedIxBrlFacts::from_html(&html);

        let company = crate::test_utils::TestData::default_company();
        let accounts = crate::test_utils::TestData::default_accounts_meta();

        let ct = Frs105CorpTax::from_parsed_facts(&facts, &company, &accounts);

        // -- company fields ------------------------------------------------

        assert_eq!(ct.company.name, company.name);
        assert_eq!(ct.company.tax_reference, company.tax_reference);
        assert_eq!(ct.company.company_number, company.company_number);
        assert_eq!(ct.accounts.fy1_year, accounts.fy1_year);
        assert_eq!(ct.accounts.fy2_year, accounts.fy2_year);
        assert_eq!(ct.accounts.fy1_rate, accounts.fy1_rate);
        assert_eq!(ct.accounts.fy2_rate, accounts.fy2_rate);
        assert_eq!(
            ct.accounts.period().start,
            accounts.period().start
        );
        assert_eq!(
            ct.accounts.period().end,
            accounts.period().end
        );

        // -- profits & gains page -----------------------------------------

        assert_eq!(ct.adjusted_trading_profit, 748.0);
        assert_eq!(ct.trading_losses_brought_forward, 0.0);
        assert_eq!(ct.net_trading_profits, 748.0);
        assert_eq!(ct.net_chargeable_gains, 0.0);
        assert_eq!(ct.profits_before_deductions, 748.0);
        assert_eq!(ct.profits_before_charges, 748.0);
        assert_eq!(ct.profits_chargeable_to_corporation_tax, 748.0);

        // -- tax chargeable page ----------------------------------------

        assert_eq!(ct.fy1_profit, 186.0);
        assert_eq!(ct.fy2_profit, 562.0);
        assert_eq!(ct.fy1_tax, 35.34);
        assert_eq!(ct.fy2_tax, 106.78);
        assert_eq!(ct.corporation_tax_chargeable, 142.12);
        assert_eq!(ct.marginal_relief, 0.0);
        assert_eq!(ct.corporation_tax_chargeable_payable, 142.12);
        assert_eq!(ct.total_reliefs_deductions_tax, 0.0);
        assert_eq!(ct.net_corporation_tax_payable, 142.12);
        assert_eq!(ct.tax_chargeable, 142.12);
        assert_eq!(ct.tax_payable, 142.12);

        // -- previous year -------------------------------------------------

        assert_eq!(ct.prev_profit_chargeable, 4010.0);
        assert_eq!(ct.prev_fy1_profit, 986.0);
        assert_eq!(ct.prev_fy2_profit, 3024.0);
        assert_eq!(ct.prev_fy1_tax, 187.34);
        assert_eq!(ct.prev_fy2_tax, 574.56);
        assert_eq!(ct.prev_corporation_tax_chargeable, 761.9);
        assert!(ct.prev_fy1_profit > 0.0);
        assert!(ct.prev_fy2_profit > 0.0);
        assert!(ct.prev_corporation_tax_chargeable > 0.0);

        // -- capital allowances -------------------------------------------

        assert_eq!(ct.annual_investment_allowance, 591.0);

        // -- R&D page -----------------------------------------------------

        assert_eq!(ct.rnd_enhanced_expenditure, 465.0);
        assert_eq!(ct.rnd_creative_enhanced_total, 465.0);
        assert!(ct.is_sme);
        assert!(!ct.is_large_company);
        assert!(!ct.partner_in_a_firm);

        // -- per-year hash maps ------------------------------------------

        assert_eq!(
            *ct.profit_per_accounts_by_fy
                .get(&accounts.fy2_year)
                .unwrap_or(&0.0),
            1804.94
        );
        assert_eq!(
            *ct.profit_per_accounts_by_fy
                .get(&accounts.fy1_year)
                .unwrap_or(&0.0),
            4837.88
        );
        assert_eq!(
            *ct.aia_by_fy.get(&accounts.fy2_year).unwrap_or(&0.0),
            591.0
        );
        assert_eq!(
            *ct.rnd_by_fy.get(&accounts.fy2_year).unwrap_or(&0.0),
            465.0
        );

        // -- invariants ---------------------------------------------------

        // profit_before_tax must equal profit_per_accounts_by_fy[fy2]
        assert_eq!(
            *ct.profit_per_accounts_by_fy
                .get(&ct.accounts.fy2_year)
                .unwrap_or(&0.0),
            ct.profit_before_tax
        );

        // corporation_tax_chargeable must equal fy1_tax + fy2_tax
        assert_eq!(ct.fy1_tax + ct.fy2_tax, ct.corporation_tax_chargeable);
    }
}
