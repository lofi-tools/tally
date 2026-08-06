//! CT600 corporation-tax return model.
//!
//! [`Ct600Return`] holds every field of the CT600 filing message: the GovTalk
//! envelope, the `ct:` IR envelope (header + principal contact), and the
//! company tax return with the calculated figures and the attached iXBRL
//! documents.
//!
//! The message is built from the computed iXBRL inputs
//! ([`Frs105Accounts`] + [`Frs105CorpTax`]) via [`Ct600Return::from_inputs`]
//! and serialised through the [`XmlNode`] intermediate representation the
//! `ixbrl` crate uses for its own reports, to the same XML shape as the
//! reference `ct600` tool's `--output-ct` message — plus the three
//! schema-required elements that message omits (the IRmark `Type` attribute,
//! the Declaration `Name`/`Status`, and the PaymentToPerson
//! `Recipient`/`NomineeReference`), so the output validates against the CT
//! schema.  The message round-trips back into the typed struct via
//! [`Ct600Return::from_xml`] (through the same [`XmlNode`] IR).

use base64::{engine::general_purpose, Engine as _};
use chrono::{Local, NaiveDate, NaiveDateTime};
use ixbrl::ixbrl_fmt::{el, elt, elt_text, XmlNode};
use ixbrl::reports::uk_frs105_accounts::Frs105Accounts;
use ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax;

use crate::form::Ct600FormValues;
use crate::{CT_NS, ENV_NS};

// ============================================================================
// Typed model
// ============================================================================

/// GovTalk envelope parameters (the `config.json` credentials in the
/// reference tool).  [`EnvelopeConfig::default`] mirrors the reference
/// `config.json` shipped with the `ct600` tool.
#[derive(Debug, Clone)]
pub struct EnvelopeConfig {
    /// `Class` — message class, e.g. `HMRC-CT-CT600`.
    pub class: String,
    /// `Qualifier` — e.g. `request`.
    pub qualifier: String,
    /// `Function` — e.g. `submit`.
    pub function: String,
    /// `GatewayTest` — `"1"` for the test gateway.
    pub gateway_test: String,
    /// `SenderID` — the HMRC gateway username.
    pub username: String,
    /// `Authentication` `Value` — the gateway password.
    pub password: String,
    /// `Channel` `URI` — the vendor id.
    pub vendor_id: String,
    /// `Channel` `Product` — the software product name.
    pub software: String,
    /// `Channel` `Version` — the software version.
    pub software_version: String,
    /// `ChannelRouting` `Timestamp`.
    pub timestamp: NaiveDateTime,
}

impl Default for EnvelopeConfig {
    fn default() -> Self {
        Self {
            class: "HMRC-CT-CT600".to_string(),
            qualifier: "request".to_string(),
            function: "submit".to_string(),
            gateway_test: "1".to_string(),
            username: "CTUser100".to_string(),
            password: "password".to_string(),
            vendor_id: "1234".to_string(),
            software: "ct600".to_string(),
            software_version: "1.0.0".to_string(),
            timestamp: NaiveDate::from_ymd_opt(2022, 3, 31)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        }
    }
}

/// Contact details used in the IR header `<ct:Principal><ct:Contact>` block.
#[derive(Debug, Clone)]
pub struct Contact {
    /// Honorific / title, e.g. `Ms`.
    pub title: String,
    /// First name.
    pub first_name: String,
    /// Surname.
    pub second_name: String,
    /// E-mail address.
    pub email: String,
    /// Telephone number.
    pub telephone: String,
}

impl Default for Contact {
    /// Mirrors the reference `config.json` contact block.
    fn default() -> Self {
        Self {
            title: "Ms".to_string(),
            first_name: "Sarah".to_string(),
            second_name: "McAcre".to_string(),
            email: "sarah@example.org".to_string(),
            telephone: "447900123456".to_string(),
        }
    }
}

/// Company information block (`ct:CompanyInformation`).
#[derive(Debug, Clone)]
pub struct CompanyInformation {
    /// Company name.
    pub company_name: String,
    /// Companies House registration number.
    pub registration_number: String,
    /// Corporation tax reference (UTR).
    pub reference: String,
    /// Type of company (box 4).
    pub company_type: u8,
    /// Start of the period covered by the return (box 30).
    pub period_start: NaiveDate,
    /// End of the period covered by the return (box 35).
    pub period_end: NaiveDate,
}

/// Return info summary (`ct:ReturnInfoSummary`): which attachments are filed.
#[derive(Debug, Clone)]
pub struct ReturnInfoSummary {
    /// `ct:Accounts` `ct:ThisPeriodAccounts` (box 80).
    pub this_period_accounts: bool,
    /// `ct:Computations` `ct:ThisPeriodComputations` (also box 80).
    pub this_period_computations: bool,
}

/// One financial year of the corporation-tax calculation.
#[derive(Debug, Clone)]
pub struct FinancialYear {
    /// The year covered (box 330 / 380).
    pub year: i32,
    /// Profit chargeable at the first rate (box 335 / 385).
    pub profit: f64,
    /// Rate of tax (box 340 / 390).
    pub tax_rate: f64,
    /// Tax at the first rate (box 345 / 395).
    pub tax: f64,
}

/// Declaration boxes.
///
/// The CT schema requires `Name` and `Status`, so
/// [`Ct600Return::from_inputs`] fills them with the contact's name and
/// `Director`; `date` defaults to today.  [`Ct600Return::with_declaration`]
/// overrides name / status.
#[derive(Debug, Clone)]
pub struct Declaration {
    /// Box 975 — name of the person signing.
    pub name: Option<String>,
    /// Box 980 — date signed (defaults to today).
    pub date: Option<NaiveDate>,
    /// Box 985 — status, e.g. `Director`.
    pub status: Option<String>,
}

