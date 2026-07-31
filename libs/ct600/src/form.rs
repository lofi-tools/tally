//! CT600 form field values.
//!
//! This module models the individual boxes of the HMRC CT600 corporation tax
//! return.  It mirrors the Python reference implementation's `to_values()`
//! helper: each box is described by an entry in [`BOXES`], and
//! [`Ct600FormValues`] holds the computed values as a typed struct (fields are
//! non-optional where the CT600 specification makes the box mandatory).
//!
//! [`Ct600FormValues::to_map`] serializes the typed struct into the
//! intermediary representation used by downstream code: a map from box number
//! to `{ box_id, description, value }`.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Serialize;
use ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax;

/// Every CT600 box as `(box number, description)`.
///
/// This is the canonical catalogue of boxes; [`Ct600FormValues::to_map`] uses
/// it to attach a description to every box.
pub static BOXES: &[(u16, &str)] = &[
    (1, "Company name"),
    (2, "Company registration number"),
    (3, "Tax reference"),
    (4, "Type of company"),
    (30, "Start of return"),
    (35, "End of return"),
    (40, "Repayments this period"),
    (50, "Making more than one return now"),
    (55, "Estimated figures"),
    (60, "Company part of a group that is not small"),
    (65, "Notice of disclosable avoidance schemes"),
    (70, "Compensating adjustment claimed"),
    (75, "Company qualifies for SME exemption"),
    (80, "Attached accounts and computations for this period"),
    (85, "Attached accounts and computations for a different period"),
    (90, "Reason for not attaching accounts"),
    (95, "CT600A - Loans and arrangements"),
    (100, "CT600B - Controlled foreign companies"),
    (105, "CT600C - Group & consortium"),
    (110, "CT600D - Insurance"),
    (115, "CT600E - CASCs"),
    (120, "CT600F - Tonnage tax"),
    (125, "CT600G - Northern Ireland"),
    (130, "CT600H - Cross-border royalties"),
    (135, "CT600I - Ring fence trades"),
    (140, "CT600J - Tax avoidance schemes"),
    (141, "CT600K - Restitution"),
    (142, "CT600L - R&D"),
    (143, "CT600M - Freeports"),
    (144, "CT600N - Residential property developer tax"),
    (145, "Total turnover from trade"),
    (150, "Banks and other financial concerns"),
    (155, "Trading profits"),
    (160, "Trading losses brought forward against profits"),
    (165, "Net trading profits"),
    (
        170,
        "Bank, building society or other interest, and profits from non-trading loan relationships",
    ),
    (172, "Box 170 net of carrying back deficit"),
    (
        175,
        "Annual payments not otherwise charged to Corporation Tax and from which Income Tax has not been deducted",
    ),
    (180, "Non-exempt dividends or distributions from non-UK resident companies"),
    (185, "Income from which Income Tax has been deducted"),
    (190, "Income from a property business"),
    (195, "Non-trading gains on intangible fixed assets"),
    (200, "Tonnage Tax profits"),
    (205, "Income not falling under any other heading"),
    (210, "Gross chargeable gains"),
    (215, "Allowable losses including losses brought forward"),
    (220, "Net chargeable gains"),
    (225, "Losses brought forward against certain investment income"),
    (
        230,
        "Non-trade deficits on loan relationships (including interest), and derivative contracts (financial instruments) brought forward set against non-trading profits",
    ),
    (235, "Profits before other deductions and reliefs"),
    (240, "Losses on unquoted shares"),
    (245, "Management expenses"),
    (250, "UK property business losses for this or previous accounting period"),
    (255, "Capital allowances for the purpose of management of the business"),
    (
        260,
        "Non-trade deficits for this accounting period from loan relationships and derivative contracts (financial instruments)",
    ),
    (
        263,
        "Carried forward non-trade deficits from loan relationships and derivative contracts (financial instruments)",
    ),
    (265, "Non-trading losses on intangible fixed assets"),
    (275, "Trading losses of this or a later accounting period"),
    (
        280,
        "Put an X in box 280 if amounts carried back from later accounting periods are included in box 275",
    ),
    (285, "Trading losses carried forward and claimed against total profits"),
    (290, "Non-trade capital allowances"),
    (295, "Total of deductions and reliefs"),
    (300, "Profits before qualifying donations and group relief"),
    (305, "Qualifying donations"),
    (310, "Group relief"),
    (312, "Group relief for carried forward losses"),
    (315, "Profits chargeable to Corporation Tax"),
    (320, "Ring fence profits included"),
    (325, "Northern Ireland profits included"),
    (326, "Number of associated companies in this period"),
    (327, "Number of associated companies in the 1st FY"),
    (328, "Number of associated companies in the 2nd FY"),
    (329, "Chargeable at the small profit rate"),
    (330, "FY1"),
    (335, "FY1 Profit 1"),
    (340, "FY1 Rate of Tax 1"),
    (345, "FY1 Tax 1"),
    (350, "FY1 Profit 2"),
    (355, "FY1 Rate of Tax 2"),
    (360, "FY1 Tax 2"),
    (365, "FY1 Profit 3"),
    (370, "FY1 Rate of Tax 3"),
    (375, "FY1 Tax 3"),
    (380, "FY2"),
    (385, "FY2 Profit 1"),
    (390, "FY2 Rate of Tax 1"),
    (395, "FY2 Tax 1"),
    (400, "FY2 Profit 2"),
    (405, "FY2 Rate of Tax 2"),
    (410, "FY2 Tax 2"),
    (415, "FY2 Profit 3"),
    (420, "FY2 Rate of Tax 3"),
    (425, "FY2 Tax 3"),
    (430, "Corporation Tax"),
    (435, "Marginal relief for ring fence trades"),
    (440, "Corporation Tax chargeable"),
    (445, "Community Investment relief"),
    (450, "Double Taxation Relief"),
    (
        455,
        "Put an X in box 455 if box 450 includes an underlying Rate relief claim",
    ),
    (
        460,
        "Put an X in box 460 if box 450 includes any amount carried back from a later period",
    ),
    (465, "Advance Corporation Tax"),
    (470, "Total reliefs and deduction in terms of tax"),
    (471, "CJRS and Job Support Scheme received"),
    (472, "CJRS and Job Support Scheme entitlement"),
    (473, "CJRS overpayment already assessed or voluntary disclosed"),
    (474, "Other Coronavirus overpayments"),
    (986, "Energy (Oil and Gas) Profits Levy"),
    (475, "Net Corporation Tax liability"),
    (480, "Tax payable on loans and arrangements to participators"),
    (
        485,
        "Put an X in box 485 if you completed box A70 in the supplementary pages CT600A",
    ),
    (490, "CFC tax payable"),
    (495, "Bank Levy payable"),
    (496, "Bank surcharge payable"),
    (497, "Residential Property Developer Tax repayable"),
    (500, "CFC tax and bank Levy payable"),
    (501, "EOGPL payable"),
    (505, "Supplementary charge (ring fence trades) payable"),
    (510, "Tax chargeable"),
    (515, "Income Tax deducted from gross income included in profits"),
    (520, "Income Tax repayable to the company"),
    (
        525,
        "Self-assessment of tax payable before restitution tax and coronavirus support scheme overpayments",
    ),
    (526, "Coronavirus support schemes overpayment now due"),
    (527, "Restitution tax"),
    (528, "Self-assessment of tax payable"),
    (530, "Research and Development credit"),
    (535, "Not currently used"),
    (540, "Creative tax credit"),
    (545, "Total of R&D credit and creative tax credit"),
    (550, "Land remediation tax credit"),
    (555, "Life assurance company tax credit"),
    (560, "Total land remediation and life assurance company tax credit"),
    (565, "Capital allowances first-year tax credit"),
    (570, "Surplus Research and Development credits or creative tax credit payable"),
    (575, "Land remediation or life assurance company tax credit payable"),
    (580, "Capital allowances first-year tax credit payable"),
    (
        585,
        "Ring fence Corporation Tax included and 590 Ring fence supplementary charge included",
    ),
    (586, "NI Corporation Tax included"),
    (595, "Tax already paid (and not already repaid)"),
    (600, "Tax outstanding"),
    (605, "Tax overpaid including surplus or payable credits"),
    (610, "Group tax refunds surrendered to this company"),
    (615, "Research and Development expenditure credits surrendered to this company"),
    (616, "Export: Yes — goods"),
    (617, "Export: Yes — services"),
    (618, "Export: No — neither"),
    (620, "Franked investment income/exempt ABGH distributions"),
    (625, "Number of 51% group companies"),
    (
        630,
        "should have made (whether it has or not) instalment payments as a large company under the Corporation Tax (instalment Payments) Regulations 1998",
    ),
    (
        631,
        "Should have made (whether it has or not) instalment payments as a very large company under the Corporation Tax (instalment Payments) Regulations 1998",
    ),
    (635, "is within a group payments arrangement for the period"),
    (640, "has written down or sold intangible assets"),
    (645, "has made cross-border royalty payments"),
    (647, "Eat Out to Help Out Scheme: reimbursed discounts included as taxable income"),
    (
        650,
        "Put an X in box 650 if the claim is made by a small or medium-sized enterprise (SME), including a SME subcontractor to a large company",
    ),
    (655, "Put an X in box 655 if the claim is made by a large company"),
    (656, "An R&D claim notification form has been submitted"),
    (657, "An additional information form has been submitted"),
    (659, "R&D expenditure qualifying for SME R&D relief"),
    (660, "R&D enhanced expenditure"),
    (665, "Creative enhanced expenditure"),
    (670, "R&D and creative enhanced expenditure"),
    (675, "R&D enhanced expenditure of a SME on work sub contracted to it by a large company"),
    (680, "Vaccines research expenditure"),
    (685, "Enter the total enhanced expenditure"),
    (690, "Annual investment allowance"),
    (691, "Machinery/plant super-deduction — Capital allowances"),
    (692, "Machinery/plant super-deduction — Balancing charges"),
    (693, "Machinery and plant — special rate allowance — Capital allowances"),
    (694, "Machinery and plant — special rate allowance — Balancing charges"),
    (695, "Machinery and plant — special rate pool - Capital allowance"),
    (700, "Machinery and plant — special rate pool - Balancing charges"),
    (705, "Machinery and plant — main pool - Capital allowance"),
    (710, "Machinery and plant — main pool - Balancing charges"),
    (711, "Structures and buildings - Capital allowances"),
    (715, "Business premises renovation - Capital allowances"),
    (720, "Business premises renovation - Balancing charges"),
    (725, "Other allowances and charges - Capital allowances"),
    (730, "Other allowances and charges - Balancing charges"),
    (713, "Electric charge points - Capital allowances"),
    (714, "Electric charge points - Balancing charges"),
    (721, "Enterprise zones - Capital allowances"),
    (722, "Enterprise zones - Balancing charges"),
    (723, "Zero emissions goods vehicles - Capital allowances"),
    (724, "Zero emissions goods vehicles - Balancing charges"),
    (726, "Zero emissions cars - Capital allowances"),
    (727, "Zero emissions cars - Balancing charges"),
    (735, "Annual Investment Allowance - Capital allowances"),
    (736, "Structures and buildings"),
    (740, "Business premises renovation - Capital allowances"),
    (745, "Business premises renovation - Balancing charges"),
    (741, "Machinery and plant — super-deduction - Capital allowances"),
    (742, "Machinery and plant — super-deduction - Balancing charges"),
    (743, "Machinery and plant — special rate allowance - Capital allowances"),
    (744, "Machinery and plant — special rate allowance - Balancing charges"),
    (750, "Other allowances and charges - Capital allowances"),
    (755, "Other allowances and charges - Balancing charges"),
    (737, "Electric charge points - Capital allowances"),
    (738, "Electric charge points - Balancing charges"),
    (746, "Enterprise Zones - Capital allowances"),
    (747, "Enterprise Zones - Balancing charges"),
    (748, "Zero emissions goods vehicles - Capital allowances"),
    (749, "Zero emissions goods vehicles - Balancing charges"),
    (751, "Zero emissions cars - Capital allowances"),
    (752, "Zero emissions cars - Balancing charges"),
    (760, "Machinery and plant on which first-year allowance is claimed"),
    (765, "Designated environmentally friendly machinery and plant"),
    (770, "Machinery and plant on long-life assets and integral features"),
    (771, "Structures and buildings"),
    (772, "Machinery and plant — super-deduction"),
    (773, "Machinery and plant — special rate allowance"),
    (775, "Other machinery and plant"),
    (780, "Losses of trades carried on wholly or partly in the UK"),
    (
        785,
        "Losses of trades carried on wholly or partly in the UK (maximum available for surrender as group relief)",
    ),
    (790, "Losses of trades carried on wholly outside the UK (amount)"),
    (795, "Non-trade deficits on loan relationships and derivative contracts (amount)"),
    (
        800,
        "Non-trade deficits on loan relationships and derivative contracts (maximum available for surrender as group relief)",
    ),
    (805, "UK property business losses (amount)"),
    (810, "UK property business losses (maximum available for surrender as group relief)"),
    (815, "Overseas property business losses (amount)"),
    (820, "Losses from miscellaneous transactions (amount)"),
    (825, "Capital losses (amount)"),
    (830, "Non-trading losses on intangible fixed assets (amount)"),
    (
        835,
        "Non-trading losses on intangible fixed assets (maximum available for surrender as group relief)",
    ),
    (840, "Non-trade capital allowances (maximum available for surrender as group relief)"),
    (845, "Qualifying donations (maximum available for surrender as group relief)"),
    (
        850,
        "Management expenses (amount)855 Management expenses (maximum available for surrender as group relief)",
    ),
    (856, "Amount of group relief claimed which relates to NI trading losses used against rest of UK/mainstream profits"),
    (857, "Amount of group relief claimed which relates to NI trading losses used against NI trading profits"),
    (858, "Amount of group relief claimed which relates to rest of UK/mainstream losses used against NI trading profits"),
    (860, "Small Repayments"),
    (865, "Repayment of Corporation Tax"),
    (870, "Repayment of Income Tax"),
    (875, "Payable Research and Development Tax Credit"),
    (880, "Payable research and development expenditure credit"),
    (885, "Payable creative tax credit"),
    (890, "Payable land remediation or life assurance company tax credit"),
    (895, "Payable capital allowances first-year tax credit"),
    (900, "The following amount is to be surrendered"),
    (905, "Joint notice is attached"),
    (910, "Joint notice will follow"),
    (915, "Please stop repayment of the following amount until we send you the Notice"),
    (920, "Name of bank or building society"),
    (925, "Branch sort code"),
    (930, "Account number"),
    (935, "Name of account"),
    (940, "Building society reference"),
    (945, "enter your status, for example company secretary or authorised agent"),
    (950, "enter the name of your company"),
    (955, "enter the name of the nominated person"),
    (960, "enter the address of the nominated person"),
    (965, "enter your reference for the nominated person"),
    (970, "enter your name"),
    (975, "Name"),
    (980, "Date"),
    (985, "Status"),
];

