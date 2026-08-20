//! The validation checks.
//!
//! Faithful ports of the checks Arelle's `validate/UK` plugin (JFCVC v4.0 +
//! HMRC iXBRL style guide v2.2) runs on generated documents, driven by the
//! rules data file ([`crate::rules`]) and the taxonomy concept table
//! ([`crate::taxonomy`]).
//!
//! Divergences from Arelle (both deliberate):
//! - **ct-comp computation documents are recognised** and skip the accounts
//!   mandatory-items rule.  Arelle's 2020-era namespace regex does not match
//!   the `http://www.hmrc.gov.uk/schemas/ct/comp` namespace, so it
//!   misclassifies computation filings as accounts and demands the whole
//!   FRS-2022 mandatory set (JFCVC.3312).  That is an Arelle artifact, not a
//!   real HMRC rule.
//! - The full XBRL 2.1 schema-validation pass (which Arelle performs on the
//!   whole instance) is approximated by targeted schema checks against the
//!   concept table; for final sign-off use Arelle or the filing gateway.

use std::collections::{HashMap, HashSet};

use crate::document::{normalise_spaces, Context, Document, Fact, Period};
use crate::report::{Issue, Report};
use crate::rules::{CategoryRules, Rules};
use crate::taxonomy::{PeriodType, Taxonomy};

pub const CH_ENTITY_SCHEME: &str = "http://www.companieshouse.gov.uk/";

/// Which taxonomy / document family a document belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Accounts,
    Computation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    English,
    Welsh,
}

impl Default for Lang {
    fn default() -> Self {
        Lang::English
    }
}

/// Detect whether the document is an accounts or a computation (ct-comp)
/// filing, from the schemaRef hrefs.
pub fn detect_doc_kind(doc: &Document) -> DocKind {
    for href in &doc.schema_refs {
        if href.contains("hmrc.gov.uk/schemas/ct/comp") || href.contains("govtalk.gov.uk/uk/fr/tax/uk-hmrc-ct") {
            return DocKind::Computation;
        }
    }
    // Fall back on the namespaces the facts are tagged with.
    if doc
        .facts
        .iter()
        .any(|f| f.qname.starts_with("ct-comp:") || f.qname.starts_with("dpl:"))
    {
        return DocKind::Computation;
    }
    DocKind::Accounts
}

/// Determine the JFCVC taxonomy type (`FRS-2022`, `FRS`, `ukGAAP`, `ukIFRS`,
/// `charities`) from the schemaRef hrefs, like Arelle's namespace sniffing.
pub fn taxonomy_type(doc: &Document) -> Option<&'static str> {
    let hrefs: Vec<&str> = doc.schema_refs.iter().map(|s| s.as_str()).collect();
    let joined = hrefs.join(" ");
    let all: String = doc
        .facts
        .iter()
        .map(|f| f.qname.clone())
        .collect::<Vec<_>>()
        .join(" ");
    let hay = format!("{joined} {all}");
    if hay.contains("xbrl.frc.org.uk/char/") || hay.contains("www.xbrl.org/uk/char/") {
        return Some("charities");
    }
    if hay.contains("xbrl.frc.org.uk/IFRS/") || hay.contains("www.xbrl.org/uk/ifrs/") {
        return Some("ukIFRS");
    }
    if hay.contains("www.xbrl.org/uk/gaap/") {
        return Some("ukGAAP");
    }
    if hay.contains("xbrl.frc.org.uk") {
        // FRS vs FRS-2022: the 2023-01-01 taxonomy renamed
        // AccountsTypeFullOrAbbreviated to AccountsType.
        if hay.contains("2023") {
            return Some("FRS-2022");
        }
        return Some("FRS");
    }
    None
}

/// The document's report language, from a `ReportPrincipalLanguage` fact
/// tagged on a context with `LanguagesDimension`/`Welsh` (like Arelle's
/// `ValidateUK._lang`).  Documents that don't declare a language default to
/// the caller-supplied language.
pub fn doc_lang(doc: &Document) -> Lang {
    for f in doc.facts.iter().filter(|f| f.local_name == "ReportPrincipalLanguage") {
        if let Some(ctx) = doc.contexts.get(&f.context_ref) {
            if ctx.member_for("LanguagesDimension").is_some_and(|m| m == "Welsh") {
                return Lang::Welsh;
            }
        }
    }
    Lang::English
}

/// Run every check applicable to the document.
pub fn validate(doc: &Document, taxonomy: &Taxonomy, rules: &Rules, lang: Lang) -> Report {
    // A document that declares Welsh (`ReportPrincipalLanguage` with
    // `LanguagesDimension`/Welsh) selects the Welsh statement-text patterns,
    // like Arelle's plugin.
    let lang = if doc_lang(doc) == Lang::Welsh {
        Lang::Welsh
    } else {
        lang
    };
    let mut ctx = Ctx {
        doc,
        taxonomy,
        rules,
        lang,
        report: Report::default(),
    };
    let kind = detect_doc_kind(doc);
    let txmy = taxonomy_type(doc);

    // Generic checks (both document kinds).
    ctx.check_inline_xbrl_hygiene();
    ctx.check_duplicate_facts();
    ctx.check_numeric_conventions();
    ctx.check_schema_conformance();
    ctx.check_companies_house_number();

    match kind {
        DocKind::Accounts => {
            if let Some(t) = txmy {
                ctx.check_mandatory_items(t);
            }
            ctx.check_generic_dimension_members();
            ctx.check_category_dispatch(txmy);
        }
        DocKind::Computation => {
            // JFCVC.3315 also applies to computation (officer dimensions).
            ctx.check_generic_dimension_members();
        }
    }
    ctx.report
}

struct Ctx<'a> {
    doc: &'a Document,
    taxonomy: &'a Taxonomy,
    rules: &'a Rules,
    lang: Lang,
    report: Report,
}

impl<'a> Ctx<'a> {
    fn error(&mut self, code: &str, message: String, location: Option<String>) {
        self.report.issues.push(
            Issue::error(code, message)
                .with_location(location.unwrap_or_else(|| "-".to_string())),
        );
    }

    fn warn(&mut self, code: &str, message: String, location: Option<String>) {
        self.report.issues.push(
            Issue::warning(code, message)
                .with_location(location.unwrap_or_else(|| "-".to_string())),
        );
    }

    fn facts(&self, local_name: &str) -> Vec<&Fact> {
        self.doc
            .facts
            .iter()
            .filter(|f| f.local_name == local_name)
            .collect()
    }

    /// A fact is "present and usable" (non-nil, known context) — the Rust
    /// analogue of Arelle's `_checkValidFact`.  Empty values are allowed
    /// here: fixed-item marker facts (e.g. `AccountingStandardsApplied`) are
    /// schema-valid with no value and still satisfy the mandatory-presence
    /// checks; value-specific checks are separate.
    fn is_valid_fact(&self, f: &Fact) -> bool {
        !f.is_nil && self.doc.contexts.contains_key(&f.context_ref)
    }

    fn context(&self, f: &Fact) -> Option<&Context> {
        self.doc.contexts.get(&f.context_ref)
    }

    fn has_valid_fact(&self, local_name: &str) -> bool {
        self.facts(local_name).iter().any(|f| self.is_valid_fact(f))
    }

    fn has_valid_fact_any(&self, names: &[&str]) -> bool {
        names.iter().any(|n| self.has_valid_fact(n))
    }

    /// The first dimension value for (concept, dimension), like Arelle's
    /// `accountStatus` / `accountsType` cached properties.
    fn dimension_value(&self, concept: &str, dimension: &str) -> Option<String> {
        for f in self.facts(concept) {
            if !self.is_valid_fact(f) {
                continue;
            }
            if let Some(ctx) = self.context(f) {
                if let Some(m) = ctx.member_for(dimension) {
                    return Some(m.to_string());
                }
            }
        }
        None
    }

    fn account_status(&self) -> Option<String> {
        self.dimension_value("AccountsStatusAuditedOrUnaudited", "AccountsStatusDimension")
    }

    fn accounts_type(&self) -> Option<String> {
        self.dimension_value("AccountsType", "AccountsTypeDimension")
            .or_else(|| {
                self.dimension_value(
                    "AccountsTypeFullOrAbbreviated",
                    "AccountsTypeDimension",
                )
            })
    }

