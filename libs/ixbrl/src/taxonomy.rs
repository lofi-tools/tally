use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================
// Core Taxonomy Enum
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "standard", content = "data")]
pub enum Taxonomy {
    #[serde(rename = "FRS-105")]
    FRS105(FRS105Taxonomy),

    #[serde(rename = "FRS-102")]
    FRS102(FRS102Taxonomy),

    #[serde(rename = "FRS-101")]
    FRS101(FRS101Taxonomy),

    #[serde(rename = "FRSSE")]
    FRSSE(FRSSETaxonomy),

    #[serde(rename = "IFRS")]
    IFRS(IFRSTaxonomy),

    #[serde(rename = "US-GAAP")]
    USGAAP(USGAAPTaxonomy),

    #[serde(rename = "other")]
    Other(GenericTaxonomy),
}

// ============================================================
// FRS-105 Specific Taxonomy
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS105Taxonomy {
    // Generic fields (common to all taxonomies)
    pub title: String,
    pub style: Option<String>,
    pub contexts: Vec<Context>,
    pub namespaces: HashMap<String, String>,
    pub schema: Vec<String>,
    pub document: Option<Document>,

    // FRS-105 specific fields
    pub frs105: FRS105Specific,
    pub computations: Vec<FRS105Computation>,
    pub worksheets: Vec<FRS105Worksheet>,
    pub note_templates: FRS105NoteTemplates,
    pub metadata: Vec<FRS105Metadata>,
    pub tags: HashMap<String, String>,
    pub segment: FRS105SegmentMaps,
    pub sign_reversed: HashMap<String, bool>,
}