/// The value held by a CT600 form field.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum FieldValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

/// A single entry in the intermediary box map.
///
/// `box_id -> BoxValue { box_id, description, value }`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BoxValue {
    pub box_id: u16,
    pub description: &'static str,
    pub value: Option<FieldValue>,
}

/// Company header values: boxes 1, 2, 3, 4, 30 and 35.
///
/// All of these are mandatory on the CT600, so every field is non-optional.
#[derive(Debug, Clone, PartialEq)]
pub struct CompanyFormValues {
    /// Box 1 — Company name.
    pub company_name: String,
    /// Box 2 — Company registration number.
    pub company_number: String,
    /// Box 3 — Tax reference.
    pub tax_reference: String,
    /// Box 4 — Type of company.
    pub type_of_company: u8,
    /// Box 30 — Start of return.
    pub start: NaiveDate,
    /// Box 35 — End of return.
    pub end: NaiveDate,
}

impl CompanyFormValues {
    /// Derive the company header boxes from a computed [`Frs105CorpTax`].
    pub fn from_tax(tax: &Frs105CorpTax) -> Self {
        Self {
            company_name: tax.company_name().to_string(),
            company_number: tax.company_number().to_string(),
            tax_reference: tax.tax_reference().to_string(),
            type_of_company: tax.type_of_company(),
            start: tax.start(),
            end: tax.end(),
        }
    }
}