/// The full CT600 corporation-tax return: GovTalk envelope + IR envelope.
///
/// Field values mirror the boxes computed by [`Ct600FormValues`]; the
/// serialised XML matches the reference `ct600` tool's `--output-ct` output
/// element-for-element, with the three schema-required elements the
/// reference omits (see [`Ct600Return::to_xml`]).
#[derive(Debug, Clone)]
pub struct Ct600Return {
    /// GovTalk envelope parameters.
    pub envelope: EnvelopeConfig,
    /// Principal contact in the IR header.
    pub contact: Contact,
    /// IR header `<ct:Sender>` (e.g. `Company`).
    pub sender: String,
    /// Company information block.
    pub company: CompanyInformation,
    /// Accounts / computations attachment summary.
    pub return_info: ReturnInfoSummary,
    /// Box 145 — total turnover from trade.
    pub turnover: f64,
    /// Box 155 — trading profits (mirrors [`Ct600FormValues::trading_profits`], which
    /// derives from the net trading profits).
    pub trading_profits: f64,
    /// Box 165 — net trading profits.
    pub net_trading_profits: f64,
    /// Box 235 — profits before other deductions and reliefs.
    pub profits_before_other_deductions: f64,
    /// Box 300 — profits before qualifying donations and group relief.
    pub profits_before_charges_and_group_relief: f64,
    /// Box 315 — profits chargeable to Corporation Tax.
    pub chargeable_profits: f64,
    /// Financial year 1 details.
    pub fy1: FinancialYear,
    /// Financial year 2 details.
    pub fy2: FinancialYear,
    /// Box 430 — Corporation Tax.
    pub corporation_tax: f64,
    /// Box 440 — Corporation Tax chargeable.
    pub net_corporation_tax_chargeable: f64,
    /// Box 475 — net Corporation Tax liability.
    pub net_corporation_tax_liability: f64,
    /// Box 510 — tax chargeable.
    pub tax_chargeable: f64,
    /// Box 525 — self-assessment of tax payable.
    pub tax_payable: f64,
    /// Box 528 — self-assessment of tax payable incl. restitution tax.
    pub tax_payable_including_restitution_tax: f64,
    /// Box 650 — SME R&amp;D claim made.
    pub sme_claim: bool,
    /// Box 660 — R&amp;D enhanced expenditure.
    pub rnd_enhanced_expenditure: Option<f64>,
    /// Box 670 — R&amp;D and creative enhanced expenditure.
    pub rnd_and_creative_enhanced_expenditure: Option<f64>,
    /// Box 690 — annual investment allowance.
    pub aia_capital_allowances: f64,
    /// Repayment address lines (`ct:PaymentToPerson` `ct:Address` `ct:Line`).
    pub payment_address_lines: Vec<String>,
    /// Repayment recipient name (`ct:PaymentToPerson` `ct:Recipient`).
    pub payment_recipient: String,
    /// Repayment reference (`ct:PaymentToPerson` `ct:NomineeReference`).
    pub payment_nominee_reference: String,
    /// Declaration boxes (975 / 980 / 985).
    pub declaration: Declaration,
    /// Raw computation iXBRL document, base64-encoded in the message.
    pub computation_document: Option<String>,
    /// Raw accounts iXBRL document, base64-encoded in the message.
    pub accounts_document: Option<String>,
}

impl Ct600Return {
    /// The corporation tax reference (UTR), used in both the outer
    /// `GovTalkDetails` key and the IR header key.
    pub fn tax_reference(&self) -> &str {
        &self.company.reference
    }

    /// Build the return from the computed iXBRL inputs.
    ///
    /// The form figures are derived from the [`Frs105CorpTax`] via
    /// [`Ct600FormValues::from_tax`]; the attached iXBRL documents come from
    /// the two structs' `to_ixbrl()` renderings.  Envelope credentials,
    /// contact details and the repayment-address placeholder default to the
    /// reference tool's values and can be overridden on the returned struct.
    pub fn from_inputs(accounts: &Frs105Accounts, corp_tax: &Frs105CorpTax) -> Self {
        let values = Ct600FormValues::from_tax(corp_tax);
        let company = &values.company;
        let contact = Contact::default();
        let contact_name = format!("{} {}", contact.first_name, contact.second_name);
        Self {
            envelope: EnvelopeConfig::default(),
            contact,
            sender: "Company".to_string(),
            company: CompanyInformation {
                company_name: company.company_name.clone(),
                registration_number: company.company_number.clone(),
                reference: company.tax_reference.clone(),
                company_type: company.type_of_company,
                period_start: company.start,
                period_end: company.end,
            },
            return_info: ReturnInfoSummary {
                this_period_accounts: values.attached_accounts,
                this_period_computations: values.attached_accounts,
            },
            turnover: values.turnover,
            trading_profits: values.trading_profits,
            net_trading_profits: values.net_trading_profits,
            profits_before_other_deductions: values.profits_before_other_deductions_and_reliefs,
            profits_before_charges_and_group_relief: values
                .profits_before_charges_and_group_relief,
            chargeable_profits: values.profits_chargeable_to_corporation_tax,
            fy1: FinancialYear {
                year: values.fy1_year,
                profit: values.fy1_profit,
                tax_rate: values.fy1_tax_rate,
                tax: values.fy1_tax,
            },
            fy2: FinancialYear {
                year: values.fy2_year,
                profit: values.fy2_profit,
                tax_rate: values.fy2_tax_rate,
                tax: values.fy2_tax,
            },
            corporation_tax: values.corporation_tax,
            net_corporation_tax_chargeable: values.corporation_tax_chargeable,
            net_corporation_tax_liability: values.net_corporation_tax_liability,
            tax_chargeable: values.tax_chargeable,
            tax_payable: values.tax_payable,
            tax_payable_including_restitution_tax: values.tax_payable,
            sme_claim: values.sme_rnd_claim,
            rnd_enhanced_expenditure: values.sme_rnd_expenditure,
            rnd_and_creative_enhanced_expenditure: values.sme_rnd_expenditure,
            aia_capital_allowances: values.annual_investment_allowance,
            payment_address_lines: vec!["Address line 1".to_string(), "Address line 2".to_string()],
            payment_recipient: contact_name.clone(),
            // The CT schema's NomineeReference pattern requires at least one
            // character; the company's UTR is a meaningful default.
            payment_nominee_reference: company.tax_reference.clone(),
            declaration: Declaration {
                name: Some(contact_name),
                date: Some(Local::now().date_naive()),
                status: Some("Director".to_string()),
            },
            computation_document: Some(corp_tax.to_ixbrl()),
            accounts_document: Some(accounts.to_ixbrl()),
        }
    }

    /// Fill in the declaration boxes (975 name, 985 status).  Box 980 (date)
    /// already defaults to today.
    pub fn with_declaration(
        mut self,
        name: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        self.declaration.name = Some(name.into());
        self.declaration.status = Some(status.into());
        self
    }

    /// Serialise the return to the CT600 GovTalk XML message.
    ///
    /// The tree is built through the [`XmlNode`] intermediate representation
    /// (the same one the `ixbrl` reports use), so the output round-trips
    /// through [`XmlNode::from_xml_string`].
    pub fn to_xml(&self) -> String {
        let root = elt("GovTalkMessage", &[("xmlns", ENV_NS), ("xmlns:ct", CT_NS)]).children(vec![
            elt_text("EnvelopeVersion", &[], "2.0"),
            self.header_node(),
            self.govtalk_details_node(),
            elt("Body", &[]).child(self.ir_envelope_node()),
        ]);
        format!("<?xml version='1.0' encoding='UTF-8'?>\n{}", root.to_xml_string())
    }

