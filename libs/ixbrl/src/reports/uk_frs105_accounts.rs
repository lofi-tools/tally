//! Unaudited micro-entity accounts (FRS 105).
//!
//! Maps a [`GnucashBook`] (the ledger) plus company details (a [`Company`]
//! and an [`AccountsMetadata`]) to the "Unaudited Micro-Entity Accounts"
//! iXBRL document: a title page, a company-information page, the statement
//! of financial position and the notes to the accounts.
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
//! For the example company (`example_data/example2/input.gnucash`) the
//! rendered output matches the reference fixture
//! `example_data/example2/output-accounts.html` byte for byte (after
//! stripping the reference's random element ids).

use std::collections::HashMap;

use chrono::Datelike;

use crate::company::Company;
use crate::ixbrl_fmt::*;
use crate::GnucashBook;

/// Company details required by the accounts report that are not part of
/// [`Company`] (directors, addresses, accountant/auditor, VAT number, SIC
/// codes, employees, report dates, taxonomy dimension values, ...), plus
/// the base64-encoded logo and signature assets used on the report pages.
///
/// Deserialisable from the example company's data file
/// (`example_data/example2/input-company.json`), which the tests load as
/// the single source of truth.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AccountsMetadata {
    /// Names of the directors, in order (used for the officer contexts).
    pub directors: Vec<String>,
    /// Contact department / person name.
    pub contact_name: String,
    /// Registered-office address lines (one fact per line).
    pub address_lines: Vec<String>,
    /// County / region of the registered office.
    pub county: String,
    /// City / town of the registered office.
    pub location: String,
    /// Registered-office postcode.
    pub postcode: String,
    /// Contact e-mail address.
    pub email: String,
    /// Telephone country code (e.g. "+44").
    pub phone_country: String,
    /// Telephone area code.
    pub phone_area: String,
    /// Telephone local number.
    pub phone_number: String,
    /// Website main page URL.
    pub website_url: String,
    /// Website description.
    pub website_description: String,
    /// VAT registration number.
    pub vat_registration: String,
    /// SIC codes registered with Companies House.
    pub sic_codes: Vec<String>,
    /// Summary of business activities.
    pub activities: String,
    /// Average monthly number of employees, indexed by calendar year
    /// (e.g. `"2020" -> 2`, `"2019" -> 1`; JSON object keys are strings).
    pub average_employees: HashMap<String, u32>,
    /// Jurisdiction, e.g. "England and Wales".
    pub jurisdiction: String,
    /// Accountant's name.
    pub accountant_name: String,
    /// Accountant's firm.
    pub accountant_business: String,
    /// Accountant's office address.
    pub accountant_address: String,
    /// Auditor's name.
    pub auditor_name: String,
    /// Auditor's firm.
    pub auditor_business: String,
    /// Auditor's office address.
    pub auditor_address: String,
    /// Report title, shown on the title page.
    pub report_title: String,
    /// Date the report was published / issued.
    pub report_date: chrono::NaiveDate,
    /// Date the financial statements were authorised for issue.
    pub authorised_date: chrono::NaiveDate,
    /// Date of incorporation / formation.
    pub incorporation_date: chrono::NaiveDate,
    /// Name of the director who signed the report.
    pub signed_by: String,
    /// Taxonomy dimension value for the industry sector.
    pub industry_sector_dimension: String,
    /// Taxonomy dimension value for the accounting standards.
    pub accounting_standards_dimension: String,
    /// Taxonomy dimension value for the accounts type.
    pub accounts_type_dimension: String,
    /// Taxonomy dimension value for the accounts status.
    pub accounts_status_dimension: String,
    /// Taxonomy dimension value for the legal form.
    pub legal_form_dimension: String,
    /// Taxonomy dimension value for the country of formation.
    pub country_dimension: String,
    /// Taxonomy dimension value for the contact country.
    pub contact_country_dimension: String,
    /// Taxonomy dimension value for the phone-number type.
    pub phone_type_dimension: String,
    /// Base64-encoded company logo, embedded on the title page.
    pub logo_b64: String,
    /// Base64-encoded director's signature, embedded on the statement of
    /// financial position.
    pub signature_b64: String,
}

impl AccountsMetadata {
    /// The average monthly number of employees for a given calendar year
    /// (0 when the year is absent from the data).
    pub fn average_employees_for(&self, year: i32) -> u32 {
        self.average_employees.get(&year.to_string()).copied().unwrap_or(0)
    }
}