/// Typed CT600 form values.
///
/// Fields are non-optional where the CT600 specification makes the box
/// mandatory; tick boxes that are not always completed, and figures that may
/// not apply (such as R&D enhanced expenditure), are `Option`al.
#[derive(Debug, Clone, PartialEq)]
pub struct Ct600FormValues {
    pub company: CompanyFormValues,
    /// Box 40 — Repayments this period.
    pub repayments_this_period: bool,
    /// Box 80 — Attached accounts and computations for this period.
    pub attached_accounts: bool,
    /// Box 145 — Total turnover from trade.
    pub turnover: f64,
    /// Box 155 — Trading profits.
    pub trading_profits: f64,
    /// Box 165 — Net trading profits.
    pub net_trading_profits: f64,
    /// Box 235 — Profits before other deductions and reliefs.
    pub profits_before_other_deductions_and_reliefs: f64,
    /// Box 300 — Profits before qualifying donations and group relief.
    pub profits_before_charges_and_group_relief: f64,
    /// Box 315 — Profits chargeable to Corporation Tax.
    pub profits_chargeable_to_corporation_tax: f64,
    /// Box 330 — FY1.
    pub fy1_year: i32,
    /// Box 335 — FY1 Profit 1.
    pub fy1_profit: f64,
    /// Box 340 — FY1 Rate of Tax 1.
    pub fy1_tax_rate: f64,
    /// Box 345 — FY1 Tax 1.
    pub fy1_tax: f64,
    /// Box 380 — FY2.
    pub fy2_year: i32,
    /// Box 385 — FY2 Profit 1.
    pub fy2_profit: f64,
    /// Box 390 — FY2 Rate of Tax 1.
    pub fy2_tax_rate: f64,
    /// Box 395 — FY2 Tax 1.
    pub fy2_tax: f64,
    /// Box 430 — Corporation Tax.
    pub corporation_tax: f64,
    /// Box 440 — Corporation Tax chargeable.
    pub corporation_tax_chargeable: f64,
    /// Box 475 — Net Corporation Tax liability.
    pub net_corporation_tax_liability: f64,
    /// Box 510 — Tax chargeable.
    pub tax_chargeable: f64,
    /// Box 525 / 528 — Self-assessment of tax payable.
    pub tax_payable: f64,
    /// Box 650 — SME R&D claim.
    pub sme_rnd_claim: bool,
    /// Box 660 / 670 — R&D enhanced expenditure (only when a claim is made).
    pub sme_rnd_expenditure: Option<f64>,
    /// Box 690 — Annual investment allowance.
    pub annual_investment_allowance: f64,
}