    /// Deserialise a [`Ct600Return`] from the [`XmlNode`] intermediate
    /// representation (step 2 of the round trip: XML string -> `XmlNode` ->
    /// `Ct600Return`).
    ///
    /// This round-trips messages produced by [`Self::to_xml`] (with the
    /// schema fixes in place); the reference `--output-ct` message — which
    /// lacks the `Recipient` / `NomineeReference` / Declaration `Name` /
    /// `Status` elements — is not expected to parse.
    ///
    /// All serialised fields are recovered; the declaration date (box 980)
    /// is not serialised by [`Self::to_xml`], so it is always `None`.
    pub fn from_xml_node(node: &XmlNode) -> Result<Ct600Return, String> {
        // -- envelope (GovTalk header + details) ----------------------------
        let header = req(node, &["Header"])?;
        let md = req(header, &["MessageDetails"])?;
        let id_auth = req(header, &["SenderDetails", "IDAuthentication"])?;
        let auth = req(id_auth, &["Authentication"])?;
        let channel_routing = req(node, &["GovTalkDetails", "ChannelRouting"])?;
        let channel = req(channel_routing, &["Channel"])?;

        let envelope = EnvelopeConfig {
            class: text_at(md, &["Class"])?,
            qualifier: text_at(md, &["Qualifier"])?,
            function: text_at(md, &["Function"])?,
            gateway_test: text_at(md, &["GatewayTest"])?,
            username: text_at(id_auth, &["SenderID"])?,
            password: text_at(auth, &["Value"])?,
            vendor_id: text_at(channel, &["URI"])?,
            software: text_at(channel, &["Product"])?,
            software_version: text_at(channel, &["Version"])?,
            timestamp: {
                let raw = text_at(channel_routing, &["Timestamp"])?;
                NaiveDateTime::parse_from_str(raw.trim(), "%Y-%m-%dT%H:%M:%S")
                    .map_err(|e| format!("ct600: bad timestamp '{raw}': {e}"))?
            },
        };

        // -- IR header: principal contact + sender --------------------------
        let ir = req(node, &["Body", "ct:IRenvelope"])?;
        let irh = req(ir, &["ct:IRheader"])?;
        let contact_node = req(irh, &["ct:Principal", "ct:Contact"])?;
        let contact_name_node = req(contact_node, &["ct:Name"])?;
        let telephone_node = req(contact_node, &["ct:Telephone"])?;

        let contact = Contact {
            title: text_at(contact_name_node, &["ct:Ttl"])?,
            first_name: text_at(contact_name_node, &["ct:Fore"])?,
            second_name: text_at(contact_name_node, &["ct:Sur"])?,
            email: text_at(contact_node, &["ct:Email"])?,
            telephone: text_at(telephone_node, &["ct:Number"])?,
        };
        let sender = text_at(irh, &["ct:Sender"])?;

        // -- company tax return ---------------------------------------------
        let ctr = req(ir, &["ct:CompanyTaxReturn"])?;
        let ci = req(ctr, &["ct:CompanyInformation"])?;
        let pc = req(ci, &["ct:PeriodCovered"])?;
        let ris = req(ctr, &["ct:ReturnInfoSummary"])?;
        let turnover_el = req(ctr, &["ct:Turnover"])?;
        let ctc = req(ctr, &["ct:CompanyTaxCalculation"])?;
        let trading = req(ctc, &["ct:Income", "ct:Trading"])?;
        let ctc_chargeable = req(ctc, &["ct:CorporationTaxChargeable"])?;
        let cto = req(ctr, &["ct:CalculationOfTaxOutstandingOrOverpaid"])?;
        let ee = req(ctr, &["ct:EnhancedExpenditure"])?;
        let ac = req(ctr, &["ct:AllowancesAndCharges"])?;
        let oar = req(ctr, &["ct:OverpaymentsAndRepayments"])?;
        let declaration_el = req(ctr, &["ct:Declaration"])?;
        let xbrl_sub = req(ctr, &["ct:AttachedFiles", "ct:XBRLsubmission"])?;

        let company = CompanyInformation {
            company_name: text_at(ci, &["ct:CompanyName"])?,
            registration_number: text_at(ci, &["ct:RegistrationNumber"])?,
            reference: text_at(ci, &["ct:Reference"])?,
            company_type: {
                let raw = text_at(ci, &["ct:CompanyType"])?;
                raw.trim().parse::<u8>().map_err(|e| {
                    format!("ct600: bad company type '{raw}': {e}")
                })?
            },
            period_start: parse_date(&text_at(pc, &["ct:From"])?, "ct:PeriodCovered/ct:From")?,
            period_end: parse_date(&text_at(pc, &["ct:To"])?, "ct:PeriodCovered/ct:To")?,
        };

        let return_info = ReturnInfoSummary {
            this_period_accounts: parse_yesno(
                &text_at(ris, &["ct:Accounts", "ct:ThisPeriodAccounts"])?,
                "ct:ThisPeriodAccounts",
            )?,
            this_period_computations: parse_yesno(
                &text_at(ris, &["ct:Computations", "ct:ThisPeriodComputations"])?,
                "ct:ThisPeriodComputations",
            )?,
        };

        let fy = |name: &str| -> Result<FinancialYear, String> {
            let block = req(ctc_chargeable, &[name])?;
            let details = req(block, &["ct:Details"])?;
            Ok(FinancialYear {
                year: {
                    let raw = text_at(block, &["ct:Year"])?;
                    raw.trim().parse::<i32>()
                        .map_err(|e| format!("ct600: bad year '{raw}': {e}"))?
                },
                profit: parse_f64(&text_at(details, &["ct:Profit"])?, "ct:Profit")?,
                tax_rate: parse_f64(&text_at(details, &["ct:TaxRate"])?, "ct:TaxRate")?,
                tax: parse_f64(&text_at(details, &["ct:Tax"])?, "ct:Tax")?,
            })
        };

        // The repayment block is optional (only serialised when there are
        // address lines).
        let (payment_address_lines, payment_recipient, payment_nominee_reference) =
            match req(oar, &["ct:PaymentToPerson"]) {
                Ok(p) => {
                    let addr = req(p, &["ct:Address"])?;
                    (
                        children_named(addr, "ct:Line")
                            .into_iter()
                            .map(node_text)
                            .collect(),
                        text_at(p, &["ct:Recipient"])?,
                        text_at(p, &["ct:NomineeReference"])?,
                    )
                }
                Err(_) => (Vec::new(), String::new(), String::new()),
            };

        let declaration = Declaration {
            name: child(declaration_el, "ct:Name").map(node_text),
            status: child(declaration_el, "ct:Status").map(node_text),
            // Box 980 (date) is not serialised.
            date: None,
        };

        // Attached iXBRL documents come back base64-decoded.
        let attachment = |name: &str| -> Result<Option<String>, String> {
            let sec = match child(xbrl_sub, name) {
                Some(s) => s,
                None => return Ok(None),
            };
            let inst = match child(sec, "ct:Instance") {
                Some(i) => i,
                None => return Ok(None),
            };
            let raw = text_at(inst, &["ct:EncodedInlineXBRLDocument"])?;
            let bytes = general_purpose::STANDARD.decode(raw.trim()).map_err(|e| {
                format!("ct600: bad base64 attachment '{name}': {e}")
            })?;
            String::from_utf8(bytes)
                .map(Some)
                .map_err(|e| format!("ct600: attachment '{name}' not UTF-8: {e}"))
        };

        Ok(Ct600Return {
            envelope,
            contact,
            sender,
            company,
            return_info,
            turnover: parse_f64(
                &text_at(turnover_el, &["ct:Total"])?,
                "ct:Turnover/ct:Total",
            )?,
            trading_profits: parse_f64(&text_at(trading, &["ct:Profits"])?, "ct:Profits")?,
            net_trading_profits: parse_f64(
                &text_at(trading, &["ct:NetProfits"])?,
                "ct:NetProfits",
            )?,
            profits_before_other_deductions: parse_f64(
                &text_at(ctc, &["ct:ProfitsBeforeOtherDeductions"])?,
                "ct:ProfitsBeforeOtherDeductions",
            )?,
            profits_before_charges_and_group_relief: parse_f64(
                &text_at(
                    ctc,
                    &["ct:ChargesAndReliefs", "ct:ProfitsBeforeDonationsAndGroupRelief"],
                )?,
                "ct:ProfitsBeforeDonationsAndGroupRelief",
            )?,
            chargeable_profits: parse_f64(
                &text_at(ctc, &["ct:ChargeableProfits"])?,
                "ct:ChargeableProfits",
            )?,
            fy1: fy("ct:FinancialYearOne")?,
            fy2: fy("ct:FinancialYearTwo")?,
            corporation_tax: parse_f64(
                &text_at(ctc, &["ct:CorporationTax"])?,
                "ct:CorporationTax",
            )?,
            net_corporation_tax_chargeable: parse_f64(
                &text_at(ctc, &["ct:NetCorporationTaxChargeable"])?,
                "ct:NetCorporationTaxChargeable",
            )?,
            net_corporation_tax_liability: parse_f64(
                &text_at(cto, &["ct:NetCorporationTaxLiability"])?,
                "ct:NetCorporationTaxLiability",
            )?,
            tax_chargeable: parse_f64(&text_at(cto, &["ct:TaxChargeable"])?, "ct:TaxChargeable")?,
            tax_payable: parse_f64(&text_at(cto, &["ct:TaxPayable"])?, "ct:TaxPayable")?,
            tax_payable_including_restitution_tax: parse_f64(
                &text_at(cto, &["ct:TaxPayableIncludingRestitutionTax"])?,
                "ct:TaxPayableIncludingRestitutionTax",
            )?,
            sme_claim: parse_yesno(&text_at(ee, &["ct:SMEclaim"])?, "ct:SMEclaim")?,
            rnd_enhanced_expenditure: child(ee, "ct:RandDEnhancedExpenditure")
                .map(|n| parse_f64(&node_text(n), "ct:RandDEnhancedExpenditure"))
                .transpose()?,
            rnd_and_creative_enhanced_expenditure: child(ee, "ct:RandDAndCreativeEnhancedExpenditure")
                .map(|n| parse_f64(&node_text(n), "ct:RandDAndCreativeEnhancedExpenditure"))
                .transpose()?,
            aia_capital_allowances: parse_f64(
                &text_at(ac, &["ct:AIACapitalAllowancesInc"])?,
                "ct:AIACapitalAllowancesInc",
            )?,
            payment_address_lines,
            payment_recipient,
            payment_nominee_reference,
            declaration,
            computation_document: attachment("ct:Computation")?,
            accounts_document: attachment("ct:Accounts")?,
        })
    }

