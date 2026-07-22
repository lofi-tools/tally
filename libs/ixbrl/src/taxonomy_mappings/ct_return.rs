use std::collections::HashMap;

use crate::GnucashBook;

#[derive(Debug, Clone)]
pub struct CorporationTaxReturn {
    pub company_name: String,
    pub tax_reference: String,
    pub company_number: String,
    pub return_period_start: chrono::NaiveDate,
    pub return_period_end: chrono::NaiveDate,
    pub accounting_period_start: chrono::NaiveDate,
    pub accounting_period_end: chrono::NaiveDate,
    pub fy1_year: i32,
    pub fy2_year: i32,
    pub fy1_rate: f64,
    pub fy2_rate: f64,

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

fn round_down(v: f64) -> f64 {
    v.floor()
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

impl CorporationTaxReturn {
    pub fn from_gnucash(
        gnucash: &GnucashBook,
        accounting_start: chrono::NaiveDate,
        accounting_end: chrono::NaiveDate,
        company_name: &str,
        tax_reference: &str,
        company_number: &str,
    ) -> Self {
        let accounts = gnucash.raw_accounts();
        let txns = gnucash.raw_transactions();
        let splits = gnucash.raw_splits();

        let mut period_splits: Vec<(f64, String)> = Vec::new();
        let mut prev_splits: Vec<(f64, String)> = Vec::new();

        let prev_start = accounting_start - chrono::Duration::days(365);
        let prev_end = accounting_end - chrono::Duration::days(365);

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

            if tx_date >= accounting_start && tx_date <= accounting_end {
                period_splits.push((val, path.clone()));
            }
            if tx_date >= prev_start && tx_date <= prev_end {
                prev_splits.push((val, path.clone()));
            }
        }

        Self::from_splits(
            &period_splits,
            &prev_splits,
            accounting_start,
            accounting_end,
            company_name,
            tax_reference,
            company_number,
        )
    }

    fn from_splits(
        period_splits: &[(f64, String)],
        prev_splits: &[(f64, String)],
        accounting_start: chrono::NaiveDate,
        accounting_end: chrono::NaiveDate,
        company_name: &str,
        tax_reference: &str,
        company_number: &str,
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

        let rnd_enhanced_current = round_down(sum_abs(
            period_splits,
            "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs",
        ));

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

        let fy1_end = chrono::NaiveDate::from_ymd_opt(2020, 3, 31).unwrap();
        let total_days = (accounting_end - accounting_start).num_days() + 1;
        let fy1_days = {
            let s = accounting_start;
            let e = fy1_end.min(accounting_end);
            if e >= s { (e - s).num_days() + 1 } else { 0 }
        };
        let fy2_days = total_days - fy1_days;

        let fy1_profit = (profits_chargeable * fy1_days as f64 / total_days as f64).round();
        let fy2_profit = (profits_chargeable * fy2_days as f64 / total_days as f64).round();
        let fy1_rate = 19.0;
        let fy2_rate = 19.0;
        let fy1_tax = round2(fy1_profit * fy1_rate / 100.0);
        let fy2_tax = round2(fy2_profit * fy2_rate / 100.0);
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
        let rnd_enhanced_prev = round_down(sum_abs(
            prev_splits,
            "R&D Enhanced Expenditure:Expenditure:Project Iguana:Staffing Costs",
        ));
        let tax_expense_prev = sum_abs(prev_splits, "Equity:Corporation Tax:Corporation Tax");

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
            expenses_by_fy.insert(id.to_string(), HashMap::from([(2020, cur), (2019, prev)]));
        }

        CorporationTaxReturn {
            company_name: company_name.to_string(),
            tax_reference: tax_reference.to_string(),
            company_number: company_number.to_string(),
            return_period_start: accounting_start,
            return_period_end: accounting_end,
            accounting_period_start: accounting_start,
            accounting_period_end: accounting_end,
            fy1_year: 2019,
            fy2_year: 2020,
            fy1_rate,
            fy2_rate,

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

            profit_per_accounts_by_fy: HashMap::from([
                (2019, profit_before_tax_prev),
                (2020, profit_before_tax_current),
            ]),
            aia_by_fy: HashMap::from([(2019, aia_prev), (2020, aia_current)]),
            rnd_by_fy: HashMap::from([(2019, rnd_enhanced_prev), (2020, rnd_enhanced_current)]),
            turnover_by_fy: HashMap::from([(2019, ct_turnover_prev), (2020, ct_turnover_current)]),
            costs_by_fy: HashMap::from([(2019, total_costs_prev), (2020, total_costs_current)]),
            profit_before_tax_by_fy: HashMap::from([
                (2019, profit_before_tax_prev),
                (2020, profit_before_tax_current),
            ]),
            tax_expense_by_fy: HashMap::from([
                (2019, tax_expense_prev),
                (2020, tax_expense_current),
            ]),
            profit_after_tax_by_fy: HashMap::from([
                (2019, (profit_before_tax_prev - tax_expense_prev).round()),
                (2020, profit_after_tax),
            ]),
            wages_by_fy: HashMap::from([(2019, salaries_prev), (2020, salaries_current)]),
            pensions_by_fy: HashMap::from([(2019, pensions_prev), (2020, pensions_current)]),
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
        let mut out = String::new();

        out.push_str("<?xml version='1.0' encoding='ASCII'?>\n");
        out.push_str("<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:ix=\"http://www.xbrl.org/2013/inlineXBRL\" xmlns:link=\"http://www.xbrl.org/2003/linkbase\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" xmlns:xbrli=\"http://www.xbrl.org/2003/instance\" xmlns:xbrldi=\"http://xbrl.org/2006/xbrldi\" xmlns:ixt2=\"http://www.xbrl.org/inlineXBRL/transformation/2011-07-31\" xmlns:iso4217=\"http://www.xbrl.org/2003/iso4217\" xmlns:ct-comp=\"http://www.hmrc.gov.uk/schemas/ct/comp/2023-01-01\" xmlns:dpl=\"http://xbrl.frc.org.uk/dpl/2023-01-01\" xmlns:uk-bus=\"http://xbrl.frc.org.uk/cd/2023-01-01/business\" xmlns:uk-core=\"http://xbrl.frc.org.uk/fr/2023-01-01/core\" xmlns:uk-geo=\"http://xbrl.frc.org.uk/cd/2023-01-01/countries\">");

        out.push_str("<head><title>Corporation Tax Statement</title><style type=\"text/css\">\n");
        out.push_str(include_str!("ct_return_style.css"));
        out.push_str("</style></head><body>");

        out.push_str("<div class=\"hidden\"><ix:header>");
        out.push_str("<ix:hidden>");
        self.ix_non_numeric(
            &mut out,
            "ct-comp:NameOfProductionSoftware",
            "ctxt-0",
            "ixbrl-reporter",
        );
        self.ix_non_numeric(
            &mut out,
            "ct-comp:VersionOfProductionSoftware",
            "ctxt-0",
            "1.2.1",
        );
        self.ix_non_numeric(
            &mut out,
            "ct-comp:CompanyName",
            "ctxt-0",
            &self.company_name,
        );
        self.ix_non_numeric(
            &mut out,
            "ct-comp:TaxReference",
            "ctxt-0",
            &self.tax_reference,
        );
        out.push_str("</ix:hidden>");

        out.push_str("<ix:references>");
        out.push_str("<link:schemaRef xlink:type=\"simple\" xlink:href=\"http://www.hmrc.gov.uk/schemas/ct/comp/2023-01-01/ct-comp-2023.xsd\"></link:schemaRef>");
        out.push_str("<link:schemaRef xlink:type=\"simple\" xlink:href=\"https://xbrl.frc.org.uk/dpl/2023-01-01/dpl-2023-01-01.xsd\"></link:schemaRef>");
        out.push_str("</ix:references>");

        out.push_str("<ix:resources>");
        self.write_context_instant(
            &mut out,
            "ctxt-0",
            &self.company_number,
            &self.accounting_period_end,
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Company"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-1",
            &self.company_number,
            &self.accounting_period_start,
            &self.accounting_period_end,
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Company"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-2",
            &self.company_number,
            &self.accounting_period_start,
            &self.accounting_period_end,
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:ManagementExpenses"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-3",
            &self.company_number,
            &self.accounting_period_start,
            &self.accounting_period_end,
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Company"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-4",
            &self.company_number,
            &self.accounting_period_start,
            &self.accounting_period_end,
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Trade"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-5",
            &self.company_number,
            &self.accounting_period_start,
            &self.accounting_period_end,
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Trade"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-8",
            &self.company_number,
            &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
            &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Trade"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-9",
            &self.company_number,
            &self.accounting_period_start,
            &self.accounting_period_end,
            Some("dpl:DetailedAnalysisDimension"),
            Some("dpl:Item1"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-10",
            &self.company_number,
            &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
            &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            Some("dpl:DetailedAnalysisDimension"),
            Some("dpl:Item1"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-11",
            &self.company_number,
            &self.accounting_period_start,
            &self.accounting_period_end,
            None,
            None,
        );
        self.write_context_duration(
            &mut out,
            "ctxt-12",
            &self.company_number,
            &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
            &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            None,
            None,
        );
        self.write_context_duration(
            &mut out,
            "ctxt-13",
            &self.company_number,
            &self.accounting_period_start,
            &self.accounting_period_end,
            Some("dpl:ExpenseTypeDimension"),
            Some("dpl:AdministrativeExpenses"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-14",
            &self.company_number,
            &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
            &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            Some("dpl:ExpenseTypeDimension"),
            Some("dpl:AdministrativeExpenses"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-15",
            &self.company_number,
            &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
            &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:ManagementExpenses"),
        );
        self.write_context_duration(
            &mut out,
            "ctxt-16",
            &self.company_number,
            &chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
            &chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            Some("ct-comp:BusinessTypeDimension"),
            Some("ct-comp:Company"),
        );

        self.write_unit(&mut out, "iso4217:GBP");
        out.push_str("</ix:resources></ix:header></div>");

        self.write_corporation_tax_return_page(&mut out);
        self.write_capital_allowances_page(&mut out);
        self.write_profits_and_gains_page(&mut out);
        self.write_losses_page(&mut out);
        self.write_tax_chargeable_page(&mut out);
        self.write_rnd_page(&mut out);

        out.push_str("</body></html>");
        out
    }

    fn write_context_instant(
        &self,
        out: &mut String,
        id: &str,
        scheme_id: &str,
        date: &chrono::NaiveDate,
        dim: Option<&str>,
        val: Option<&str>,
    ) {
        out.push_str(&format!("<xbrli:context id=\"{}\"><xbrli:entity><xbrli:identifier scheme=\"http://www.companieshouse.gov.uk/\">{}</xbrli:identifier>", id, scheme_id));
        if let Some(d) = dim {
            out.push_str(&format!("<xbrli:segment><xbrldi:explicitMember dimension=\"{}\">{}</xbrldi:explicitMember></xbrli:segment>", d, val.unwrap_or("")));
        }
        out.push_str("</xbrli:entity>");
        out.push_str(&format!(
            "<xbrli:period><xbrli:instant>{}</xbrli:instant></xbrli:period></xbrli:context>",
            date
        ));
    }

    fn write_context_duration(
        &self,
        out: &mut String,
        id: &str,
        scheme_id: &str,
        start: &chrono::NaiveDate,
        end: &chrono::NaiveDate,
        dim: Option<&str>,
        val: Option<&str>,
    ) {
        out.push_str(&format!("<xbrli:context id=\"{}\"><xbrli:entity><xbrli:identifier scheme=\"http://www.companieshouse.gov.uk/\">{}</xbrli:identifier>", id, scheme_id));
        if let Some(d) = dim {
            out.push_str(&format!("<xbrli:segment><xbrldi:explicitMember dimension=\"{}\">{}</xbrldi:explicitMember></xbrli:segment>", d, val.unwrap_or("")));
        }
        out.push_str("</xbrli:entity>");
        out.push_str(&format!("<xbrli:period><xbrli:startDate>{}</xbrli:startDate><xbrli:endDate>{}</xbrli:endDate></xbrli:period></xbrli:context>", start, end));
    }

    fn write_unit(&self, out: &mut String, unit: &str) {
        out.push_str(&format!(
            "<xbrli:unit id=\"iso4217\"><xbrli:measure>{}</xbrli:measure></xbrli:unit>",
            unit
        ));
    }

    fn ix_non_numeric(&self, out: &mut String, name: &str, ctx: &str, value: &str) {
        out.push_str(&format!(
            "<ix:nonNumeric name=\"{}\" contextRef=\"{}\">{}</ix:nonNumeric>",
            name, ctx, value
        ));
    }

    fn ix_non_fraction(&self, out: &mut String, name: &str, ctx: &str, unit: &str, value: f64) {
        out.push_str(&format!("<ix:nonFraction name=\"{}\" contextRef=\"{}\" unitRef=\"{}\" decimals=\"2\" format=\"ixt2:numcomma-decimals\">{}</ix:nonFraction>", name, ctx, unit, format_f64(value)));
    }

    fn write_corporation_tax_return_page(&self, out: &mut String) {
        out.push_str(
            "<div class=\"page\"><div class=\"page .header\"><h2>Corporation Tax Return</h2></div>",
        );

        out.push_str("<div class=\"row\"><div class=\"cell\"><span class=\"label\">Company name</span></div><div class=\"cell\"><span class=\"value\">");
        self.ix_non_numeric(out, "ct-comp:CompanyName", "ctxt-0", &self.company_name);
        out.push_str("</span></div></div>");

        out.push_str("<div class=\"row\"><div class=\"cell\"><span class=\"label\">Tax reference</span></div><div class=\"cell\"><span class=\"value\">");
        self.ix_non_numeric(out, "ct-comp:TaxReference", "ctxt-0", &self.tax_reference);
        out.push_str("</span></div></div>");

        out.push_str("<div class=\"row\"><div class=\"cell\"><span class=\"label\">Company number</span></div><div class=\"cell\"><span class=\"value\">");
        out.push_str(&self.company_number);
        out.push_str("</span></div></div>");

        out.push_str("<div class=\"row\"><div class=\"cell\"><span class=\"label\">Return period start</span></div><div class=\"cell\"><span class=\"value\">");
        self.ix_non_numeric(
            out,
            "ct-comp:StartOfPeriodCoveredByReturn",
            "ctxt-0",
            &format_date(&self.return_period_start),
        );
        out.push_str("</span></div></div>");

        out.push_str("<div class=\"row\"><div class=\"cell\"><span class=\"label\">Return period end</span></div><div class=\"cell\"><span class=\"value\">");
        self.ix_non_numeric(
            out,
            "ct-comp:EndOfPeriodCoveredByReturn",
            "ctxt-0",
            &format_date(&self.return_period_end),
        );
        out.push_str("</span></div></div>");

        out.push_str("<div class=\"row\"><div class=\"cell\"><span class=\"label\">Period of account start</span></div><div class=\"cell\"><span class=\"value\">");
        self.ix_non_numeric(
            out,
            "ct-comp:PeriodOfAccountStartDate",
            "ctxt-0",
            &format_date(&self.accounting_period_start),
        );
        out.push_str("</span></div></div>");

        out.push_str("<div class=\"row\"><div class=\"cell\"><span class=\"label\">Period of account end</span></div><div class=\"cell\"><span class=\"value\">");
        self.ix_non_numeric(
            out,
            "ct-comp:PeriodOfAccountEndDate",
            "ctxt-0",
            &format_date(&self.accounting_period_end),
        );
        out.push_str("</span></div></div>");

        out.push_str("<div class=\"row\"><div class=\"cell\"><span class=\"label\">Partner in a firm</span></div><div class=\"cell\"><span class=\"value\">");
        self.ix_non_numeric(
            out,
            "ct-comp:CompanyIsAPartnerInAFirm",
            "ctxt-1",
            &self.partner_in_a_firm.to_string(),
        );
        out.push_str("</span></div></div></div>");
    }

    fn write_capital_allowances_page(&self, out: &mut String) {
        out.push_str("<div class=\"page\"><div class=\"page .header\"><h2>Capital allowances and balancing charges</h2></div>");
        out.push_str("<div class=\"row\"><div class=\"cell\"><span class=\"label\">Annual investment allowance</span></div><div class=\"cell\"><span class=\"value\">");
        self.ix_non_fraction(
            out,
            "ct-comp:MainPoolAnnualInvestmentAllowance",
            "ctxt-2",
            "iso4217:GBP",
            self.annual_investment_allowance,
        );
        out.push_str("</span></div></div></div>");
    }

    fn write_profits_and_gains_page(&self, out: &mut String) {
        out.push_str(
            "<div class=\"page\"><div class=\"page .header\"><h2>Profits and gains</h2></div>",
        );
        let fields: &[(&str, &str, &str, f64)] = &[
            (
                "Trading profits",
                "ct-comp:AdjustedTradingProfitOfThisPeriod",
                "ctxt-3",
                self.adjusted_trading_profit,
            ),
            (
                "Trading losses brought forward",
                "ct-comp:TradingLossesBroughtForward",
                "ctxt-3",
                self.trading_losses_brought_forward,
            ),
            (
                "Net trading profits",
                "ct-comp:NetTradingProfits",
                "ctxt-3",
                self.net_trading_profits,
            ),
            (
                "Net chargeable gains",
                "ct-comp:NetChargeableGains",
                "ctxt-1",
                self.net_chargeable_gains,
            ),
            (
                "Profits before other deductions and reliefs",
                "ct-comp:ProfitsBeforeOtherDeductionsAndReliefs",
                "ctxt-3",
                self.profits_before_deductions,
            ),
            (
                "Profits before donations and group relief",
                "ct-comp:ProfitsBeforeChargesAndGroupRelief",
                "ctxt-3",
                self.profits_before_charges,
            ),
            (
                "Qualifying donations",
                "ct-comp:QualifyingDonations",
                "ctxt-1",
                self.qualifying_donations,
            ),
            (
                "Group relief claimed",
                "ct-comp:GroupReliefClaimed",
                "ctxt-1",
                self.group_relief,
            ),
            (
                "Group relief for carried forward losses",
                "ct-comp:GroupReliefClaimedForCarriedForwardLosses",
                "ctxt-1",
                self.group_relief_carried_forward,
            ),
            (
                "Profits chargeable to Corporation Tax",
                "ct-comp:TotalProfitsChargeableToCorporationTax",
                "ctxt-3",
                self.profits_chargeable_to_corporation_tax,
            ),
        ];
        for (label, tag, ctx, val) in fields {
            out.push_str("<div class=\"row\"><div class=\"cell\"><span class=\"label\">");
            out.push_str(label);
            out.push_str("</span></div><div class=\"cell\"><span class=\"value\">");
            self.ix_non_fraction(out, tag, ctx, "iso4217:GBP", *val);
            out.push_str("</span></div></div>");
        }
        out.push_str("</div>");
    }

    fn write_losses_page(&self, out: &mut String) {
        out.push_str("<div class=\"page\"><div class=\"page .header\"><h2>Losses</h2></div>");
        self.ix_non_fraction(
            out,
            "ct-comp:TradingLossesOfThisOrLaterAP",
            "ctxt-3",
            "iso4217:GBP",
            self.losses_of_trades_uk,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:LossesFromMiscellaneousTransactions",
            "ctxt-1",
            "iso4217:GBP",
            self.losses_from_miscellaneous,
        );
        out.push_str("</div>");
    }

    fn write_tax_chargeable_page(&self, out: &mut String) {
        out.push_str(
            "<div class=\"page\"><div class=\"page .header\"><h2>Tax chargeable</h2></div>",
        );

        self.ix_non_numeric(
            out,
            "ct-comp:FinancialYear1CoveredByTheReturn",
            "ctxt-1",
            &self.fy1_year.to_string(),
        );
        self.ix_non_numeric(
            out,
            "ct-comp:FinancialYear2CoveredByTheReturn",
            "ctxt-1",
            &self.fy2_year.to_string(),
        );
        self.ix_non_fraction(
            out,
            "ct-comp:FY1AmountOfProfitChargeableAtFirstRate",
            "ctxt-3",
            "iso4217:GBP",
            self.fy1_profit,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:FY2AmountOfProfitChargeableAtFirstRate",
            "ctxt-3",
            "iso4217:GBP",
            self.fy2_profit,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:FY1FirstRateOfTax",
            "ctxt-1",
            "iso4217:GBP",
            self.fy1_rate,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:FY2FirstRateOfTax",
            "ctxt-1",
            "iso4217:GBP",
            self.fy2_rate,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:FY1TaxAtFirstRate",
            "ctxt-3",
            "iso4217:GBP",
            self.fy1_tax,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:FY2TaxAtFirstRate",
            "ctxt-3",
            "iso4217:GBP",
            self.fy2_tax,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:CorporationTaxChargeable",
            "ctxt-3",
            "iso4217:GBP",
            self.corporation_tax_chargeable,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:MarginalRateReliefForRingFenceTradesPayable",
            "ctxt-1",
            "iso4217:GBP",
            self.marginal_relief,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:CorporationTaxChargeablePayable",
            "ctxt-3",
            "iso4217:GBP",
            self.corporation_tax_chargeable_payable,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:TotalReliefsAndDeductionsInTermsOfTaxPayable",
            "ctxt-1",
            "iso4217:GBP",
            self.total_reliefs_deductions_tax,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:NetCorporationTaxPayable",
            "ctxt-3",
            "iso4217:GBP",
            self.net_corporation_tax_payable,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:TaxChargeable",
            "ctxt-3",
            "iso4217:GBP",
            self.tax_chargeable,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:TaxPayable",
            "ctxt-3",
            "iso4217:GBP",
            self.tax_payable,
        );

        out.push_str("</div>");
    }

    fn write_rnd_page(&self, out: &mut String) {
        out.push_str("<div class=\"page\"><div class=\"page .header\"><h2>R&D / Creative enhanced expenditure</h2></div>");
        self.ix_non_numeric(
            out,
            "ct-comp:CompanyIsAPartnerInAFirm",
            "ctxt-1",
            &self.is_sme.to_string(),
        );
        self.ix_non_numeric(
            out,
            "ct-comp:CompanyIsAPartnerInAFirm",
            "ctxt-1",
            &self.is_large_company.to_string(),
        );
        self.ix_non_fraction(
            out,
            "ct-comp:SubsidisedQualifyingExpenditureOnIn-HouseDirectRD",
            "ctxt-4",
            "iso4217:GBP",
            self.rnd_qualifying_expenditure,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
            "ctxt-4",
            "iso4217:GBP",
            self.rnd_enhanced_expenditure,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:AdjustmentsCreativeProductionCompanyAdjustment",
            "ctxt-5",
            "iso4217:GBP",
            self.creative_enhanced_expenditure,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
            "ctxt-4",
            "iso4217:GBP",
            self.rnd_creative_enhanced_total,
        );
        self.ix_non_fraction(
            out,
            "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
            "ctxt-8",
            "iso4217:GBP",
            self.rnd_subcontracted_large,
        );
        out.push_str("</div>");
    }
}

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
        let ct = CorporationTaxReturn::from_gnucash(
            &gnucash,
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap(),
            "Example Biz Ltd.",
            "8596148860",
            "12345678",
        );

        assert_eq!(ct.company_name, "Example Biz Ltd.");
        assert_eq!(ct.tax_reference, "8596148860");
        assert_eq!(ct.company_number, "12345678");
        assert_eq!(ct.fy1_year, 2019);
        assert_eq!(ct.fy2_year, 2020);

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
}