impl Ct600FormValues {
    /// Derive the form values from a computed [`Frs105CorpTax`].
    pub fn from_tax(tax: &Frs105CorpTax) -> Self {
        Self {
            company: CompanyFormValues::from_tax(tax),
            repayments_this_period: false,
            attached_accounts: true,
            turnover: tax.turnover_revenue(),
            trading_profits: tax.net_trading_profits(),
            net_trading_profits: tax.net_trading_profits(),
            profits_before_other_deductions_and_reliefs: tax
                .profits_before_other_deductions_and_reliefs(),
            profits_before_charges_and_group_relief: tax
                .profits_before_charges_and_group_relief(),
            profits_chargeable_to_corporation_tax: tax
                .total_profits_chargeable_to_corporation_tax(),
            fy1_year: tax.fy1(),
            fy1_profit: tax.fy1_profit(),
            fy1_tax_rate: tax.fy1_tax_rate(),
            fy1_tax: tax.fy1_tax(),
            fy2_year: tax.fy2(),
            fy2_profit: tax.fy2_profit(),
            fy2_tax_rate: tax.fy2_tax_rate(),
            fy2_tax: tax.fy2_tax(),
            corporation_tax: tax.corporation_tax_chargeable(),
            corporation_tax_chargeable: tax.corporation_tax_chargeable(),
            net_corporation_tax_liability: tax.corporation_tax_chargeable(),
            tax_chargeable: tax.tax_chargeable(),
            tax_payable: tax.tax_payable(),
            sme_rnd_claim: true,
            sme_rnd_expenditure: tax.sme_rnd_expenditure_deduction(),
            annual_investment_allowance: tax.investment_allowance(),
        }
    }

