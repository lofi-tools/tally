use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Context, DimensionMap, Document, Period, Segment, ValueKind, WorksheetKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS102Taxonomy {
    pub title: String,
    pub style: Option<String>,
    pub contexts: Vec<Context>,
    pub namespaces: HashMap<String, String>,
    pub schema: Vec<String>,
    pub document: Option<Document>,
    pub frs102: FRS102Specific,
    pub computations: Vec<FRS102Computation>,
    pub worksheets: Vec<FRS102Worksheet>,
    pub note_templates: FRS102NoteTemplates,
    pub metadata: Vec<FRS102Metadata>,
    pub tags: HashMap<String, String>,
    pub segment: FRS102SegmentMaps,
    pub sign_reversed: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS102Specific {
    pub section1_small_entities: bool,
    pub section1a_small_entities: bool,
    pub audit_exempt: bool,
    pub employee_threshold: u32,
    pub turnover_threshold: f64,
    pub balance_sheet_threshold: f64,
    pub requires_cash_flow: bool,
    pub requires_related_party_disclosures: bool,
    pub requires_segment_reporting: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS102Computation {
    pub id: String,
    pub description: String,
    pub kind: FRS102ComputationKind,
    pub period: Period,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accounts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<Segment>>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse_sign: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FRS102ComputationKind {
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "sum")]
    Sum,
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "adjustment")]
    Adjustment,
    #[serde(rename = "subtotal")]
    Subtotal,
    #[serde(rename = "total")]
    Total,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS102Worksheet {
    pub id: FRS102WorksheetId,
    pub kind: WorksheetKind,
    pub computations: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FRS102WorksheetId {
    #[serde(rename = "balance-sheet")]
    BalanceSheet,
    #[serde(rename = "profit-and-loss")]
    ProfitAndLoss,
    #[serde(rename = "cash-flow")]
    CashFlow,
    #[serde(rename = "fixed-assets")]
    FixedAssets,
    #[serde(rename = "intangible-assets")]
    IntangibleAssets,
    #[serde(rename = "investments")]
    Investments,
    #[serde(rename = "inventories")]
    Inventories,
    #[serde(rename = "debtors")]
    Debtors,
    #[serde(rename = "creditors")]
    Creditors,
    #[serde(rename = "provisions")]
    Provisions,
    #[serde(rename = "share-capital")]
    ShareCapital,
    #[serde(rename = "reserves")]
    Reserves,
    #[serde(rename = "staff-costs")]
    StaffCosts,
    #[serde(rename = "directors-remuneration")]
    DirectorsRemuneration,
    #[serde(rename = "taxation")]
    Taxation,
    #[serde(rename = "related-parties")]
    RelatedParties,
    #[serde(rename = "operating-leases")]
    OperatingLeases,
    #[serde(rename = "financial-instruments")]
    FinancialInstruments,
    #[serde(rename = "segment-reporting")]
    SegmentReporting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS102NoteTemplates {
    pub accounting_policies: String,
    pub critical_estimates: String,
    pub going_concern: String,
    pub property_plant_equipment: String,
    pub intangible_assets: String,
    pub investments: String,
    pub inventories: String,
    pub debtors: String,
    pub creditors: String,
    pub provisions: String,
    pub share_capital: String,
    pub reserves: String,
    pub staff_costs: String,
    pub directors_remuneration: String,
    pub taxation: String,
    pub related_parties: String,
    pub operating_leases: String,
    pub financial_instruments: String,
    pub segment_reporting: String,
    pub post_balance_sheet_events: String,
    pub contingencies: String,
    pub commitments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS102Metadata {
    pub id: FRS102MetadataId,
    pub config: Option<String>,
    pub context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<ValueKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FRS102MetadataId {
    #[serde(rename = "report-title")]
    ReportTitle,
    #[serde(rename = "report-date")]
    ReportDate,
    #[serde(rename = "authorised-date")]
    AuthorisedDate,
    #[serde(rename = "period-start")]
    PeriodStart,
    #[serde(rename = "period-end")]
    PeriodEnd,
    #[serde(rename = "company-name")]
    CompanyName,
    #[serde(rename = "company-number")]
    CompanyNumber,
    #[serde(rename = "vat-registration")]
    VatRegistration,
    #[serde(rename = "software-name")]
    SoftwareName,
    #[serde(rename = "software-version")]
    SoftwareVersion,
    #[serde(rename = "balance-sheet-date")]
    BalanceSheetDate,
    #[serde(rename = "activities")]
    Activities,
    #[serde(rename = "sic1")]
    Sic1,
    #[serde(rename = "sic2")]
    Sic2,
    #[serde(rename = "sic3")]
    Sic3,
    #[serde(rename = "sic4")]
    Sic4,
    #[serde(rename = "industry-sector")]
    IndustrySector,
    #[serde(rename = "is-dormant")]
    IsDormant,
    #[serde(rename = "trading-status")]
    TradingStatus,
    #[serde(rename = "accounting-standards")]
    AccountingStandards,
    #[serde(rename = "accounts-type")]
    AccountsType,
    #[serde(rename = "accounts-status")]
    AccountsStatus,
    #[serde(rename = "entity-legal-form")]
    EntityLegalForm,
    #[serde(rename = "entity-legal-country")]
    EntityLegalCountry,
    #[serde(rename = "entity-legal-date")]
    EntityLegalDate,
    #[serde(rename = "average-employees-this")]
    AverageEmployeesThis,
    #[serde(rename = "average-employees-previous")]
    AverageEmployeesPrevious,
    #[serde(rename = "officer1")]
    Officer1,
    #[serde(rename = "officer2")]
    Officer2,
    #[serde(rename = "officer3")]
    Officer3,
    #[serde(rename = "officer4")]
    Officer4,
    #[serde(rename = "officer5")]
    Officer5,
    #[serde(rename = "officer6")]
    Officer6,
    #[serde(rename = "officer7")]
    Officer7,
    #[serde(rename = "officer8")]
    Officer8,
    #[serde(rename = "officer9")]
    Officer9,
    #[serde(rename = "officer10")]
    Officer10,
    #[serde(rename = "signing-officer")]
    SigningOfficer,
    #[serde(rename = "signed-by")]
    SignedBy,
    #[serde(rename = "signers-name")]
    SignersName,
    #[serde(rename = "jurisdiction")]
    Jurisdiction,
    #[serde(rename = "contact-name")]
    ContactName,
    #[serde(rename = "contact-address1")]
    ContactAddress1,
    #[serde(rename = "contact-address2")]
    ContactAddress2,
    #[serde(rename = "contact-address3")]
    ContactAddress3,
    #[serde(rename = "contact-location")]
    ContactLocation,
    #[serde(rename = "contact-county")]
    ContactCounty,
    #[serde(rename = "contact-postcode")]
    ContactPostcode,
    #[serde(rename = "contact-email")]
    ContactEmail,
    #[serde(rename = "contact-phone-country")]
    ContactPhoneCountry,
    #[serde(rename = "contact-phone-area")]
    ContactPhoneArea,
    #[serde(rename = "contact-phone-number")]
    ContactPhoneNumber,
    #[serde(rename = "website-url")]
    WebsiteUrl,
    #[serde(rename = "website-description")]
    WebsiteDescription,
    #[serde(rename = "accountants-report-date")]
    AccountantsReportDate,
    #[serde(rename = "accountant-name")]
    AccountantName,
    #[serde(rename = "accountant-business")]
    AccountantBusiness,
    #[serde(rename = "accountant-address")]
    AccountantAddress,
    #[serde(rename = "auditors-report-date")]
    AuditorsReportDate,
    #[serde(rename = "auditor-name")]
    AuditorName,
    #[serde(rename = "auditor-business")]
    AuditorBusiness,
    #[serde(rename = "auditor-address")]
    AuditorAddress,
    #[serde(rename = "directors-report-date")]
    DirectorsReportDate,
    #[serde(rename = "directors-report-signing-officer")]
    DirectorsReportSigningOfficer,
    #[serde(rename = "is-revised")]
    IsRevised,
    #[serde(rename = "revised-auditors-report-date")]
    RevisedAuditorsReportDate,
    #[serde(rename = "directors-report-consistent-with-revised-accounts")]
    DirectorsReportConsistentWithRevisedAccounts,
    #[serde(rename = "section1-small-entities")]
    Section1SmallEntities,
    #[serde(rename = "section1a-small-entities")]
    Section1aSmallEntities,
    #[serde(rename = "audit-exempt")]
    AuditExempt,
    #[serde(rename = "requires-cash-flow")]
    RequiresCashFlow,
    #[serde(rename = "requires-related-party-disclosures")]
    RequiresRelatedPartyDisclosures,
    #[serde(rename = "requires-segment-reporting")]
    RequiresSegmentReporting,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FRS102SegmentMaps {
    pub accounting_standards: DimensionMap,
    pub accounts_status: DimensionMap,
    pub accounts_type: DimensionMap,
    pub countries_regions: DimensionMap,
    pub entity_legal_form: DimensionMap,
    pub equity: DimensionMap,
    pub industry_sector: DimensionMap,
    pub matures: DimensionMap,
    pub officer: DimensionMap,
    pub phone_number_type: DimensionMap,
    pub financial_instruments: DimensionMap,
    pub related_party: DimensionMap,
    pub segment: DimensionMap,
}

impl super::TaxonomyValidation for FRS102Taxonomy {
    fn validate(&self) -> Result<(), super::TaxonomyError> {
        let is_small = self.frs102.section1a_small_entities || self.frs102.section1_small_entities;

        if is_small && self.frs102.employee_threshold > 50 {
            return Err(super::TaxonomyError::InvalidValue(
                "employee_threshold".to_string(),
                format!(
                    "Section 1A requires ≤50 employees, got {}",
                    self.frs102.employee_threshold
                ),
            ));
        }

        self.validate_computation_dependencies()
    }

    fn validate_computation_dependencies(&self) -> Result<(), super::TaxonomyError> {
        Ok(())
    }

    fn validate_metadata_completeness(&self) -> Result<(), super::TaxonomyError> {
        Ok(())
    }
}