    /// Deserialise a [`Ct600Return`] from its serialised CT600 GovTalk XML,
    /// in two steps: first into the [`XmlNode`] intermediate representation,
    /// then into the struct.  See [`Self::from_xml_node`] for the shape of
    /// message expected (one produced by [`Self::to_xml`]).
    pub fn from_xml(xml: &str) -> Result<Ct600Return, String> {
        let node = XmlNode::from_xml_string(xml)?;
        Self::from_xml_node(&node)
    }

    // -- node builders -------------------------------------------------------

    fn header_node(&self) -> XmlNode {
        let e = &self.envelope;
        elt("Header", &[]).children(vec![
            elt("MessageDetails", &[]).children(vec![
                elt_text("Class", &[], &e.class),
                elt_text("Qualifier", &[], &e.qualifier),
                elt_text("Function", &[], &e.function),
                el("TransactionID"),
                el("CorrelationID"),
                elt_text("Transformation", &[], "XML"),
                elt_text("GatewayTest", &[], &e.gateway_test),
            ]),
            elt("SenderDetails", &[]).children(vec![elt("IDAuthentication", &[]).children(vec![
                elt_text("SenderID", &[], &e.username),
                elt("Authentication", &[]).children(vec![
                    elt_text("Method", &[], "clear"),
                    elt_text("Role", &[], "principal"),
                    elt_text("Value", &[], &e.password),
                ]),
            ])]),
        ])
    }

    fn govtalk_details_node(&self) -> XmlNode {
        let e = &self.envelope;
        elt("GovTalkDetails", &[]).children(vec![
            elt("Keys", &[]).child(elt_text("Key", &[("Type", "UTR")], self.tax_reference())),
            elt("TargetDetails", &[]).child(elt_text("Organisation", &[], "HMRC")),
            elt("ChannelRouting", &[]).children(vec![
                elt("Channel", &[]).children(vec![
                    elt_text("URI", &[], &e.vendor_id),
                    elt_text("Product", &[], &e.software),
                    elt_text("Version", &[], &e.software_version),
                ]),
                elt_text(
                    "Timestamp",
                    &[],
                    &e.timestamp.format("%Y-%m-%dT%H:%M:%S").to_string(),
                ),
            ]),
        ])
    }

