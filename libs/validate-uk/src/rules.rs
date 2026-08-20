//! The JFCVC rule matrix, loaded from `rules/jfcvc-v4.json`.
//!
//! The JSON is the extracted JFCVC v4.0 rule set (mandatory items per
//! taxonomy type, per-category dispatch, statement text patterns, generic
//! dimension requirements, single-concept evaluations and the composite
//! code evaluations) taken from Arelle's `validate/UK` plugin.  Keeping it in
//! a data file means the checker is data-driven: rule changes (e.g. a new
//! JFCVC release) are edits to the JSON, not to Rust code.

use std::collections::HashMap;

use serde::Deserialize;

const RULES_JSON: &str = include_str!("../rules/jfcvc-v4.json");

/// The full JFCVC rule matrix.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rules {
    pub taxonomy_types: HashMap<String, TaxonomyTypeRules>,
    pub generic_dimension_validations: HashMap<String, GdvRule>,
    pub text_patterns: HashMap<String, TextPatternRule>,
    pub single_concept_evaluations: HashMap<String, SingleConceptRule>,
    pub code_evaluations: HashMap<String, CodeEvaluation>,
    pub categories: HashMap<String, CategoryRules>,
}

/// Mandatory concepts for one taxonomy type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyTypeRules {
    pub mandatory: Vec<String>,
    #[serde(default)]
    pub must_have_one: Vec<String>,
}

/// A generic-dimension validation: when a context uses a member of the
/// dimension (e.g. `Director1`), the document must also contain one of
/// `facts` tagged on a context using that member, with non-empty text.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GdvRule {
    #[serde(default)]
    pub min: Option<u32>,
    #[serde(default)]
    pub max: Option<u32>,
    pub facts: Vec<String>,
}

/// A statement text pattern: the fact tagged `concept` must contain, for
/// every group, at least one phrase (word-bounded, case-insensitive).
#[derive(Debug, Clone, Deserialize)]
pub struct TextPatternRule {
    pub concept: String,
    pub en: LangPattern,
    #[serde(default)]
    pub cy: Option<LangPattern>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LangPattern {
    pub groups: Vec<Vec<String>>,
}

/// A single-concept evaluation (e.g. the medium-company statement).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleConceptRule {
    pub concept: String,
    #[serde(default)]
    pub warning: bool,
}

/// Composite code evaluations (audit facts, director signing, profit/loss,
/// charity audit, group balance-sheet dates).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeEvaluation {
    pub r#type: String,
    #[serde(default)]
    pub concept: Option<String>,
}

/// The checks to run for one accounts category (e.g. `unauditedMicroCompany`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRules {
    #[serde(default)]
    pub must: Vec<String>,
    #[serde(default)]
    pub at_least_one: Vec<String>,
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub fallback: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl Rules {
    /// The embedded JFCVC v4.0 rule matrix.
    pub fn embedded() -> Self {
        serde_json::from_str(RULES_JSON).expect("embedded jfcvc rules are valid JSON")
    }

    /// Load rules from a JSON file (for testing / external rule versions).
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("invalid rules JSON: {e}"))
    }

    pub fn taxonomy_type(&self, name: &str) -> Option<&TaxonomyTypeRules> {
        self.taxonomy_types.get(name)
    }

    pub fn text_pattern(&self, code: &str) -> Option<&TextPatternRule> {
        self.text_patterns.get(code)
    }

    pub fn single_concept(&self, code: &str) -> Option<&SingleConceptRule> {
        self.single_concept_evaluations.get(code)
    }

    pub fn code_evaluation(&self, code: &str) -> Option<&CodeEvaluation> {
        self.code_evaluations.get(code)
    }

    pub fn category(&self, name: &str) -> Option<&CategoryRules> {
        self.categories.get(name)
    }

    /// Whether a code's failure is reported as a warning rather than an error.
    pub fn is_warning_code(&self, category: Option<&CategoryRules>, code: &str) -> bool {
        if let Some(cat) = category {
            if cat.warnings.iter().any(|w| w == code) {
                return true;
            }
        }
        self.single_concept(code).map(|s| s.warning).unwrap_or(false)
    }
}