/// The unaudited micro-entity accounts (FRS 105) statement of financial
/// position.
///
/// Each balance-sheet value is stored as `[current_period, previous_period]`
/// in whole-pence precision; rounding to whole pounds happens at render time.
#[derive(Debug, Clone)]
pub struct Frs105Accounts {
    /// The company the accounts are prepared for.
    pub company: Company,
    /// Additional company details used by the report.
    pub metadata: AccountsMetadata,
    /// Tangible / fixed assets.
    pub fixed_assets: [f64; 2],
    /// Current assets (debtors + VAT refund due + bank).
    pub current_assets: [f64; 2],
    /// Prepayments and accrued income (always zero for this report).
    pub prepayments_and_accrued_income: [f64; 2],
    /// Creditors: amounts falling due within one year.
    pub creditors_within_1_year: [f64; 2],
    /// Net current assets / (liabilities).
    pub net_current_assets: [f64; 2],
    /// Total assets less current liabilities.
    pub total_assets_less_liabilities: [f64; 2],
    /// Creditors: amounts falling due after one year (always zero).
    pub creditors_after_1_year: [f64; 2],
    /// Provisions for liabilities (always zero).
    pub provisions_for_liabilities: [f64; 2],
    /// Accrued liabilities and deferred income (always zero).
    pub accruals_and_deferred_income: [f64; 2],
    /// Net assets.
    pub net_assets: [f64; 2],
    /// Capital and reserves (share capital + profit/loss + dividends +
    /// corporation tax).
    pub capital_and_reserves: [f64; 2],
}

