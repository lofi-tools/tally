//! The UK taxonomy concept table.
//!
//! The schema-level facts the checks need (a concept's data type, periodType,
//! balance, enumeration values) are not part of the JFCVC rules — they live in
//! the FRC / HMRC taxonomy XSDs, which are not vendored in this repo.  Two
//! ways to get them:
//!
//! - [`Taxonomy::embedded`] loads the generated table
//!   (`uk-2023-01-01.json`: every ct-comp computation concept, plus the FRC
//!   concepts the report generators emit; regenerate with
//!   `scripts/generate_concept_table.py`).
//! - [`Taxonomy::from_directory`] parses a downloaded taxonomy directory
//!   (`bus.xsd`, `frc-core.xsd`, `types.xsd`, `direp.xsd`, `countries.xsd`,
//!   `dpl.xsd`, `ct-comp-2.xsd`) on the fly.
//!
//! Concept lookups are by local name — the FRC and HMRC taxonomies use
//! distinct local names across namespaces, and the report generators tag
//! facts with unique local names.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::document::local_of;
use ixbrl_ir::XmlNode;

const EMBEDDED_TABLE: &str = include_str!("taxonomy/uk-2023-01-01.json");

/// A taxonomy concept's periodType, when declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodType {
    Instant,
    Duration,
}

/// Schema-level metadata for one taxonomy concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Concept {
    pub name: String,
    /// The concept's data type local name (e.g. `monetaryItemType`,
    /// `stringItemType`, `fixedItemType`).
    pub data_type: String,
    pub period: Option<PeriodType>,
    pub balance: Option<String>,
    pub nillable: bool,
    /// Enumeration values, when the type restricts to a closed list.
    pub enums: Vec<String>,
    /// XSD pattern facet (e.g. `[0-9]{10}` for `taxReferenceItemType`).
    pub pattern: Option<String>,
    /// XSD minLength facet (e.g. 1 for `nonEmptyStringItemType`).
    pub min_length: Option<usize>,
}

impl Concept {
    /// `types:fixedItemType` — an empty "marker" fact whose meaning is carried
    /// by the context's dimension member (e.g. `AccountingStandardsApplied`).
    pub fn is_fixed(&self) -> bool {
        self.data_type.ends_with("fixedItemType") || self.data_type == "fixed"
    }

    /// Whether the data type is numeric (monetary / decimal / integer / ...).
    pub fn is_numeric_type(&self) -> bool {
        let t = self.data_type.to_ascii_lowercase();
        [
            "monetary", "decimal", "integer", "shares", "percent", "pure",
            "area", "volume", "mass", "energy",
        ]
        .iter()
        .any(|k| t.contains(k))
    }

    /// The concept carries an XSD minLength facet (the value must be at
    /// least this many characters, e.g. `nonEmptyStringItemType`).
    pub fn min_length(&self) -> Option<usize> {
        self.min_length
    }

    pub fn is_boolean_type(&self) -> bool {
        self.data_type.ends_with("booleanItemType")
    }

    pub fn is_date_type(&self) -> bool {
        self.data_type.ends_with("dateItemType")
    }

    /// Dimension domain / member concepts carry no numeric or text meaning.
    pub fn is_domain_or_member(&self) -> bool {
        self.data_type.ends_with("domainItemType") || self.data_type.ends_with("memberItemType")
    }
}

/// A loaded taxonomy concept table.
#[derive(Debug, Clone, Default)]
pub struct Taxonomy {
    pub concepts: HashMap<String, Concept>,
    /// True for the embedded table.  It covers the full ct-comp computation
    /// taxonomy but only the FRC slice the report generators emit, so unknown
    /// concepts are silently skipped (they are expected in documents built
    /// from other sources); a full taxonomy loaded from a directory reports
    /// them.
    pub is_subset: bool,
}