    fn ir_envelope_node(&self) -> XmlNode {
        elt("ct:IRenvelope", &[]).children(vec![
            elt("ct:IRheader", &[]).children(vec![
                elt("ct:Keys", &[])
                    .child(elt_text("ct:Key", &[("Type", "UTR")], self.tax_reference())),
                elt_text("ct:PeriodEnd", &[], &self.company.period_end.to_string()),
                elt("ct:Principal", &[]).child(
                    elt("ct:Contact", &[]).children(vec![
                        elt("ct:Name", &[]).children(vec![
                            elt_text("ct:Ttl", &[], &self.contact.title),
                            elt_text("ct:Fore", &[], &self.contact.first_name),
                            elt_text("ct:Sur", &[], &self.contact.second_name),
                        ]),
                        elt_text("ct:Email", &[], &self.contact.email),
                        elt("ct:Telephone", &[])
                            .child(elt_text("ct:Number", &[], &self.contact.telephone)),
                    ]),
                ),
                // The CT schema requires the `Type` attribute on the IRmark
                // (the reference `--output-ct` message omits it).
                elt("ct:IRmark", &[("Type", "generic")]),
                elt_text("ct:Sender", &[], &self.sender),
            ]),
            self.company_tax_return_node(),
        ])
    }

    fn company_tax_return_node(&self) -> XmlNode {
        let c = &self.company;
        let r = &self.return_info;

        let company_information = elt("ct:CompanyInformation", &[]).children(vec![
            elt_text("ct:CompanyName", &[], &c.company_name),
            elt_text("ct:RegistrationNumber", &[], &c.registration_number),
            elt_text("ct:Reference", &[], &c.reference),
            elt_text("ct:CompanyType", &[], &format!("{:02}", c.company_type)),
            elt("ct:PeriodCovered", &[]).children(vec![
                elt_text("ct:From", &[], &c.period_start.to_string()),
                elt_text("ct:To", &[], &c.period_end.to_string()),
            ]),
        ]);

        let return_info = elt("ct:ReturnInfoSummary", &[]).children(vec![
            elt("ct:Accounts", &[])
                .child(elt_text("ct:ThisPeriodAccounts", &[], yesno(r.this_period_accounts))),
            elt("ct:Computations", &[]).child(elt_text(
                "ct:ThisPeriodComputations",
                &[],
                yesno(r.this_period_computations),
            )),
        ]);

        let turnover =
            elt("ct:Turnover", &[]).child(elt_text("ct:Total", &[], &pounds(self.turnover)));

        let tax_calculation = elt("ct:CompanyTaxCalculation", &[]).children(vec![
            elt("ct:Income", &[]).child(elt("ct:Trading", &[]).children(vec![
                elt_text("ct:Profits", &[], &pounds(self.trading_profits)),
                elt_text("ct:NetProfits", &[], &pounds(self.net_trading_profits)),
            ])),
            elt_text(
                "ct:ProfitsBeforeOtherDeductions",
                &[],
                &pounds(self.profits_before_other_deductions),
            ),
            elt("ct:ChargesAndReliefs", &[]).child(elt_text(
                "ct:ProfitsBeforeDonationsAndGroupRelief",
                &[],
                &pounds(self.profits_before_charges_and_group_relief),
            )),
            elt_text("ct:ChargeableProfits", &[], &pounds(self.chargeable_profits)),
            elt("ct:CorporationTaxChargeable", &[]).children(vec![
                financial_year_node("FinancialYearOne", &self.fy1),
                financial_year_node("FinancialYearTwo", &self.fy2),
            ]),
            elt_text("ct:CorporationTax", &[], &money(self.corporation_tax)),
            elt_text(
                "ct:NetCorporationTaxChargeable",
                &[],
                &money(self.net_corporation_tax_chargeable),
            ),
        ]);

        let tax_outstanding = elt("ct:CalculationOfTaxOutstandingOrOverpaid", &[]).children(vec![
            elt_text(
                "ct:NetCorporationTaxLiability",
                &[],
                &money(self.net_corporation_tax_liability),
            ),
            elt_text("ct:TaxChargeable", &[], &money(self.tax_chargeable)),
            elt_text("ct:TaxPayable", &[], &money(self.tax_payable)),
            elt_text(
                "ct:TaxPayableIncludingRestitutionTax",
                &[],
                &money(self.tax_payable_including_restitution_tax),
            ),
        ]);

        let mut enhanced = vec![elt_text("ct:SMEclaim", &[], yesno(self.sme_claim))];
        if let Some(v) = self.rnd_enhanced_expenditure {
            enhanced.push(elt_text("ct:RandDEnhancedExpenditure", &[], &pounds(v)));
        }
        if let Some(v) = self.rnd_and_creative_enhanced_expenditure {
            enhanced.push(elt_text(
                "ct:RandDAndCreativeEnhancedExpenditure",
                &[],
                &pounds(v),
            ));
        }
        let enhanced_expenditure = elt("ct:EnhancedExpenditure", &[]).children(enhanced);

        let allowances = elt("ct:AllowancesAndCharges", &[]).child(elt_text(
            "ct:AIACapitalAllowancesInc",
            &[],
            &pounds(self.aia_capital_allowances),
        ));

        let mut repayments = Vec::new();
        if !self.payment_address_lines.is_empty() {
            let address = elt("ct:Address", &[]).children(
                self.payment_address_lines
                    .iter()
                    .map(|line| elt_text("ct:Line", &[], line))
                    .collect(),
            );
            repayments.push(elt("ct:PaymentToPerson", &[]).children(vec![
                // The CT schema requires Recipient before Address and a
                // NomineeReference after it (the reference omits both).
                elt_text("ct:Recipient", &[], &self.payment_recipient),
                address,
                elt_text("ct:NomineeReference", &[], &self.payment_nominee_reference),
            ]));
        }
        let overpayments = elt("ct:OverpaymentsAndRepayments", &[]).children(repayments);

        // The CT schema requires Name and Status in the Declaration; fall
        // back to the contact name / `Director` if not explicitly set.
        let declaration_name = self
            .declaration
            .name
            .clone()
            .unwrap_or_else(|| format!("{} {}", self.contact.first_name, self.contact.second_name));
        let declaration = elt("ct:Declaration", &[]).children(vec![
            elt_text("ct:AcceptDeclaration", &[], "yes"),
            elt_text("ct:Name", &[], &declaration_name),
            elt_text(
                "ct:Status",
                &[],
                self.declaration.status.as_deref().unwrap_or("Director"),
            ),
        ]);

        let mut xbrl_submission = Vec::new();
        if let Some(doc) = &self.computation_document {
            xbrl_submission.push(elt("ct:Computation", &[]).child(instance_node(doc)));
        }
        if let Some(doc) = &self.accounts_document {
            xbrl_submission.push(elt("ct:Accounts", &[]).child(instance_node(doc)));
        }
        let attached_files = elt("ct:AttachedFiles", &[]).child(
            elt("ct:XBRLsubmission", &[]).children(xbrl_submission),
        );

        elt("ct:CompanyTaxReturn", &[("ReturnType", "new")]).children(vec![
            company_information,
            return_info,
            turnover,
            tax_calculation,
            tax_outstanding,
            enhanced_expenditure,
            allowances,
            overpayments,
            declaration,
            attached_files,
        ])
    }
}