impl Frs105Accounts {
    /// Compute the statement of financial position from the ledger and the
    /// company details.
    pub fn new(gnucash: &GnucashBook, company: &Company, metadata: &AccountsMetadata) -> Self {
        let accounts = gnucash.raw_accounts();

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
        for split in gnucash.raw_splits() {
            let tx = match gnucash.raw_transactions().iter().find(|t| t.guid == split.tx_guid) {
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
        let line = |acct: &str,
                    end: chrono::NaiveDate,
                    splits: &[(chrono::NaiveDate, String, f64)],
                    account_types: &HashMap<String, String>|
         -> f64 {
            let mut total = 0.0;
            for (date, path, val) in splits {
                if *date > end {
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

        // `at-end` balance-sheet dates for the current and previous period.
        // The previous period is the calendar year before the current one
        // (matching the reference metadata, e.g. 2019-12-31).
        let ends = [
            company.accounting_period_end,
            company.accounting_period_start - chrono::Duration::days(1),
        ];

        // Round to whole pence so computed values match the reference exactly.
        let round2 = |v: f64| (v * 100.0).round() / 100.0;

        let fixed_assets = ends.map(|end| line("Assets:Capital Equipment", end, &splits, &account_types));

        let debtors =
            ends.map(|end| line("Accounts Receivable", end, &splits, &account_types)
                + line("Assets:Owed To Us", end, &splits, &account_types));
        let vat_refund_due = ends.map(|end| {
            line("VAT:Input", end, &splits, &account_types)
                + line("VAT:Settlement:Input", end, &splits, &account_types)
                + line(
                    "Assets:VAT Repayments Due",
                    end,
                    &splits,
                    &account_types,
                )
        });
        let bank = ends.map(|end| line("Bank Accounts", end, &splits, &account_types));
        let current_assets = std::array::from_fn(|i| debtors[i] + vat_refund_due[i] + bank[i]);

        let prepayments_and_accrued_income = [0.0; 2];

        let trade_creditors =
            ends.map(|end| line("Accounts Payable", end, &splits, &account_types));
        let other_creditors = ends.map(|end| {
            line("VAT:Output", end, &splits, &account_types)
                + line("VAT:Settlement:Output", end, &splits, &account_types)
                + line("Liabilities:Credit Cards", end, &splits, &account_types)
                + line(
                    "Liabilities:Owed Corporation Tax",
                    end,
                    &splits,
                    &account_types,
                )
        });
        let creditors_within_1_year =
            std::array::from_fn(|i| trade_creditors[i] + other_creditors[i]);

        let net_current_assets = std::array::from_fn(|i| {
            current_assets[i] + prepayments_and_accrued_income[i] + creditors_within_1_year[i]
        });
        let total_assets_less_liabilities = std::array::from_fn(|i| {
            fixed_assets[i] + current_assets[i] + prepayments_and_accrued_income[i]
                + creditors_within_1_year[i]
        });

        let creditors_after_1_year = [0.0; 2];
        let provisions_for_liabilities = [0.0; 2];
        let accruals_and_deferred_income = [0.0; 2];

        let net_assets = std::array::from_fn(|i| {
            total_assets_less_liabilities[i]
                + creditors_after_1_year[i]
                + provisions_for_liabilities[i]
                + accruals_and_deferred_income[i]
        });

        let share_capital_equity =
            ends.map(|end| line("Equity:Shareholdings", end, &splits, &account_types));
        let profit_loss = ends.map(|end| {
            line("Income", end, &splits, &account_types)
                + line("Expenses", end, &splits, &account_types)
        });
        let dividends =
            ends.map(|end| line("Equity:Dividends", end, &splits, &account_types));
        let corporation_tax =
            ends.map(|end| line("Equity:Corporation Tax", end, &splits, &account_types));
        let capital_and_reserves = std::array::from_fn(|i| {
            share_capital_equity[i] + profit_loss[i] + dividends[i] + corporation_tax[i]
        });

        Frs105Accounts {
            company: company.clone(),
            metadata: metadata.clone(),
            fixed_assets: fixed_assets.map(round2),
            current_assets: current_assets.map(round2),
            prepayments_and_accrued_income,
            creditors_within_1_year: creditors_within_1_year.map(round2),
            net_current_assets: net_current_assets.map(round2),
            total_assets_less_liabilities: total_assets_less_liabilities.map(round2),
            creditors_after_1_year,
            provisions_for_liabilities,
            accruals_and_deferred_income,
            net_assets: net_assets.map(round2),
            capital_and_reserves: capital_and_reserves.map(round2),
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
        let metadata = &self.metadata;
        let period_start = company.accounting_period_start;
        let period_end = company.accounting_period_end;
        // The previous period is the calendar year before the current one
        // (e.g. 2019-01-01..2019-12-31), matching the reference metadata.
        let prev_end = period_start - chrono::Duration::days(1);
        let prev_start = chrono::NaiveDate::from_ymd_opt(prev_end.year(), 1, 1).unwrap();
        let current_year = period_end.year().to_string();
        let prev_year = prev_end.year().to_string();

        // -- ix:header ------------------------------------------------------

        let hidden = elt("ix:hidden", &[]).children(vec![
            non_numeric("uk-bus:ReportTitle", "ctxt-0", &metadata.report_title),
            non_numeric_fmt(
                "uk-bus:BusinessReportPublicationDate",
                "ctxt-1",
                &format_date(&metadata.report_date),
                "ixt2:datedaymonthyearen",
            ),
            non_numeric_fmt(
                "uk-core:DateAuthorisationFinancialStatementsForIssue",
                "ctxt-2",
                &format_date(&metadata.authorised_date),
                "ixt2:datedaymonthyearen",
            ),
            non_numeric_fmt(
                "uk-bus:StartDateForPeriodCoveredByReport",
                "ctxt-1",
                &format_date(&period_start),
                "ixt2:datedaymonthyearen",
            ),
            non_numeric_fmt(
                "uk-bus:EndDateForPeriodCoveredByReport",
                "ctxt-1",
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
            non_numeric("uk-bus:VATRegistrationNumber", "ctxt-0", &metadata.vat_registration),
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
            non_numeric(
                "uk-bus:DescriptionPrincipalActivities",
                "ctxt-0",
                &metadata.activities,
            ),
            non_numeric(
                "uk-bus:SICCodeRecordedUKCompaniesHouse1",
                "ctxt-0",
                metadata.sic_codes.first().map(String::as_str).unwrap_or(""),
            ),
            non_numeric(
                "uk-bus:SICCodeRecordedUKCompaniesHouse2",
                "ctxt-0",
                metadata.sic_codes.get(1).map(String::as_str).unwrap_or(""),
            ),
            non_numeric("uk-bus:MainIndustrySector", "ctxt-3", ""),
            non_numeric("uk-bus:EntityDormantTruefalse", "ctxt-0", "false"),
            non_numeric("uk-bus:EntityTradingStatus", "ctxt-0", ""),
            non_numeric("uk-bus:AccountingStandardsApplied", "ctxt-4", ""),
            non_numeric("uk-bus:AccountsType", "ctxt-5", ""),
            non_numeric("uk-bus:AccountsStatusAuditedOrUnaudited", "ctxt-6", ""),
            non_numeric("uk-bus:LegalFormEntity", "ctxt-7", ""),
            non_numeric("uk-bus:CountryFormationOrIncorporation", "ctxt-8", ""),
            non_numeric_fmt(
                "uk-bus:DateFormationOrIncorporation",
                "ctxt-1",
                &format_date(&metadata.incorporation_date),
                "ixt2:datedaymonthyearen",
            ),
            employees_non_fraction(
                "ctxt-0",
                &metadata.average_employees_for(period_end.year()).to_string(),
            ),
            employees_non_fraction(
                "ctxt-9",
                &metadata.average_employees_for(prev_end.year()).to_string(),
            ),
            non_numeric("uk-core:DirectorSigningFinancialStatements", "ctxt-11", ""),
            non_numeric(
                "uk-bus:NameContactDepartmentOrPerson",
                "ctxt-13",
                &metadata.contact_name,
            ),
            non_numeric(
                "uk-bus:AddressLine1",
                "ctxt-13",
                metadata.address_lines.first().map(String::as_str).unwrap_or(""),
            ),
            non_numeric(
                "uk-bus:AddressLine2",
                "ctxt-13",
                metadata.address_lines.get(1).map(String::as_str).unwrap_or(""),
            ),
            non_numeric(
                "uk-bus:PrincipalLocation-CityOrTown",
                "ctxt-13",
                &metadata.location,
            ),
            non_numeric("uk-bus:CountyRegion", "ctxt-13", &metadata.county),
            non_numeric("uk-bus:PostalCodeZip", "ctxt-13", &metadata.postcode),
            non_numeric("uk-bus:E-mailAddress", "ctxt-13", &metadata.email),
            non_numeric("uk-bus:CountryCode", "ctxt-14", &metadata.phone_country),
            non_numeric("uk-bus:AreaCode", "ctxt-14", &metadata.phone_area),
            non_numeric("uk-bus:LocalNumber", "ctxt-14", &metadata.phone_number),
            non_numeric("uk-bus:WebsiteMainPageURL", "ctxt-13", &metadata.website_url),
            non_numeric(
                "uk-bus:DescriptionOrOtherInformationOnWebsite",
                "ctxt-13",
                &metadata.website_description,
            ),
        ]);

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

        let resources = elt("ix:resources", &[]).children(vec![
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
                &metadata.report_date,
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
            context_duration_full(
                "ctxt-3",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-bus:MainIndustrySectorDimension",
                    &metadata.industry_sector_dimension,
                )],
            ),
            context_duration_full(
                "ctxt-4",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-bus:AccountingStandardsDimension",
                    &metadata.accounting_standards_dimension,
                )],
            ),
            context_duration_full(
                "ctxt-5",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-bus:AccountsTypeDimension",
                    &metadata.accounts_type_dimension,
                )],
            ),
            context_duration_full(
                "ctxt-6",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-bus:AccountsStatusDimension",
                    &metadata.accounts_status_dimension,
                )],
            ),
            context_duration_full(
                "ctxt-7",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-bus:LegalFormEntityDimension",
                    &metadata.legal_form_dimension,
                )],
            ),
            context_duration_full(
                "ctxt-8",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-geo:CountriesRegionsDimension",
                    &metadata.country_dimension,
                )],
            ),
            context_duration(
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
            context_duration_full(
                "ctxt-11",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[("uk-bus:EntityOfficersDimension", "uk-bus:Director2")],
            ),
            context_duration_full(
                "ctxt-12",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[("uk-bus:EntityOfficersDimension", "uk-bus:Director3")],
            ),
            context_duration_full(
                "ctxt-13",
                &company.company_number,
                &period_start,
                &period_end,
                None,
                None,
                &[(
                    "uk-geo:CountriesRegionsDimension",
                    &metadata.contact_country_dimension,
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
                    &metadata.phone_type_dimension,
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
                elt_text("title", &[], "Unaudited Micro-Entity Accounts"),
                elt_text(
                    "style",
                    &[("type", "text/css")],
                    include_str!("uk_frs105_accounts.css"),
                ),
            ]),
            elt("body", &[]).children(vec![
                elt("div", &[("class", "hidden")]).child(header),
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
    /// iXBRL (`tax_reference`, `registration_date` and the corporation-tax
    /// rate defaults); fields that *are* serialised (name, company number,
    /// accounting-period dates) are recovered from the document and override
    /// the supplied values.  Similarly, metadata fields that have no iXBRL
    /// fact (`jurisdiction`, `signed_by`) come back empty.
    ///
    /// Balance-sheet values are rendered at whole pounds (`decimals = 0`),
    /// so the round trip preserves them to the nearest pound; the sign is
    /// recovered from the enclosing cell's `negative` class.
    pub fn from_ixbrl_node(node: &XmlNode, company: &Company) -> Frs105Accounts {
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
        let parse_date = |raw: &str| -> chrono::NaiveDate {
            let cleaned = raw.replace('\u{00A0}', " ");
            chrono::NaiveDate::parse_from_str(&cleaned, "%d %B %Y")
                .unwrap_or(company.accounting_period_start)
        };

        // -- company ----------------------------------------------------------

        let period_start = parse_date(&text("uk-bus:StartDateForPeriodCoveredByReport"));
        let period_end = parse_date(&text("uk-bus:EndDateForPeriodCoveredByReport"));
        let prev_year = (period_start - chrono::Duration::days(1)).year();

        let company = Company {
            name: text("uk-bus:EntityCurrentLegalOrRegisteredName"),
            tax_reference: company.tax_reference.clone(), // not serialised
            company_number: text("uk-bus:UKCompaniesHouseRegisteredNumber"),
            registration_date: company.registration_date,
            accounting_period_start: period_start,
            accounting_period_end: period_end,
            fy1_year: company.fy1_year,
            fy2_year: company.fy2_year,
            fy1_rate: company.fy1_rate,
            fy2_rate: company.fy2_rate,
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

        let metadata = AccountsMetadata {
            directors,
            contact_name: text("uk-bus:NameContactDepartmentOrPerson"),
            address_lines,
            county: text("uk-bus:CountyRegion"),
            location: text("uk-bus:PrincipalLocation-CityOrTown"),
            postcode: text("uk-bus:PostalCodeZip"),
            email: text("uk-bus:E-mailAddress"),
            phone_country: text("uk-bus:CountryCode"),
            phone_area: text("uk-bus:AreaCode"),
            phone_number: text("uk-bus:LocalNumber"),
            website_url: text("uk-bus:WebsiteMainPageURL"),
            website_description: text("uk-bus:DescriptionOrOtherInformationOnWebsite"),
            vat_registration: text("uk-bus:VATRegistrationNumber"),
            sic_codes,
            activities: text("uk-bus:DescriptionPrincipalActivities"),
            average_employees: HashMap::from([
                (period_end.year().to_string(), num("uk-core:AverageNumberEmployeesDuringPeriod", "ctxt-0") as u32),
                (prev_year.to_string(), num("uk-core:AverageNumberEmployeesDuringPeriod", "ctxt-9") as u32),
            ]),
            jurisdiction: String::new(), // not serialised to iXBRL
            accountant_name: text("uk-accrep:NameAccountantResponsible"),
            accountant_business: text("uk-bus:NameEntityAccountants"),
            accountant_address: text("uk-accrep:NameOrLocationAccountantsOffice"),
            auditor_name: text("uk-aurep:NameIndividualAuditor"),
            auditor_business: text("uk-bus:NameEntityAuditors"),
            auditor_address: text("uk-aurep:NameOrLocationOfficePerformingAudit"),
            report_title: text("uk-bus:ReportTitle"),
            report_date: parse_date(&text("uk-bus:BusinessReportPublicationDate")),
            authorised_date: parse_date(&text("uk-core:DateAuthorisationFinancialStatementsForIssue")),
            incorporation_date: parse_date(&text("uk-bus:DateFormationOrIncorporation")),
            signed_by: String::new(), // not serialised to iXBRL
            industry_sector_dimension: dim("ctxt-3", "uk-bus:MainIndustrySectorDimension"),
            accounting_standards_dimension: dim("ctxt-4", "uk-bus:AccountingStandardsDimension"),
            accounts_type_dimension: dim("ctxt-5", "uk-bus:AccountsTypeDimension"),
            accounts_status_dimension: dim("ctxt-6", "uk-bus:AccountsStatusDimension"),
            legal_form_dimension: dim("ctxt-7", "uk-bus:LegalFormEntityDimension"),
            country_dimension: dim("ctxt-8", "uk-geo:CountriesRegionsDimension"),
            contact_country_dimension: dim("ctxt-13", "uk-geo:CountriesRegionsDimension"),
            phone_type_dimension: dim("ctxt-14", "uk-bus:PhoneNumberTypeDimension"),
            logo_b64: imgs.get("Company logo").cloned().unwrap_or_default(),
            signature_b64: imgs.get("Director's signature").cloned().unwrap_or_default(),
        };

        Frs105Accounts {
            company,
            metadata,
            fixed_assets: [
                fact("uk-core:FixedAssets", "ctxt-15"),
                fact("uk-core:FixedAssets", "ctxt-16"),
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

    /// Deserialise a [`Frs105Accounts`] from its serialised iXBRL HTML, in
    /// two steps: first into the [`XmlNode`] intermediate representation,
    /// then into the struct.
    pub fn from_ixbrl(html: &str, company: &Company) -> Result<Frs105Accounts, String> {
        let node = XmlNode::from_xml_string(html)?;
        Ok(Self::from_ixbrl_node(&node, company))
    }

    // -- Page builders --------------------------------------------------------

    /// Title page: company number, logo, company name, report title and the
    /// period end date.
    fn build_title_page(&self) -> XmlNode {
        let company = &self.company;
        let metadata = &self.metadata;
        page(vec![div("titlepage", vec![
            div("company-number", vec![span(vec![
                span_text(" Company registration no. "),
                span(vec![non_numeric(
                    "uk-bus:UKCompaniesHouseRegisteredNumber",
                    "ctxt-0",
                    &company.company_number,
                )]),
                span_text(" ("),
                span(vec![span_text(&metadata.jurisdiction)]),
                span_text(")"),
            ])]),
            elt(
                "img",
                &[
                    ("alt", "Company logo"),
                    ("src", &format!("data:image/png;base64,{}", &metadata.logo_b64)),
                ],
            ),
            div("company-name", vec![span(vec![span(vec![non_numeric(
                "uk-bus:EntityCurrentLegalOrRegisteredName",
                "ctxt-0",
                &company.name,
            )])])]),
            div("title", vec![span(vec![span(vec![non_numeric(
                "uk-bus:ReportTitle",
                "ctxt-0",
                &metadata.report_title,
            )])])]),
            div("subtitle", vec![span(vec![
                span_text("For the year ended "),
                span(vec![date_fact(
                    "uk-bus:EndDateForPeriodCoveredByReport",
                    "ctxt-1",
                    &company.accounting_period_end,
                )]),
            ])]),
        ])])
    }

    /// Company-information page: header, then a table of directors, company
    /// number, registered office, accountant and auditor.
    fn build_company_info_page(&self) -> XmlNode {
        let company = &self.company;
        let metadata = &self.metadata;

        let directors_cell = td_no_class(
            metadata
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
            span(vec![span_text(&metadata.jurisdiction)]),
        ])]);

        let office_children: Vec<XmlNode> = metadata
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
                        &metadata.location,
                    ),
                    span_text(", "),
                ]),
                span_text(" "),
                span(vec![non_numeric(
                    "uk-bus:PostalCodeZip",
                    "ctxt-13",
                    &metadata.postcode,
                )]),
            ])))
            .collect();
        let registered_office_cell = td_no_class(office_children);

        let accountant_cell = td_no_class(vec![
            span(vec![span(vec![non_numeric(
                "uk-accrep:NameAccountantResponsible",
                "ctxt-0",
                &metadata.accountant_name,
            )])]),
            el("br"),
            span(vec![span(vec![non_numeric(
                "uk-bus:NameEntityAccountants",
                "ctxt-0",
                &metadata.accountant_business,
            )])]),
            el("br"),
            span(vec![span(vec![non_numeric(
                "uk-accrep:NameOrLocationAccountantsOffice",
                "ctxt-0",
                &metadata.accountant_address,
            )])]),
        ]);

        let auditor_cell = td_no_class(vec![
            span(vec![span(vec![non_numeric(
                "uk-aurep:NameIndividualAuditor",
                "ctxt-0",
                &metadata.auditor_name,
            )])]),
            el("br"),
            span(vec![span(vec![non_numeric(
                "uk-bus:NameEntityAuditors",
                "ctxt-0",
                &metadata.auditor_business,
            )])]),
            el("br"),
            span(vec![span(vec![non_numeric(
                "uk-aurep:NameOrLocationOfficePerformingAudit",
                "ctxt-0",
                &metadata.auditor_address,
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
        let rows = vec![
            worksheet_header_row_accts(current_year, prev_year),
            worksheet_currency_row_accts(),
            spacer_row(),
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
        ];

        let notes = elt("div", &[]).child(elt("div", &[]).children(vec![
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
                        &self.company.accounting_period_end,
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

        let approval = elt("div", &[]).child(elt("div", &[]).children(vec![
            elt("p", &[]).child(span(vec![
                span_text("Approved by the board of directors and authorised for publication on "),
                span(vec![date_fact(
                    "uk-core:DateAuthorisationFinancialStatementsForIssue",
                    "ctxt-2",
                    &self.metadata.authorised_date,
                )]),
                span_text("."),
            ])),
            elt("p", &[]).child(span(vec![
                span_text("Signed on behalf of the board, by "),
                span(vec![span_text(&self.metadata.signed_by)]),
                span_text("."),
            ])),
            elt(
                "img",
                &[
                    ("alt", "Director's signature"),
                    ("src", &format!(
                        "data:image/png;base64,{}",
                        &self.metadata.signature_b64
                    )),
                ],
            ),
        ]));

        page(vec![elt("div", &[]).children(vec![
            self.page_header("Statement of financial position", HeaderSubtitle::AsAt),
            worksheet(vec![table("sheet table", rows)]),
            notes,
            approval,
        ])])
    }

    /// Notes to the accounts: company-information note and employees note.
    fn build_notes_page(&self, current_year: &str, prev_year: &str) -> XmlNode {
        let company = &self.company;
        let metadata = &self.metadata;
        // Employee figures are indexed by calendar year in the metadata.
        let employees_cur_year = company.accounting_period_end.year();
        let employees_prev_year =
            (company.accounting_period_start - chrono::Duration::days(1)).year();

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
                        metadata.address_lines.first().map(String::as_str).unwrap_or(""),
                    ),
                    span_text(", "),
                ]),
                span_text(" "),
                span(vec![
                    non_numeric(
                        "uk-bus:AddressLine2",
                        "ctxt-13",
                        metadata.address_lines.get(1).map(String::as_str).unwrap_or(""),
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
                        &metadata.location,
                    ),
                    span_text(" "),
                ]),
                span_text(" "),
                span(vec![non_numeric(
                    "uk-bus:PostalCodeZip",
                    "ctxt-13",
                    &metadata.postcode,
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
                            &metadata.average_employees_for(employees_cur_year).to_string(),
                        ),
                    ])])),
                    elt("td", &[("class", "data value")]).child(span(vec![span(vec![
                        employees_non_fraction(
                            "ctxt-9",
                            &metadata.average_employees_for(employees_prev_year).to_string(),
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
                    &company.accounting_period_end,
                )]),
            ]),
            HeaderSubtitle::AsAt => span(vec![
                span_text("As at "),
                span(vec![date_fact(
                    "uk-bus:BalanceSheetDate",
                    "ctxt-2",
                    &company.accounting_period_end,
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
    use crate::test_utils::TestData;

    async fn load_example() -> (Company, GnucashBook) {
        let company = example_company();
        let gnucash = GnucashBook::try_from_gnucash_file(
            TestData::accounts_path(&company.company_number)
                .expect("example company accounts path"),
        )
        .await
        .expect("open gnucash");
        (company, gnucash)
    }

    /// The example company's identity fields from the JSON, mirroring the
    /// constructor arguments of [`Company::new`]; the remaining JSON keys
    /// are flattened into [`AccountsMetadata`].
    #[derive(serde::Deserialize)]
    struct CompanyData {
        name: String,
        tax_reference: String,
        company_number: String,
        accounting_period_start: chrono::NaiveDate,
        accounting_period_end: chrono::NaiveDate,
    }

    /// The top-level shape of `input-company.json`: a nested `company`
    /// identity block plus the flat metadata fields (incl. logo/signature).
    #[derive(serde::Deserialize)]
    struct ExampleCompanyData {
        company: CompanyData,
        #[serde(flatten)]
        metadata: AccountsMetadata,
    }

    /// Load the example company's data file — the single source of truth for
    /// the company identity, the report metadata and the logo/signature
    /// assets.
    fn load_example_data() -> ExampleCompanyData {
        let json = std::fs::read_to_string("example_data/example2/input-company.json")
            .expect("read example company data file");
        serde_json::from_str(&json).expect("parse example company data file")
    }

    /// The example [`Company`] (identity + accounting period) from the JSON.
    fn example_company() -> Company {
        let data = load_example_data().company;
        Company::new(
            data.name,
            data.tax_reference,
            data.company_number,
            data.accounting_period_start,
            data.accounting_period_end,
        )
    }

    /// The example report metadata (incl. logo/signature) from the JSON.
    fn example_metadata() -> AccountsMetadata {
        load_example_data().metadata
    }

    #[test]
    fn test_example_company_data_from_json() {
        // Company identity round-trips from the JSON.
        let company = example_company();
        assert_eq!(company.name, "Example Biz Ltd.");
        assert_eq!(company.company_number, "12345678");
        assert_eq!(company.tax_reference, "8596148860");
        assert_eq!(
            company.accounting_period_start,
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()
        );
        assert_eq!(
            company.accounting_period_end,
            chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()
        );

        // Must stay in sync with the hardcoded TestData company used by the
        // corp-tax tests.
        let t = TestData::default_company();
        assert_eq!(company.name, t.name);
        assert_eq!(company.company_number, t.company_number);
        assert_eq!(company.tax_reference, t.tax_reference);
        assert_eq!(company.accounting_period_start, t.accounting_period_start);
        assert_eq!(company.accounting_period_end, t.accounting_period_end);

        // Metadata round-trips from the same file.
        let m = example_metadata();
        assert_eq!(m.directors, vec!["A Bloggs", "B Smith", "C Jones"]);
        assert_eq!(m.sic_codes, vec!["62020", "62021"]);
        assert_eq!(
            m.average_employees,
            HashMap::from([("2020".to_string(), 2), ("2019".to_string(), 1)])
        );
        assert_eq!(
            m.report_date,
            chrono::NaiveDate::from_ymd_opt(2021, 3, 1).unwrap()
        );
        assert_eq!(
            m.incorporation_date,
            chrono::NaiveDate::from_ymd_opt(2017, 4, 5).unwrap()
        );
        assert!(!m.logo_b64.is_empty());
        assert!(!m.signature_b64.is_empty());
    }

    #[tokio::test]
    async fn test_accounts_from_example2() {
        let (company, gnucash) = load_example().await;
        let accounts = Frs105Accounts::new(&gnucash, &company, &example_metadata());

        // Balance-sheet values (whole-pence, computed from the ledger).
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
    async fn test_accounts_output_matches_reference_fixture() {
        // Regenerate the fixture with:
        //   nix run .#racc-gnucash   # -> .cache/accts-micro-gnucash.html
        // then strip the reference's random `id="elt-*"` attributes and
        // copy to example_data/example2/output-accounts.html.  The Rust
        // output below must match it byte for byte.
        let (company, gnucash) = load_example().await;
        let accounts = Frs105Accounts::new(&gnucash, &company, &example_metadata());
        let out = accounts.to_ixbrl();

        // Write the Rust output for external validation (arelle).
        std::fs::create_dir_all("../../.cache").unwrap();
        std::fs::write("../../.cache/accts-micro-rust.html", &out).unwrap();

        let expected =
            std::fs::read_to_string("example_data/example2/output-accounts.html")
                .expect("read reference fixture");
        assert_eq!(
            out, expected,
            "accounts output must match the reference fixture"
        );
    }

    #[tokio::test]
    async fn test_accounts_ixbrl_structure() {
        let (company, gnucash) = load_example().await;
        let out = Frs105Accounts::new(&gnucash, &company, &example_metadata()).to_ixbrl();

        // Header structure
        assert!(out.contains("<div class=\"hidden\"><ix:header><ix:hidden>"));
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

    #[tokio::test]
    async fn test_accounts_ixbrl_round_trip() {
        // Serialise, write the output to .cache, then deserialise in two
        // steps (XML -> XmlNode -> Frs105Accounts) and compare against the
        // original.
        let (company, gnucash) = load_example().await;
        let accounts = Frs105Accounts::new(&gnucash, &company, &example_metadata());
        let html = accounts.to_ixbrl();

        std::fs::create_dir_all("../../.cache").unwrap();
        std::fs::write("../../.cache/accts-micro-roundtrip.html", &html).unwrap();

        let node = XmlNode::from_xml_string(&html).expect("parse ixbrl");
        let back = Frs105Accounts::from_ixbrl_node(&node, &company);

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
            back.company.accounting_period_start,
            accounts.company.accounting_period_start
        );
        assert_eq!(
            back.company.accounting_period_end,
            accounts.company.accounting_period_end
        );

        // Metadata fields that are serialised to iXBRL round-trip.
        assert_eq!(back.metadata.directors, accounts.metadata.directors);
        assert_eq!(
            back.metadata.contact_name,
            accounts.metadata.contact_name
        );
        assert_eq!(
            back.metadata.address_lines,
            accounts.metadata.address_lines
        );
        assert_eq!(back.metadata.county, accounts.metadata.county);
        assert_eq!(back.metadata.location, accounts.metadata.location);
        assert_eq!(back.metadata.postcode, accounts.metadata.postcode);
        assert_eq!(back.metadata.email, accounts.metadata.email);
        assert_eq!(back.metadata.phone_country, accounts.metadata.phone_country);
        assert_eq!(back.metadata.phone_area, accounts.metadata.phone_area);
        assert_eq!(back.metadata.phone_number, accounts.metadata.phone_number);
        assert_eq!(back.metadata.website_url, accounts.metadata.website_url);
        assert_eq!(
            back.metadata.website_description,
            accounts.metadata.website_description
        );
        assert_eq!(
            back.metadata.vat_registration,
            accounts.metadata.vat_registration
        );
        assert_eq!(back.metadata.sic_codes, accounts.metadata.sic_codes);
        assert_eq!(back.metadata.activities, accounts.metadata.activities);
        assert_eq!(
            back.metadata.average_employees,
            accounts.metadata.average_employees
        );
        assert_eq!(
            back.metadata.accountant_name,
            accounts.metadata.accountant_name
        );
        assert_eq!(
            back.metadata.accountant_business,
            accounts.metadata.accountant_business
        );
        assert_eq!(
            back.metadata.accountant_address,
            accounts.metadata.accountant_address
        );
        assert_eq!(back.metadata.auditor_name, accounts.metadata.auditor_name);
        assert_eq!(
            back.metadata.auditor_business,
            accounts.metadata.auditor_business
        );
        assert_eq!(
            back.metadata.auditor_address,
            accounts.metadata.auditor_address
        );
        assert_eq!(back.metadata.report_title, accounts.metadata.report_title);
        assert_eq!(back.metadata.report_date, accounts.metadata.report_date);
        assert_eq!(
            back.metadata.authorised_date,
            accounts.metadata.authorised_date
        );
        assert_eq!(
            back.metadata.incorporation_date,
            accounts.metadata.incorporation_date
        );
        assert_eq!(
            back.metadata.industry_sector_dimension,
            accounts.metadata.industry_sector_dimension
        );
        assert_eq!(
            back.metadata.accounting_standards_dimension,
            accounts.metadata.accounting_standards_dimension
        );
        assert_eq!(
            back.metadata.accounts_type_dimension,
            accounts.metadata.accounts_type_dimension
        );
        assert_eq!(
            back.metadata.accounts_status_dimension,
            accounts.metadata.accounts_status_dimension
        );
        assert_eq!(
            back.metadata.legal_form_dimension,
            accounts.metadata.legal_form_dimension
        );
        assert_eq!(
            back.metadata.country_dimension,
            accounts.metadata.country_dimension
        );
        assert_eq!(
            back.metadata.contact_country_dimension,
            accounts.metadata.contact_country_dimension
        );
        assert_eq!(
            back.metadata.phone_type_dimension,
            accounts.metadata.phone_type_dimension
        );
        assert_eq!(back.metadata.logo_b64, accounts.metadata.logo_b64);
        assert_eq!(
            back.metadata.signature_b64,
            accounts.metadata.signature_b64
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
    fn test_format_date() {
        let d = chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap();
        assert_eq!(format_date(&d), "31\u{00A0}December\u{00A0}2020");
    }
}