impl Taxonomy {
    /// The embedded table generated from the 2023-01-01 FRC / HMRC XSDs.
    pub fn embedded() -> Self {
        let table: HashMap<String, RawConcept> =
            serde_json::from_str(EMBEDDED_TABLE).expect("embedded concept table is valid JSON");
        let concepts = table
            .into_iter()
            .map(|(name, raw)| (name.clone(), raw.into_concept(name)))
            .collect();
        Taxonomy {
            concepts,
            is_subset: true,
        }
    }

    /// Parse a taxonomy directory: every `*.xsd` file contributes element and
    /// simpleType definitions (bus, core, types, direp, countries, dpl,
    /// ct-comp).
    pub fn from_directory(dir: &Path) -> Result<Self, String> {
        let mut concepts: HashMap<String, Concept> = HashMap::new();
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("cannot read taxonomy directory {}: {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .map(|x| x.eq_ignore_ascii_case("xsd"))
                    .unwrap_or(false)
            })
            .collect();
        files.sort();
        if files.is_empty() {
            return Err(format!("no .xsd files found in {}", dir.display()));
        }
        for path in files {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let node = XmlNode::from_xml_string(&text)
                .map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
            extract_from_node(&node, &mut concepts);
        }
        // Named types carry the facets (pattern / minLength); elements
        // reference those types by local name through their `data_type`, so
        // copy the facets from the type entry onto every referencing element.
        resolve_facets(&mut concepts);
        Ok(Taxonomy {
            concepts,
            is_subset: false,
        })
    }

    pub fn concept(&self, local_name: &str) -> Option<&Concept> {
        self.concepts.get(local_name)
    }
}

#[derive(Debug, Deserialize)]
struct RawConcept {
    name: Option<String>,
    #[serde(rename = "t")]
    data_type: Option<String>,
    #[serde(rename = "p")]
    period: Option<String>,
    #[serde(rename = "b")]
    balance: Option<String>,
    #[serde(rename = "n")]
    nillable: Option<bool>,
    #[serde(rename = "e")]
    enums: Option<Vec<String>>,
    #[serde(rename = "pat")]
    pattern: Option<String>,
    #[serde(rename = "ml")]
    min_length: Option<usize>,
}

impl RawConcept {
    fn into_concept(self, name: String) -> Concept {
        Concept {
            name: self.name.unwrap_or(name),
            data_type: self.data_type.unwrap_or_default(),
            period: self.period.as_deref().and_then(|p| match p {
                "instant" => Some(PeriodType::Instant),
                "duration" => Some(PeriodType::Duration),
                _ => None,
            }),
            balance: self.balance,
            nillable: self.nillable.unwrap_or(false),
            enums: self.enums.unwrap_or_default(),
            pattern: self.pattern,
            min_length: self.min_length,
        }
    }
}

