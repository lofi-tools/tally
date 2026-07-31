//! CT600 form field values.
//!
//! This module models the individual boxes of the HMRC CT600 corporation tax
//! return.  It mirrors the Python reference implementation's `to_values()`
//! helper: each box is represented by a [`Definition`] holding its box number,
//! label and — where known — a value.
//!
//! Unlike the Python reference, which re-parses the generated iXBRL document
//! to recover these values, [`to_values`] derives them directly from a computed
//! [`Frs105CorpTax`].

use chrono::NaiveDate;
use ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax;

/// A single CT600 form field (box).
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub number: u16,
    pub label: &'static str,
    pub value: Option<FieldValue>,
}

impl Definition {
    pub fn new(number: u16, label: &'static str) -> Self {
        Self {
            number,
            label,
            value: None,
        }
    }

    pub fn set(mut self, value: FieldValue) -> Self {
        self.value = Some(value);
        self
    }

    pub fn set_opt(mut self, value: Option<FieldValue>) -> Self {
        self.value = value;
        self
    }
}

/// The value held by a CT600 form field.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Bool(bool),
    Number(f64),
    Text(String),
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

/// Derive the CT600 form field values from a computed [`Frs105CorpTax`].
///
/// This is the Rust equivalent of the Python reference's `to_values()` method
/// on the CT600 class.
pub fn to_values(tax: &Frs105CorpTax) -> Vec<Definition> {
    let text = |v: &str| FieldValue::Text(v.to_string());
    let num = |v: f64| FieldValue::Number(v);
    let date = |d: &NaiveDate| FieldValue::Text(format_date(d));

    vec![
        Definition::new(1, "Company name").set(text(tax.company_name())),
        Definition::new(2, "Company registration number").set(text(tax.company_number())),
        Definition::new(3, "Tax reference").set(text(tax.tax_reference())),
        Definition::new(4, "Type of company").set(num(tax.type_of_company() as f64)),
        Definition::new(30, "Start of return").set(date(&tax.start())),
        Definition::new(35, "End of return").set(date(&tax.end())),
        Definition::new(40, "Repayments this period").set(FieldValue::Bool(false)),
        Definition::new(50, "Making more than one return now"),
        Definition::new(55, "Estimated figures"),
        Definition::new(60, "Company part of a group that is not small"),
        Definition::new(65, "Notice of disclosable avoidance schemes"),
        Definition::new(70, "Compensating adjustment claimed"),
        Definition::new(75, "Company qualifies for SME exemption"),
        Definition::new(80, "Attached accounts and computations for this period")
            .set(FieldValue::Bool(true)),
        Definition::new(85, "Attached accounts and computations for a different period"),
        Definition::new(90, "Reason for not attaching accounts"),
        Definition::new(95, "CT600A - Loans and arrangements"),
        Definition::new(100, "CT600B - Controlled foreign companies"),
        Definition::new(105, "CT600C - Group & consortium"),
        Definition::new(110, "CT600D - Insurance"),
        Definition::new(115, "CT600E - CASCs"),
        Definition::new(120, "CT600F - Tonnage tax"),
        Definition::new(125, "CT600G - Northern Ireland"),
        Definition::new(130, "CT600H - Cross-border royalties"),
        Definition::new(135, "CT600I - Ring fence trades"),
        Definition::new(140, "CT600J - Tax avoidance schemes"),
        Definition::new(141, "CT600K - Restitution"),
        Definition::new(142, "CT600L - R&D"),
        Definition::new(143, "CT600M - Freeports"),
        Definition::new(144, "CT600N - Residential property developer tax"),
        Definition::new(145, "Total turnover from trade").set(num(tax.turnover_revenue())),
        Definition::new(150, "Banks and other financial concerns"),
        Definition::new(155, "Trading profits").set(num(tax.net_trading_profits())),
        Definition::new(160, "Trading losses brought forward against profits"),
        Definition::new(165, "Net trading profits").set(num(tax.net_trading_profits())),
        Definition::new(170, "Bank, building society or other interest, and profits from non-trading loan relationships"),
        Definition::new(172, "Box 170 net of carrying back deficit"),
        Definition::new(175, "Annual payments not otherwise charged to Corporation Tax and from which Income Tax has not been deducted"),
        Definition::new(180, "Non-exempt dividends or distributions from non-UK resident companies"),
        Definition::new(185, "Income from which Income Tax has been deducted"),
        Definition::new(190, "Income from a property business"),
        Definition::new(195, "Non-trading gains on intangible fixed assets"),
        Definition::new(200, "Tonnage Tax profits"),
        Definition::new(205, "Income not falling under any other heading"),
        Definition::new(210, "Gross chargeable gains"),
        Definition::new(215, "Allowable losses including losses brought forward"),
        Definition::new(220, "Net chargeable gains"),
        Definition::new(225, "Losses brought forward against certain investment income"),
        Definition::new(230, "Non-trade deficits on loan relationships (including interest), and derivative contracts (financial instruments) brought forward set against non-trading profits"),
        Definition::new(235, "Profits before other deductions and reliefs")
            .set(num(tax.profits_before_other_deductions_and_reliefs())),
        Definition::new(240, "Losses on unquoted shares"),
        Definition::new(245, "Management expenses"),
        Definition::new(250, "UK property business losses for this or previous accounting period"),
        Definition::new(255, "Capital allowances for the purpose of management of the business"),
        Definition::new(260, "Non-trade deficits for this accounting period from loan relationships and derivative contracts (financial instruments)"),
        Definition::new(263, "Carried forward non-trade deficits from loan relationships and derivative contracts (financial instruments)"),
        Definition::new(265, "Non-trading losses on intangible fixed assets"),
        Definition::new(275, "Trading losses of this or a later accounting period"),
        Definition::new(280, "Put an X in box 280 if amounts carried back from later accounting periods are included in box 275"),
        Definition::new(285, "Trading losses carried forward and claimed against total profits"),
        Definition::new(290, "Non-trade capital allowances"),
        Definition::new(295, "Total of deductions and reliefs"),
        Definition::new(300, "Profits before qualifying donations and group relief")
            .set(num(tax.profits_before_charges_and_group_relief())),
        Definition::new(305, "Qualifying donations"),
        Definition::new(310, "Group relief"),
        Definition::new(312, "Group relief for carried forward losses"),
        Definition::new(315, "Profits chargeable to Corporation Tax")
            .set(num(tax.total_profits_chargeable_to_corporation_tax())),
        Definition::new(320, "Ring fence profits included"),
        Definition::new(325, "Northern Ireland profits included"),
        Definition::new(326, "Number of associated companies in this period"),
        Definition::new(327, "Number of associated companies in the 1st FY"),
        Definition::new(328, "Number of associated companies in the 2nd FY"),
        Definition::new(329, "Chargeable at the small profit rate"),
        Definition::new(330, "FY1").set(text(&tax.fy1().to_string())),
        Definition::new(335, "FY1 Profit 1").set(num(tax.fy1_profit())),
        Definition::new(340, "FY1 Rate of Tax 1").set(num(tax.fy1_tax_rate())),
        Definition::new(345, "FY1 Tax 1").set(num(tax.fy1_tax())),
        Definition::new(350, "FY1 Profit 2"),
        Definition::new(355, "FY1 Rate of Tax 2"),
        Definition::new(360, "FY1 Tax 2"),
        Definition::new(365, "FY1 Profit 3"),
        Definition::new(370, "FY1 Rate of Tax 3"),
        Definition::new(375, "FY1 Tax 3"),
        Definition::new(380, "FY2").set(text(&tax.fy2().to_string())),
        Definition::new(385, "FY2 Profit 1").set(num(tax.fy2_profit())),
        Definition::new(390, "FY2 Rate of Tax 1").set(num(tax.fy2_tax_rate())),
        Definition::new(395, "FY2 Tax 1").set(num(tax.fy2_tax())),
        Definition::new(400, "FY2 Profit 2"),
        Definition::new(405, "FY2 Rate of Tax 2"),
        Definition::new(410, "FY2 Tax 2"),
        Definition::new(415, "FY2 Profit 3"),
        Definition::new(420, "FY2 Rate of Tax 3"),
        Definition::new(425, "FY2 Tax 3"),
        Definition::new(430, "Corporation Tax").set(num(tax.corporation_tax_chargeable())),
        Definition::new(435, "Marginal relief for ring fence trades"),
        Definition::new(440, "Corporation Tax chargeable").set(num(tax.corporation_tax_chargeable())),
        Definition::new(445, "Community Investment relief"),
        Definition::new(450, "Double Taxation Relief"),
        Definition::new(455, "Put an X in box 455 if box 450 includes an underlying Rate relief claim"),
        Definition::new(460, "Put an X in box 460 if box 450 includes any amount carried back from a later period"),
        Definition::new(465, "Advance Corporation Tax"),
        Definition::new(470, "Total reliefs and deduction in terms of tax"),
        Definition::new(471, "CJRS and Job Support Scheme received"),
        Definition::new(472, "CJRS and Job Support Scheme entitlement"),
        Definition::new(473, "CJRS overpayment already assessed or voluntary disclosed"),
        Definition::new(474, "Other Coronavirus overpayments"),
        Definition::new(986, "Energy (Oil and Gas) Profits Levy"),
        Definition::new(475, "Net Corporation Tax liability").set(num(tax.corporation_tax_chargeable())),
        Definition::new(480, "Tax payable on loans and arrangements to participators"),
        Definition::new(485, "Put an X in box 485 if you completed box A70 in the supplementary pages CT600A"),
        Definition::new(490, "CFC tax payable"),
        Definition::new(495, "Bank Levy payable"),
        Definition::new(496, "Bank surcharge payable"),
        Definition::new(497, "Residential Property Developer Tax repayable"),
        Definition::new(500, "CFC tax and bank Levy payable"),
        Definition::new(501, "EOGPL payable"),
        Definition::new(505, "Supplementary charge (ring fence trades) payable"),
        Definition::new(510, "Tax chargeable").set(num(tax.tax_chargeable())),
        Definition::new(515, "Income Tax deducted from gross income included in profits"),
        Definition::new(520, "Income Tax repayable to the company"),
        Definition::new(525, "Self-assessment of tax payable before restitution tax and coronavirus support scheme overpayments").set(num(tax.tax_payable())),
        Definition::new(526, "Coronavirus support schemes overpayment now due"),
        Definition::new(527, "Restitution tax"),
        Definition::new(528, "Self-assessment of tax payable").set(num(tax.tax_payable())),
        Definition::new(530, "Research and Development credit"),
        Definition::new(535, "Not currently used"),
        Definition::new(540, "Creative tax credit"),
        Definition::new(545, "Total of R&D credit and creative tax credit"),
        Definition::new(550, "Land remediation tax credit"),
        Definition::new(555, "Life assurance company tax credit"),
        Definition::new(560, "Total land remediation and life assurance company tax credit"),
        Definition::new(565, "Capital allowances first-year tax credit"),
        Definition::new(570, "Surplus Research and Development credits or creative tax credit payable"),
        Definition::new(575, "Land remediation or life assurance company tax credit payable"),
        Definition::new(580, "Capital allowances first-year tax credit payable"),
        Definition::new(585, "Ring fence Corporation Tax included and 590 Ring fence supplementary charge included"),
        Definition::new(586, "NI Corporation Tax included"),
        Definition::new(595, "Tax already paid (and not already repaid)"),
        Definition::new(600, "Tax outstanding"),
        Definition::new(605, "Tax overpaid including surplus or payable credits"),
        Definition::new(610, "Group tax refunds surrendered to this company"),
        Definition::new(615, "Research and Development expenditure credits surrendered to this company"),
        Definition::new(616, "Export: Yes — goods"),
        Definition::new(617, "Export: Yes — services"),
        Definition::new(618, "Export: No — neither"),
        Definition::new(620, "Franked investment income/exempt ABGH distributions"),
        Definition::new(625, "Number of 51% group companies"),
        Definition::new(630, "should have made (whether it has or not) instalment payments as a large company under the Corporation Tax (instalment Payments) Regulations 1998"),
        Definition::new(631, "Should have made (whether it has or not) instalment payments as a very large company under the Corporation Tax (instalment Payments) Regulations 1998"),
        Definition::new(635, "is within a group payments arrangement for the period"),
        Definition::new(640, "has written down or sold intangible assets"),
        Definition::new(645, "has made cross-border royalty payments"),
        Definition::new(647, "Eat Out to Help Out Scheme: reimbursed discounts included as taxable income"),
        Definition::new(650, "Put an X in box 650 if the claim is made by a small or medium-sized enterprise (SME), including a SME subcontractor to a large company").set(FieldValue::Bool(true)),
        Definition::new(655, "Put an X in box 655 if the claim is made by a large company"),
        Definition::new(656, "An R&D claim notification form has been submitted"),
        Definition::new(657, "An additional information form has been submitted"),
        Definition::new(659, "R&D expenditure qualifying for SME R&D relief"),
        Definition::new(660, "R&D enhanced expenditure")
            .set_opt(tax.sme_rnd_expenditure_deduction().map(FieldValue::Number)),
        Definition::new(665, "Creative enhanced expenditure"),
        Definition::new(670, "R&D and creative enhanced expenditure")
            .set_opt(tax.sme_rnd_expenditure_deduction().map(FieldValue::Number)),
        Definition::new(675, "R&D enhanced expenditure of a SME on work sub contracted to it by a large company"),
        Definition::new(680, "Vaccines research expenditure"),
        Definition::new(685, "Enter the total enhanced expenditure"),
        Definition::new(690, "Annual investment allowance").set(num(tax.investment_allowance())),
        Definition::new(691, "Machinery/plant super-deduction — Capital allowances"),
        Definition::new(692, "Machinery/plant super-deduction — Balancing charges"),
        Definition::new(693, "Machinery and plant — special rate allowance — Capital allowances"),
        Definition::new(694, "Machinery and plant — special rate allowance — Balancing charges"),
        Definition::new(695, "Machinery and plant — special rate pool - Capital allowance"),
        Definition::new(700, "Machinery and plant — special rate pool - Balancing charges"),
        Definition::new(705, "Machinery and plant — main pool - Capital allowance"),
        Definition::new(710, "Machinery and plant — main pool - Balancing charges"),
        Definition::new(711, "Structures and buildings - Capital allowances"),
        Definition::new(715, "Business premises renovation - Capital allowances"),
        Definition::new(720, "Business premises renovation - Balancing charges"),
        Definition::new(725, "Other allowances and charges - Capital allowances"),
        Definition::new(730, "Other allowances and charges - Balancing charges"),
        Definition::new(713, "Electric charge points - Capital allowances"),
        Definition::new(714, "Electric charge points - Balancing charges"),
        Definition::new(721, "Enterprise zones - Capital allowances"),
        Definition::new(722, "Enterprise zones - Balancing charges"),
        Definition::new(723, "Zero emissions goods vehicles - Capital allowances"),
        Definition::new(724, "Zero emissions goods vehicles - Balancing charges"),
        Definition::new(726, "Zero emissions cars - Capital allowances"),
        Definition::new(727, "Zero emissions cars - Balancing charges"),
        Definition::new(735, "Annual Investment Allowance - Capital allowances"),
        Definition::new(736, "Structures and buildings"),
        Definition::new(740, "Business premises renovation - Capital allowances"),
        Definition::new(745, "Business premises renovation - Balancing charges"),
        Definition::new(741, "Machinery and plant — super-deduction - Capital allowances"),
        Definition::new(742, "Machinery and plant — super-deduction - Balancing charges"),
        Definition::new(743, "Machinery and plant — special rate allowance - Capital allowances"),
        Definition::new(744, "Machinery and plant — special rate allowance - Balancing charges"),
        Definition::new(750, "Other allowances and charges - Capital allowances"),
        Definition::new(755, "Other allowances and charges - Balancing charges"),
        Definition::new(737, "Electric charge points - Capital allowances"),
        Definition::new(738, "Electric charge points - Balancing charges"),
        Definition::new(746, "Enterprise Zones - Capital allowances"),
        Definition::new(747, "Enterprise Zones - Balancing charges"),
        Definition::new(748, "Zero emissions goods vehicles - Capital allowances"),
        Definition::new(749, "Zero emissions goods vehicles - Balancing charges"),
        Definition::new(751, "Zero emissions cars - Capital allowances"),
        Definition::new(752, "Zero emissions cars - Balancing charges"),
        Definition::new(760, "Machinery and plant on which first-year allowance is claimed"),
        Definition::new(765, "Designated environmentally friendly machinery and plant"),
        Definition::new(770, "Machinery and plant on long-life assets and integral features"),
        Definition::new(771, "Structures and buildings"),
        Definition::new(772, "Machinery and plant — super-deduction"),
        Definition::new(773, "Machinery and plant — special rate allowance"),
        Definition::new(775, "Other machinery and plant"),
        Definition::new(780, "Losses of trades carried on wholly or partly in the UK"),
        Definition::new(785, "Losses of trades carried on wholly or partly in the UK (maximum available for surrender as group relief)"),
        Definition::new(790, "Losses of trades carried on wholly outside the UK (amount)"),
        Definition::new(795, "Non-trade deficits on loan relationships and derivative contracts (amount)"),
        Definition::new(800, "Non-trade deficits on loan relationships and derivative contracts (maximum available for surrender as group relief)"),
        Definition::new(805, "UK property business losses (amount)"),
        Definition::new(810, "UK property business losses (maximum available for surrender as group relief)"),
        Definition::new(815, "Overseas property business losses (amount)"),
        Definition::new(820, "Losses from miscellaneous transactions (amount)"),
        Definition::new(825, "Capital losses (amount)"),
        Definition::new(830, "Non-trading losses on intangible fixed assets (amount)"),
        Definition::new(835, "Non-trading losses on intangible fixed assets (maximum available for surrender as group relief)"),
        Definition::new(840, "Non-trade capital allowances (maximum available for surrender as group relief)"),
        Definition::new(845, "Qualifying donations (maximum available for surrender as group relief)"),
        Definition::new(850, "Management expenses (amount)855 Management expenses (maximum available for surrender as group relief)"),
        Definition::new(856, "Amount of group relief claimed which relates to NI trading losses used against rest of UK/mainstream profits"),
        Definition::new(857, "Amount of group relief claimed which relates to NI trading losses used against NI trading profits"),
        Definition::new(858, "Amount of group relief claimed which relates to rest of UK/mainstream losses used against NI trading profits"),
        Definition::new(860, "Small Repayments"),
        Definition::new(865, "Repayment of Corporation Tax"),
        Definition::new(870, "Repayment of Income Tax"),
        Definition::new(875, "Payable Research and Development Tax Credit"),
        Definition::new(880, "Payable research and development expenditure credit"),
        Definition::new(885, "Payable creative tax credit"),
        Definition::new(890, "Payable land remediation or life assurance company tax credit"),
        Definition::new(895, "Payable capital allowances first-year tax credit"),
        Definition::new(900, "The following amount is to be surrendered"),
        Definition::new(905, "Joint notice is attached"),
        Definition::new(910, "Joint notice will follow"),
        Definition::new(915, "Please stop repayment of the following amount until we send you the Notice"),
        Definition::new(920, "Name of bank or building society"),
        Definition::new(925, "Branch sort code"),
        Definition::new(930, "Account number"),
        Definition::new(935, "Name of account"),
        Definition::new(940, "Building society reference"),
        Definition::new(945, "enter your status, for example company secretary or authorised agent"),
        Definition::new(950, "enter the name of your company"),
        Definition::new(955, "enter the name of the nominated person"),
        Definition::new(960, "enter the address of the nominated person"),
        Definition::new(965, "enter your reference for the nominated person"),
        Definition::new(970, "enter your name"),
        Definition::new(975, "Name"),
        Definition::new(980, "Date"),
        Definition::new(985, "Status"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
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

    #[test]
    fn test_to_values_company_boxes() {
        let tax = sample_tax();
        let values = to_values(&tax);

        let by_number = |n: u16| values.iter().find(|d| d.number == n).unwrap();

        assert_eq!(
            by_number(1).value,
            Some(FieldValue::Text("Acme Ltd".to_string()))
        );
        assert_eq!(
            by_number(2).value,
            Some(FieldValue::Text("09876543".to_string()))
        );
        assert_eq!(
            by_number(3).value,
            Some(FieldValue::Text("1234567890".to_string()))
        );
        assert_eq!(by_number(4).value, Some(FieldValue::Number(0.0)));
        assert_eq!(
            by_number(30).value,
            Some(FieldValue::Text("1 January 2025".to_string()))
        );
        assert_eq!(
            by_number(35).value,
            Some(FieldValue::Text("31 December 2025".to_string()))
        );
        assert_eq!(by_number(40).value, Some(FieldValue::Bool(false)));
        assert_eq!(by_number(80).value, Some(FieldValue::Bool(true)));
        assert_eq!(by_number(650).value, Some(FieldValue::Bool(true)));
    }

    #[test]
    fn test_to_values_profits_and_tax_boxes() {
        let tax = sample_tax();
        let values = to_values(&tax);

        let by_number = |n: u16| values.iter().find(|d| d.number == n).unwrap();

        assert_eq!(by_number(155).value, Some(FieldValue::Number(12345.0)));
        assert_eq!(by_number(165).value, Some(FieldValue::Number(12345.0)));
        assert_eq!(by_number(330).value, Some(FieldValue::Text("2025".to_string())));
        assert_eq!(by_number(335).value, Some(FieldValue::Number(6000.0)));
        assert_eq!(by_number(340).value, Some(FieldValue::Number(19.0)));
        assert_eq!(by_number(345).value, Some(FieldValue::Number(1140.0)));
        assert_eq!(by_number(380).value, Some(FieldValue::Text("2026".to_string())));
        assert_eq!(by_number(385).value, Some(FieldValue::Number(6345.0)));
        assert_eq!(by_number(430).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(440).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(475).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(510).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(525).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(528).value, Some(FieldValue::Number(2345.55)));
        assert_eq!(by_number(660).value, Some(FieldValue::Number(5000.0)));
        assert_eq!(by_number(670).value, Some(FieldValue::Number(5000.0)));
        assert_eq!(by_number(690).value, Some(FieldValue::Number(1000.0)));
    }

    #[test]
    fn test_to_values_unset_boxes_have_no_value() {
        let tax = sample_tax();
        let values = to_values(&tax);

        for n in [50u16, 55, 60, 90, 95, 150, 160, 220, 530] {
            let d = values.iter().find(|d| d.number == n).unwrap();
            assert_eq!(d.value, None, "box {n} should be unset");
        }
    }

    #[test]
    fn test_to_values_has_expected_structure() {
        let tax = sample_tax();
        let values = to_values(&tax);

        // Box 985 (Status) is the last definition.
        let last = values.last().unwrap();
        assert_eq!(last.number, 985);
        assert_eq!(last.label, "Status");

        // Every definition has a label and a number.
        for d in &values {
            assert!(!d.label.is_empty(), "box {} missing label", d.number);
        }
    }

    #[test]
    fn test_definition_builder() {
        let d = Definition::new(1, "Company name");
        assert_eq!(d.number, 1);
        assert_eq!(d.value, None);

        let d = d.set(FieldValue::Text("Acme Ltd".to_string()));
        assert_eq!(d.value, Some(FieldValue::Text("Acme Ltd".to_string())));

        let d = Definition::new(2, "x").set_opt(None);
        assert_eq!(d.value, None);
    }
}