    /// Override the company header values (e.g. with data fetched from
    /// Companies House).
    pub fn with_company(mut self, company: CompanyFormValues) -> Self {
        self.company = company;
        self
    }

    /// Serialize the typed values into the intermediary box map:
    /// `box_id -> { box_id, description, value }`.
    ///
    /// Every box in [`BOXES`] appears in the map; boxes without a computed
    /// value have `value: None`.
    pub fn to_map(&self) -> BTreeMap<u16, BoxValue> {
        BOXES
            .iter()
            .map(|&(box_id, description)| {
                let value = self.value_for(box_id);
                (box_id, BoxValue {
                    box_id,
                    description,
                    value,
                })
            })
            .collect()
    }

    /// The value for a single box, if this struct models it.
    fn value_for(&self, box_id: u16) -> Option<FieldValue> {
        let c = &self.company;
        match box_id {
            1 => Some(FieldValue::Text(c.company_name.clone())),
            2 => Some(FieldValue::Text(c.company_number.clone())),
            3 => Some(FieldValue::Text(c.tax_reference.clone())),
            4 => Some(FieldValue::Number(c.type_of_company as f64)),
            30 => Some(FieldValue::Text(format_date(&c.start))),
            35 => Some(FieldValue::Text(format_date(&c.end))),
            40 => Some(FieldValue::Bool(self.repayments_this_period)),
            80 => Some(FieldValue::Bool(self.attached_accounts)),
            145 => Some(FieldValue::Number(self.turnover)),
            155 => Some(FieldValue::Number(self.trading_profits)),
            165 => Some(FieldValue::Number(self.net_trading_profits)),
            235 => Some(FieldValue::Number(
                self.profits_before_other_deductions_and_reliefs,
            )),
            300 => Some(FieldValue::Number(
                self.profits_before_charges_and_group_relief,
            )),
            315 => Some(FieldValue::Number(
                self.profits_chargeable_to_corporation_tax,
            )),
            330 => Some(FieldValue::Text(self.fy1_year.to_string())),
            335 => Some(FieldValue::Number(self.fy1_profit)),
            340 => Some(FieldValue::Number(self.fy1_tax_rate)),
            345 => Some(FieldValue::Number(self.fy1_tax)),
            380 => Some(FieldValue::Text(self.fy2_year.to_string())),
            385 => Some(FieldValue::Number(self.fy2_profit)),
            390 => Some(FieldValue::Number(self.fy2_tax_rate)),
            395 => Some(FieldValue::Number(self.fy2_tax)),
            430 => Some(FieldValue::Number(self.corporation_tax)),
            440 => Some(FieldValue::Number(self.corporation_tax_chargeable)),
            475 => Some(FieldValue::Number(self.net_corporation_tax_liability)),
            510 => Some(FieldValue::Number(self.tax_chargeable)),
            525 => Some(FieldValue::Number(self.tax_payable)),
            528 => Some(FieldValue::Number(self.tax_payable)),
            650 => Some(FieldValue::Bool(self.sme_rnd_claim)),
            660 => self.sme_rnd_expenditure.map(FieldValue::Number),
            670 => self.sme_rnd_expenditure.map(FieldValue::Number),
            690 => Some(FieldValue::Number(self.annual_investment_allowance)),
            _ => None,
        }
    }
}

