use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Context, DimensionMap, Document, Period, Segment, ValueKind, WorksheetKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS105Taxonomy {
    pub title: String,
    pub style: Option<String>,
    pub contexts: Vec<Context>,
    pub namespaces: HashMap<String, String>,
    pub schema: Vec<String>,
    pub document: Option<Document>,
    pub frs105: FRS105Specific,
    pub computations: Vec<FRS105Computation>,
    pub worksheets: Vec<FRS105Worksheet>,
    pub note_templates: FRS105NoteTemplates,
    pub metadata: Vec<FRS105Metadata>,
    pub tags: HashMap<String, String>,
    pub segment: FRS105SegmentMaps,
    pub sign_reversed: HashMap<String, bool>,
}

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
    pub current_period: u32,
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

impl super::TaxonomyValidation for FRS105Taxonomy {
    fn validate(&self) -> Result<(), super::TaxonomyError> {
        use super::TaxonomyError;

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

    fn validate_computation_dependencies(&self) -> Result<(), super::TaxonomyError> {
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
                return Err(super::TaxonomyError::CircularDependency(format!(
                    "Circular dependency detected: {} -> {}",
                    parent, child
                )));
            }
            visited.insert(key);
        }

        Ok(())
    }

    fn validate_metadata_completeness(&self) -> Result<(), super::TaxonomyError> {
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
                return Err(super::TaxonomyError::MissingMetadata(format!(
                    "{:?}",
                    required
                )));
            }
        }

        Ok(())
    }
}
