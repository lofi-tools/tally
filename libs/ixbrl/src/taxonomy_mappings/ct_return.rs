use std::collections::HashMap;

use crate::company::Company;
use crate::ixbrl_writer::IxbrlWriter;
use crate::GnucashBook;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedIxBrlFacts {
    pub numeric: HashMap<String, f64>,
    pub non_numeric: HashMap<String, String>,
    pub numeric_by_ctx: HashMap<(String, String), f64>,
    pub non_numeric_by_ctx: HashMap<(String, String), String>,
}

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
pub struct CorporationTaxReturn {
    pub company: Company,

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

#[derive(Debug, Clone)]
pub struct CorporationTaxReturnBuilder<'a> {
    company: &'a Company,
    period_splits: Vec<(f64, String)>,
    prev_splits: Vec<(f64, String)>,
    rd_project_defs: Vec<(&'a str, Vec<(&'a str, &'a str)>, &'a str)>,
}

impl<'a> CorporationTaxReturnBuilder<'a> {
    pub fn add_rd_project(
        mut self,
        name: &'a str,
        items: &[(&'a str, &'a str)],
        enhanced_path: &'a str,
    ) -> Self {
        self.rd_project_defs.push((name, items.to_vec(), enhanced_path));
        self
    }

    pub fn build(self) -> CorporationTaxReturn {
        CorporationTaxReturn::from_splits(
            self.company,
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

impl CorporationTaxReturn {
    pub fn builder<'a>(gnucash: &GnucashBook, company: &'a Company) -> CorporationTaxReturnBuilder<'a> {
        let accounts = gnucash.raw_accounts();
        let txns = gnucash.raw_transactions();
        let splits = gnucash.raw_splits();

        let mut period_splits: Vec<(f64, String)> = Vec::new();
        let mut prev_splits: Vec<(f64, String)> = Vec::new();

        let prev_start = company.prev_period_start();
        let prev_end = company.prev_period_end();

        for split in splits {
            let tx = match txns.iter().find(|t| t.guid == split.tx_guid) {
                Some(t) => t,
                None => continue,
            };
            let tx_date = tx.post_datetime.date();
            let acc = match accounts.iter().find(|a| a.guid == split.account_guid) {
                Some(a) => a,
                None => continue,
            };
            if acc.r#type == "ROOT" || acc.r#type == "TEMPLATE" {
                continue;
            }
            let path = Self::account_path(accounts, acc);
            let val = split.value.to_string().parse::<f64>().unwrap_or(0.0);

            if tx_date >= company.accounting_period_start && tx_date <= company.accounting_period_end {
                period_splits.push((val, path.clone()));
            }
            if tx_date >= prev_start && tx_date <= prev_end {
                prev_splits.push((val, path.clone()));
            }
        }

        CorporationTaxReturnBuilder {
            company,
            period_splits,
            prev_splits,
            rd_project_defs: Vec::new(),
        }
    }

    fn from_splits(
        company: &Company,
        period_splits: &[(f64, String)],
        prev_splits: &[(f64, String)],
        rd_project_defs: &[(&str, Vec<(&str, &str)>, &str)],
    ) -> Self {
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

        let rd_projects: Vec<RdProject> = rd_project_defs.iter().map(|(name, items, enhanced_path)| {
            let items: Vec<RdExpenditureItem> = items.iter().map(|(label, path)| {
                RdExpenditureItem {
                    label: label.to_string(),
                    account_path: path.to_string(),
                    values_by_fy: HashMap::from([
                        (company.fy1_year, round_down(sum_abs(prev_splits, path))),
                        (company.fy2_year, round_down(sum_abs(period_splits, path))),
                    ]),
                }
            }).collect();
            let enhanced: HashMap<i32, f64> = HashMap::from([
                (company.fy1_year, round_down(sum_abs(prev_splits, enhanced_path))),
                (company.fy2_year, round_down(sum_abs(period_splits, enhanced_path))),
            ]);
            RdProject { name: name.to_string(), items, enhanced_by_fy: enhanced }
        }).collect();

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

        let fy1_end = chrono::NaiveDate::from_ymd_opt(company.fy2_year, 3, 31).unwrap();
        let total_days = (company.accounting_period_end - company.accounting_period_start).num_days() + 1;
        let fy1_days = {
            let s = company.accounting_period_start;
            let e = fy1_end.min(company.accounting_period_end);
            if e >= s { (e - s).num_days() + 1 } else { 0 }
        };
        let fy2_days = total_days - fy1_days;

        let fy1_profit = (profits_chargeable * fy1_days as f64 / total_days as f64).round();
        let fy2_profit = (profits_chargeable * fy2_days as f64 / total_days as f64).round();
        let fy1_tax = round2(fy1_profit * company.fy1_rate / 100.0);
        let fy2_tax = round2(fy2_profit * company.fy2_rate / 100.0);
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
        let profit_after_tax = (profit_before_tax_current - tax_expense_current).round();

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
        let prev_period_start = company.prev_period_start();
        let prev_period_end = company.prev_period_end();
        let prev_total_days = (prev_period_end - prev_period_start).num_days() + 1;
        let prev_fy1_end = chrono::NaiveDate::from_ymd_opt(company.fy2_year - 1, 3, 31).unwrap();
        let prev_fy1_days = {
            let s = prev_period_start;
            let e = prev_fy1_end.min(prev_period_end);
            if e >= s { (e - s).num_days() + 1 } else { 0 }
        };
        let prev_fy2_days = prev_total_days - prev_fy1_days;
        let prev_fy1_profit = (prev_profit_chargeable * prev_fy1_days as f64 / prev_total_days as f64).round();
        let prev_fy2_profit = (prev_profit_chargeable * prev_fy2_days as f64 / prev_total_days as f64).round();
        let prev_fy1_tax = round2(prev_fy1_profit * company.fy1_rate / 100.0);
        let prev_fy2_tax = round2(prev_fy2_profit * company.fy2_rate / 100.0);
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
            expenses_by_fy.insert(id.to_string(), HashMap::from([(company.fy2_year, cur), (company.fy1_year, prev)]));
        }