// FRS-105 Specific Structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS105Specific {
    pub micro_entity_provisions: bool,
    pub audit_exempt: bool,
    pub abridged_accounts: bool,
    pub employee_threshold: EmployeeThreshold,
    pub turnover_threshold: f64,
    pub balance_sheet_threshold: f64,
    pub requires_directors_report: bool,
    pub requires_accountants_report: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeThreshold {
    pub current_period: u32, // ≤ 10 for FRS-105
    pub previous_period: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS105NoteTemplates {
    pub micro_entity_provisions: String,
    pub micro_entity_pl_provisions: String,
    pub small_company_audit_exempt: String,
    pub no_audit_required: String,
    pub members_agreed_abridged_accounts: String,
    pub directors_acknowledge: String,
    pub company_information: String,
    pub software_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS105Computation {
    pub id: String,
    pub description: String,
    pub kind: FRS105ComputationKind,
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
pub enum FRS105ComputationKind {
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "sum")]
    Sum,
    #[serde(rename = "group")]
    Group,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS105Worksheet {
    pub id: FRS105WorksheetId,
    pub kind: WorksheetKind,
    pub computations: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FRS105WorksheetId {
    #[serde(rename = "balance-sheet")]
    BalanceSheet,
    #[serde(rename = "profit-and-loss")]
    ProfitAndLoss,
    #[serde(rename = "fixed-assets")]
    FixedAssets,
    #[serde(rename = "share-capital")]
    ShareCapital,
    #[serde(rename = "staff-costs")]
    StaffCosts,
    #[serde(rename = "charges")]
    Charges,
    #[serde(rename = "financial-income")]
    FinancialIncome,
    #[serde(rename = "financial-costs")]
    FinancialCosts,
    #[serde(rename = "zero")]
    Zero,
    #[serde(rename = "corporation-tax")]
    CorporationTax,
    #[serde(rename = "cash")]
    Cash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS105Metadata {
    pub id: FRS105MetadataId,
    pub config: Option<String>,
    pub context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<ValueKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FRS105MetadataId {
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
    #[serde(rename = "officer11")]
    Officer11,
    #[serde(rename = "officer12")]
    Officer12,
    #[serde(rename = "officer13")]
    Officer13,
    #[serde(rename = "officer14")]
    Officer14,
    #[serde(rename = "officer15")]
    Officer15,
    #[serde(rename = "officer16")]
    Officer16,
    #[serde(rename = "officer17")]
    Officer17,
    #[serde(rename = "officer18")]
    Officer18,
    #[serde(rename = "officer19")]
    Officer19,
    #[serde(rename = "officer20")]
    Officer20,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FRS105SegmentMaps {
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
}

// ============================================================
// FRS-102 Specific Taxonomy
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS102Taxonomy {
    // Generic fields
    pub title: String,
    pub style: Option<String>,
    pub contexts: Vec<Context>,
    pub namespaces: HashMap<String, String>,
    pub schema: Vec<String>,
    pub document: Option<Document>,

    // FRS-102 specific
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
pub struct FRS102Metadata {
    pub id: FRS102MetadataId,
    pub config: Option<String>,
    pub context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<ValueKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FRS102MetadataId {
    // Same as FRS-105 but with additional ones
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
    // FRS-102 specific metadata
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
    // FRS-102 additional dimensions
    pub financial_instruments: DimensionMap,
    pub related_party: DimensionMap,
    pub segment: DimensionMap,
}

// ============================================================
// Other Taxonomies (Placeholders)
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS101Taxonomy {
    pub title: String,
    pub style: Option<String>,
    // ... FRS-101 specific fields
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRSSETaxonomy {
    pub title: String,
    pub style: Option<String>,
    // ... FRSSE specific fields
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IFRSTaxonomy {
    pub title: String,
    pub style: Option<String>,
    // ... IFRS specific fields
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct USGAAPTaxonomy {
    pub title: String,
    pub style: Option<String>,
    // ... US GAAP specific fields
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericTaxonomy {
    pub title: String,
    pub style: Option<String>,
    pub contexts: Vec<Context>,
    pub computations: Vec<GenericComputation>,
    pub metadata: Vec<GenericMetadata>,
    pub tags: HashMap<String, String>,
    pub schema: Vec<String>,
    pub namespaces: HashMap<String, String>,
    pub document: Option<Document>,
    pub sign_reversed: HashMap<String, bool>,
    pub segment: DimensionMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericComputation {
    pub id: String,
    pub description: String,
    pub kind: String,
    pub period: Period,
    pub inputs: Option<Vec<String>>,
    pub accounts: Option<Vec<String>>,
    pub segments: Option<Vec<Segment>>,
    pub reverse_sign: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericMetadata {
    pub id: String,
    pub config: Option<String>,
    pub context: String,
    pub kind: Option<ValueKind>,
    pub value: Option<String>,
}

// ============================================================
// Common Types (Shared Across Taxonomies)
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub id: String,
    #[serde(rename = "entity")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_scheme: Option<String>,
    #[serde(rename = "scheme")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<PeriodRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instant: Option<InstantRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<Segment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodRef {
    pub from: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstantRef {
    pub from: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub dimension: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Period {
    #[serde(rename = "in-year")]
    InYear,
    #[serde(rename = "at-end")]
    AtEnd,
    #[serde(rename = "at-start")]
    AtStart,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueKind {
    #[serde(rename = "date")]
    Date,
    #[serde(rename = "bool")]
    Bool,
    #[serde(rename = "count")]
    Count,
    #[serde(rename = "string")]
    String,
    #[serde(rename = "number")]
    Number,
    #[serde(rename = "currency")]
    Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorksheetKind {
    #[serde(rename = "simple")]
    Simple,
    #[serde(rename = "detailed")]
    Detailed,
    #[serde(rename = "narrative")]
    Narrative,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DimensionMap {
    pub dimension: String,
    pub map: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub elements: Vec<PageElement>,
    pub id: Option<String>,
    pub kind: DocumentKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentKind {
    #[serde(rename = "composite")]
    Composite,
    #[serde(rename = "page")]
    Page,
    #[serde(rename = "element")]
    Element,
    #[serde(rename = "html")]
    Html,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageElement {
    pub kind: ElementKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<Vec<PageElement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<HtmlRoot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worksheet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElementKind {
    #[serde(rename = "page")]
    Page,
    #[serde(rename = "html")]
    Html,
    #[serde(rename = "element")]
    Element,
    #[serde(rename = "composite")]
    Composite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtmlRoot {
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<HtmlContent>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HtmlContent {
    Simple(String),
    Tagged {
        tag: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        attributes: Option<HashMap<String, String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<Vec<HtmlContent>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        template: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ifdef: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        worksheet: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        element: Option<Box<HtmlContent>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        level: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
}

// ============================================================
// Validation and Utility Traits
// ============================================================

pub trait TaxonomyValidation {
    fn validate(&self) -> Result<(), TaxonomyError>;
    fn validate_computation_dependencies(&self) -> Result<(), TaxonomyError>;
    fn validate_metadata_completeness(&self) -> Result<(), TaxonomyError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TaxonomyError {
    #[error("Missing required computation: {0}")]
    MissingComputation(String),
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),
    #[error("Missing required metadata: {0}")]
    MissingMetadata(String),
    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),
    #[error("Inconsistent threshold: {0}")]
    InconsistentThreshold(String),
}

// ============================================================
// FRS-105 Specific Validation
// ============================================================

impl TaxonomyValidation for FRS105Taxonomy {
    fn validate(&self) -> Result<(), TaxonomyError> {
        // Validate FRS-105 specific requirements
        if self.frs105.employee_threshold.current_period > 10 {
            return Err(TaxonomyError::InvalidValue(
                "employee_threshold".to_string(),
                format!(
                    "FRS-105 requires ≤10 employees, got {}",
                    self.frs105.employee_threshold.current_period
                ),
            ));
        }

        if self.frs105.turnover_threshold > 632_000.0 {
            return Err(TaxonomyError::InvalidValue(
                "turnover_threshold".to_string(),
                format!(
                    "FRS-105 requires ≤£632,000 turnover, got £{}",
                    self.frs105.turnover_threshold
                ),
            ));
        }

        if self.frs105.balance_sheet_threshold > 316_000.0 {
            return Err(TaxonomyError::InvalidValue(
                "balance_sheet_threshold".to_string(),
                format!(
                    "FRS-105 requires ≤£316,000 balance sheet total, got £{}",
                    self.frs105.balance_sheet_threshold
                ),
            ));
        }

        // Validate required worksheets
        let required_worksheets = vec![
            FRS105WorksheetId::BalanceSheet,
            FRS105WorksheetId::ProfitAndLoss,
            FRS105WorksheetId::StaffCosts,
        ];

        for required in required_worksheets {
            if !self.worksheets.iter().any(|w| w.id == required) {
                return Err(TaxonomyError::MissingComputation(format!(
                    "Required FRS-105 worksheet: {:?}",
                    required
                )));
            }
        }

        self.validate_computation_dependencies()
    }

    fn validate_computation_dependencies(&self) -> Result<(), TaxonomyError> {
        let mut visited = std::collections::HashSet::new();
        let mut stack = Vec::new();

        for comp in &self.computations {
            if let Some(inputs) = &comp.inputs {
                for input in inputs {
                    stack.push((comp.id.clone(), input.clone()));
                }
            }
        }

        while let Some((parent, child)) = stack.pop() {
            let key = format!("{}->{}", parent, child);
            if visited.contains(&key) {
                return Err(TaxonomyError::CircularDependency(format!(
                    "Circular dependency detected: {} -> {}",
                    parent, child
                )));
            }
            visited.insert(key);
        }

        Ok(())
    }

    fn validate_metadata_completeness(&self) -> Result<(), TaxonomyError> {
        let required_metadata = vec![
            FRS105MetadataId::CompanyName,
            FRS105MetadataId::CompanyNumber,
            FRS105MetadataId::PeriodStart,
            FRS105MetadataId::PeriodEnd,
            FRS105MetadataId::BalanceSheetDate,
            FRS105MetadataId::AccountingStandards,
            FRS105MetadataId::AccountsType,
            FRS105MetadataId::AccountsStatus,
        ];

        for required in required_metadata {
            if !self.metadata.iter().any(|m| m.id == required) {
                return Err(TaxonomyError::MissingMetadata(format!("{:?}", required)));
            }
        }

        Ok(())
    }
}

// ============================================================
// Serialization/Deserialization Helpers
// ============================================================

impl Taxonomy {
    /// Parse from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Get the standard name
    pub fn standard_name(&self) -> &'static str {
        match self {
            Taxonomy::FRS105(_) => "FRS-105",
            Taxonomy::FRS102(_) => "FRS-102",
            Taxonomy::FRS101(_) => "FRS-101",
            Taxonomy::FRSSE(_) => "FRSSE",
            Taxonomy::IFRS(_) => "IFRS",
            Taxonomy::USGAAP(_) => "US-GAAP",
            Taxonomy::Other(_) => "Other",
        }
    }

    /// Get the title
    pub fn title(&self) -> &str {
        match self {
            Taxonomy::FRS105(t) => &t.title,
            Taxonomy::FRS102(t) => &t.title,
            Taxonomy::FRS101(t) => &t.title,
            Taxonomy::FRSSE(t) => &t.title,
            Taxonomy::IFRS(t) => &t.title,
            Taxonomy::USGAAP(t) => &t.title,
            Taxonomy::Other(t) => &t.title,
        }
    }

    /// Get the style
    pub fn style(&self) -> Option<&str> {
        match self {
            Taxonomy::FRS105(t) => t.style.as_deref(),
            Taxonomy::FRS102(t) => t.style.as_deref(),
            Taxonomy::FRS101(t) => t.style.as_deref(),
            Taxonomy::FRSSE(t) => t.style.as_deref(),
            Taxonomy::IFRS(t) => t.style.as_deref(),
            Taxonomy::USGAAP(t) => t.style.as_deref(),
            Taxonomy::Other(t) => t.style.as_deref(),
        }
    }

    /// Get metadata by ID (generic)
    pub fn get_metadata(&self, id: &str) -> Option<serde_json::Value> {
        match self {
            Taxonomy::FRS105(t) => t
                .metadata
                .iter()
                .find(|m| format!("{:?}", m.id) == id)
                .map(|m| serde_json::json!({ "id": format!("{:?}", m.id), "value": m.value })),
            Taxonomy::FRS102(t) => t
                .metadata
                .iter()
                .find(|m| format!("{:?}", m.id) == id)
                .map(|m| serde_json::json!({ "id": format!("{:?}", m.id), "value": m.value })),
            Taxonomy::FRS101(_) => None, // Implement as needed
            Taxonomy::FRSSE(_) => None,
            Taxonomy::IFRS(_) => None,
            Taxonomy::USGAAP(_) => None,
            Taxonomy::Other(t) => t
                .metadata
                .iter()
                .find(|m| m.id == id)
                .map(|m| serde_json::json!({ "id": m.id, "value": m.value })),
        }
    }

    /// Validate the taxonomy
    pub fn validate(&self) -> Result<(), TaxonomyError> {
        match self {
            Taxonomy::FRS105(t) => t.validate(),
            Taxonomy::FRS102(t) => t.validate(),
            // Implement for other taxonomies...
            _ => Ok(()), // Placeholder
        }
    }
}

// Implement validation for FRS102
impl TaxonomyValidation for FRS102Taxonomy {
    fn validate(&self) -> Result<(), TaxonomyError> {
        // FRS-102 validation logic
        // Check if it's small entities or full FRS-102
        let is_small = self.frs102.section1a_small_entities || self.frs102.section1_small_entities;

        if is_small {
            // Small entities have some exemptions
            if self.frs102.employee_threshold > 50 {
                return Err(TaxonomyError::InvalidValue(
                    "employee_threshold".to_string(),
                    format!(
                        "Section 1A requires ≤50 employees, got {}",
                        self.frs102.employee_threshold
                    ),
                ));
            }
        }

        self.validate_computation_dependencies()
    }

    fn validate_computation_dependencies(&self) -> Result<(), TaxonomyError> {
        // Similar to FRS-105 but with FRS-102 specific checks
        Ok(())
    }

    fn validate_metadata_completeness(&self) -> Result<(), TaxonomyError> {
        Ok(())
    }
}

// ============================================================
// Example Usage
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frs105_validation() {
        let tax = FRS105Taxonomy {
            title: "Test FRS-105".to_string(),
            style: None,
            contexts: vec![],
            namespaces: HashMap::new(),
            schema: vec![],
            document: None,
            frs105: FRS105Specific {
                micro_entity_provisions: true,
                audit_exempt: true,
                abridged_accounts: true,
                employee_threshold: EmployeeThreshold {
                    current_period: 5,
                    previous_period: 4,
                },
                turnover_threshold: 100_000.0,
                balance_sheet_threshold: 200_000.0,
                requires_directors_report: true,
                requires_accountants_report: false,
            },
            computations: vec![FRS105Computation {
                id: "turnover".to_string(),
                description: "Turnover".to_string(),
                kind: FRS105ComputationKind::Sum,
                period: Period::InYear,
                inputs: Some(vec!["main-income".to_string()]),
                accounts: None,
                segments: None,
                reverse_sign: None,
            }],
            worksheets: vec![
                FRS105Worksheet {
                    id: FRS105WorksheetId::BalanceSheet,
                    kind: WorksheetKind::Simple,
                    computations: vec!["fixed-assets".to_string()],
                    description: None,
                    note: None,
                },
                FRS105Worksheet {
                    id: FRS105WorksheetId::ProfitAndLoss,
                    kind: WorksheetKind::Simple,
                    computations: vec!["turnover".to_string()],
                    description: None,
                    note: None,
                },
                FRS105Worksheet {
                    id: FRS105WorksheetId::StaffCosts,
                    kind: WorksheetKind::Simple,
                    computations: vec!["staff-costs".to_string()],
                    description: None,
                    note: None,
                },
            ],
            note_templates: FRS105NoteTemplates {
                micro_entity_provisions: "Micro-entity provisions".to_string(),
                micro_entity_pl_provisions: "PL provisions".to_string(),
                small_company_audit_exempt: "Audit exempt".to_string(),
                no_audit_required: "No audit".to_string(),
                members_agreed_abridged_accounts: "Members agreed".to_string(),
                directors_acknowledge: "Directors acknowledge".to_string(),
                company_information: "Company info".to_string(),
                software_version: Some("1.0".to_string()),
            },
            metadata: vec![
                FRS105Metadata {
                    id: FRS105MetadataId::CompanyName,
                    config: Some("metadata.business.company-name".to_string()),
                    context: "report-period".to_string(),
                    kind: None,
                    value: None,
                },
                FRS105Metadata {
                    id: FRS105MetadataId::CompanyNumber,
                    config: Some("metadata.business.company-number".to_string()),
                    context: "report-period".to_string(),
                    kind: None,
                    value: None,
                },
                FRS105Metadata {
                    id: FRS105MetadataId::PeriodStart,
                    config: Some("metadata.accounting.periods.0.start".to_string()),
                    context: "report-date".to_string(),
                    kind: Some(ValueKind::Date),
                    value: None,
                },
                FRS105Metadata {
                    id: FRS105MetadataId::PeriodEnd,
                    config: Some("metadata.accounting.periods.0.end".to_string()),
                    context: "report-date".to_string(),
                    kind: Some(ValueKind::Date),
                    value: None,
                },
                FRS105Metadata {
                    id: FRS105MetadataId::BalanceSheetDate,
                    config: Some("metadata.accounting.balance-sheet-date".to_string()),
                    context: "end-of-reporting-period".to_string(),
                    kind: Some(ValueKind::Date),
                    value: None,
                },
                FRS105Metadata {
                    id: FRS105MetadataId::AccountingStandards,
                    config: None,
                    context: "accounting-standards".to_string(),
                    kind: None,
                    value: Some("micro-entities".to_string()),
                },
                FRS105Metadata {
                    id: FRS105MetadataId::AccountsType,
                    config: None,
                    context: "accounts-type".to_string(),
                    kind: None,
                    value: Some("abridged-accounts".to_string()),
                },
                FRS105Metadata {
                    id: FRS105MetadataId::AccountsStatus,
                    config: None,
                    context: "accounts-status".to_string(),
                    kind: None,
                    value: Some("audit-exempt-no-accountants-report".to_string()),
                },
            ],
            tags: HashMap::new(),
            segment: FRS105SegmentMaps::default(),
            sign_reversed: HashMap::new(),
        };

        // Should pass validation
        assert!(tax.validate().is_ok());
    }

    #[test]
    fn test_frs105_validation_fails_employee_threshold() {
        let mut tax = create_valid_frs105();
        tax.frs105.employee_threshold.current_period = 15; // Exceeds FRS-105 limit of 10

        let result = tax.validate();
        assert!(result.is_err());
        if let Err(TaxonomyError::InvalidValue(field, msg)) = result {
            assert_eq!(field, "employee_threshold");
            assert!(msg.contains("≤10 employees"));
        }
    }

    #[test]
    fn test_taxonomy_enum_serialization() {
        let tax = Taxonomy::FRS105(create_valid_frs105());
        let json = tax.to_json().unwrap();

        // Should contain the standard tag
        assert!(json.contains(r#""standard": "FRS-105""#));

        // Can deserialize back
        let deserialized = Taxonomy::from_json(&json).unwrap();
        match deserialized {
            Taxonomy::FRS105(_) => {}
            _ => panic!("Expected FRS-105"),
        }
    }

    #[test]
    fn test_taxonomy_enum_get_metadata() {
        let tax = Taxonomy::FRS105(create_valid_frs105());
        let metadata = tax.get_metadata("CompanyName");
        assert!(metadata.is_some());
    }

    fn create_valid_frs105() -> FRS105Taxonomy {
        FRS105Taxonomy {
            title: "Test FRS-105".to_string(),
            style: None,
            contexts: vec![],
            namespaces: HashMap::new(),
            schema: vec![],
            document: None,
            frs105: FRS105Specific {
                micro_entity_provisions: true,
                audit_exempt: true,
                abridged_accounts: true,
                employee_threshold: EmployeeThreshold {
                    current_period: 5,
                    previous_period: 4,
                },
                turnover_threshold: 100_000.0,
                balance_sheet_threshold: 200_000.0,
                requires_directors_report: true,
                requires_accountants_report: false,
            },
            computations: vec![FRS105Computation {
                id: "turnover".to_string(),
                description: "Turnover".to_string(),
                kind: FRS105ComputationKind::Sum,
                period: Period::InYear,
                inputs: Some(vec!["main-income".to_string()]),
                accounts: None,
                segments: None,
                reverse_sign: None,
            }],
            worksheets: vec![
                FRS105Worksheet {
                    id: FRS105WorksheetId::BalanceSheet,
                    kind: WorksheetKind::Simple,
                    computations: vec![],
                    description: None,
                    note: None,
                },
                FRS105Worksheet {
                    id: FRS105WorksheetId::ProfitAndLoss,
                    kind: WorksheetKind::Simple,
                    computations: vec![],
                    description: None,
                    note: None,
                },
                FRS105Worksheet {
                    id: FRS105WorksheetId::StaffCosts,
                    kind: WorksheetKind::Simple,
                    computations: vec![],
                    description: None,
                    note: None,
                },
            ],
            note_templates: FRS105NoteTemplates {
                micro_entity_provisions: "Micro-entity provisions".to_string(),
                micro_entity_pl_provisions: "PL provisions".to_string(),
                small_company_audit_exempt: "Audit exempt".to_string(),
                no_audit_required: "No audit".to_string(),
                members_agreed_abridged_accounts: "Members agreed".to_string(),
                directors_acknowledge: "Directors acknowledge".to_string(),
                company_information: "Company info".to_string(),
                software_version: None,
            },
            metadata: vec![
                FRS105Metadata {
                    id: FRS105MetadataId::CompanyName,
                    config: Some("metadata.business.company-name".to_string()),
                    context: "report-period".to_string(),
                    kind: None,
                    value: None,
                },
                FRS105Metadata {
                    id: FRS105MetadataId::CompanyNumber,
                    config: Some("metadata.business.company-number".to_string()),
                    context: "report-period".to_string(),
                    kind: None,
                    value: None,
                },
                FRS105Metadata {
                    id: FRS105MetadataId::PeriodStart,
                    config: Some("metadata.accounting.periods.0.start".to_string()),
                    context: "report-date".to_string(),
                    kind: Some(ValueKind::Date),
                    value: None,
                },
                FRS105Metadata {
                    id: FRS105MetadataId::PeriodEnd,
                    config: Some("metadata.accounting.periods.0.end".to_string()),
                    context: "report-date".to_string(),
                    kind: Some(ValueKind::Date),
                    value: None,
                },
                FRS105Metadata {
                    id: FRS105MetadataId::BalanceSheetDate,
                    config: Some("metadata.accounting.balance-sheet-date".to_string()),
                    context: "end-of-reporting-period".to_string(),
                    kind: Some(ValueKind::Date),
                    value: None,
                },
                FRS105Metadata {
                    id: FRS105MetadataId::AccountingStandards,
                    config: None,
                    context: "accounting-standards".to_string(),
                    kind: None,
                    value: Some("micro-entities".to_string()),
                },
                FRS105Metadata {
                    id: FRS105MetadataId::AccountsType,
                    config: None,
                    context: "accounts-type".to_string(),
                    kind: None,
                    value: Some("abridged-accounts".to_string()),
                },
                FRS105Metadata {
                    id: FRS105MetadataId::AccountsStatus,
                    config: None,
                    context: "accounts-status".to_string(),
                    kind: None,
                    value: Some("audit-exempt-no-accountants-report".to_string()),
                },
            ],
            tags: HashMap::new(),
            segment: FRS105SegmentMaps::default(),
            sign_reversed: HashMap::new(),
        }
    }
}