/// Format a date as `1 January 2020` (day without leading zero), matching the
/// display format used in the iXBRL output.
fn format_date(d: &NaiveDate) -> String {
    let day = d.format("%d").to_string();
    let month = d.format("%B").to_string();
    let year = d.format("%Y").to_string();
    format!(
        "{} {} {}",
        day.trim_start_matches('0'),
        month,
        year
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ixbrl::company::Company;
    use ixbrl::ixbrl_fmt::ParsedIxBrlFacts;

    fn sample_tax() -> Frs105CorpTax {
        let mut facts = ParsedIxBrlFacts::default();
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
            "1 January 2025".to_string(),
        );
        facts.non_numeric.insert(
            "ct-comp:PeriodOfAccountEndDate".to_string(),
            "31 December 2025".to_string(),
        );
        for (name, ctx, v) in [
            ("ct-comp:NetTradingProfits", "ctxt-3", 12345.0),
            ("ct-comp:FY1AmountOfProfitChargeableAtFirstRate", "ctxt-3", 6000.0),
            ("ct-comp:FY2AmountOfProfitChargeableAtFirstRate", "ctxt-3", 6345.0),
            ("ct-comp:FY1FirstRateOfTax", "ctxt-1", 19.0),
            ("ct-comp:FY2FirstRateOfTax", "ctxt-1", 19.0),
            ("ct-comp:FY1TaxAtFirstRate", "ctxt-3", 1140.0),
            ("ct-comp:FY2TaxAtFirstRate", "ctxt-3", 1205.55),
            ("ct-comp:CorporationTaxChargeable", "ctxt-3", 2345.55),
            ("ct-comp:TaxChargeable", "ctxt-3", 2345.55),
            ("ct-comp:TaxPayable", "ctxt-3", 2345.55),
            ("ct-comp:MainPoolAnnualInvestmentAllowance", "ctxt-2", 1000.0),
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

        let company = Company::new(
            "Acme Ltd",
            "1234567890",
            "09876543",
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        );
        Frs105CorpTax::from_parsed_facts(&facts, &company)
    }

    fn sample_values() -> Ct600FormValues {
        Ct600FormValues::from_tax(&sample_tax())
    }

    fn by_number(map: &BTreeMap<u16, BoxValue>, n: u16) -> &BoxValue {
        map.get(&n).unwrap_or_else(|| panic!("box {n} missing"))
    }

    #[test]
    fn test_company_values_from_tax() {
        let values = sample_values();
        assert_eq!(values.company.company_name, "Acme Ltd");
        assert_eq!(values.company.company_number, "09876543");
        assert_eq!(values.company.tax_reference, "1234567890");
        assert_eq!(values.company.type_of_company, 0);
        assert_eq!(
            values.company.start,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
        );
        assert_eq!(
            values.company.end,
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()
        );
    }

    #[test]
    fn test_to_map_company_boxes() {
        let map = sample_values().to_map();

        assert_eq!(
            by_number(&map, 1).value,
            Some(FieldValue::Text("Acme Ltd".to_string()))
        );
        assert_eq!(
            by_number(&map, 2).value,
            Some(FieldValue::Text("09876543".to_string()))
        );
        assert_eq!(
            by_number(&map, 3).value,
            Some(FieldValue::Text("1234567890".to_string()))
        );
        assert_eq!(by_number(&map, 4).value, Some(FieldValue::Number(0.0)));
        assert_eq!(
            by_number(&map, 30).value,
            Some(FieldValue::Text("1 January 2025".to_string()))
        );
        assert_eq!(
            by_number(&map, 35).value,
            Some(FieldValue::Text("31 December 2025".to_string()))
        );
        assert_eq!(by_number(&map, 40).value, Some(FieldValue::Bool(false)));
        assert_eq!(by_number(&map, 80).value, Some(FieldValue::Bool(true)));
        assert_eq!(by_number(&map, 650).value, Some(FieldValue::Bool(true)));
    }

    #[test]
    fn test_to_map_profits_and_tax_boxes() {
        let map = sample_values().to_map();

        assert_eq!(by_number(&map, 155).value, Some(FieldValue::Number(12345.0)));
        assert_eq!(by_number(&map, 165).value, Some(FieldValue::Number(12345.0)));
        assert_eq!(
            by_number(&map, 330).value,
            Some(FieldValue::Text("2025".to_string()))
        );
        assert_eq!(by_number(&map, 335).value, Some(FieldValue::Number(6000.0)));
        assert_eq!(by_number(&map, 340).value, Some(FieldValue::Number(19.0)));
        assert_eq!(by_number(&map, 345).value, Some(FieldValue::Number(1140.0)));
        assert_eq!(
            by_number(&map, 380).value,
            Some(FieldValue::Text("2026".to_string()))
        );
        assert_eq!(by_number(&map, 385).value, Some(FieldValue::Number(6345.0)));
        assert_eq!(by_number(&map, 390).value, Some(FieldValue::Number(19.0)));
        assert_eq!(by_number(&map, 395).value, Some(FieldValue::Number(1205.55)));
        assert_eq!(by_number(&map, 430).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(&map, 440).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(&map, 475).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(&map, 510).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(&map, 525).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(&map, 528).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(&map, 660).value, Some(FieldValue::Number(5000.0)));
        assert_eq!(by_number(&map, 670).value, Some(FieldValue::Number(5000.0)));
        assert_eq!(by_number(&map, 690).value, Some(FieldValue::Number(1000.0)));
    }

    #[test]
    fn test_to_map_unset_boxes_have_no_value() {
        let map = sample_values().to_map();

        for n in [50u16, 55, 60, 90, 95, 150, 160, 220, 530] {
            assert_eq!(by_number(&map, n).value, None, "box {n} should be unset");
        }
    }

    #[test]
    fn test_to_map_covers_every_box_in_catalogue() {
        let map = sample_values().to_map();

        assert_eq!(map.len(), BOXES.len());
        for &(number, description) in BOXES {
            let entry = by_number(&map, number);
            assert_eq!(entry.box_id, number);
            assert_eq!(entry.description, description);
        }

        // Box 986 (Energy Profits Levy) is the highest-numbered box in the
        // catalogue.
        let (last_id, last) = map.iter().next_back().unwrap();
        assert_eq!(*last_id, 986);
        assert_eq!(last.description, "Energy (Oil and Gas) Profits Levy");

        // Box 985 (Status) is present with its expected label.
        let status = by_number(&map, 985);
        assert_eq!(status.description, "Status");
    }

    #[test]
    fn test_with_company_overrides() {
        let company = CompanyFormValues {
            company_name: "CH Ltd".to_string(),
            company_number: "14510633".to_string(),
            tax_reference: "9876543210".to_string(),
            type_of_company: 1,
            start: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        };

        let values = sample_values().with_company(company);
        let map = values.to_map();

        assert_eq!(values.company.company_name, "CH Ltd");
        assert_eq!(
            by_number(&map, 1).value,
            Some(FieldValue::Text("CH Ltd".to_string()))
        );
        assert_eq!(
            by_number(&map, 2).value,
            Some(FieldValue::Text("14510633".to_string()))
        );
        assert_eq!(by_number(&map, 4).value, Some(FieldValue::Number(1.0)));
    }
}