        CorporationTaxReturn {
            company: company.clone(),

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
                (company.fy1_year, profit_before_tax_prev),
                (company.fy2_year, profit_before_tax_current),
            ]),
            aia_by_fy: HashMap::from([(company.fy1_year, aia_prev), (company.fy2_year, aia_current)]),
            rnd_by_fy: HashMap::from([(company.fy1_year, rnd_enhanced_prev), (company.fy2_year, rnd_enhanced_current)]),
            turnover_by_fy: HashMap::from([(company.fy1_year, ct_turnover_prev), (company.fy2_year, ct_turnover_current)]),
            costs_by_fy: HashMap::from([(company.fy1_year, total_costs_prev), (company.fy2_year, total_costs_current)]),
            profit_before_tax_by_fy: HashMap::from([
                (company.fy1_year, profit_before_tax_prev),
                (company.fy2_year, profit_before_tax_current),
            ]),
            tax_expense_by_fy: HashMap::from([
                (company.fy1_year, tax_expense_prev),
                (company.fy2_year, tax_expense_current),
            ]),
            profit_after_tax_by_fy: HashMap::from([
                (company.fy1_year, (profit_before_tax_prev - tax_expense_prev).round()),
                (company.fy2_year, profit_after_tax),
            ]),
            wages_by_fy: HashMap::from([(company.fy1_year, salaries_prev), (company.fy2_year, salaries_current)]),
            pensions_by_fy: HashMap::from([(company.fy1_year, pensions_prev), (company.fy2_year, pensions_current)]),
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
        let mut w = IxbrlWriter::new();

        w.write_raw("<?xml version='1.0' encoding='ASCII'?>\n");
        w.write_raw("<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:ix=\"http://www.xbrl.org/2013/inlineXBRL\" xmlns:link=\"http://www.xbrl.org/2003/linkbase\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" xmlns:xbrli=\"http://www.xbrl.org/2003/instance\" xmlns:xbrldi=\"http://xbrl.org/2006/xbrldi\" xmlns:ixt2=\"http://www.xbrl.org/inlineXBRL/transformation/2011-07-31\" xmlns:iso4217=\"http://www.xbrl.org/2003/iso4217\" xmlns:ct-comp=\"http://www.hmrc.gov.uk/schemas/ct/comp/2023-01-01\" xmlns:dpl=\"http://xbrl.frc.org.uk/dpl/2023-01-01\" xmlns:uk-bus=\"http://xbrl.frc.org.uk/cd/2023-01-01/business\" xmlns:uk-core=\"http://xbrl.frc.org.uk/fr/2023-01-01/core\" xmlns:uk-geo=\"http://xbrl.frc.org.uk/cd/2023-01-01/countries\">");

        w.write_raw("<head><title>Corporation Tax Statement</title><style type=\"text/css\">\n");
        w.write_raw(include_str!("ct_return_style.css"));
        w.write_raw("</style></head><body>");

        w.write_raw("<div style=\"display:none\"><ix:header>");
        w.write_raw("<ix:hidden>");
        self.ix_non_numeric(
            &mut w,
            "ct-comp:NameOfProductionSoftware",
            "ctxt-0",
            "ixbrl-reporter",
            None,
        );
        self.ix_non_numeric(
            &mut w,
            "ct-comp:VersionOfProductionSoftware",
            "ctxt-0",
            "1.2.1",
            None,
        );
        self.ix_non_numeric(
            &mut w,
            "ct-comp:CompanyName",
            "ctxt-0",
            &self.company.name,
            None,
        );
        self.ix_non_numeric(
            &mut w,
            "ct-comp:TaxReference",
            "ctxt-0",
            &self.company.tax_reference,
            None,
        );
        w.write_raw("</ix:hidden>");

        w.write_raw("<ix:references>");
        w.write_raw("<link:schemaRef xlink:type=\"simple\" xlink:href=\"http://www.hmrc.gov.uk/schemas/ct/comp/2023-01-01/ct-comp-2023.xsd\"></link:schemaRef>");
        w.write_raw("<link:schemaRef xlink:type=\"simple\" xlink:href=\"https://xbrl.frc.org.uk/dpl/2023-01-01/dpl-2023-01-01.xsd\"></link:schemaRef>");
        w.write_raw("</ix:references>");

        w.write_raw("<ix:resources>");
        self.write_context_instant(
            &mut w,
            "ctxt-0",
            &self.company.company_number,
            &self.company.accounting_period_end,
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Company"),
        );
        self.write_context_duration(
            &mut w,
            "ctxt-1",
            &self.company.company_number,
            &self.company.accounting_period_start,
            &self.company.accounting_period_end,
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Company"),
        );
        self.write_context_duration(
            &mut w,
            "ctxt-2",
            &self.company.company_number,
            &self.company.accounting_period_start,
            &self.company.accounting_period_end,
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:ManagementExpenses"),
        );
        self.write_context_duration(
            &mut w,
            "ctxt-3",
            &self.company.company_number,
            &self.company.accounting_period_start,
            &self.company.accounting_period_end,
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Company"),
        );
        self.write_context_duration_full(
            &mut w,
            "ctxt-4",
            &self.company.company_number,
            &self.company.accounting_period_start,
            &self.company.accounting_period_end,
            Some("ct-comp:BusinessNameDimension"),
            Some(&self.company.name),
            &[
                ("ct-comp:BusinessTypeDimension", "ct-comp:Trade"),
                ("ct-comp:LossReformDimension", "ct-comp:Post-lossReform"),
                ("ct-comp:TerritoryDimension", "ct-comp:UK"),
            ],
        );
        self.write_context_duration_full(
            &mut w,
            "ctxt-5",
            &self.company.company_number,
            &self.company.accounting_period_start,
            &self.company.accounting_period_end,
            Some("ct-comp:BusinessNameDimension"),
            Some(&self.company.name),
            &[
                ("ct-comp:BusinessTypeDimension", "ct-comp:Trade"),
                ("ct-comp:LossReformDimension", "ct-comp:Post-lossReform"),
                ("ct-comp:TerritoryDimension", "ct-comp:UK"),
            ],
        );
        self.write_context_duration_full(
            &mut w,
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
        );
        self.write_context_duration_full(
            &mut w,
            "ctxt-9",
            &self.company.company_number,
            &self.company.accounting_period_start,
            &self.company.accounting_period_end,
            None,
            None,
            &[
                ("dpl:DetailedAnalysisDimension", "dpl:Item1"),
                ("uk-geo:CountriesRegionsDimension", "uk-geo:UnitedKingdom"),
            ],
        );
        self.write_context_duration_full(
            &mut w,
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
        );
        self.write_context_duration_full(
            &mut w,
            "ctxt-11",
            &self.company.company_number,
            &self.company.accounting_period_start,
            &self.company.accounting_period_end,
            None,
            None,
            &[],
        );
        self.write_context_duration_full(
            &mut w,
            "ctxt-12",
            &self.company.company_number,
            &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
            &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            None,
            None,
            &[],
        );
        self.write_context_duration(
            &mut w,
            "ctxt-13",
            &self.company.company_number,
            &self.company.accounting_period_start,
            &self.company.accounting_period_end,
            Some("dpl:ExpenseTypeDimension"),
            Some("dpl:AdministrativeExpenses"),
        );
        self.write_context_duration(
            &mut w,
            "ctxt-14",
            &self.company.company_number,
            &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
            &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            Some("dpl:ExpenseTypeDimension"),
            Some("dpl:AdministrativeExpenses"),
        );
        self.write_context_duration(
            &mut w,
            "ctxt-15",
            &self.company.company_number,
            &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
            &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:ManagementExpenses"),
        );
        self.write_context_duration(
            &mut w,
            "ctxt-16",
            &self.company.company_number,
            &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
            &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Company"),
        );

        self.write_unit(&mut w, "iso4217:GBP");
        w.write_raw("</ix:resources></ix:header></div>");

        w.write_raw("<div id=\"report\" class=\"report\">");
        self.write_corporation_tax_return_page(&mut w);
        self.write_capital_allowances_page(&mut w);
        self.write_profits_and_gains_page(&mut w);
        self.write_losses_page(&mut w);
        self.write_tax_chargeable_page(&mut w);
        self.write_rnd_page(&mut w);
        if !self.rd_projects.is_empty() {
            self.write_rnd_worksheet_page(&mut w);
        }
        self.write_tax_calculation_worksheet(&mut w);
        w.write_raw("</div>");

        w.write_raw("</body></html>");
        w.into_string()
    }

    pub fn from_ixbrl(html: &str) -> ParsedIxBrlFacts {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut facts = ParsedIxBrlFacts::default();
        let mut reader = Reader::from_str(html);
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let attrs: HashMap<String, String> = e
                        .attributes()
                        .filter_map(|a| a.ok())
                        .map(|a| {
                            (
                                String::from_utf8_lossy(a.key.as_ref()).to_string(),
                                String::from_utf8_lossy(&a.value).to_string(),
                            )
                        })
                        .collect();

                    let name = attrs.get("name").cloned();
                    let ctx = attrs.get("contextRef").cloned();

                    if let (Some(name), Some(ctx)) = (name, ctx) {
                        if tag == "ix:nonFraction" || tag == "ix:nonNumeric" {
                            let mut text_buf = Vec::new();
                            if let Ok(Event::Text(text)) = reader.read_event_into(&mut text_buf) {
                                let raw = text.unescape().unwrap_or_default().to_string();
                                let val = raw.trim();
                                if tag == "ix:nonFraction" {
                                    if let Ok(v) = val.parse::<f64>() {
                                        facts.numeric.insert(name.clone(), v);
                                        facts.numeric_by_ctx.insert((name, ctx), v);
                                    }
                                } else {
                                    facts.non_numeric.insert(name.clone(), val.to_string());
                                    facts.non_numeric_by_ctx
                                        .insert((name, ctx), val.to_string());
                                }
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        facts
    }

    fn write_context_instant(
        &self,
        w: &mut IxbrlWriter,
        id: &str,
        scheme_id: &str,
        date: &chrono::NaiveDate,
        dim: Option<&str>,
        val: Option<&str>,
    ) {
        w.open_element("xbrli:context", &[("id", id)]);
        w.open_element("xbrli:entity", &[]);
        w.write_element(
            "xbrli:identifier",
            &[("scheme", "http://www.companieshouse.gov.uk/")],
            scheme_id,
        );
        if let Some(d) = dim {
            w.open_element("xbrli:segment", &[]);
            w.write_element(
                "xbrldi:explicitMember",
                &[("dimension", d)],
                val.unwrap_or(""),
            );
            w.close_element("xbrli:segment");
        }
        w.close_element("xbrli:entity");
        w.open_element("xbrli:period", &[]);
        w.write_element("xbrli:instant", &[], &date.to_string());
        w.close_element("xbrli:period");
        w.close_element("xbrli:context");
    }

    fn write_context_duration(
        &self,
        w: &mut IxbrlWriter,
        id: &str,
        scheme_id: &str,
        start: &chrono::NaiveDate,
        end: &chrono::NaiveDate,
        dim: Option<&str>,
        val: Option<&str>,
    ) {
        let dims = match (dim, val) {
            (Some(d), Some(v)) => vec![(d, v)],
            _ => vec![],
        };
        self.write_context_duration_full(w, id, scheme_id, start, end, None, None, &dims);
    }

    fn write_context_duration_full(
        &self,
        w: &mut IxbrlWriter,
        id: &str,
        scheme_id: &str,
        start: &chrono::NaiveDate,
        end: &chrono::NaiveDate,
        typed_dim: Option<&str>,
        typed_val: Option<&str>,
        explicit_dims: &[(&str, &str)],
    ) {
        w.open_element("xbrli:context", &[("id", id)]);
        w.open_element("xbrli:entity", &[]);
        w.write_element(
            "xbrli:identifier",
            &[("scheme", "http://www.companieshouse.gov.uk/")],
            scheme_id,
        );
        if typed_dim.is_some() || !explicit_dims.is_empty() {
            w.open_element("xbrli:segment", &[]);
            if let Some(d) = typed_dim {
                w.open_element("xbrldi:typedMember", &[("dimension", d)]);
                w.write_element(
                    "ct-comp:BusinessNameDomain",
                    &[],
                    typed_val.unwrap_or(""),
                );
                w.close_element("xbrldi:typedMember");
            }
            for (dim, val) in explicit_dims {
                w.write_element("xbrldi:explicitMember", &[("dimension", dim)], val);
            }
            w.close_element("xbrli:segment");
        }
        w.close_element("xbrli:entity");
        w.open_element("xbrli:period", &[]);
        w.write_element("xbrli:startDate", &[], &start.to_string());
        w.write_element("xbrli:endDate", &[], &end.to_string());
        w.close_element("xbrli:period");
        w.close_element("xbrli:context");
    }

    fn write_unit(&self, w: &mut IxbrlWriter, measure: &str) {
        w.open_element("xbrli:unit", &[("id", "U-GBP")]);
        w.write_element("xbrli:measure", &[], measure);
        w.close_element("xbrli:unit");
    }

    fn ix_non_numeric(
        &self,
        w: &mut IxbrlWriter,
        name: &str,
        ctx: &str,
        value: &str,
        format: Option<&str>,
    ) {
        let mut attrs = vec![("name", name), ("contextRef", ctx)];
        let fmt_str;
        if let Some(f) = format {
            fmt_str = f.to_string();
            attrs.push(("format", &fmt_str));
        }
        w.open_element("ix:nonNumeric", &attrs);
        w.write_raw(value);
        w.close_element("ix:nonNumeric");
    }

    fn write_fact(&self, w: &mut IxbrlWriter, ref_num: &str, label: &str, value: &str) {
        w.open_element("div", &[("class", "fact")]);
        w.write_element("div", &[("class", "ref")], ref_num);
        let desc = format!("{}:", label);
        w.write_element("div", &[("class", "description")], &desc);
        w.open_element("div", &[("class", "factvalue")]);
        w.write_raw(value);
        w.close_element("div");
        w.close_element("div");
    }

    fn write_fact_numeric(
        &self,
        w: &mut IxbrlWriter,
        ref_num: &str,
        label: &str,
        name: &str,
        ctx: &str,
        unit: &str,
        value: f64,
    ) {
        let formatted = format!("{:.2}", value);
        w.open_element("div", &[("class", "fact")]);
        w.write_element("div", &[("class", "ref")], ref_num);
        let desc = format!("{}:", label);
        w.write_element("div", &[("class", "description")], &desc);
        w.open_element("div", &[("class", "factvalue")]);
        w.open_element(
            "ix:nonFraction",
            &[
                ("name", name),
                ("contextRef", ctx),
                ("unitRef", unit),
                ("format", "ixt2:numdotdecimal"),
                ("decimals", "2"),
                ("scale", "0"),
            ],
        );
        w.write_raw(&formatted);
        w.close_element("ix:nonFraction");
        w.close_element("div");
        w.close_element("div");
    }

    fn write_fact_non_numeric(
        &self,
        w: &mut IxbrlWriter,
        ref_num: &str,
        label: &str,
        name: &str,
        ctx: &str,
        value: &str,
        format: Option<&str>,
    ) {
        w.open_element("div", &[("class", "fact")]);
        w.write_element("div", &[("class", "ref")], ref_num);
        let desc = format!("{}:", label);
        w.write_element("div", &[("class", "description")], &desc);
        w.open_element("div", &[("class", "factvalue")]);
        let mut attrs = vec![("name", name), ("contextRef", ctx)];
        let fmt_str;
        if let Some(f) = format {
            fmt_str = f.to_string();
            attrs.push(("format", &fmt_str));
        }
        w.open_element("ix:nonNumeric", &attrs);
        w.write_raw(value);
        w.close_element("ix:nonNumeric");
        w.close_element("div");
        w.close_element("div");
    }

    fn open_page(&self, w: &mut IxbrlWriter, id: &str, facts_id: &str, title: &str) {
        w.open_element("div", &[("class", "page"), ("id", id)]);
        w.open_element("div", &[("class", "facts"), ("id", facts_id)]);
        w.write_element("h2", &[], title);
    }

    fn close_page(&self, w: &mut IxbrlWriter) {
        w.close_element("div");
        w.close_element("div");
    }

    fn write_corporation_tax_return_page(&self, w: &mut IxbrlWriter) {
        self.open_page(w, "elt-001", "elt-002", "Corporation Tax Return");

        self.write_fact_non_numeric(
            w, "1", "Company name", "ct-comp:CompanyName", "ctxt-0",
            &self.company.name, None,
        );
        self.write_fact_non_numeric(
            w, "3", "Tax reference", "ct-comp:TaxReference", "ctxt-0",
            &self.company.tax_reference, None,
        );
        self.write_fact(w, "-", "Company number", &self.company.company_number);
        self.write_fact_non_numeric(
            w, "30", "Return period start", "ct-comp:StartOfPeriodCoveredByReturn", "ctxt-0",
            &format_date(&self.company.return_period_start()), Some("ixt2:datedaymonthyearen"),
        );
        self.write_fact_non_numeric(
            w, "35", "Return period end", "ct-comp:EndOfPeriodCoveredByReturn", "ctxt-0",
            &format_date(&self.company.return_period_end()), Some("ixt2:datedaymonthyearen"),
        );
        self.write_fact_non_numeric(
            w, "-", "Period of account start", "ct-comp:PeriodOfAccountStartDate", "ctxt-0",
            &format_date(&self.company.accounting_period_start), Some("ixt2:datedaymonthyearen"),
        );
        self.write_fact_non_numeric(
            w, "-", "Period of account end", "ct-comp:PeriodOfAccountEndDate", "ctxt-0",
            &format_date(&self.company.accounting_period_end), Some("ixt2:datedaymonthyearen"),
        );
        self.write_fact_non_numeric(
            w, "-", "Partner in a firm", "ct-comp:CompanyIsAPartnerInAFirm", "ctxt-1",
            &self.partner_in_a_firm.to_string(), None,
        );

        self.close_page(w);
    }

    fn write_capital_allowances_page(&self, w: &mut IxbrlWriter) {
        self.open_page(w, "elt-010", "elt-011", "Capital allowances and balancing charges");

        self.write_fact_numeric(
            w, "690", "Annual investment allowance",
            "ct-comp:MainPoolAnnualInvestmentAllowance", "ctxt-2", "U-GBP",
            self.annual_investment_allowance,
        );

        self.close_page(w);
    }

    fn write_profits_and_gains_page(&self, w: &mut IxbrlWriter) {
        self.open_page(w, "elt-020", "elt-021", "Profits and gains");

        let fields: &[(&str, &str, &str, &str, f64)] = &[
            ("155", "Trading profits", "ct-comp:AdjustedTradingProfitOfThisPeriod", "ctxt-3", self.adjusted_trading_profit),
            ("160", "Trading losses brought forward", "ct-comp:TradingLossesBroughtForward", "ctxt-3", self.trading_losses_brought_forward),
            ("165", "Net trading profits", "ct-comp:NetTradingProfits", "ctxt-3", self.net_trading_profits),
            ("220", "Net chargeable gains", "ct-comp:NetChargeableGains", "ctxt-1", self.net_chargeable_gains),
            ("235", "Profits before other deductions and reliefs", "ct-comp:ProfitsBeforeOtherDeductionsAndReliefs", "ctxt-3", self.profits_before_deductions),
            ("300", "Profits before donations and group relief", "ct-comp:ProfitsBeforeChargesAndGroupRelief", "ctxt-3", self.profits_before_charges),
            ("305", "Qualifying donations", "ct-comp:QualifyingDonations", "ctxt-1", self.qualifying_donations),
            ("310", "Group relief claimed", "ct-comp:GroupReliefClaimed", "ctxt-1", self.group_relief),
            ("320", "Group relief for carried forward losses", "ct-comp:GroupReliefClaimedForCarriedForwardLosses", "ctxt-1", self.group_relief_carried_forward),
            ("335", "Profits chargeable to Corporation Tax", "ct-comp:TotalProfitsChargeableToCorporationTax", "ctxt-3", self.profits_chargeable_to_corporation_tax),
        ];
        for (ref_num, label, tag, ctx, val) in fields {
            self.write_fact_numeric(w, ref_num, label, tag, ctx, "U-GBP", *val);
        }

        self.close_page(w);
    }

    fn write_losses_page(&self, w: &mut IxbrlWriter) {
        self.open_page(w, "elt-030", "elt-031", "Losses");

        self.write_fact_numeric(
            w, "-", "Trading losses of this or later AP",
            "ct-comp:TradingLossesOfThisOrLaterAP", "ctxt-3", "U-GBP",
            self.losses_of_trades_uk,
        );
        self.write_fact_numeric(
            w, "-", "Losses from miscellaneous transactions",
            "ct-comp:LossesFromMiscellaneousTransactions", "ctxt-1", "U-GBP",
            self.losses_from_miscellaneous,
        );

        self.close_page(w);
    }

    fn write_tax_chargeable_page(&self, w: &mut IxbrlWriter) {
        self.open_page(w, "elt-040", "elt-041", "Tax chargeable");

        self.write_fact_non_numeric(
            w, "400", "Financial year 1 covered by the return",
            "ct-comp:FinancialYear1CoveredByTheReturn", "ctxt-1",
            &self.company.fy1_year.to_string(), None,
        );
        self.write_fact_non_numeric(
            w, "405", "Financial year 2 covered by the return",
            "ct-comp:FinancialYear2CoveredByTheReturn", "ctxt-1",
            &self.company.fy2_year.to_string(), None,
        );
        self.write_fact_numeric(
            w, "410", "FY1 profit chargeable at first rate",
            "ct-comp:FY1AmountOfProfitChargeableAtFirstRate", "ctxt-3", "U-GBP",
            self.fy1_profit,
        );
        self.write_fact_numeric(
            w, "415", "FY2 profit chargeable at first rate",
            "ct-comp:FY2AmountOfProfitChargeableAtFirstRate", "ctxt-3", "U-GBP",
            self.fy2_profit,
        );
        self.write_fact_numeric(
            w, "420", "FY1 first rate of tax",
            "ct-comp:FY1FirstRateOfTax", "ctxt-1", "U-GBP",
            self.company.fy1_rate,
        );
        self.write_fact_numeric(
            w, "425", "FY2 first rate of tax",
            "ct-comp:FY2FirstRateOfTax", "ctxt-1", "U-GBP",
            self.company.fy2_rate,
        );
        self.write_fact_numeric(
            w, "430", "FY1 tax at first rate",
            "ct-comp:FY1TaxAtFirstRate", "ctxt-3", "U-GBP",
            self.fy1_tax,
        );
        self.write_fact_numeric(
            w, "435", "FY2 tax at first rate",
            "ct-comp:FY2TaxAtFirstRate", "ctxt-3", "U-GBP",
            self.fy2_tax,
        );
        self.write_fact_numeric(
            w, "440", "Corporation tax chargeable",
            "ct-comp:CorporationTaxChargeable", "ctxt-3", "U-GBP",
            self.corporation_tax_chargeable,
        );
        self.write_fact_numeric(
            w, "445", "Marginal rate relief",
            "ct-comp:MarginalRateReliefForRingFenceTradesPayable", "ctxt-1", "U-GBP",
            self.marginal_relief,
        );
        self.write_fact_numeric(
            w, "450", "Corporation tax chargeable payable",
            "ct-comp:CorporationTaxChargeablePayable", "ctxt-3", "U-GBP",
            self.corporation_tax_chargeable_payable,
        );
        self.write_fact_numeric(
            w, "455", "Total reliefs and deductions",
            "ct-comp:TotalReliefsAndDeductionsInTermsOfTaxPayable", "ctxt-1", "U-GBP",
            self.total_reliefs_deductions_tax,
        );
        self.write_fact_numeric(
            w, "460", "Net corporation tax payable",
            "ct-comp:NetCorporationTaxPayable", "ctxt-3", "U-GBP",
            self.net_corporation_tax_payable,
        );
        self.write_fact_numeric(
            w, "465", "Tax chargeable",
            "ct-comp:TaxChargeable", "ctxt-3", "U-GBP",
            self.tax_chargeable,
        );
        self.write_fact_numeric(
            w, "470", "Tax payable",
            "ct-comp:TaxPayable", "ctxt-3", "U-GBP",
            self.tax_payable,
        );

        self.close_page(w);
    }

    fn write_rnd_page(&self, w: &mut IxbrlWriter) {
        self.open_page(w, "elt-050", "elt-051", "R&D / Creative enhanced expenditure");

        self.write_fact_non_numeric(
            w, "560", "SME company", "ct-comp:CompanyIsAPartnerInAFirm", "ctxt-1",
            &self.is_sme.to_string(), None,
        );
        self.write_fact_non_numeric(
            w, "565", "Large company", "ct-comp:CompanyIsAPartnerInAFirm", "ctxt-1",
            &self.is_large_company.to_string(), None,
        );
        self.write_fact_numeric(
            w, "575", "Qualifying expenditure",
            "ct-comp:SubsidisedQualifyingExpenditureOnIn-HouseDirectRD", "ctxt-4", "U-GBP",
            self.rnd_qualifying_expenditure,
        );
        self.write_fact_numeric(
            w, "580", "Enhanced expenditure",
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME", "ctxt-4", "U-GBP",
            self.rnd_enhanced_expenditure,
        );
        self.write_fact_numeric(
            w, "585", "Creative enhanced expenditure",
            "ct-comp:AdjustmentsCreativeProductionCompanyAdjustment", "ctxt-5", "U-GBP",
            self.creative_enhanced_expenditure,
        );
        self.write_fact_numeric(
            w, "590", "R&D and creative total",
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME", "ctxt-4", "U-GBP",
            self.rnd_creative_enhanced_total,
        );
        self.write_fact_numeric(
            w, "-", "Subcontracted large",
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME", "ctxt-8", "U-GBP",
            self.rnd_subcontracted_large,
        );

        self.close_page(w);
    }

    fn write_rnd_worksheet_page(&self, w: &mut IxbrlWriter) {
        let fy1 = self.company.fy1_year;
        let fy2 = self.company.fy2_year;

        w.open_element("div", &[("class", "page")]);
        w.open_element("div", &[("class", "worksheet")]);
        w.write_element("h2", &[], "SME R&D");

        w.open_element("table", &[("class", "sheet table")]);

        // Header row
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label cell")]);
        w.write_raw("&#160;");
        w.close_element("td");
        w.write_element("td", &[("class", "column header cell")], &fy2.to_string());
        w.write_element("td", &[("class", "column header cell")], &fy1.to_string());
        w.close_element("tr");

        // Currency row
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label cell")]);
        w.write_raw("&#160;");
        w.close_element("td");
        w.open_element("td", &[("class", "column currency cell")]);
        w.write_raw("&#163;");
        w.close_element("td");
        w.open_element("td", &[("class", "column currency cell")]);
        w.write_raw("&#163;");
        w.close_element("td");
        w.close_element("tr");

        for project in &self.rd_projects {
            // Blank row
            w.open_element("tr", &[("class", "row")]);
            w.open_element("td", &[("class", "label cell")]);
            w.write_raw("&#160;");
            w.close_element("td");
            w.close_element("tr");

            // Project heading
            w.open_element("tr", &[("class", "row")]);
            w.open_element("td", &[("class", "label breakdown heading cell")]);
            w.write_element("span", &[], &project.name);
            w.close_element("td");
            w.close_element("tr");

            // Item rows
            for item in &project.items {
                let v2 = item.values_by_fy.get(&fy2).copied().unwrap_or(0.0);
                let v1 = item.values_by_fy.get(&fy1).copied().unwrap_or(0.0);
                w.open_element("tr", &[("class", "row")]);
                w.open_element("td", &[("class", "label breakdown item cell")]);
                w.write_element("span", &[], &item.label);
                w.close_element("td");
                self.write_data_cell(w, -v2);
                self.write_data_cell(w, -v1);
                w.close_element("tr");
            }

            // Subtotal
            let total2: f64 = project.items.iter().map(|i| i.values_by_fy.get(&fy2).copied().unwrap_or(0.0)).sum();
            let total1: f64 = project.items.iter().map(|i| i.values_by_fy.get(&fy1).copied().unwrap_or(0.0)).sum();
            w.open_element("tr", &[("class", "row")]);
            w.write_element("td", &[("class", "label breakdown total cell")], "Total");
            self.write_data_cell_total(w, -total2);
            self.write_data_cell_total(w, -total1);
            w.close_element("tr");

            // Blank row
            w.open_element("tr", &[("class", "row")]);
            w.open_element("td", &[("class", "label cell")]);
            w.write_raw("&#160;");
            w.close_element("td");
            w.close_element("tr");

            // Enhanced heading
            w.open_element("tr", &[("class", "row")]);
            w.open_element("td", &[("class", "label breakdown heading cell")]);
            w.write_element("span", &[], "SME R&D tax relief (130%)");
            w.close_element("td");
            w.close_element("tr");

            // Enhanced project row
            let enh2 = project.enhanced_by_fy.get(&fy2).copied().unwrap_or(0.0);
            let enh1 = project.enhanced_by_fy.get(&fy1).copied().unwrap_or(0.0);
            w.open_element("tr", &[("class", "row")]);
            w.open_element("td", &[("class", "label breakdown item cell")]);
            w.write_element("span", &[], &project.name);
            w.close_element("td");
            self.write_data_cell(w, -enh2);
            self.write_data_cell(w, -enh1);
            w.close_element("tr");

            // Enhanced total with ix:nonFraction
            w.open_element("tr", &[("class", "row")]);
            w.write_element("td", &[("class", "label breakdown total cell")], "Total");
            self.write_data_cell_total_ix(w, "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME", "ctxt-4", -enh2);
            self.write_data_cell_total_ix(w, "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME", "ctxt-8", -enh1);
            w.close_element("tr");
        }

        w.close_element("table");
        w.close_element("div");
        w.close_element("div");
    }

    fn write_tax_calculation_worksheet(&self, w: &mut IxbrlWriter) {
        let fy1 = self.company.fy1_year;
        let fy2 = self.company.fy2_year;

        w.open_element("div", &[("class", "page")]);
        w.open_element("div", &[("class", "worksheet")]);
        w.write_element("h2", &[], "Tax calculation");

        w.open_element("table", &[("class", "sheet table")]);

        // Header row
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label cell")]);
        w.write_raw("&#160;");
        w.close_element("td");
        w.write_element("td", &[("class", "column header cell")], &fy2.to_string());
        w.write_element("td", &[("class", "column header cell")], &fy1.to_string());
        w.close_element("tr");

        // Currency row
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label cell")]);
        w.write_raw("&#160;");
        w.close_element("td");
        w.open_element("td", &[("class", "column currency cell")]);
        w.write_raw("&#163;");
        w.close_element("td");
        w.open_element("td", &[("class", "column currency cell")]);
        w.write_raw("&#163;");
        w.close_element("td");
        w.close_element("tr");

        // Blank row
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label cell")]);
        w.write_raw("&#160;");
        w.close_element("td");
        w.close_element("tr");

        // "Taxable profits" heading
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label breakdown heading cell")]);
        w.write_element("span", &[], "Taxable profits");
        w.close_element("td");
        w.close_element("tr");

        // Profit per accounts
        let ppa2 = *self.profit_per_accounts_by_fy.get(&fy2).unwrap_or(&0.0);
        let ppa1 = *self.profit_per_accounts_by_fy.get(&fy1).unwrap_or(&0.0);
        self.write_table_row_with_ix(w, "Profit (loss) per accounts",
            "ct-comp:ProfitLossPerAccounts", "ctxt-4", "ctxt-8", ppa2, ppa1);

        // AIA (negative)
        let aia2 = *self.aia_by_fy.get(&fy2).unwrap_or(&0.0);
        let aia1 = *self.aia_by_fy.get(&fy1).unwrap_or(&0.0);
        self.write_table_row_with_ix_neg(w, "Annual investment allowance",
            "ct-comp:MainPoolAnnualInvestmentAllowance", "ctxt-2", "ctxt-15", aia2, aia1);

        // SME R&D tax relief (negative)
        let rnd2 = *self.rnd_by_fy.get(&fy2).unwrap_or(&0.0);
        let rnd1 = *self.rnd_by_fy.get(&fy1).unwrap_or(&0.0);
        self.write_table_row_with_ix_neg(w, "SME R&D tax relief (130%)",
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME", "ctxt-4", "ctxt-8", rnd2, rnd1);

        // Taxable profits total (plain value, no ix tag)
        let total2 = ppa2 - aia2 - rnd2;
        let total1 = ppa1 - aia1 - rnd1;
        self.write_table_row_total(w, "Total", total2, total1);

        // Blank row
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label cell")]);
        w.write_raw("&#160;");
        w.close_element("td");
        w.close_element("tr");

        // Trading losses brought forward
        self.write_table_row_with_ix(w, "Trading losses brought forward",
            "ct-comp:TradingLossesBroughtForward", "ctxt-3", "ctxt-16",
            self.trading_losses_brought_forward, self.trading_losses_brought_forward);

        // Blank row
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label cell")]);
        w.write_raw("&#160;");
        w.close_element("td");
        w.close_element("tr");

        // Profits chargeable
        self.write_table_row_with_ix(w, "Profits chargeable to corporation tax",
            "ct-comp:NetTradingProfits", "ctxt-3", "ctxt-16",
            self.net_trading_profits, self.prev_profit_chargeable);

        // Blank row
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label cell")]);
        w.write_raw("&#160;");
        w.close_element("td");
        w.close_element("tr");

        // Trading losses
        self.write_table_row_with_ix(w, "Trading losses",
            "ct-comp:TradingLossesOfThisOrLaterAP", "ctxt-3", "ctxt-16",
            self.losses_of_trades_uk, self.losses_of_trades_uk);

        // Blank row
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label cell")]);
        w.write_raw("&#160;");
        w.close_element("td");
        w.close_element("tr");

        // "Profits, by financial year" heading
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label breakdown heading cell")]);
        w.write_element("span", &[], "Profits, by financial year");
        w.close_element("td");
        w.close_element("tr");

        // FY1 profit
        self.write_table_row_with_ix(w, "FY1",
            "ct-comp:FY1AmountOfProfitChargeableAtFirstRate", "ctxt-3", "ctxt-16",
            self.fy1_profit, self.prev_fy1_profit);

        // FY2 profit
        self.write_table_row_with_ix(w, "FY2",
            "ct-comp:FY2AmountOfProfitChargeableAtFirstRate", "ctxt-3", "ctxt-16",
            self.fy2_profit, self.prev_fy2_profit);

        // Profits total
        self.write_table_row_with_ix(w, "Total",
            "ct-comp:TotalProfitsChargeableToCorporationTax", "ctxt-3", "ctxt-16",
            self.profits_chargeable_to_corporation_tax, self.prev_profit_chargeable);

        // Blank row
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label cell")]);
        w.write_raw("&#160;");
        w.close_element("td");
        w.close_element("tr");

        // "Corporation tax chargeable" heading
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label breakdown heading cell")]);
        w.write_element("span", &[], "Corporation tax chargeable");
        w.close_element("td");
        w.close_element("tr");

        // FY1 tax (negative)
        self.write_table_row_with_ix_neg(w, "FY1 (19%)",
            "ct-comp:FY1TaxAtFirstRate", "ctxt-3", "ctxt-16",
            self.fy1_tax, self.prev_fy1_tax);

        // FY2 tax (negative)
        self.write_table_row_with_ix_neg(w, "FY2 (19%)",
            "ct-comp:FY2TaxAtFirstRate", "ctxt-3", "ctxt-16",
            self.fy2_tax, self.prev_fy2_tax);

        // CT chargeable total (negative)
        self.write_table_row_with_ix_neg(w, "Total",
            "ct-comp:CorporationTaxChargeable", "ctxt-3", "ctxt-16",
            self.corporation_tax_chargeable, self.prev_corporation_tax_chargeable);

        w.close_element("table");
        w.close_element("div");
        w.close_element("div");
    }

    fn write_table_row_with_ix(&self, w: &mut IxbrlWriter, label: &str, name: &str, ctx_cur: &str, ctx_prev: &str, val_cur: f64, val_prev: f64) {
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label breakdown item cell")]);
        w.write_element("span", &[], label);
        w.close_element("td");
        // Current year
        w.open_element("td", &[("class", "data value cell")]);
        w.open_element("span", &[]);
        w.open_element("span", &[]);
        w.write_raw("</span>");
        w.open_element("ix:nonFraction", &[
            ("name", name),
            ("contextRef", ctx_cur),
            ("format", "ixt2:numdotdecimal"),
            ("unitRef", "U-GBP"),
            ("decimals", "2"),
            ("scale", "0"),
        ]);
        w.write_raw(&format!("{:.2}", val_cur));
        w.close_element("ix:nonFraction");
        w.write_raw("<span>&#160;&#160;</span>");
        w.close_element("span");
        w.close_element("td");
        // Previous year
        w.open_element("td", &[("class", "data value cell")]);
        w.open_element("span", &[]);
        w.open_element("span", &[]);
        w.write_raw("</span>");
        w.open_element("ix:nonFraction", &[
            ("name", name),
            ("contextRef", ctx_prev),
            ("format", "ixt2:numdotdecimal"),
            ("unitRef", "U-GBP"),
            ("decimals", "2"),
            ("scale", "0"),
        ]);
        w.write_raw(&format!("{:.2}", val_prev));
        w.close_element("ix:nonFraction");
        w.write_raw("<span>&#160;&#160;</span>");
        w.close_element("span");
        w.close_element("td");
        w.close_element("tr");
    }

    fn write_table_row_with_ix_neg(&self, w: &mut IxbrlWriter, label: &str, name: &str, ctx_cur: &str, ctx_prev: &str, val_cur: f64, val_prev: f64) {
        w.open_element("tr", &[("class", "row")]);
        w.open_element("td", &[("class", "label breakdown item cell")]);
        w.write_element("span", &[], label);
        w.close_element("td");
        // Current year (negative)
        if val_cur != 0.0 {
            w.open_element("td", &[("class", "data value negative cell")]);
            w.open_element("span", &[]);
            w.write_raw("<span>( </span>");
            w.open_element("ix:nonFraction", &[
                ("name", name),
                ("contextRef", ctx_cur),
                ("format", "ixt2:numdotdecimal"),
                ("unitRef", "U-GBP"),
                ("decimals", "2"),
                ("scale", "0"),
            ]);
            w.write_raw(&format!("{:.2}", val_cur));
            w.close_element("ix:nonFraction");
            w.write_raw("<span> )</span>");
            w.close_element("span");
            w.close_element("td");
        } else {
            w.open_element("td", &[("class", "data value nil cell")]);
            w.open_element("span", &[]);
            w.open_element("span", &[]);
            w.write_raw("</span>");
            w.open_element("ix:nonFraction", &[
                ("name", name),
                ("contextRef", ctx_cur),
                ("format", "ixt2:numdotdecimal"),
                ("unitRef", "U-GBP"),
                ("decimals", "2"),
                ("scale", "0"),
            ]);
            w.write_raw("0.00");
            w.close_element("ix:nonFraction");
            w.write_raw("<span>&#160;&#160;</span>");
            w.close_element("span");
            w.close_element("td");
        }
        // Previous year (negative)
        if val_prev != 0.0 {
            w.open_element("td", &[("class", "data value negative cell")]);
            w.open_element("span", &[]);
            w.write_raw("<span>( </span>");
            w.open_element("ix:nonFraction", &[
                ("name", name),
                ("contextRef", ctx_prev),
                ("format", "ixt2:numdotdecimal"),
                ("unitRef", "U-GBP"),
                ("decimals", "2"),
                ("scale", "0"),
            ]);
            w.write_raw(&format!("{:.2}", val_prev));
            w.close_element("ix:nonFraction");
            w.write_raw("<span> )</span>");
            w.close_element("span");
            w.close_element("td");
        } else {
            w.open_element("td", &[("class", "data value nil cell")]);
            w.open_element("span", &[]);
            w.open_element("span", &[]);
            w.write_raw("</span>");
            w.open_element("ix:nonFraction", &[
                ("name", name),
                ("contextRef", ctx_prev),
                ("format", "ixt2:numdotdecimal"),
                ("unitRef", "U-GBP"),
                ("decimals", "2"),
                ("scale", "0"),
            ]);
            w.write_raw("0.00");
            w.close_element("ix:nonFraction");
            w.write_raw("<span>&#160;&#160;</span>");
            w.close_element("span");
            w.close_element("td");
        }
        w.close_element("tr");
    }

    fn write_table_row_total(&self, w: &mut IxbrlWriter, label: &str, val_cur: f64, val_prev: f64) {
        w.open_element("tr", &[("class", "row")]);
        w.write_element("td", &[("class", "label breakdown total cell")], label);
        self.write_data_cell_total(w, val_cur);
        self.write_data_cell_total(w, val_prev);
        w.close_element("tr");
    }

    fn write_data_cell(&self, w: &mut IxbrlWriter, value: f64) {
        if value == 0.0 {
            w.open_element("td", &[("class", "data value nil cell")]);
            w.open_element("span", &[]);
            w.write_raw("0.00&#160;&#160;");
            w.close_element("span");
            w.close_element("td");
        } else if value < 0.0 {
            w.open_element("td", &[("class", "data value negative cell")]);
            w.open_element("span", &[]);
            w.write_raw("<span>( </span>");
            w.write_element("span", &[], &format!("{:.2}", value.abs()));
            w.write_raw("<span> )</span>");
            w.close_element("span");
            w.close_element("td");
        } else {
            w.open_element("td", &[("class", "data value cell")]);
            w.write_element("span", &[], &format!("{:.2}", value));
            w.close_element("td");
        }
    }

    fn write_data_cell_total(&self, w: &mut IxbrlWriter, value: f64) {
        if value == 0.0 {
            w.open_element("td", &[("class", "data value breakdown total nil cell")]);
            w.open_element("span", &[]);
            w.write_raw("0.00&#160;&#160;");
            w.close_element("span");
            w.close_element("td");
        } else if value < 0.0 {
            w.open_element("td", &[("class", "data value breakdown total negative cell")]);
            w.open_element("span", &[]);
            w.write_raw("<span>( </span>");
            w.write_element("span", &[], &format!("{:.2}", value.abs()));
            w.write_raw("<span> )</span>");
            w.close_element("span");
            w.close_element("td");
        } else {
            w.open_element("td", &[("class", "data value breakdown total cell")]);
            w.write_element("span", &[], &format!("{:.2}", value));
            w.close_element("td");
        }
    }

    fn write_data_cell_total_ix(&self, w: &mut IxbrlWriter, name: &str, ctx: &str, value: f64) {
        if value < 0.0 {
            w.open_element("td", &[("class", "data value breakdown total negative cell")]);
            w.open_element("span", &[]);
            w.write_raw("<span>( </span>");
            w.open_element("ix:nonFraction", &[
                ("name", name),
                ("contextRef", ctx),
                ("unitRef", "U-GBP"),
                ("format", "ixt2:numdotdecimal"),
                ("decimals", "2"),
                ("scale", "0"),
            ]);
            w.write_raw(&format!("{:.2}", value.abs()));
            w.close_element("ix:nonFraction");
            w.write_raw("<span> )</span>");
            w.close_element("span");
            w.close_element("td");
        } else if value == 0.0 {
            w.open_element("td", &[("class", "data value breakdown total nil cell")]);
            w.open_element("span", &[]);
            w.open_element("ix:nonFraction", &[
                ("name", name),
                ("contextRef", ctx),
                ("unitRef", "U-GBP"),
                ("format", "ixt2:numdotdecimal"),
                ("decimals", "2"),
                ("scale", "0"),
            ]);
            w.write_raw("0.00");
            w.close_element("ix:nonFraction");
            w.open_element("span", &[]);
            w.write_raw("&#160;&#160;");
            w.close_element("span");
            w.close_element("span");
            w.close_element("td");
        } else {
            w.open_element("td", &[("class", "data value breakdown total cell")]);
            w.open_element("span", &[]);
            w.open_element("ix:nonFraction", &[
                ("name", name),
                ("contextRef", ctx),
                ("unitRef", "U-GBP"),
                ("format", "ixt2:numdotdecimal"),
                ("decimals", "2"),
                ("scale", "0"),
            ]);
            w.write_raw(&format!("{:.2}", value));
            w.close_element("ix:nonFraction");
            w.open_element("span", &[]);
            w.write_raw("&#160;&#160;");
            w.close_element("span");
            w.close_element("span");
            w.close_element("td");
        }
    }
}

#[allow(dead_code)]
fn format_f64(v: f64) -> String {
    let formatted = format!("{:.2}", v);
    let parts: Vec<&str> = formatted.split('.').collect();
    let int_part = parts[0];
    let dec_part = parts.get(1).unwrap_or(&"00");
    let mut result = String::new();
    let bytes = int_part.as_bytes();
    let len = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*b as char);
    }
    result.push('.');
    result.push_str(dec_part);
    result
}

fn format_date(d: &chrono::NaiveDate) -> String {
    let day = d.format("%d").to_string();
    let month = d.format("%B").to_string();
    let year = d.format("%Y").to_string();
    format!(
        "{}&#160;{}&#160;{}",
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
        let gnucash =
            crate::GnucashBook::try_from_gnucash_file("example_data/example2/example2.gnucash")
                .await
                .expect("open gnucash");
        let company = crate::company::Company::new(
            "Example Biz Ltd.",
            "8596148860",
            "12345678",
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap(),
        );
        let ct = CorporationTaxReturn::builder(&gnucash, &company)
            .add_rd_project("Project Iguana", &[
                ("Staffing Costs", "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs"),
                ("Software/Consumables", "R&D Enhanced Expenditure:Expenditure:Project Iguana:Software/Consumables"),
                ("External Workers", "R&D Enhanced Expenditure:Expenditure:Project Iguana:External Workers"),
            ], "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs")
            .build();

        assert_eq!(ct.company.name, "Example Biz Ltd.");
        assert_eq!(ct.company.tax_reference, "8596148860");
        assert_eq!(ct.company.company_number, "12345678");
        assert_eq!(ct.company.fy1_year, 2019);
        assert_eq!(ct.company.fy2_year, 2020);

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
        assert!(ixbrl.contains("Example Biz Ltd."));
        assert!(ixbrl.contains("8596148860"));

        std::fs::create_dir_all("../../.cache").unwrap();
        std::fs::write("../../.cache/ct_return_example2.html", &ixbrl).unwrap();
    }

    #[tokio::test]
    async fn test_try_from_gnucash_file_sql() {
        let gnucash =
            crate::GnucashBook::try_from_gnucash_file("example_data/example2/example2.gnucash")
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
        assert_eq!(format_date(&d), "1&#160;January&#160;2020");
    }

    #[tokio::test]
    async fn test_rnd_worksheet_output() {
        let gnucash =
            crate::GnucashBook::try_from_gnucash_file("example_data/example2/example2.gnucash")
                .await
                .expect("open gnucash");
        let company = crate::company::Company::new(
            "Example Biz Ltd.",
            "8596148860",
            "12345678",
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap(),
        );
        let ct = CorporationTaxReturn::builder(&gnucash, &company)
            .add_rd_project("Project Iguana", &[
                ("Staffing Costs", "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs"),
                ("Software/Consumables", "R&D Enhanced Expenditure:Expenditure:Project Iguana:Software/Consumables"),
                ("External Workers", "R&D Enhanced Expenditure:Expenditure:Project Iguana:External Workers"),
            ], "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs")
            .build();

        assert!(!ct.rd_projects.is_empty());
        let p = &ct.rd_projects[0];
        assert_eq!(p.name, "Project Iguana");
        assert_eq!(p.items.len(), 3);
        assert_eq!(p.items[0].label, "Staffing Costs");
        assert_eq!(p.items[0].values_by_fy[&2020], 465.0);
        assert_eq!(p.items[1].label, "Software/Consumables");
        assert_eq!(p.items[1].values_by_fy[&2020], 0.0);
        assert_eq!(p.items[2].label, "External Workers");
        assert_eq!(p.items[2].values_by_fy[&2020], 0.0);

        let ixbrl = ct.to_ixbrl();
        assert!(ixbrl.contains("SME R&amp;D</h2>"));
        assert!(ixbrl.contains("Staffing Costs"));
        assert!(ixbrl.contains("Software/Consumables"));
        assert!(ixbrl.contains("External Workers"));
        assert!(ixbrl.contains("SME R&amp;D tax relief (130%)"));
        assert!(ixbrl.contains("Project Iguana"));
        assert!(ixbrl.contains("ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME"));
        assert!(ixbrl.contains("sheet table"));
    }

    #[tokio::test]
    async fn test_ixbrl_tag_structure_matches_reference() {
        let gnucash =
            crate::GnucashBook::try_from_gnucash_file("example_data/example2/example2.gnucash")
                .await
                .expect("open gnucash");
        let company = crate::company::Company::new(
            "Example Biz Ltd.",
            "8596148860",
            "12345678",
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap(),
        );
        let ct = CorporationTaxReturn::builder(&gnucash, &company)
            .add_rd_project("Project Iguana", &[
                ("Staffing Costs", "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs"),
                ("Software/Consumables", "R&D Enhanced Expenditure:Expenditure:Project Iguana:Software/Consumables"),
                ("External Workers", "R&D Enhanced Expenditure:Expenditure:Project Iguana:External Workers"),
            ], "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs")
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

    async fn build_example2_ct() -> CorporationTaxReturn {
        let gnucash =
            crate::GnucashBook::try_from_gnucash_file("example_data/example2/example2.gnucash")
                .await
                .expect("open gnucash");
        let company = crate::company::Company::new(
            "Example Biz Ltd.",
            "8596148860",
            "12345678",
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap(),
        );
        CorporationTaxReturn::builder(&gnucash, &company)
            .add_rd_project("Project Iguana", &[
                ("Staffing Costs", "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs"),
                ("Software/Consumables", "R&D Enhanced Expenditure:Expenditure:Project Iguana:Software/Consumables"),
                ("External Workers", "R&D Enhanced Expenditure:Expenditure:Project Iguana:External Workers"),
            ], "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs")
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
        let expected_tax = (ct.fy1_tax * 100.0).round() / 100.0
            + (ct.fy2_tax * 100.0).round() / 100.0;
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
            (ct.profit_before_tax - ct.tax_expense).round()
        );
    }

    #[tokio::test]
    async fn test_invariant_by_fy_consistency() {
        let ct = build_example2_ct().await;
        let fy2 = ct.company.fy2_year;
        assert_eq!(
            *ct.turnover_by_fy.get(&fy2).unwrap_or(&0.0),
            ct.turnover
        );
        assert_eq!(
            *ct.costs_by_fy.get(&fy2).unwrap_or(&0.0),
            ct.total_costs
        );
        assert_eq!(
            *ct.profit_per_accounts_by_fy.get(&fy2).unwrap_or(&0.0),
            ct.profit_before_tax
        );
    }

    #[test]
    fn test_from_ixbrl_round_trip() {
        let html = std::fs::read_to_string("../../.cache/ct_return_example2.html")
            .expect("read cached ixbrl");
        let facts = CorporationTaxReturn::from_ixbrl(&html);

        assert_eq!(
            facts.non_numeric.get("ct-comp:CompanyName").unwrap(),
            "Example Biz Ltd."
        );
        assert_eq!(
            facts.non_numeric.get("ct-comp:TaxReference").unwrap(),
            "8596148860"
        );

        assert_eq!(
            facts.numeric_by_ctx.get(&("ct-comp:NetTradingProfits".into(), "ctxt-3".into())),
            Some(&748.0)
        );
        assert_eq!(
            facts.numeric_by_ctx.get(&(
                "ct-comp:CorporationTaxChargeable".into(),
                "ctxt-3".into()
            )),
            Some(&142.12)
        );
        assert_eq!(
            facts.numeric_by_ctx.get(&(
                "ct-comp:NetCorporationTaxPayable".into(),
                "ctxt-3".into()
            )),
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

    #[test]
    fn test_from_ixbrl_worksheet_fy_split() {
        let html = std::fs::read_to_string("../../.cache/ct_return_example2.html")
            .expect("read cached ixbrl");
        let facts = CorporationTaxReturn::from_ixbrl(&html);

        let fy1_cur = facts
            .numeric_by_ctx
            .get(&(
                "ct-comp:FY1AmountOfProfitChargeableAtFirstRate".into(),
                "ctxt-3".into(),
            ));
        let fy1_prev = facts
            .numeric_by_ctx
            .get(&(
                "ct-comp:FY1AmountOfProfitChargeableAtFirstRate".into(),
                "ctxt-16".into(),
            ));
        assert_eq!(fy1_cur, Some(&186.0));
        assert!(fy1_prev.is_some());
    }
}
