//! In-process validation of generated iXBRL documents against the UK Joint
//! Filing Validation Checks (JFCVC v4.0) and the FRC / HMRC taxonomy schemas,
//! without a Python / Arelle dependency.
//!
//! The checks are driven by two data sources:
//!
//! - [`rules::Rules`] — the JFCVC rule matrix extracted from Arelle's
//!   `validate/UK` plugin (`rules/jfcvc-v4.json`): mandatory items per
//!   taxonomy type, the per-category dispatch, statement text patterns,
//!   generic dimension requirements, and the numeric / inline-XBRL
//!   conventions.
//! - [`taxonomy::Taxonomy`] — the schema-level concept table parsed from the
//!   FRC / HMRC XSDs ([`taxonomy::Taxonomy::from_directory`]) or the embedded
//!   generated subset ([`taxonomy::Taxonomy::embedded`]).
//!
//! Typical use:
//!
//! ```no_run
//! use validate_uk::Validator;
//!
//! let validator = Validator::new();
//! let html = std::fs::read_to_string("accounts.html").unwrap();
//! let report = validator.validate(&html);
//! assert!(report.is_ok(), "validation errors: {:?}", report.issues);
//! ```
//!
//! See the crate-level docs of [`checks`] for the (deliberate) divergences
//! from Arelle — most notably that ct-comp computation documents are
//! recognised and skip the accounts mandatory-items rule, and that the full
//! XBRL 2.1 schema-validation pass is approximated by targeted checks (keep
//! Arelle or the filing gateway for final sign-off).

pub mod checks;
pub mod document;
pub mod report;
pub mod rules;
pub mod taxonomy;

pub use report::{Issue, IssueKind, Report};
pub use rules::Rules;
pub use taxonomy::Taxonomy;

/// A ready-to-use validator with the embedded JFCVC rules and UK taxonomy
/// concept table.
#[derive(Debug, Clone)]
pub struct Validator {
    pub taxonomy: Taxonomy,
    pub rules: Rules,
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator {
    /// The embedded rules + taxonomy subset table (covers every concept the
    /// report generators emit — no external files needed).
    pub fn new() -> Self {
        Validator {
            taxonomy: Taxonomy::embedded(),
            rules: Rules::embedded(),
        }
    }

    /// A validator over a full taxonomy parsed from a downloaded FRC / HMRC
    /// XSD directory (unknown concepts are then reported).
    pub fn with_taxonomy_dir(dir: &std::path::Path) -> Result<Self, String> {
        Ok(Validator {
            taxonomy: Taxonomy::from_directory(dir)?,
            rules: Rules::embedded(),
        })
    }

    /// Validate an iXBRL document (accounts or computation) and return the
    /// report.  An [`Issue`] with [`IssueKind::Error`] means the document
    /// fails the same checks Arelle's `validate/UK` plugin runs.
    pub fn validate(&self, html: &str) -> Report {
        self.validate_lang(html, checks::Lang::English)
    }

    /// Validate with an explicit report language (Welsh statement text
    /// patterns are applied when [`checks::Lang::Welsh`]).
    pub fn validate_lang(&self, html: &str, lang: checks::Lang) -> Report {
        let doc = document::parse(html);
        checks::validate(&doc, &self.taxonomy, &self.rules, lang)
    }

    /// Validate an already-parsed document.
    pub fn validate_document(&self, doc: &document::Document) -> Report {
        checks::validate(doc, &self.taxonomy, &self.rules, checks::Lang::English)
    }
}