    fn accounting_standards(&self) -> Option<String> {
        self.dimension_value(
            "AccountingStandardsApplied",
            "AccountingStandardsDimension",
        )
    }

    fn applicable_legislation(&self) -> Option<String> {
        self.dimension_value("ApplicableLegislation", "ApplicableLegislationDimension")
    }

    fn legal_form(&self) -> Option<String> {
        self.dimension_value("LegalFormEntity", "LegalFormEntityDimension")
    }

    fn scope_accounts(&self) -> Option<String> {
        self.dimension_value("ScopeAccounts", "ScopeAccountsDimension")
    }

    fn is_dormant(&self) -> bool {
        let facts = self.facts("EntityDormantTruefalse");
        if facts.is_empty() {
            return false;
        }
        facts.iter().all(|f| {
            !f.is_nil && self.is_valid_fact(f) && f.value.trim().eq_ignore_ascii_case("true")
        })
    }

    fn is_llp(&self) -> bool {
        self.legal_form().as_deref() == Some("LimitedLiabilityPartnershipLLP")
    }

    // ------------------------------------------------------------------
    // Generic checks
    // ------------------------------------------------------------------

    /// HMRC.SG.3.3 / HMRC.SG.3.8 / ix11.8.1.2: inline-XBRL hygiene (root
    /// element, scripts, javascript URLs, image sources, hidden header).
    fn check_inline_xbrl_hygiene(&mut self) {
        if !self.doc.header_in_display_none {
            self.warn(
                "ix11.8.1.2:headerDisplayNone",
                "Inline XBRL ix:header is recommended to be nested in a <div> with style display:none".into(),
                None,
            );
        }
        if !self.doc.root_tag.starts_with("html") {
            self.error(
                "HMRC.SG.3.3",
                format!(
                    "InlineXBRL root element <{}> MUST be html and have the xhtml namespace.",
                    self.doc.root_tag
                ),
                None,
            );
        }
        if self.doc.has_script_elements {
            self.error("HMRC.SG.3.3", "Script element is disallowed.".into(), None);
        }
        for (el, attr, value) in &self.doc.links {
            if value.to_ascii_lowercase().contains("javascript:") {
                self.error(
                    "HMRC.SG.3.3",
                    format!("Element {el} javascript {value:?} is disallowed."),
                    Some(attr.clone()),
                );
            }
            if el == "img"
                && !ALLOWED_IMG_PREFIXES
                    .iter()
                    .any(|p| value.starts_with(p))
            {
                self.error(
                    "HMRC.SG.3.8",
                    format!(
                        "Image scope must be base-64 encoded string (starting with data:image/*;base64), *=gif, jpeg or png.  src disallowed: {:?}.",
                        &value[..value.len().min(128)]
                    ),
                    Some(attr.clone()),
                );
            }
        }
        for css in self.doc.style_elements.iter().chain(
            self.doc
                .style_attributes
                .iter()
                .map(|(_, v)| v),
        ) {
            if let Some(url) = external_css_url(css) {
                self.error(
                    "HMRC.SG.3.8",
                    format!("Style has disallowed image reference: url({url:?})."),
                    None,
                );
            }
        }
    }

