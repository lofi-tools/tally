pub mod frs102;
pub mod frs105;
pub mod generic;
pub mod stubs;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use self::frs102::FRS102Taxonomy;
use self::frs105::FRS105Taxonomy;
use self::generic::GenericTaxonomy;
use self::stubs::{FRS101Taxonomy, FRSSETaxonomy, IFRSTaxonomy, USGAAPTaxonomy};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "standard", content = "data")]
pub enum Taxonomy {
    #[serde(rename = "FRS-105")]
    FRS105(Box<FRS105Taxonomy>),

    #[serde(rename = "FRS-102")]
    FRS102(Box<FRS102Taxonomy>),

    #[serde(rename = "FRS-101")]
    FRS101(FRS101Taxonomy),

    #[serde(rename = "FRSSE")]
    FRSSE(FRSSETaxonomy),

    #[serde(rename = "IFRS")]
    IFRS(IFRSTaxonomy),

    #[serde(rename = "US-GAAP")]
    USGAAP(USGAAPTaxonomy),

    #[serde(rename = "other")]
    Other(Box<GenericTaxonomy>),
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
pub struct HtmlTagged {
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<HtmlContent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ifdef: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worksheet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<Box<HtmlContent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HtmlContent {
    Simple(String),
    Tagged(Box<HtmlTagged>),
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
// Serialization/Deserialization Helpers
// ============================================================

impl Taxonomy {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

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
            Taxonomy::FRS101(_) => None,
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

    pub fn validate(&self) -> Result<(), TaxonomyError> {
        match self {
            Taxonomy::FRS105(t) => t.validate(),
            Taxonomy::FRS102(t) => t.validate(),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taxonomy::frs105::*;
    use std::collections::HashMap;

    #[test]
    fn test_frs105_validation() {
        let tax = create_valid_frs105();
        assert!(tax.validate().is_ok());
    }

    #[test]
    fn test_frs105_validation_fails_employee_threshold() {
        let mut tax = create_valid_frs105();
        tax.frs105.employee_threshold.current_period = 15;

        let result = tax.validate();
        assert!(result.is_err());
        if let Err(TaxonomyError::InvalidValue(field, msg)) = result {
            assert_eq!(field, "employee_threshold");
            assert!(msg.contains("≤10 employees"));
        }
    }

    #[test]
    fn test_taxonomy_enum_serialization() {
        let tax = Taxonomy::FRS105(Box::new(create_valid_frs105()));
        let json = tax.to_json().unwrap();

        assert!(json.contains(r#""standard": "FRS-105""#));

        let deserialized = Taxonomy::from_json(&json).unwrap();
        match deserialized {
            Taxonomy::FRS105(_) => {}
            _ => panic!("Expected FRS-105"),
        }
    }

    #[test]
    fn test_taxonomy_enum_get_metadata() {
        let tax = Taxonomy::FRS105(Box::new(create_valid_frs105()));
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