// ============================================================================
// Serialisation helpers
// ============================================================================

/// A `ct:`-namespaced financial-year block (`ct:FinancialYearOne` / `Two`).
fn financial_year_node(name: &str, fy: &FinancialYear) -> XmlNode {
    elt(&format!("ct:{name}"), &[]).children(vec![
        elt_text("ct:Year", &[], &fy.year.to_string()),
        elt("ct:Details", &[]).children(vec![
            elt_text("ct:Profit", &[], &pounds(fy.profit)),
            elt_text("ct:TaxRate", &[], &money(fy.tax_rate)),
            elt_text("ct:Tax", &[], &money(fy.tax)),
        ]),
    ])
}

/// An attached iXBRL instance: the raw document base64-encoded inside
/// `ct:EncodedInlineXBRLDocument`.
fn instance_node(document: &str) -> XmlNode {
    let encoded = general_purpose::STANDARD.encode(document.as_bytes());
    elt("ct:Instance", &[]).child(elt_text("ct:EncodedInlineXBRLDocument", &[], &encoded))
}

/// Format a money / rate value with two decimal places (e.g. `142.12`),
/// matching the reference tool's `kind="money"` / `kind="rate"`.
fn money(v: f64) -> String {
    format!("{v:.2}")
}

/// Format a whole-pound value: truncate to whole pounds, then two decimal
/// places (e.g. `11218.12` -> `11218.00`), matching the reference tool's
/// `kind="pounds"`.  (Formatting the truncated float — not the integer — so
/// the `.2` precision is honoured.)
fn pounds(v: f64) -> String {
    format!("{:.2}", v.trunc())
}

/// `yes` / `no` for boolean boxes, matching `kind="yesno"`.
fn yesno(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

// ============================================================================
// Deserialisation helpers
// ============================================================================

/// The concatenated direct text of a node's `Text` children.
///
/// [`XmlNode::from_xml_string`] trims each text event, and `elt_text`
/// produces a single text child per element, so the values this recovers
/// (numbers, names, base64) are single, already-trimmed runs.
fn node_text(node: &XmlNode) -> String {
    match node {
        XmlNode::Text(t) => t.clone(),
        XmlNode::Elem { children, .. } => children
            .iter()
            .filter_map(|c| match c {
                XmlNode::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// The first direct child element with the given (literal) name.
fn child<'a>(node: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    match node {
        XmlNode::Elem { children, .. } => children
            .iter()
            .find(|c| matches!(c, XmlNode::Elem { name: n, .. } if n == name)),
        _ => None,
    }
}

/// All direct child elements with the given (literal) name.
fn children_named<'a>(node: &'a XmlNode, name: &str) -> Vec<&'a XmlNode> {
    match node {
        XmlNode::Elem { children, .. } => children
            .iter()
            .filter(|c| matches!(c, XmlNode::Elem { name: n, .. } if n == name))
            .collect(),
        _ => Vec::new(),
    }
}

/// Descend `path` from `node`, failing with the full path in the error.
fn req<'a>(mut node: &'a XmlNode, path: &[&str]) -> Result<&'a XmlNode, String> {
    for p in path {
        node = child(node, p)
            .ok_or_else(|| format!("ct600: missing element '{}'", path.join("/")))?;
    }
    Ok(node)
}

/// The direct text at `path` from `node`.
fn text_at(node: &XmlNode, path: &[&str]) -> Result<String, String> {
    Ok(node_text(req(node, path)?))
}

/// Parse a money / whole-pound value (formatted with two decimals).
fn parse_f64(raw: &str, path: &str) -> Result<f64, String> {
    raw.trim()
        .parse::<f64>()
        .map_err(|e| format!("ct600: bad value '{raw}' for '{path}': {e}"))
}

/// Parse a `%Y-%m-%d` date.
fn parse_date(raw: &str, path: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map_err(|e| format!("ct600: bad date '{raw}' for '{path}': {e}"))
}