    /// JFCVC.3314: inconsistent duplicate fact values.
    fn check_duplicate_facts(&mut self) {
        // Group by (concept, context, unit) like Arelle's
        // `conceptContextUnitHash`.
        let mut groups: HashMap<(String, String, String), Vec<&Fact>> = HashMap::new();
        for f in &self.doc.facts {
            if f.is_nil {
                continue;
            }
            let unit = f.unit_ref.clone().unwrap_or_default();
            groups
                .entry((f.local_name.clone(), f.context_ref.clone(), unit))
                .or_default()
                .push(f);
        }
        for (key, facts) in groups {
            if facts.len() < 2 {
                continue;
            }
            if facts_consistent(&facts) {
                continue;
            }
            let values: Vec<String> = facts.iter().map(|f| f.value.clone()).collect();
            self.error(
                "JFCVC.3314",
                format!(
                    "Inconsistent duplicate fact values {}: {}.",
                    key.0,
                    values
                        .iter()
                        .map(|v| format!("\"{v}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Some(key.1),
            );
        }
    }

    /// HMRC.5.4 (precision attribute), HMRC.5.3 (negative value without a
    /// bracketed-negative label) and HMRC.SG.4.5 (insignificant digits).
    fn check_numeric_conventions(&mut self) {
        for f in &self.doc.facts {
            if !f.is_numeric {
                continue;
            }
            if f.precision.is_some() {
                self.error(
                    "HMRC.5.4",
                    format!(
                        "Numeric fact {} of context {} has a precision attribute.",
                        f.local_name, f.context_ref
                    ),
                    Some(f.context_ref.clone()),
                );
            }
            if let Some(decimals) = &f.decimals {
                if decimals != "INF" {
                    if let Some(insig) = insignificant_digits(&f.value, decimals, f.scale) {
                        self.error(
                            "HMRC.SG.4.5",
                            format!(
                                "Fact {} of context {} decimals {decimals} value {} has nonzero digits in insignificant portion {insig}.",
                                f.local_name, f.context_ref, f.value
                            ),
                            Some(f.context_ref.clone()),
                        );
                    }
                }
            }
        }
    }

    /// Schema-level checks driven by the taxonomy concept table.
    fn check_schema_conformance(&mut self) {
        // Build issues locally first: the per-fact lookups borrow `self`
        // immutably, so mutation happens after the loop.
        let mut issues: Vec<Issue> = Vec::new();
        for f in &self.doc.facts {
            let Some(concept) = self.taxonomy.concept(&f.local_name) else {
                if !self.taxonomy.is_subset {
                    issues.push(
                        Issue::warning(
                            "schema.unknownConcept",
                            format!("Concept {} is not defined in the loaded taxonomy.", f.local_name),
                        )
                        .with_location(f.context_ref.clone()),
                    );
                }
                continue;
            };
            let loc = f.context_ref.clone();
            let ctx = self.context(f);

            // fixedItemType concepts are empty markers; a value is a schema error.
            if concept.is_fixed() && !f.is_nil && !f.value.trim().is_empty() {
                issues.push(
                    Issue::error(
                        "schema.fixedValue",
                        format!(
                            "Concept {} is a fixed item type and must have no value, but has {:?}.",
                            f.local_name, f.value
                        ),
                    )
                    .with_location(loc.clone()),
                );
            }

            // periodType alignment.
            if let Some(period) = concept.period {
                if let Some(ctx) = ctx {
                    let ok = match (period, &ctx.period) {
                        (PeriodType::Instant, Period::Instant(_)) => true,
                        (PeriodType::Duration, Period::Duration(_, _)) => true,
                        (PeriodType::Instant, Period::Duration(_, _)) => false,
                        (PeriodType::Duration, Period::Instant(_)) => false,
                        (_, Period::Forever) => true,
                    };
                    if !ok {
                        issues.push(
                            Issue::error(
                                "schema.periodMismatch",
                                format!(
                                    "Concept {} has periodType {:?} but is tagged on a {} context.",
                                    f.local_name, period, period_kind(&ctx.period)
                                ),
                            )
                            .with_location(loc.clone()),
                        );
                    }
                }
            }

            if concept.is_domain_or_member() {
                continue;
            }

            // Numeric vs non-numeric element choice.
            if concept.is_numeric_type() && !f.is_numeric {
                issues.push(
                    Issue::error(
                        "schema.typeMismatch",
                        format!(
                            "Concept {} has numeric type {} but is tagged as ix:nonNumeric.",
                            f.local_name, concept.data_type
                        ),
                    )
                    .with_location(loc.clone()),
                );
            } else if !concept.is_numeric_type() && f.is_numeric && !concept.is_fixed() {
                issues.push(
                    Issue::error(
                        "schema.typeMismatch",
                        format!(
                            "Concept {} has non-numeric type {} but is tagged as ix:nonFraction.",
                            f.local_name, concept.data_type
                        ),
                    )
                    .with_location(loc.clone()),
                );
            }

            // XSD pattern facet (e.g. taxReferenceItemType must match
            // `[0-9]{10}`; taxDistrictItemType `[0-9]{3}`).  The taxonomy's
            // patterns are all `[0-9]{N}` digit-count forms; anything else is
            // skipped (never false-positives).
            if let Some(pattern) = concept.pattern.as_deref() {
                if !f.value.trim().is_empty() {
                    match digit_count_pattern(pattern) {
                        Some(n) if f.value.trim().chars().count() != n
                            || !f.value.trim().chars().all(|c| c.is_ascii_digit()) =>
                        {
                            issues.push(
                                Issue::error(
                                    "schema.patternValue",
                                    format!(
                                        "Concept {} has a value {:?} that does not match the pattern facet {:?} ({} digits).",
                                        f.local_name, f.value, pattern, n
                                    ),
                                )
                                .with_location(loc.clone()),
                            );
                        }
                        // pattern matched, or the pattern is an unsupported
                        // shape (Some(_) matched / None = skip)
                        _ => {}
                    }
                }
            }

            // XSD minLength facet (e.g. nonEmptyStringItemType: value must
            // not be empty).
            if let Some(min_len) = concept.min_length() {
                if !f.is_nil && f.value.trim().chars().count() < min_len {
                    issues.push(
                        Issue::error(
                            "schema.minLength",
                            format!(
                                "Concept {} has a value {:?} shorter than the minLength facet {}.",
                                f.local_name, f.value, min_len
                            ),
                        )
                        .with_location(loc.clone()),
                    );
                }
            }

            // gYear facts (e.g. FinancialYear1CoveredByTheReturn) must be a
            // four-digit year.
            if concept.data_type.ends_with("gYearItemType") && !f.value.trim().is_empty() {
                let v = f.value.trim();
                let ok = v.len() == 4 && v.chars().all(|c| c.is_ascii_digit());
                if !ok {
                    issues.push(
                        Issue::error(
                            "schema.gYearValue",
                            format!(
                                "Concept {} has a value {:?} that is not a four-digit year (gYearItemType).",
                                f.local_name, f.value
                            ),
                        )
                        .with_location(loc.clone()),
                    );
                }
            }

            // Boolean values.
            if concept.is_boolean_type() && !f.value.trim().is_empty() {
                let v = f.value.trim().to_ascii_lowercase();
                if v != "true" && v != "false" {
                    issues.push(
                        Issue::error(
                            "schema.booleanValue",
                            format!(
                                "Concept {} is booleanItemType but has value {:?}.",
                                f.local_name, f.value
                            ),
                        )
                        .with_location(loc.clone()),
                    );
                }
            }

            // Enumeration membership.
            if !concept.enums.is_empty() && !f.value.trim().is_empty() {
                if !concept.enums.iter().any(|e| e == f.value.trim()) {
                    issues.push(
                        Issue::error(
                            "schema.enumValue",
                            format!(
                                "Value {:?} for concept {} is not one of the enumerated values {:?}.",
                                f.value, f.local_name, concept.enums
                            ),
                        )
                        .with_location(loc.clone()),
                    );
                }
            }
        }
        self.report.issues.extend(issues);
    }

    /// JFCVC.3316: UKCompaniesHouseRegisteredNumber must match the entity
    /// identifier of every Companies House context.
    fn check_companies_house_number(&mut self) {
        if !self.doc.has_companies_house_context {
            return;
        }
        let mut issues: Vec<Issue> = Vec::new();
        for f in self.facts("UKCompaniesHouseRegisteredNumber") {
            if !self.is_valid_fact(f) {
                continue;
            }
            for ctx in self.doc.contexts.values() {
                if ctx.entity_scheme.as_deref() == Some(CH_ENTITY_SCHEME) {
                    if let Some(id) = &ctx.entity_identifier {
                        if f.value.trim() != id.trim() {
                            issues.push(
                                Issue::error(
                                    "JFCVC.3316",
                                    format!(
                                        "Context entity identifier {} does not match Company Reference Number (UKCompaniesHouseRegisteredNumber) {} (context id {})",
                                        id, f.value, ctx.id
                                    ),
                                )
                                .with_location(ctx.id.clone()),
                            );
                        }
                    }
                }
            }
        }
        self.report.issues.extend(issues);
    }

    // ------------------------------------------------------------------
    // JFCVC.3312 / atLeastOne
    // ------------------------------------------------------------------

    fn check_mandatory_items(&mut self, txmy: &str) {
        let Some(ttype) = self.rules.taxonomy_type(txmy) else {
            return;
        };
        let mandatory = &ttype.mandatory;
        let mut mandatory_facts: HashMap<String, Vec<&Fact>> = HashMap::new();
        for name in mandatory {
            mandatory_facts.insert(
                name.clone(),
                self.facts(name)
                    .into_iter()
                    .filter(|f| self.is_valid_fact(f))
                    .collect(),
            );
        }

        // EndDate: an instant context whose date equals the fact value.
        let mut end_date: Option<String> = None;
        let mut missing: Vec<&str> = Vec::new();
        for f in mandatory_facts.get("EndDateForPeriodCoveredByReport").into_iter().flatten() {
            if let Some(ctx) = self.context(f) {
                if ctx.is_instant() && date_equal(ctx.instant_date(), fact_date(f).as_deref()) {
                    end_date = fact_date(f);
                    break;
                }
            }
        }
        if end_date.is_none() {
            missing.push("EndDateForPeriodCoveredByReport");
        }

        // StartDate: an instant context whose date equals endDate.
        let mut start_date: Option<String> = None;
        if let Some(end) = &end_date {
            for f in mandatory_facts.get("StartDateForPeriodCoveredByReport").into_iter().flatten() {
                if let Some(ctx) = self.context(f) {
                    if ctx.is_instant() && date_equal(ctx.instant_date(), Some(end)) {
                        start_date = fact_date(f);
                        break;
                    }
                }
            }
        }
        if start_date.is_none() {
            missing.push("StartDateForPeriodCoveredByReport");
        }

        let mut concepts_to_check: Vec<&str> = mandatory.iter().map(|s| s.as_str()).collect();
        if self.doc.has_companies_house_context {
            // Like Arelle's checkFacts, the registered-number fact joins the
            // mandatory set when Companies House contexts are present.
            concepts_to_check.push("UKCompaniesHouseRegisteredNumber");
            mandatory_facts.insert(
                "UKCompaniesHouseRegisteredNumber".to_string(),
                self.facts("UKCompaniesHouseRegisteredNumber")
                    .into_iter()
                    .filter(|f| self.is_valid_fact(f))
                    .collect(),
            );
        }

        if let (Some(start), Some(end)) = (&start_date, &end_date) {
            for concept in concepts_to_check {
                if matches!(
                    concept,
                    "StartDateForPeriodCoveredByReport" | "EndDateForPeriodCoveredByReport"
                ) {
                    continue;
                }
                let found = mandatory_facts.get(concept).into_iter().flatten().any(|f| {
                    self.context(f).is_some_and(|ctx| match (&ctx.period, start, end) {
                        (Period::Instant(d), _, e) => date_equal(Some(d), Some(e)),
                        (Period::Duration(s, e), s0, e0) => {
                            date_equal(Some(s), Some(s0)) && date_equal(Some(e), Some(e0))
                        }
                        (Period::Forever, _, _) => false,
                    })
                });
                if !found {
                    missing.push(concept);
                }
            }
        }

        if !missing.is_empty() {
            let mut sorted = missing.clone();
            sorted.sort_unstable();
            self.error(
                "JFCVC.3312",
                format!(
                    "The following mandatory concepts are either not tagged on a fact or are tagged on facts that have contexts that do not align with the dates as reported in 'StartDateForPeriodCoveredByReport' and 'EndDateForPeriodCoveredByReport': {}",
                    sorted.join(", ")
                ),
                None,
            );
        }

        // JFCVC.3312.atLeastOne (charities registration numbers).
        if !ttype.must_have_one.is_empty()
            && let (Some(start), Some(end)) = (&start_date, &end_date)
        {
            let found = ttype.must_have_one.iter().any(|concept| {
                self.facts(concept).iter().any(|f| {
                    self.is_valid_fact(f)
                        && self.context(f).is_some_and(|ctx| match (&ctx.period, start, end) {
                            (Period::Instant(d), _, e) => date_equal(Some(d), Some(e)),
                            (Period::Duration(s, e), s0, e0) => {
                                date_equal(Some(s), Some(s0)) && date_equal(Some(e), Some(e0))
                            }
                            (Period::Forever, _, _) => false,
                        })
                })
            });
            if !found {
                let mut sorted = ttype.must_have_one.clone();
                sorted.sort_unstable();
                self.error(
                    "JFCVC.3312.atLeastOne",
                    format!("At least one of the facts is MANDATORY: {}", sorted.join(", ")),
                    None,
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // JFCVC.3315 — generic dimension members
    // ------------------------------------------------------------------

    fn check_generic_dimension_members(&mut self) {
        // Collect members used across all facts' contexts.
        let mut required: HashMap<String, HashSet<(String, String)>> = HashMap::new();
        // (factName -> set of (member, altFacts))
        for f in &self.doc.facts {
            let Some(ctx) = self.context(f) else { continue };
            for (_, member) in &ctx.dimensions {
                let (base, n) = split_member_number(member);
                let Some(gdv) = self.rules.generic_dimension_validations.get(&base) else {
                    continue;
                };
                let in_range = match (n, gdv.min, gdv.max) {
                    (Some(n), Some(lo), Some(hi)) => n >= lo && n <= hi,
                    (None, _, _) => true,
                    (Some(_), None, None) => true,
                    (Some(n), Some(lo), None) => n >= lo,
                    (Some(n), None, Some(hi)) => n <= hi,
                };
                if !in_range {
                    continue;
                }
                for fact_name in &gdv.facts {
                    required
                        .entry(fact_name.clone())
                        .or_default()
                        .insert((member.clone(), gdv.facts.join("|")));
                }
            }
        }
        for (fact_name, reqs) in required {
            for (member, alts) in reqs {
                let alt_names: Vec<&str> = alts.split('|').collect();
                let satisfied = alt_names.iter().any(|name| {
                    self.facts(name).iter().any(|f| {
                        self.is_valid_fact(f)
                            && !f.value.trim().is_empty()
                            && self.context(f).is_some_and(|c| {
                                c.dimensions.iter().any(|(_, m)| m == &member)
                            })
                    })
                });
                if !satisfied {
                    self.error(
                        "JFCVC.3315",
                        format!(
                            "Generic dimension members have no associated name or description item, member names (name or description item): {}({})",
                            member, fact_name
                        ),
                        None,
                    );
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Per-category dispatch (ValidateUK.validate / validateCharities)
    // ------------------------------------------------------------------

    fn check_category_dispatch(&mut self, txmy: Option<&str>) {
        let category = self.dispatch_category(txmy);
        let Some(category) = category else {
            return;
        };
        let Some(rules) = self.rules.category(category) else {
            return;
        };
        self.run_category(rules);
    }

    fn dispatch_category(&self, txmy: Option<&str>) -> Option<&'static str> {
        let status = self.account_status();
        let unaudited = matches!(
            status.as_deref(),
            Some("AuditExempt-NoAccountantsReport" | "AuditExemptWithAccountantsReport")
        );
        if txmy == Some("charities") {
            return self.dispatch_charity_category(status.as_deref());
        }
        if unaudited {
            if self.is_dormant() {
                return Some(if self.is_llp() {
                    "unauditedDormantLLP"
                } else {
                    "unauditedDormantCompany"
                });
            }
            if self.accounting_standards().as_deref() == Some("Micro-entities") {
                return Some(if self.is_llp() {
                    "unauditedMicroLLP"
                } else {
                    "unauditedMicroCompany"
                });
            }
            match self.accounts_type().as_deref() {
                Some("AbridgedAccounts") => {
                    return Some(if self.is_llp() {
                        "unauditedLLPAbridgedAccounts"
                    } else {
                        "unauditedCompanyAbridgedAccounts"
                    });
                }
                Some("AbbreviatedAccounts") => {
                    return Some(if self.is_llp() {
                        "unauditedLLPAbbreviatedAccounts"
                    } else {
                        "unauditedCompanyAbbreviatedAccounts"
                    });
                }
                _ => {}
            }
            if matches!(
                self.scope_accounts().as_deref(),
                Some("GroupAccountsOnly" | "ConsolidatedGroupCompanyAccounts")
            ) {
                return Some(if self.is_llp() {
                    "unauditedLLPGroupAccounts"
                } else {
                    "unauditedCompanyGroupAccounts"
                });
            }
            if self.is_llp()
                && self.applicable_legislation().as_deref()
                    == Some("SmallCompaniesRegimeForAccounts")
            {
                return Some("unauditedLLPFullAccounts");
            }
            return Some("unauditedSmallCompanyFullAccounts");
        }
        // Audited.
        if self.accounts_type().as_deref() == Some("AbridgedAccounts") {
            return Some(if self.is_llp() {
                "auditedAbridgedLLPAccounts"
            } else {
                "auditedCompanyAbridgedAccounts"
            });
        }
        if self.applicable_legislation().as_deref() == Some("SmallCompaniesRegimeForAccounts") {
            return Some(if self.is_llp() {
                "auditedSmallLLP"
            } else {
                "auditedSmallCompany"
            });
        }
        if self.applicable_legislation().as_deref()
            == Some("Medium-sizedCompaniesRegimeForAccounts")
        {
            return Some(if self.is_llp() {
                "auditedMediumLLP"
            } else {
                "auditedMediumCompany"
            });
        }
        if self.accounting_standards().as_deref() == Some("Micro-entities") {
            return Some(if self.is_llp() {
                "auditedMicroLLP"
            } else {
                "auditedMicroCompany"
            });
        }
        let group = matches!(
            self.scope_accounts().as_deref(),
            Some("GroupAccountsOnly" | "ConsolidatedGroupCompanyAccounts")
        );
        if !(group
            || self.accounts_type().as_deref() == Some("AbridgedAccounts")
            || self.applicable_legislation().as_deref()
                == Some("SmallCompaniesRegimeForAccounts"))
        {
            return Some(if self.is_llp() {
                "auditedOtherLLP"
            } else {
                "auditedOtherCompany"
            });
        }
        if group {
            return Some(if self.is_llp() {
                "auditedGroupLLP"
            } else {
                "auditedGroupCompany"
            });
        }
        None
    }

    fn dispatch_charity_category(&self, status: Option<&str>) -> Option<&'static str> {
        let has_reg_number = self.has_valid_fact_any(&[
            "CharityRegistrationNumberEnglandWales",
            "CharityRegistrationNumberScotland",
            "CharityRegistrationNumberNorthernIreland",
        ]);
        if !has_reg_number {
            return None;
        }
        if status == Some("Audited") {
            if self.applicable_legislation().as_deref()
                == Some("SmallCompaniesRegimeForAccounts")
                && !self.is_llp()
            {
                return Some("auditedSmallCharity");
            }
            if !(matches!(
                self.scope_accounts().as_deref(),
                Some("GroupAccountsOnly" | "ConsolidatedGroupCompanyAccounts")
            ) || self.accounts_type().as_deref() == Some("AbridgedAccounts"))
            {
                return Some("auditedOtherCharity");
            }
            return None;
        }
        if matches!(
            status,
            Some("AuditExempt-NoAccountantsReport" | "AuditExemptWithAccountantsReport")
        ) {
            if self.is_dormant() {
                return Some("unauditedDormantCharity");
            }
            return Some("unauditedCharitySmallAndGroupAccounts");
        }
        None
    }

    fn run_category(&mut self, rules: &CategoryRules) {
        // `must` — each must pass.
        for code in &rules.must {
            let (passed, msg) = self.evaluate_code(code);
            if !passed {
                self.report_code_failure(code, rules, msg);
            }
        }
        // `atLeastOne` — all failing are reported only when none pass.
        if !rules.at_least_one.is_empty() {
            let mut failed = Vec::new();
            for code in &rules.at_least_one {
                let (passed, msg) = self.evaluate_code(code);
                if !passed {
                    failed.push((code.clone(), msg));
                }
            }
            if failed.len() == rules.at_least_one.len() {
                for (code, msg) in failed {
                    self.report_code_failure(&code, rules, msg);
                }
            }
        }
        // `primary` / `fallback` — the audited-group / audited-micro pattern.
        if let Some(primary) = &rules.primary {
            let (p_ok, _) = self.evaluate_code(primary);
            if !p_ok {
                let mut fallback_failed: Vec<(String, Option<String>)> = Vec::new();
                for code in &rules.fallback {
                    let (ok, msg) = self.evaluate_code(code);
                    if !ok {
                        fallback_failed.push((code.clone(), msg));
                    }
                }
                if !fallback_failed.is_empty() {
                    // Primary is reported only when the fallback also fails.
                    self.report_code_failure(primary, rules, None);
                    for (code, msg) in fallback_failed {
                        self.report_code_failure(&code, rules, msg);
                    }
                }
            }
        }
    }

    fn report_code_failure(
        &mut self,
        code: &str,
        category: &CategoryRules,
        msg: Option<String>,
    ) {
        let message = msg.unwrap_or_else(|| {
            format!("The check {code} is not satisfied by the document.")
        });
        if self.rules.is_warning_code(Some(category), code) {
            self.warn(code, message, None);
        } else {
            self.error(code, message, None);
        }
    }

    /// Evaluate a single code: returns (passed, failure message).
    fn evaluate_code(&self, code: &str) -> (bool, Option<String>) {
        if let Some(eval) = self.rules.code_evaluation(code) {
            return match eval.r#type.as_str() {
                "audit" => self.eval_audit_facts(),
                "charityAudit" => self.eval_charity_audit_facts(),
                "directorSigning" => self.eval_director_signing(false),
                "directorSigningCharity" => self.eval_director_signing(true),
                "profitLoss" => self.eval_profit_loss(eval.concept.as_deref().unwrap_or("ProfitLoss")),
                "group" => self.eval_group_facts(),
                _ => (true, None),
            };
        }
        if let Some(pattern) = self.rules.text_pattern(code) {
            return self.eval_text_pattern(pattern);
        }
        if let Some(single) = self.rules.single_concept(code) {
            if self.has_valid_fact(&single.concept) {
                return (true, None);
            }
            return (
                false,
                Some(format!(
                    "The concept of {} must exist and have a non-nil value.",
                    single.concept
                )),
            );
        }
        (true, None)
    }

    /// A statement fact must contain, for every phrase group, at least one of
    /// the group's phrases (word-bounded, case-insensitive).
    fn eval_text_pattern(&self, pattern: &crate::rules::TextPatternRule) -> (bool, Option<String>) {
        let lang = if self.lang == Lang::Welsh {
            pattern.cy.as_ref()
        } else {
            Some(&pattern.en)
        };
        let Some(lang) = lang else {
            return (false, Some(format!("No text pattern for language.")));
        };
        let facts = self.facts(&pattern.concept);
        if facts.is_empty() {
            return (
                false,
                Some(format!(
                    "The document is expected to have a fact tagged with the following concept: {}",
                    pattern.concept
                )),
            );
        }
        let mut any_matched = false;
        let mut last_fact = None;
        for f in facts {
            if !self.is_valid_fact(f) {
                continue;
            }
            let text = normalise_spaces(&f.value);
            let matched = lang.groups.iter().all(|group| {
                group
                    .iter()
                    .any(|phrase| contains_phrase(&text, phrase))
            });
            if matched {
                any_matched = true;
                break;
            }
            last_fact = Some(f);
        }
        if any_matched {
            (true, None)
        } else {
            (
                false,
                Some(match last_fact {
                    Some(f) => format!(
                        "The value for the fact tagged with the concept {} does not contain the required text: {:?}",
                        pattern.concept, f.value
                    ),
                    None => format!(
                        "The document is expected to have a fact tagged with the following concept: {}",
                        pattern.concept
                    ),
                }),
            )
        }
    }

    fn eval_audit_facts(&self) -> (bool, Option<String>) {
        let mut missing = Vec::new();
        if !self.has_valid_fact("DateAuditorsReport") {
            missing.push("DateAuditorsReport");
        }
        if !self.has_valid_fact("OpinionAuditorsOnEntity") {
            missing.push("OpinionAuditorsOnEntity");
        }
        let has_entity_auditors =
            self.has_valid_fact("NameEntityAuditors") && self.has_valid_fact("NameSeniorStatutoryAuditor");
        if !has_entity_auditors && !self.has_valid_fact("NameIndividualAuditor") {
            missing.extend([
                "NameIndividualAuditor",
                "NameSeniorStatutoryAuditor",
                "NameEntityAuditors",
            ]);
        }
        if missing.is_empty() {
            (true, None)
        } else {
            (
                false,
                Some(format!(
                    "An audited report must contain facts tagged with the concepts of DateAuditorsReport, OpinionAuditorsOnEntity as well as either NameIndividualAuditor or the combination of NameSeniorStatutoryAuditor and NameEntityAuditors. There are no facts tagged with the concepts: {}",
                    missing.join(", ")
                )),
            )
        }
    }

    fn eval_charity_audit_facts(&self) -> (bool, Option<String>) {
        let mut missing = Vec::new();
        if !self.has_valid_fact_any(&["DateAuditorsReport", "DateCharityAuditorsReport"]) {
            missing.extend(["DateAuditorsReport", "DateCharityAuditorsReport"]);
        }
        if !self.has_valid_fact_any(&[
            "OpinionAuditorsOnEntity",
            "QualifiedOpinion",
            "UnqualifiedOpinion",
            "AdverseOpinion",
            "DisclaimerOpinion",
        ]) {
            missing.extend([
                "OpinionAuditorsOnEntity",
                "QualifiedOpinion",
                "UnqualifiedOpinion",
                "AdverseOpinion",
                "DisclaimerOpinion",
            ]);
        }
        let bool_facts = self.facts("CharityAuditCarriedOutInAccordanceWithCharitiesAct2011Truefalse");
        let charity_act = !bool_facts.is_empty()
            && bool_facts.iter().all(|f| {
                self.is_valid_fact(f) && f.value.trim().eq_ignore_ascii_case("true")
            });
        let (name_individual, name_senior, name_entity) = if charity_act {
            (
                "NameIndividualCharityAuditor",
                "NameSeniorStatutoryCharityAuditor",
                "NameEntityCharityAuditors",
            )
        } else {
            (
                "NameIndividualAuditor",
                "NameSeniorStatutoryAuditor",
                "NameEntityAuditors",
            )
        };
        if !self.has_valid_fact(name_individual)
            && !(self.has_valid_fact(name_senior) && self.has_valid_fact(name_entity))
        {
            missing.extend([name_individual, name_senior, name_entity]);
        }
        if missing.is_empty() {
            (true, None)
        } else {
            (
                false,
                Some(format!(
                    "Audited charities accounts submission missing required audit-related information. There are no facts tagged with the concepts: {}",
                    missing.join(", ")
                )),
            )
        }
    }

    fn eval_director_signing(&self, charity: bool) -> (bool, Option<String>) {
        let (date, signer) = if charity {
            (
                "DateSigningTrusteesAnnualReport",
                "TrusteeSigningTrusteesAnnualReport",
            )
        } else {
            ("DateSigningDirectorsReport", "DirectorSigningDirectorsReport")
        };
        if self.has_valid_fact(date) && self.has_valid_fact(signer) {
            (true, None)
        } else {
            (
                false,
                Some(format!(
                    "Facts tagged with the {date} and {signer} must exist with non-nil values."
                )),
            )
        }
    }

    fn eval_profit_loss(&self, concept: &str) -> (bool, Option<String>) {
        // ProfitLoss (or CharityFunds) is required unless the entity is
        // flagged as never-traded / no-longer-trading.
        let not_trading = self
            .facts("EntityTradingStatus")
            .iter()
            .any(|f| {
                self.is_valid_fact(f)
                    && self.context(f).is_some_and(|c| {
                        c.member_for("EntityTradingStatusDimension").is_some_and(|m| {
                            m == "EntityHasNeverTraded" || m == "EntityNoLongerTradingButTradedInPast"
                        })
                    })
            });
        if self.has_valid_fact(concept) || not_trading {
            (true, None)
        } else {
            (
                false,
                Some(format!(
                    "A fact tagged with {concept} must exist if a fact tagged with EntityTradingStatus with the dimension of EntityTradingStatusDimension/(EntityHasNeverTraded OR EntityNoLongerTradingButTradedInPast) does not exist or has a nil value"
                )),
            )
        }
    }

    /// BalanceSheetDate with GroupCompanyDataDimension/Consolidated must equal
    /// BalanceSheetDate with default dimensions.
    fn eval_group_facts(&self) -> (bool, Option<String>) {
        let mut end_date_fact = None;
        for f in self.facts("EndDateForPeriodCoveredByReport") {
            if self.is_valid_fact(f)
                && self
                    .context(f)
                    .is_some_and(|c| c.dimensions.is_empty())
            {
                end_date_fact = Some(f);
                break;
            }
        }
        let Some(end_fact) = end_date_fact else {
            return (true, None);
        };
        let end_date = self.context(end_fact).and_then(|c| c.instant_date().map(String::from));
        let mut default_fact = None;
        let mut consolidated_fact = None;
        for f in self.facts("BalanceSheetDate") {
            if !self.is_valid_fact(f) {
                continue;
            }
            let Some(ctx) = self.context(f) else { continue };
            if ctx.dimensions.is_empty()
                && end_date
                    .as_deref()
                    .is_some_and(|d| ctx.instant_date() == Some(d))
            {
                default_fact = Some(f);
            }
            if ctx.member_for("GroupCompanyDataDimension").as_deref() == Some("Consolidated") {
                consolidated_fact = Some(f);
            }
        }
        match (consolidated_fact, default_fact) {
            (Some(c), Some(d)) if c.value.trim() == d.value.trim() => (true, None),
            _ => (
                false,
                Some(format!(
                    "A fact tagged with BalanceSheetDate with the dimension of GroupCompanyDataDimension/Consolidated must equal a fact tagged with BalanceSheetDate with the default dimension."
                )),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const ALLOWED_IMG_PREFIXES: &[&str] = &[
    "data:image/gif;base64",
    "data:image/jpeg;base64",
    "data:image/jpg;base64",
    "data:image/png;base64",
];

/// Whether the two ISO date strings are equal (or both missing).
fn date_equal(a: Option<&str>, b: Option<&str>) -> bool {
    matches!((a, b), (Some(x), Some(y)) if x == y)
}

/// Parse a fact's value as an ISO date, honouring the `ix:format`
/// transformation (only the ixt2 date formats the generators emit).
fn fact_date(f: &Fact) -> Option<String> {
    let raw = normalise_spaces(&f.value);
    match f.format.as_deref() {
        Some("ixt2:datedaymonthyearen") => parse_day_month_year(&raw),
        Some("ixt2:dateyeardaymonthen") => parse_year_day_month(&raw),
        Some("ixt2:datemonthdayyearen") => parse_month_day_year(&raw),
        _ => {
            // No (or unknown) format: the value should already be ISO.
            let v = raw.trim();
            if is_iso_date(v) {
                Some(v.to_string())
            } else {
                None
            }
        }
    }
}

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
}

fn month_number(name: &str) -> Option<&'static str> {
    Some(match name {
        "january" => "01",
        "february" => "02",
        "march" => "03",
        "april" => "04",
        "may" => "05",
        "june" => "06",
        "july" => "07",
        "august" => "08",
        "september" => "09",
        "october" => "10",
        "november" => "11",
        "december" => "12",
        _ => return None,
    })
}

fn parse_day_month_year(s: &str) -> Option<String> {
    let mut parts = s.split_whitespace();
    let day = parts.next()?;
    let month = parts.next()?;
    let year = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let m = month_number(&month.to_ascii_lowercase())?;
    Some(format!("{year}-{m}-{day:0>2}"))
}

fn parse_year_day_month(s: &str) -> Option<String> {
    let mut parts = s.split_whitespace();
    let year = parts.next()?;
    let day = parts.next()?;
    let month = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let m = month_number(&month.to_ascii_lowercase())?;
    Some(format!("{year}-{m}-{day:0>2}"))
}

fn parse_month_day_year(s: &str) -> Option<String> {
    let mut parts = s.split_whitespace();
    let month = parts.next()?;
    let day = parts.next()?;
    let year = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let m = month_number(&month.to_ascii_lowercase())?;
    Some(format!("{year}-{m}-{day:0>2}"))
}

fn period_kind(p: &Period) -> &'static str {
    match p {
        Period::Instant(_) => "instant",
        Period::Duration(_, _) => "duration",
        Period::Forever => "forever",
    }
}

/// Split a dimension member local name like `Director1` into ("Director", 1).
fn split_member_number(member: &str) -> (String, Option<u32>) {
    let digits_start = member
        .char_indices()
        .find(|(_, c)| c.is_ascii_digit())
        .map(|(i, _)| i);
    match digits_start {
        Some(i) if i > 0 && member[i..].chars().all(|c| c.is_ascii_digit()) => (
            member[..i].to_string(),
            member[i..].parse::<u32>().ok(),
        ),
        _ => (member.to_string(), None),
    }
}

/// Case-insensitive, word-bounded phrase search.
fn contains_phrase(text: &str, phrase: &str) -> bool {
    let text_l = text.to_ascii_lowercase();
    let phrase_l = phrase.to_ascii_lowercase();
    if phrase_l.is_empty() {
        return true;
    }
    let mut start = 0;
    while let Some(rel) = text_l[start..].find(&phrase_l) {
        let i = start + rel;
        let before_ok = i == 0 || !is_word_char(text_l.as_bytes()[i - 1]);
        let after = i + phrase_l.len();
        let after_ok = after >= text_l.len() || !is_word_char(text_l.as_bytes()[after]);
        if before_ok && after_ok {
            return true;
        }
        start = i + 1;
    }
    false
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Interpret an XSD pattern facet of the `[0-9]{N}` shape (the only form the
/// UK taxonomies use: `taxReferenceItemType` `[0-9]{10}`, `taxDistrictItemType`
/// `[0-9]{3}`) as a required digit count.  Returns `None` for any other
/// pattern shape so the caller can skip the check rather than guess.
fn digit_count_pattern(pattern: &str) -> Option<usize> {
    let p = pattern.trim();
    if p.len() < 6 || !p.starts_with("[0-9]{") || !p.ends_with('}') {
        return None;
    }
    let inner = &p[6..p.len() - 1];
    if inner.is_empty() || !inner.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    inner.parse().ok()
}

/// Whether the numeric facts in a duplicate group are consistent: numeric
/// facts are compared after rounding to the coarsest declared decimals,
/// text facts by exact value.
fn facts_consistent(facts: &[&Fact]) -> bool {
    let first = facts[0];
    if first.is_numeric {
        let decimals = facts
            .iter()
            .map(|f| f.decimals.as_deref().unwrap_or("INF"))
            .filter(|d| *d != "INF")
            .map(|d| d.parse::<i32>().unwrap_or(0))
            .min();
        match decimals {
            Some(d) => {
                let round = 10f64.powi(d);
                let vals: Vec<f64> = facts
                    .iter()
                    .filter_map(|f| f.numeric)
                    .map(|v| (v * round).round() / round)
                    .collect();
                vals.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9)
            }
            None => {
                let vals: Vec<f64> = facts.iter().filter_map(|f| f.numeric).collect();
                vals.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9)
            }
        }
    } else {
        let vals: Vec<String> = facts.iter().map(|f| f.value.trim().to_string()).collect();
        vals.windows(2).all(|w| w[0] == w[1])
    }
}

/// The nonzero digits that fall beyond the declared `decimals` precision,
/// computed from the display text (before `scale`), mirroring Arelle's
/// HMRC.SG.4.5 insignificant-digits check.
fn insignificant_digits(value: &str, decimals: &str, scale: i32) -> Option<String> {
    let d: i32 = decimals.parse().ok()?;
    let cleaned: String = value
        .trim()
        .chars()
        .filter(|c| *c != ',' && *c != '\u{a0}')
        .collect();
    let effective = d - scale;
    let (int_part, frac_part) = match cleaned.split_once('.') {
        Some((i, f)) => (i, f),
        None => (cleaned.as_str(), ""),
    };
    let frac_digits: usize = frac_part.chars().count();
    if effective >= 0 {
        let keep = effective as usize;
        if frac_digits <= keep {
            return None;
        }
        let beyond = &frac_part[keep..];
        if beyond.chars().all(|c| c == '0') {
            None
        } else {
            Some(beyond.to_string())
        }
    } else {
        // decimals < 0: rounding to 10^|d| — any nonzero digit at or below
        // that place is insignificant.
        let round_to = (-effective) as usize;
        let all = format!("{int_part}{frac_part}");
        let all_digits: usize = all.chars().count();
        if all_digits <= round_to {
            return None;
        }
        let cutoff = all_digits - round_to;
        let beyond: String = all[cutoff..].chars().collect();
        if beyond.chars().all(|c| c == '0') {
            None
        } else {
            Some(beyond)
        }
    }
}

/// Find an external `url(...)` reference in a CSS string.
fn external_css_url(css: &str) -> Option<String> {
    let lower = css.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(idx) = rest.find("url(") {
        let after = &rest[idx + 4..];
        let end = after.find(')')?;
        let inner = after[..end].trim();
        let inner = inner.trim_matches(|c| c == '\'' || c == '"');
        if !inner.starts_with("data:") {
            return Some(inner.to_string());
        }
        rest = &after[end + 1..];
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::IssueKind;

    fn setup() -> (Taxonomy, Rules) {
        (Taxonomy::embedded(), Rules::embedded())
    }

    #[test]
    fn date_parsing() {
        assert_eq!(parse_day_month_year("01 January 2020"), Some("2020-01-01".into()));
        assert_eq!(parse_day_month_year("31 December 2020"), Some("2020-12-31".into()));
        assert_eq!(parse_year_day_month("2020 01 January"), Some("2020-01-01".into()));
        assert_eq!(parse_month_day_year("January 01 2020"), Some("2020-01-01".into()));
        assert!(is_iso_date("2020-12-31"));
        assert!(!is_iso_date("31 December 2020"));
    }

    #[test]
    fn phrase_matching_is_word_bounded_and_case_insensitive() {
        assert!(contains_phrase(
            "The company is exempt from audit under section 477 of the Companies Act 2006.",
            "Exempt"
        ));
        assert!(contains_phrase(
            "accounts have been prepared in accordance with the provisions of the small companies regime",
            "in accordance with"
        ));
        assert!(contains_phrase("Members have agreed to the preparation of abridged accounts", "Members"));
        assert!(!contains_phrase("exemption", "Exempt"));
        assert!(!contains_phrase("small companies regime", "small company"));
    }

    #[test]
    fn member_number_splitting() {
        assert_eq!(split_member_number("Director1"), ("Director".to_string(), Some(1)));
        assert_eq!(split_member_number("Director40"), ("Director".to_string(), Some(40)));
        assert_eq!(split_member_number("Chairman"), ("Chairman".to_string(), None));
        assert_eq!(split_member_number("M-ProfessionalScientificTechnicalActivities"), ("M-ProfessionalScientificTechnicalActivities".to_string(), None));
    }

    #[test]
    fn insignificant_digits_detection() {
        assert_eq!(insignificant_digits("123.45", "2", 0), None);
        assert_eq!(insignificant_digits("123.456", "2", 0), Some("6".into()));
        assert_eq!(insignificant_digits("1000", "-1", 0), None);
        assert_eq!(insignificant_digits("1054", "-1", 0), Some("4".into()));
    }

    #[test]
    fn external_url_detection() {
        assert_eq!(external_css_url("background: url(https://x/y.png)"), Some("https://x/y.png".into()));
        assert_eq!(external_css_url("background: url(data:image/png;base64,abc)"), None);
        assert_eq!(external_css_url("color: red;"), None);
    }

    #[test]
    fn fixture_accounts_document_validates_clean() {
        let (taxonomy, rules) = setup();
        let html = include_str!("../../../example_data/basic-1/output-accounts.html");
        let doc = crate::document::parse(html);
        assert_eq!(detect_doc_kind(&doc), DocKind::Accounts);
        assert_eq!(taxonomy_type(&doc), Some("FRS-2022"));
        let report = validate(&doc, &taxonomy, &rules, Lang::English);
        let errors: Vec<&Issue> = report.issues.iter().filter(|i| i.kind == IssueKind::Error).collect();
        let warnings: Vec<&Issue> = report.issues.iter().filter(|i| i.kind == IssueKind::Warning).collect();
        for e in &errors {
            eprintln!("[{}] {} @ {}", e.code, e.message, e.location.as_deref().unwrap_or("-"));
        }
        assert!(errors.is_empty(), "fixture should validate clean, got: {errors:?}");
        // No warnings either (matches --captureWarnings behaviour).
        assert!(warnings.is_empty(), "fixture produced warnings: {warnings:?}");
    }

    #[test]
    fn fixture_corp_tax_document_validates_clean() {
        let (taxonomy, rules) = setup();
        let html = include_str!("../../../example_data/basic-1/output-corp-tax.html");
        let doc = crate::document::parse(html);
        assert_eq!(detect_doc_kind(&doc), DocKind::Computation);
        let report = validate(&doc, &taxonomy, &rules, Lang::English);
        let errors: Vec<&Issue> = report.issues.iter().filter(|i| i.kind == IssueKind::Error).collect();
        for e in &errors {
            eprintln!("[{}] {} @ {}", e.code, e.message, e.location.as_deref().unwrap_or("-"));
        }
        assert!(errors.is_empty(), "corp-tax fixture should validate clean, got: {errors:?}");
    }

    /// Mutate the accounts fixture into each known failure mode and check the
    /// exact JFCVC code is produced.
    #[test]
    fn mutated_fixtures_produce_expected_codes() {
        let (taxonomy, rules) = setup();
        let html = include_str!("../../../example_data/basic-1/output-accounts.html");

        // 1. StartDate tagged on the wrong (period-start) instant context
        //    (the original bug: ctxt-21 is duration; use ctxt-1, an instant
        //    at the publication date).
        let bad_start = html.replace(
            "name=\"uk-bus:StartDateForPeriodCoveredByReport\" contextRef=\"ctxt-2\"",
            "name=\"uk-bus:StartDateForPeriodCoveredByReport\" contextRef=\"ctxt-1\"",
        );
        let doc = crate::document::parse(&bad_start);
        let report = validate(&doc, &taxonomy, &rules, Lang::English);
        assert!(
            report.issues.iter().any(|i| i.code == "JFCVC.3312"),
            "expected JFCVC.3312, got: {:?}",
            report.issues
        );

        // 2. Fixed-item concept with a non-empty value.
        let bad_fixed = html.replace(
            "name=\"uk-bus:AccountingStandardsApplied\" contextRef=\"ctxt-4\"></ix:nonNumeric>",
            "name=\"uk-bus:AccountingStandardsApplied\" contextRef=\"ctxt-4\">Micro-entities</ix:nonNumeric>",
        );
        let doc = crate::document::parse(&bad_fixed);
        let report = validate(&doc, &taxonomy, &rules, Lang::English);
        assert!(
            report.issues.iter().any(|i| i.code == "schema.fixedValue"),
            "expected schema.fixedValue, got: {:?}",
            report.issues
        );

        // 3. Missing DescriptionPrincipalActivities fact entirely.
        let removed = html.replace(
            "<ix:nonNumeric name=\"uk-bus:DescriptionPrincipalActivities\" contextRef=\"ctxt-0\">Computer security consultancy and development services</ix:nonNumeric>",
            "",
        );
        let doc = crate::document::parse(&removed);
        let report = validate(&doc, &taxonomy, &rules, Lang::English);
        assert!(
            report.issues.iter().any(|i| i.code == "JFCVC.3312"),
            "expected JFCVC.3312 for missing mandatory concept, got: {:?}",
            report.issues
        );
    }

    /// Mutate the corp-tax fixture into the ct-comp schema-facet failure
    /// modes and check the exact schema.* codes are produced: tax reference
    /// pattern facet, non-empty string minLength, and gYear lexical.
    #[test]
    fn mutated_ct_comp_facets_produce_expected_codes() {
        let (taxonomy, rules) = setup();
        let html = include_str!("../../../example_data/basic-1/output-corp-tax.html");

        // 1. TaxReference does not match the [0-9]{10} pattern facet.
        let bad_tax_ref = html.replace(
            "name=\"ct-comp:TaxReference\" contextRef=\"ctxt-0\">8596148860</ix:nonNumeric>",
            "name=\"ct-comp:TaxReference\" contextRef=\"ctxt-0\">ABCDEFGHIJ</ix:nonNumeric>",
        );
        let doc = crate::document::parse(&bad_tax_ref);
        let report = validate(&doc, &taxonomy, &rules, Lang::English);
        assert!(
            report.issues.iter().any(|i| i.code == "schema.patternValue"),
            "expected schema.patternValue, got: {:?}",
            report.issues
        );

        // 2. CompanyName (nonEmptyStringItemType) with an empty value.
        let empty_name = html.replace(
            "name=\"ct-comp:CompanyName\" contextRef=\"ctxt-0\">Example Biz Ltd.</ix:nonNumeric>",
            "name=\"ct-comp:CompanyName\" contextRef=\"ctxt-0\"></ix:nonNumeric>",
        );
        let doc = crate::document::parse(&empty_name);
        let report = validate(&doc, &taxonomy, &rules, Lang::English);
        assert!(
            report.issues.iter().any(|i| i.code == "schema.minLength"),
            "expected schema.minLength, got: {:?}",
            report.issues
        );

        // 3. FinancialYear1CoveredByTheReturn (gYearItemType) not a year.
        let bad_year = html.replace(
            "name=\"ct-comp:FinancialYear1CoveredByTheReturn\" contextRef=\"ctxt-1\">2019</ix:nonNumeric>",
            "name=\"ct-comp:FinancialYear1CoveredByTheReturn\" contextRef=\"ctxt-1\">20XX</ix:nonNumeric>",
        );
        let doc = crate::document::parse(&bad_year);
        let report = validate(&doc, &taxonomy, &rules, Lang::English);
        assert!(
            report.issues.iter().any(|i| i.code == "schema.gYearValue"),
            "expected schema.gYearValue, got: {:?}",
            report.issues
        );
    }

    /// The generator's Welsh statement narratives must satisfy the checker's
    /// Welsh (cy) phrase groups — the exact `contains_phrase` path the
    /// validator uses.  The texts below are the ones
    /// `reports::uk_frs105_accounts` emits for `ReportLanguage::Welsh`.
    #[test]
    fn welsh_statement_patterns_match_generator_texts() {
        let rules = Rules::embedded();
        let cases: &[(&str, &str, &str)] = &[
            (
                "Co.Micro",
                "StatementThatAccountsHaveBeenPreparedInAccordanceWithProvisionsSmallCompaniesRegime",
                "Mae'r datganiadau ariannol hyn wedi eu paratoi yn unol â darpariaethau'r drefn micro-gwmnïau a'u cyflwyno yn unol â'r darpariaethau sy'n gymwys o dan y drefn ar gyfer cwmnïau bach.",
            ),
            (
                "Co.Sec477",
                "StatementThatCompanyEntitledToExemptionFromAuditUnderSection477CompaniesAct2006RelatingToSmallCompanies",
                "Am y cyfnod cyfrifo sy'n dod i ben 31 December 2020 roedd y cwmni wedi'i eithrio rhag archwiliad o dan adran 477 o Ddeddf Cwmnïau 2006 sy'n ymwneud â chwmnïau bach.",
            ),
            (
                "Co.AuditNR",
                "StatementThatMembersHaveNotRequiredCompanyToObtainAnAudit",
                "Mae'r aelodau heb ei gwneud yn ofynnol i'r cwmni gael archwiliad o'i ddatganiadau ariannol ar gyfer y cyfnod cyfrifo yn unol ag adran 476.",
            ),
            (
                "Co.DirResp",
                "StatementThatDirectorsAcknowledgeTheirResponsibilitiesUnderCompaniesAct",
                "Mae'r cyfarwyddwyr yn cydnabod eu cyfrifoldebau o ran cydymffurfio â gofynion y Ddeddf o ran cofnodion cyfrifeg a pharatoi datganiadau ariannol.",
            ),
        ];
        for (code, concept, welsh_text) in cases {
            let pattern = rules
                .text_pattern(code)
                .unwrap_or_else(|| panic!("{code}: no text pattern"));
            assert_eq!(&pattern.concept, concept, "{code}: concept mismatch");
            let cy = pattern.cy.as_ref().expect("welsh pattern present");
            for (gi, group) in cy.groups.iter().enumerate() {
                assert!(
                    group.iter().any(|phrase| contains_phrase(welsh_text, phrase)),
                    "{code} group {gi}: none of {:?} found in: {welsh_text}",
                    group
                );
            }
            // The English text must NOT satisfy the Welsh pattern (the two
            // languages are distinct checks).
            let en_ok = cy.groups.iter().all(|g| {
                g.iter().any(|p| contains_phrase(
                    "These financial statements have been prepared in accordance with the micro-entity provisions.",
                    p,
                ))
            });
            assert!(!en_ok, "{code}: english text must not match the welsh pattern");
        }
    }

    /// Language auto-detection: a ReportPrincipalLanguage fact on a context
    /// with LanguagesDimension/Welsh selects the Welsh patterns.
    #[test]
    fn welsh_language_is_auto_detected() {
        let html = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:ix="http://www.xbrl.org/2013/inlineXBRL" xmlns:xbrli="http://www.xbrl.org/2003/instance" xmlns:xbrldi="http://xbrl.org/2006/xbrldi" xmlns:uk-bus="http://xbrl.frc.org.uk/cd/2023-01-01/business" xmlns:lang="http://xbrl.frc.org.uk/cd/2023-01-01/languages"><body><ix:header><ix:hidden>
        <xbrli:context id="c0"><xbrli:entity><xbrli:identifier scheme="http://www.companieshouse.gov.uk/">12345678</xbrli:identifier><xbrli:segment><xbrldi:explicitMember dimension="lang:LanguagesDimension">lang:Welsh</xbrldi:explicitMember></xbrli:segment></xbrli:entity><xbrli:period><xbrli:startDate>2020-01-01</xbrli:startDate><xbrli:endDate>2020-12-31</xbrli:endDate></xbrli:period></xbrli:context>
        <ix:nonNumeric name="uk-bus:ReportPrincipalLanguage" contextRef="c0"></ix:nonNumeric>
        </ix:hidden></ix:header></body></html>"#;
        let doc = crate::document::parse(html);
        assert_eq!(doc_lang(&doc), Lang::Welsh);

        // The English fixture declares no language -> English.
        let en = crate::document::parse(include_str!(
            "../../../example_data/basic-1/output-accounts.html"
        ));
        assert_eq!(doc_lang(&en), Lang::English);
    }

    /// The Welsh patterns are distinct and enforced: the English fixture's
    /// statements fail them when Welsh is forced.
    #[test]
    fn english_statements_fail_welsh_patterns() {
        let (taxonomy, rules) = setup();
        let doc = crate::document::parse(include_str!(
            "../../../example_data/basic-1/output-accounts.html"
        ));
        let report = validate(&doc, &taxonomy, &rules, Lang::Welsh);
        for code in ["Co.Micro", "Co.Sec477", "Co.AuditNR", "Co.DirResp"] {
            assert!(
                report.issues.iter().any(|i| i.code == code),
                "expected {code} to fail under the Welsh patterns, got: {:?}",
                report.issues
            );
        }
    }

    /// JFCVC.3314: two facts of the same concept on the same context/unit
    /// with different values.
    #[test]
    fn duplicate_facts_produce_3314() {
        let (taxonomy, rules) = setup();
        let html = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:ix="http://www.xbrl.org/2013/inlineXBRL" xmlns:xbrli="http://www.xbrl.org/2003/instance" xmlns:uk-core="http://xbrl.frc.org.uk/fr/2023-01-01/core"><body><ix:header><ix:hidden>
        <xbrli:context id="c0"><xbrli:entity><xbrli:identifier scheme="http://www.companieshouse.gov.uk/">12345678</xbrli:identifier></xbrli:entity><xbrli:period><xbrli:instant>2020-12-31</xbrli:instant></xbrli:period></xbrli:context>
        <ix:nonFraction name="uk-core:ProfitLoss" contextRef="c0" unitRef="GBP" decimals="0" scale="0">100</ix:nonFraction>
        <ix:nonFraction name="uk-core:ProfitLoss" contextRef="c0" unitRef="GBP" decimals="0" scale="0">200</ix:nonFraction>
        </ix:hidden></ix:header></body></html>"#;
        let doc = crate::document::parse(html);
        let report = validate(&doc, &taxonomy, &rules, Lang::English);
        assert!(
            report.issues.iter().any(|i| i.code == "JFCVC.3314"),
            "expected JFCVC.3314, got: {:?}",
            report.issues
        );
    }
}