fn extract_from_node(node: &XmlNode, out: &mut HashMap<String, Concept>) {
    match node {
        XmlNode::Elem {
            name,
            attributes,
            children,
        } => {
            let local = local_of(name);
            if local == "element" {
                if let Some(elt_name) = attr(attributes, "name") {
                    let data_type = attr(attributes, "type")
                        .map(|t| local_of(&t))
                        .unwrap_or_default();
                    // The periodType attribute is namespaced in the FRC/HMRC
                    // schemas (`xbrli:periodType="instant"`).
                    let period = attr_any(attributes, &["periodType", "xbrli:periodType"])
                        .and_then(|p| match p.as_str() {
                            "instant" => Some(PeriodType::Instant),
                            "duration" => Some(PeriodType::Duration),
                            _ => None,
                        });
                    let balance = attr(attributes, "balance");
                    let nillable = attr(attributes, "nillable").as_deref() == Some("true");
                    let mut enums = Vec::new();
                    // inline simpleType with an enumeration restriction
                    for child in children {
                        if let XmlNode::Elem { name: cname, children: cchildren, .. } = child {
                            if local_of(cname) == "simpleType" {
                                for inner in cchildren {                                        if let XmlNode::Elem {
                                            name: iname,
                                            children: ichildren,
                                            ..
                                        } = inner
                                        {
                                        if local_of(iname) == "restriction" {
                                            for e in ichildren {
                                                if let XmlNode::Elem {
                                                    name: ename,
                                                    attributes: eattrs,
                                                    ..
                                                } = e
                                                {
                                                    if local_of(ename) == "enumeration" {
                                                        if let Some(v) = attr(eattrs, "value") {
                                                            enums.push(v);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !elt_name.is_empty() {
                        out.entry(elt_name.clone()).or_insert(Concept {
                            name: elt_name,
                            data_type,
                            period,
                            balance,
                            nillable,
                            enums,
                            pattern: None,
                            min_length: None,
                        });
                    }
                }
            } else if local == "simpleType" || local == "complexType" {
                // named simpleType (enumeration restriction) or complexType
                // (simpleContent restriction with pattern / minLength facets,
                // e.g. the ct-comp types schema's taxReferenceItemType).
                if let Some(st_name) = attr(attributes, "name") {
                    // For a complexType the restriction sits under
                    // simpleContent/restriction; for a simpleType it is a
                    // direct restriction child.
                    let rest_node = if local == "complexType" {
                        children.iter().find_map(|c| match c {
                            XmlNode::Elem {
                                name: scname,
                                children: scchildren,
                                ..
                            } if local_of(scname) == "simpleContent" => {
                                scchildren.iter().find(|g| match g {
                                    XmlNode::Elem {
                                        name: rname, ..
                                    } => local_of(rname) == "restriction",
                                    _ => false,
                                })
                            }
                            _ => None,
                        })
                    } else {
                        children.iter().find(|c| match c {
                            XmlNode::Elem {
                                name: rname, ..
                            } => local_of(rname) == "restriction",
                            _ => false,
                        })
                    };
                    if let Some(XmlNode::Elem {
                        attributes: cattrs,
                        children: cchildren,
                        ..
                    }) = rest_node
                    {
                        let base = attr(cattrs, "base")
                            .map(|b| local_of(&b))
                            .unwrap_or_default();
                        let mut enums = Vec::new();
                        let mut pattern = None;
                        let mut min_length = None;
                        for e in cchildren {
                            if let XmlNode::Elem {
                                name: ename,
                                attributes: eattrs,
                                ..
                            } = e
                            {
                                match local_of(ename).as_str() {
                                    "enumeration" => {
                                        if let Some(v) = attr(eattrs, "value") {
                                            enums.push(v);
                                        }
                                    }
                                    "pattern" => {
                                        pattern = attr(eattrs, "value");
                                    }
                                    "minLength" => {
                                        min_length = attr(eattrs, "value")
                                            .and_then(|v| v.parse().ok());
                                    }
                                    _ => {}
                                }
                            }
                        }
                        out.entry(st_name.clone()).or_insert(Concept {
                            name: st_name.clone(),
                            data_type: base,
                            period: None,
                            balance: None,
                            nillable: false,
                            enums,
                            pattern,
                            min_length,
                        });
                    }
                }
            }
            for child in children {
                extract_from_node(child, out);
            }
        }
        XmlNode::Text(_) => {}
    }
}

/// Copy pattern / minLength facets from named type entries onto the
/// elements that reference them (an element's `data_type` holds the type's
/// local name, e.g. `taxReferenceItemType`).
fn resolve_facets(concepts: &mut HashMap<String, Concept>) {
    let type_facets: Vec<(String, Option<String>, Option<usize>)> = concepts
        .iter()
        .filter(|(_, c)| c.pattern.is_some() || c.min_length.is_some())
        .map(|(name, c)| (name.clone(), c.pattern.clone(), c.min_length))
        .collect();
    for concept in concepts.values_mut() {
        let Some((_, pattern, min_length)) =
            type_facets.iter().find(|(n, _, _)| *n == concept.data_type)
        else {
            continue;
        };
        if concept.pattern.is_none() {
            concept.pattern = pattern.clone();
        }
        if concept.min_length.is_none() {
            concept.min_length = *min_length;
        }
    }
}

fn attr(attributes: &[(String, String)], key: &str) -> Option<String> {
    attributes
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

fn attr_any(attributes: &[(String, String)], keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| attr(attributes, k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_table_covers_generator_concepts() {
        let t = Taxonomy::embedded();
        for name in [
            "StartDateForPeriodCoveredByReport",
            "EndDateForPeriodCoveredByReport",
            "AccountingStandardsApplied",
            "AccountsType",
            "DescriptionPrincipalActivities",
            "BalanceSheetDate",
            "TurnoverRevenue",
            "EntityDormantTruefalse",
            "Micro-entities",
            "PrivateLimitedCompanyLtd",
            "StatementThatDirectorsAcknowledgeTheirResponsibilitiesUnderCompaniesAct",
            "AverageNumberEmployeesDuringPeriod",
        ] {
            assert!(t.concept(name).is_some(), "missing concept {name}");
        }
        let applied = t.concept("AccountingStandardsApplied").unwrap();
        assert!(applied.is_fixed(), "AccountingStandardsApplied must be fixedItemType");
        assert!(!applied.is_numeric_type());
        let desc = t.concept("DescriptionPrincipalActivities").unwrap();
        assert!(!desc.is_fixed());
        assert_eq!(desc.data_type, "stringItemType");
        let start = t.concept("StartDateForPeriodCoveredByReport").unwrap();
        assert_eq!(start.period, Some(PeriodType::Instant));
        assert!(start.is_date_type());
        let turnover = t.concept("TurnoverRevenue").unwrap();
        assert!(turnover.is_numeric_type());
        assert_eq!(turnover.period, Some(PeriodType::Duration));
    }

    /// The embedded table must cover the full ct-comp computation taxonomy
    /// (not just the subset the generators emit), so computation documents
    /// from any tool are schema-checked.  Spot-check concepts from the
    /// previously-uncovered majority, and the facets that newly drive checks.
    #[test]
    fn embedded_table_covers_full_ct_comp_taxonomy() {
        let t = Taxonomy::embedded();
        // Concepts only used by computation documents, none emitted by the
        // generators — the point of the extension.
        for name in [
            "DescriptionOfTrade",
            "TextNote",
            "TaxDistrict",
            "NetChargeableGains",
            "QualifyingDonations",
            "AdjustmentsCreativeProductionCompanyAdjustment",
            "SubsidisedQualifyingExpenditureOnIn-HouseDirectRD",
            "StructuresAndBuildingsAllowanceDescriptionOfAsset",
            "ChargeableGainsNameOfCounterparty",
        ] {
            assert!(t.concept(name).is_some(), "missing ct-comp concept {name}");
        }
        // The periodType / type metadata that powers the schema checks.
        let net = t.concept("NetTradingProfits").unwrap();
        assert!(net.is_numeric_type());
        assert_eq!(net.period, Some(PeriodType::Duration));
        let year = t.concept("FinancialYear1CoveredByTheReturn").unwrap();
        assert_eq!(year.data_type, "gYearItemType");
        // Facets: tax reference / district patterns, non-empty strings.
        let tax_ref = t.concept("TaxReference").unwrap();
        assert_eq!(tax_ref.pattern.as_deref(), Some("[0-9]{10}"));
        let tax_district = t.concept("TaxDistrict").unwrap();
        assert_eq!(tax_district.pattern.as_deref(), Some("[0-9]{3}"));
        let company_name = t.concept("CompanyName").unwrap();
        assert_eq!(company_name.min_length(), Some(1));
        let text_note = t.concept("TextNote").unwrap();
        assert_eq!(text_note.min_length(), Some(1));
    }

    /// The from_directory XSD parser must reproduce the same facets (pattern
    /// / minLength) as the embedded table, so `--taxonomy-dir` runs check
    /// identically.
    #[test]
    fn directory_parser_resolves_facets() {
        let dir = std::env::temp_dir().join("validate-uk-facet-test");
        std::fs::create_dir_all(&dir).unwrap();
        let types = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           xmlns:xbrli="http://www.xbrl.org/2003/instance"
           xmlns:ct-types="http://www.hmrc.gov.uk/schemas/ct/comp/types/2023-01-01"
           targetNamespace="http://www.hmrc.gov.uk/schemas/ct/comp/types/2023-01-01">
  <xs:complexType name="taxReferenceItemType">
    <xs:simpleContent>
      <xs:restriction base="xbrli:stringItemType">
        <xs:pattern value="[0-9]{10}"/>
        <xs:whiteSpace value="collapse"/>
      </xs:restriction>
    </xs:simpleContent>
  </xs:complexType>
  <xs:complexType name="nonEmptyStringItemType">
    <xs:simpleContent>
      <xs:restriction base="xbrli:stringItemType">
        <xs:minLength value="1"/>
      </xs:restriction>
    </xs:simpleContent>
  </xs:complexType>
</xs:schema>"#;
        let comp = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           xmlns:ct-types="http://www.hmrc.gov.uk/schemas/ct/comp/types/2023-01-01"
           xmlns:xbrli="http://www.xbrl.org/2003/instance"
           targetNamespace="http://example.comp">
  <xs:element name="TaxReference" type="ct-types:taxReferenceItemType" periodType="instant"/>
  <xs:element name="CompanyName" type="ct-types:nonEmptyStringItemType" periodType="instant"/>
</xs:schema>"#;
        std::fs::write(dir.join("a-types.xsd"), types).unwrap();
        std::fs::write(dir.join("b-comp.xsd"), comp).unwrap();
        let t = Taxonomy::from_directory(&dir).unwrap();
        assert_eq!(t.concept("TaxReference").unwrap().pattern.as_deref(), Some("[0-9]{10}"));
        assert_eq!(t.concept("CompanyName").unwrap().min_length(), Some(1));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parses_synthetic_xsd() {
        let dir = std::env::temp_dir().join("validate-uk-taxonomy-test");
        std::fs::create_dir_all(&dir).unwrap();
        let xsd = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           xmlns:types="http://xbrl.frc.org.uk/general/2023-01-01/types"
           xmlns:xbrli="http://www.xbrl.org/2003/instance"
           targetNamespace="http://example.test">
  <xs:element name="MyMoney" type="xbrli:monetaryItemType" periodType="duration" balance="debit" nillable="true"/>
  <xs:element name="MyChoice" periodType="instant">
    <xs:simpleType>
      <xs:restriction base="xs:string">
        <xs:enumeration value="Yes"/>
        <xs:enumeration value="No"/>
      </xs:restriction>
    </xs:simpleType>
  </xs:element>
  <xs:simpleType name="fixedItemType">
    <xs:restriction base="xs:token">
      <xs:length value="0"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:element name="MyMarker" type="types:fixedItemType" nillable="true"/>
</xs:schema>"#;
        std::fs::write(dir.join("test.xsd"), xsd).unwrap();
        let t = Taxonomy::from_directory(&dir).unwrap();
        assert_eq!(t.concepts.len(), 4);
        let money = t.concept("MyMoney").unwrap();
        assert_eq!(money.data_type, "monetaryItemType");
        assert_eq!(money.period, Some(PeriodType::Duration));
        assert_eq!(money.balance.as_deref(), Some("debit"));
        assert!(money.nillable);
        let choice = t.concept("MyChoice").unwrap();
        assert_eq!(choice.enums, vec!["Yes".to_string(), "No".to_string()]);
        let marker = t.concept("MyMarker").unwrap();
        assert!(marker.is_fixed());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