/// Parse `yes` / `no`.
fn parse_yesno(raw: &str, path: &str) -> Result<bool, String> {
    match raw.trim() {
        "yes" => Ok(true),
        "no" => Ok(false),
        other => Err(format!("ct600: bad yes/no value '{other}' for '{path}'")),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ixbrl::company::CompanyProfile;
    use std::collections::HashMap;

    fn example2_company() -> ixbrl::company::Company {
        let mut company =
            ixbrl::company::Company::new("Example Biz Ltd.", "8596148860", "12345678");
        // Anchored on the return-period start, matching the historical
        // constructor default used by this fixture.
        company.registration_date = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        company
    }

    /// The example company's set of accounts: the 2020 calendar-year return
    /// period, the default financial-year tax parameters and the report
    /// metadata (title, dates, employee counts; the ct600 message only uses
    /// these for the attached accounts iXBRL rendering).
    fn example2_accounts_meta() -> ixbrl::company::AccountsMeta {
        ixbrl::company::AccountsMeta {
            period: Some(ixbrl::company::AccountingPeriod {
                start: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2020, 12, 31).unwrap(),
            }),
            report_date: NaiveDate::from_ymd_opt(2021, 3, 1).unwrap(),
            authorised_date: NaiveDate::from_ymd_opt(2021, 2, 1).unwrap(),
            incorporation_date: NaiveDate::from_ymd_opt(2017, 4, 5).unwrap(),
            signed_by: "B Smith".into(),
            average_employees: HashMap::from([("2020".to_string(), 2), ("2019".to_string(), 1)]),
            ..ixbrl::company::AccountsMeta::default()
        }
    }

    async fn example2_corp_tax() -> Frs105CorpTax {
        let company = example2_company();
        let gnucash = ixbrl::GnucashBook::try_from_gnucash_file(
            "../ixbrl/example_data/example2/input.gnucash",
        )
        .await
        .expect("open example2 gnucash");
        Frs105CorpTax::builder(&gnucash, &company, &example2_accounts_meta())
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

    async fn example2_accounts() -> Frs105Accounts {
        let company = example2_company();
        let gnucash = ixbrl::GnucashBook::try_from_gnucash_file(
            "../ixbrl/example_data/example2/input.gnucash",
        )
        .await
        .expect("open example2 gnucash");
        Frs105Accounts::new(
            &gnucash,
            &company,
            &example2_profile(),
            &example2_accounts_meta(),
        )
    }

    /// The example2 company profile (directors, SIC codes, contacts); the
    /// ct600 message itself only uses this for the attached accounts iXBRL
    /// rendering.
    fn example2_profile() -> CompanyProfile {
        CompanyProfile {
            directors: vec!["A Bloggs".into(), "B Smith".into(), "C Jones".into()],
            contact_name: String::new(),
            address_lines: vec!["123 Leadbarton Street".into()],
            county: String::new(),
            location: "Threapchington".into(),
            postcode: "QQ9 9ZZ".into(),
            email: "info@example.org".into(),
            phone_country: "+44".into(),
            phone_area: String::new(),
            phone_number: String::new(),
            website_url: String::new(),
            website_description: String::new(),
            vat_registration: String::new(),
            sic_codes: vec!["62020".into(), "62021".into()],
            activities: String::new(),
            jurisdiction: "England and Wales".into(),
            accountant_name: String::new(),
            accountant_business: String::new(),
            accountant_address: String::new(),
            auditor_name: String::new(),
            auditor_business: String::new(),
            auditor_address: String::new(),
            industry_sector_dimension: String::new(),
            legal_form_dimension: String::new(),
            country_dimension: String::new(),
            contact_country_dimension: String::new(),
            phone_type_dimension: String::new(),
            logo_b64: None,
        }
    }

    /// The element skeleton of a node tree: names + attributes, text ignored.
    /// Used to compare our message's structure with the reference output.
    fn skeleton(node: &XmlNode) -> String {
        match node {
            XmlNode::Elem {
                name,
                attributes,
                children,
            } => {
                let attrs = attributes
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(",");
                let kids = children.iter().map(skeleton).collect::<Vec<_>>().join("");
                format!("{name}[{attrs}]({kids})")
            }
            XmlNode::Text(_) => String::new(),
        }
    }

    #[tokio::test]
    async fn declaration_boxes_always_serialise_with_defaults() {
        let corp_tax = example2_corp_tax().await;
        let accounts = example2_accounts().await;
        let plain = Ct600Return::from_inputs(&accounts, &corp_tax);

        // The CT schema requires Name and Status in the Declaration; by
        // default they fall back to the contact name / `Director`.
        let xml = plain.to_xml();
        assert!(xml.contains(
            "<ct:Declaration><ct:AcceptDeclaration>yes</ct:AcceptDeclaration><ct:Name>Sarah McAcre</ct:Name><ct:Status>Director</ct:Status></ct:Declaration>"
        ));

        // `with_declaration` overrides name / status; date defaults to today.
        let signed = plain.with_declaration("Jane Doe", "Secretary");
        let xml = signed.to_xml();
        assert!(xml.contains(
            "<ct:Declaration><ct:AcceptDeclaration>yes</ct:AcceptDeclaration><ct:Name>Jane Doe</ct:Name><ct:Status>Secretary</ct:Status></ct:Declaration>"
        ));
        assert!(
            signed.declaration.date.is_some(),
            "declaration date defaults to today"
        );
    }

    /// Remove the three schema-fix elements the reference `--output-ct`
    /// message lacks, so our message can be compared element-for-element
    /// with it: the IRmark `Type` attribute, the Declaration `Name` /
    /// `Status` children, and the PaymentToPerson `Recipient` /
    /// `NomineeReference` children.
    fn strip_fixes(node: &XmlNode) -> XmlNode {
        match node {
            XmlNode::Elem {
                name,
                attributes,
                children,
            } => {
                let attributes = if name == "ct:IRmark" {
                    attributes
                        .iter()
                        .filter(|(k, _)| k != "Type")
                        .cloned()
                        .collect()
                } else {
                    attributes.clone()
                };
                let mut kids: Vec<XmlNode> = children.iter().map(strip_fixes).collect();
                match name.as_str() {
                    "ct:Declaration" => kids.retain(|c| {
                        matches!(c, XmlNode::Elem { name, .. } if name == "ct:AcceptDeclaration")
                    }),
                    "ct:PaymentToPerson" => kids.retain(|c| {
                        matches!(c, XmlNode::Elem { name, .. } if name == "ct:Address")
                    }),
                    _ => {}
                }
                XmlNode::Elem {
                    name: name.clone(),
                    attributes,
                    children: kids,
                }
            }
            XmlNode::Text(_) => node.clone(),
        }
    }

    #[tokio::test]
    async fn ct600_return_round_trips_through_xml() {
        let corp_tax = example2_corp_tax().await;
        let accounts = example2_accounts().await;
        let filing = Ct600Return::from_inputs(&accounts, &corp_tax)
            .with_declaration("Jane Doe", "Secretary");

        let xml = filing.to_xml();
        let back = Ct600Return::from_xml(&xml).expect("deserialise own message");

        // -- envelope + contact ---------------------------------------------
        assert_eq!(back.envelope.class, "HMRC-CT-CT600");
        assert_eq!(back.envelope.username, "CTUser100");
        assert_eq!(back.envelope.vendor_id, "1234");
        assert_eq!(back.envelope.timestamp, filing.envelope.timestamp);
        assert_eq!(back.contact.first_name, "Sarah");
        assert_eq!(back.contact.email, "sarah@example.org");
        assert_eq!(back.contact.telephone, "447900123456");
        assert_eq!(back.sender, "Company");

        // -- company + summary ----------------------------------------------
        assert_eq!(back.company.company_name, "Example Biz Ltd.");
        assert_eq!(back.company.registration_number, "12345678");
        assert_eq!(back.company.reference, "8596148860");
        assert_eq!(back.company.company_type, 0);
        assert_eq!(
            back.company.period_end,
            NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()
        );
        assert!(back.return_info.this_period_accounts);
        assert!(back.return_info.this_period_computations);

        // -- figures ---------------------------------------------------------
        // Whole-pound boxes are truncated on serialisation, so the round
        // trip recovers the truncated value.
        assert_eq!(back.turnover, 11218.0);
        assert_eq!(back.net_trading_profits, 748.0);
        assert_eq!(back.profits_before_other_deductions, 748.0);
        assert_eq!(back.chargeable_profits, 748.0);
        assert_eq!(back.fy1.year, 2019);
        assert_eq!(back.fy1.profit, 186.0);
        assert_eq!(back.fy1.tax_rate, 19.0);
        assert_eq!(back.fy1.tax, 35.34);
        assert_eq!(back.fy2.year, 2020);
        assert_eq!(back.fy2.tax, 106.78);
        assert_eq!(back.corporation_tax, 142.12);
        assert_eq!(back.net_corporation_tax_chargeable, 142.12);
        assert_eq!(back.net_corporation_tax_liability, 142.12);
        assert_eq!(back.tax_payable, 142.12);
        assert!(back.sme_claim);
        assert_eq!(back.rnd_enhanced_expenditure, Some(465.0));
        assert_eq!(back.rnd_and_creative_enhanced_expenditure, Some(465.0));
        assert_eq!(back.aia_capital_allowances, 591.0);

        // -- payment + declaration ------------------------------------------
        assert_eq!(
            back.payment_address_lines,
            vec!["Address line 1".to_string(), "Address line 2".to_string()]
        );
        assert_eq!(back.payment_recipient, "Sarah McAcre");
        assert_eq!(back.payment_nominee_reference, "8596148860");
        assert_eq!(back.declaration.name.as_deref(), Some("Jane Doe"));
        assert_eq!(back.declaration.status.as_deref(), Some("Secretary"));
        // Box 980 (date) is not serialised, so it is not recovered.
        assert_eq!(back.declaration.date, None);

        // -- attachments -----------------------------------------------------
        assert_eq!(back.computation_document, filing.computation_document);
        assert_eq!(back.accounts_document, filing.accounts_document);

        // -- the recovered struct re-serialises to the identical message ----
        assert_eq!(back.to_xml(), xml);
    }

    #[tokio::test]
    async fn ct600_return_from_example2_matches_reference() {
        let corp_tax = example2_corp_tax().await;
        let accounts = example2_accounts().await;
        let filing = Ct600Return::from_inputs(&accounts, &corp_tax);

        let xml = filing.to_xml();

        // Write the generated message to .cache/rust-ct600 for inspection /
        // the LTS.
        std::fs::create_dir_all("../../.cache/rust-ct600").unwrap();
        std::fs::write("../../.cache/rust-ct600/ct600-rust.xml", &xml).unwrap();

        // -- envelope --------------------------------------------------------
        assert!(xml.contains("<Class>HMRC-CT-CT600</Class>"));
        assert!(xml.contains("<SenderID>CTUser100</SenderID>"));
        assert!(xml.contains("<Key Type=\"UTR\">8596148860</Key>"));
        assert!(xml.contains("<ct:CompanyName>Example Biz Ltd.</ct:CompanyName>"));
        assert!(xml.contains("<ct:RegistrationNumber>12345678</ct:RegistrationNumber>"));
        assert!(xml.contains("<ct:CompanyType>00</ct:CompanyType>"));
        assert!(xml.contains("<ct:From>2020-01-01</ct:From>"));
        assert!(xml.contains("<ct:To>2020-12-31</ct:To>"));
        assert!(xml.contains("<ct:PeriodEnd>2020-12-31</ct:PeriodEnd>"));

        // -- figures (whole-pound boxes truncated, matching the reference) ---
        assert!(xml.contains("<ct:Turnover><ct:Total>11218.00</ct:Total></ct:Turnover>"));
        assert!(xml.contains("<ct:Profits>748.00</ct:Profits>"));
        assert!(xml.contains("<ct:NetProfits>748.00</ct:NetProfits>"));
        assert!(xml.contains(
            "<ct:ProfitsBeforeOtherDeductions>748.00</ct:ProfitsBeforeOtherDeductions>"
        ));
        assert!(xml.contains(
            "<ct:ProfitsBeforeDonationsAndGroupRelief>748.00</ct:ProfitsBeforeDonationsAndGroupRelief>"
        ));
        assert!(xml.contains("<ct:ChargeableProfits>748.00</ct:ChargeableProfits>"));
        assert!(xml.contains("<ct:Year>2019</ct:Year>"));
        assert!(xml.contains("<ct:Profit>186.00</ct:Profit>"));
        assert!(xml.contains("<ct:TaxRate>19.00</ct:TaxRate>"));
        assert!(xml.contains("<ct:Tax>35.34</ct:Tax>"));
        assert!(xml.contains("<ct:Year>2020</ct:Year>"));
        assert!(xml.contains("<ct:Profit>562.00</ct:Profit>"));
        assert!(xml.contains("<ct:Tax>106.78</ct:Tax>"));
        assert!(xml.contains("<ct:CorporationTax>142.12</ct:CorporationTax>"));
        assert!(xml.contains("<ct:NetCorporationTaxChargeable>142.12</ct:NetCorporationTaxChargeable>"));
        assert!(xml.contains("<ct:NetCorporationTaxLiability>142.12</ct:NetCorporationTaxLiability>"));
        assert!(xml.contains("<ct:TaxChargeable>142.12</ct:TaxChargeable>"));
        assert!(xml.contains("<ct:TaxPayable>142.12</ct:TaxPayable>"));
        assert!(xml.contains(
            "<ct:TaxPayableIncludingRestitutionTax>142.12</ct:TaxPayableIncludingRestitutionTax>"
        ));
        assert!(xml.contains("<ct:SMEclaim>yes</ct:SMEclaim>"));
        assert!(xml.contains("<ct:RandDEnhancedExpenditure>465.00</ct:RandDEnhancedExpenditure>"));
        assert!(xml.contains(
            "<ct:RandDAndCreativeEnhancedExpenditure>465.00</ct:RandDAndCreativeEnhancedExpenditure>"
        ));
        assert!(xml.contains("<ct:AIACapitalAllowancesInc>591.00</ct:AIACapitalAllowancesInc>"));
        assert!(xml.contains("<ct:AcceptDeclaration>yes</ct:AcceptDeclaration>"));

        // -- the three schema fixes the reference message omits --------------
        assert!(xml.contains("<ct:IRmark Type=\"generic\"/>"));
        assert!(xml.contains(
            "<ct:Declaration><ct:AcceptDeclaration>yes</ct:AcceptDeclaration><ct:Name>Sarah McAcre</ct:Name><ct:Status>Director</ct:Status></ct:Declaration>"
        ));
        assert!(xml.contains(
            "<ct:PaymentToPerson><ct:Recipient>Sarah McAcre</ct:Recipient><ct:Address><ct:Line>Address line 1</ct:Line><ct:Line>Address line 2</ct:Line></ct:Address><ct:NomineeReference>8596148860</ct:NomineeReference></ct:PaymentToPerson>"
        ));

        // -- the message round-trips through the intermediate representation --
        let node = XmlNode::from_xml_string(&xml).expect("parse generated message");
        let node2 = XmlNode::from_xml_string(&node.to_xml_string()).expect("re-parse");
        assert_eq!(format!("{node:?}"), format!("{node2:?}"));

        // -- element structure matches the reference ct600.xml ---------------
        // (modulo the three schema fixes, which are stripped from both sides)
        if let Ok(reference) = std::fs::read_to_string("../../.cache/py-ct600/ct600.xml") {
            let ref_node = XmlNode::from_xml_string(&reference).expect("parse reference");
            assert_eq!(
                skeleton(&strip_fixes(&ref_node)),
                skeleton(&strip_fixes(&node)),
                "element structure must match the reference .cache/py-ct600/ct600.xml (run `nix run .#rct600-run` to regenerate it)"
            );
        } else {
            eprintln!(
                "skipping reference comparison: .cache/py-ct600/ct600.xml not present (run `nix run .#rct600-run`)"
            );
        }
    }
}
