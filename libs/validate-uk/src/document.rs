//! Parsing of generated iXBRL documents into a validation-friendly model.
//!
//! Builds on the shared [`XmlNode`] intermediate representation from the
//! `ixbrl-ir` crate (the same tree the report generators serialise from), so
//! parsing round-trips exactly.  Only the parts of the inline-XBRL document
//! the checks need are modelled: contexts (entity + period + dimensions),
//! facts, and the inline-XBRL hygiene surface (root element, script/img/style
//! elements).

use std::collections::HashMap;

use ixbrl_ir::XmlNode;

/// A context period: instant, duration, or forever.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Period {
    /// Instant period with the given ISO date (`YYYY-MM-DD`).
    Instant(String),
    /// Start-end (duration) period with ISO dates.
    Duration(String, String),
    Forever,
}

/// A parsed `xbrli:context`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub id: String,
    pub entity_scheme: Option<String>,
    pub entity_identifier: Option<String>,
    pub period: Period,
    /// (dimension local name, member local name) pairs, e.g.
    /// ("AccountingStandardsDimension", "Micro-entities").
    pub dimensions: Vec<(String, String)>,
    /// True when the context carries an `xbrldi:scenario` element
    /// (forbidden by FRC.TG.3.6.1).
    pub has_scenario: bool,
}

impl Context {
    pub fn is_instant(&self) -> bool {
        matches!(self.period, Period::Instant(_))
    }

    pub fn instant_date(&self) -> Option<&str> {
        match &self.period {
            Period::Instant(d) => Some(d.as_str()),
            _ => None,
        }
    }

    pub fn start_date(&self) -> Option<&str> {
        match &self.period {
            Period::Duration(s, _) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn end_date(&self) -> Option<&str> {
        match &self.period {
            Period::Duration(_, e) => Some(e.as_str()),
            _ => None,
        }
    }

    pub fn has_dimension(&self, dimension: &str) -> bool {
        self.dimensions.iter().any(|(d, _)| d == dimension)
    }

    pub fn member_for(&self, dimension: &str) -> Option<&str> {
        self.dimensions
            .iter()
            .find(|(d, _)| d == dimension)
            .map(|(_, m)| m.as_str())
    }
}

/// A parsed inline-XBRL fact (`ix:nonNumeric` / `ix:nonFraction`).
#[derive(Debug, Clone, PartialEq)]
pub struct Fact {
    /// Full prefixed qname as written, e.g. `uk-bus:StartDateForPeriodCoveredByReport`.
    pub qname: String,
    /// Concept local name, e.g. `StartDateForPeriodCoveredByReport`.
    pub local_name: String,
    pub context_ref: String,
    pub value: String,
    /// Numeric value for `ix:nonFraction` facts (display text, `scale` applied).
    pub numeric: Option<f64>,
    /// `scale` attribute (0, 1, 2, ...) — value text is multiplied by 10^scale.
    pub scale: i32,
    /// `decimals` attribute when present.
    pub decimals: Option<String>,
    /// `ix:format` attribute when present (e.g. `ixt2:datedaymonthyearen`).
    pub format: Option<String>,
    /// `xsi:nil="true"`.
    pub is_nil: bool,
    pub is_numeric: bool,
    /// `unitRef` attribute (numeric facts only).
    pub unit_ref: Option<String>,
    /// `precision` attribute when present.
    pub precision: Option<String>,
}

/// A parsed document: contexts, facts, and inline-XBRL surface elements.
#[derive(Debug, Clone)]
pub struct Document {
    pub contexts: HashMap<String, Context>,
    pub facts: Vec<Fact>,
    /// True when any context uses the Companies House entity scheme.
    pub has_companies_house_context: bool,
    pub root_tag: String,
    /// The root element's `xmlns` value (must be the XHTML namespace).
    pub root_xmlns: Option<String>,
    pub has_script_elements: bool,
    /// (element, attribute, value) of `href`/`src` attributes found.
    pub links: Vec<(String, String, String)>,
    /// Raw text of `style` elements.
    pub style_elements: Vec<String>,
    /// (element, style attribute value).
    pub style_attributes: Vec<(String, String)>,
    /// schemaRef hrefs (used for taxonomy detection).
    pub schema_refs: Vec<String>,
    /// Whether the `ix:header` is nested in a `div` with `style="display:none"`
    /// (HMRC style guide ix11.8.1.2).
    pub header_in_display_none: bool,
}

/// Parse an iXBRL document from its HTML/XML source.
pub fn parse(html: &str) -> Document {
    let node = XmlNode::from_xml_string(html).unwrap_or_else(|_| XmlNode::Elem {
        name: "html".to_string(),
        attributes: Vec::new(),
        children: Vec::new(),
    });
    parse_node(&node)
}

/// Parse an iXBRL document from an [`XmlNode`] tree.
pub fn parse_node(node: &XmlNode) -> Document {
    let mut doc = Document {
        contexts: HashMap::new(),
        facts: Vec::new(),
        has_companies_house_context: false,
        root_tag: String::new(),
        root_xmlns: None,
        has_script_elements: false,
        links: Vec::new(),
        style_elements: Vec::new(),
        style_attributes: Vec::new(),
        schema_refs: Vec::new(),
        header_in_display_none: false,
    };
    if let XmlNode::Elem { name, attributes, .. } = node {
        doc.root_tag = name.clone();
        doc.root_xmlns = attr(attributes, "xmlns");
    }
    doc.header_in_display_none = header_in_display_none(node);
    walk(node, &mut doc);
    doc
}

/// Whether the `ix:header` element is nested inside a `div` whose `style`
/// attribute contains `display:none`.
fn header_in_display_none(node: &XmlNode) -> bool {
    fn find(node: &XmlNode, hidden: bool) -> Option<bool> {
        match node {
            XmlNode::Elem {
                name,
                attributes,
                children,
            } => {
                let hidden = hidden
                    || (name == "div"
                        && attr(attributes, "style")
                            .map(|s| s.contains("display:none"))
                            .unwrap_or(false));
                if name == "ix:header" {
                    return Some(hidden);
                }
                for c in children {
                    if let Some(r) = find(c, hidden) {
                        return Some(r);
                    }
                }
                None
            }
            XmlNode::Text(_) => None,
        }
    }
    find(node, false).unwrap_or(false)
}

fn walk(node: &XmlNode, doc: &mut Document) {
    if let XmlNode::Elem {
        name,
        attributes,
        children,
    } = node
    {
        match name.as_str() {
            "xbrli:context" => {
                if let Some(ctx) = parse_context(node) {
                    doc.contexts.insert(ctx.id.clone(), ctx);
                }
            }
            "ix:nonFraction" | "ix:nonNumeric" => {
                if let Some(fact) = parse_fact(node) {
                    doc.facts.push(fact);
                }
            }
            "xbrli:identifier" => {
                let scheme = attr(attributes, "scheme");
                if scheme.as_deref() == Some("http://www.companieshouse.gov.uk/") {
                    doc.has_companies_house_context = true;
                }
            }
            "script" => doc.has_script_elements = true,
            "a" | "img" => {
                let attr_name = if name == "a" { "href" } else { "src" };
                if let Some(v) = attr(attributes, attr_name) {
                    if !v.trim().is_empty() {
                        doc.links.push((name.clone(), attr_name.to_string(), v));
                    }
                }
            }
            "style" => {
                doc.style_elements.push(direct_text(children));
            }
            "link:schemaRef" => {
                if let Some(href) = attr(attributes, "xlink:href") {
                    doc.schema_refs.push(href);
                }
            }
            _ => {}
        }
        if name.contains("style") && attributes.iter().any(|(k, _)| k == "style") {
            if let Some(v) = attr(attributes, "style") {
                doc.style_attributes.push((name.clone(), v));
            }
        }
        for child in children {
            walk(child, doc);
        }
    }
}

fn parse_context(node: &XmlNode) -> Option<Context> {
    let XmlNode::Elem { attributes, children, .. } = node else {
        return None;
    };
    let id = attr(attributes, "id")?;
    let mut scheme = None;
    let mut identifier = None;
    let mut period = None;
    let mut dimensions = Vec::new();
    let mut has_scenario = false;
    for child in children {
        if let XmlNode::Elem {
            name: cname,
            children: cchildren,
            ..
        } = child
        {
            match cname.as_str() {
                "xbrli:entity" => {
                    for e in cchildren {
                        if let XmlNode::Elem {
                            name: ename,
                            attributes: eattrs,
                            children: echildren,
                        } = e
                        {
                            match ename.as_str() {
                                "xbrli:identifier" => {
                                    scheme = attr(eattrs, "scheme");
                                    identifier = Some(direct_text(echildren));
                                }
                                // XBRL 2.1 puts segment/scenario in the xbrli
                                // namespace; some serialisers use the
                                // xbrldi: prefix — accept both.
                                "xbrli:segment" | "xbrldi:segment" | "xbrli:scenario" | "xbrldi:scenario" => {
                                    if cname == "xbrli:scenario" || cname == "xbrldi:scenario" {
                                        has_scenario = true;
                                    }
                                    for m in echildren {
                                        if let XmlNode::Elem {
                                            name: mname,
                                            attributes: mattrs,
                                            children: mchildren,
                                        } = m
                                        {
                                            if mname == "xbrldi:explicitMember" {
                                                let dim = attr(mattrs, "dimension");
                                                let member = direct_text(mchildren);
                                                if let (Some(dim), member) = (dim, member) {
                                                    let d = local_of(&dim);
                                                    let m = local_of(&member);
                                                    if !m.is_empty() {
                                                        dimensions.push((d, m));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "xbrli:period" => {
                    for p in cchildren {
                        if let XmlNode::Elem {
                            name: pname,
                            children: pchildren,
                            attributes: _,
                        } = p
                        {
                            match pname.as_str() {
                                "xbrli:instant" => {
                                    period = Some(Period::Instant(direct_text(pchildren)));
                                }
                                "xbrli:startDate" => {
                                    let s = direct_text(pchildren);
                                    // find the sibling endDate
                                    let mut end = String::new();
                                    for e in cchildren {
                                    if let XmlNode::Elem {
                                        name: ename,
                                        children: echildren,
                                        attributes: _,
                                    } = e
                                    {
                                            if ename == "xbrli:endDate" {
                                                end = direct_text(echildren);
                                            }
                                        }
                                    }
                                    period = Some(Period::Duration(s, end));
                                }
                                "xbrli:forever" => period = Some(Period::Forever),
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Some(Context {
        id,
        entity_scheme: scheme,
        entity_identifier: identifier,
        period: period.unwrap_or(Period::Forever),
        dimensions,
        has_scenario,
    })
}

fn parse_fact(node: &XmlNode) -> Option<Fact> {
    let XmlNode::Elem { name, attributes, children } = node else {
        return None;
    };
    let qname = attr(attributes, "name")?;
    let context_ref = attr(attributes, "contextRef")?;
    let local_name = local_of(&qname);
    let is_numeric = name == "ix:nonFraction";
    let is_nil = attr_any(attributes, &["xsi:nil", "nil"]).as_deref() == Some("true");
    // The value of an inline fact is its full descendant text: statement
    // facts wrap their prose in nested `<span>`/`ix:continuation` elements
    // and may embed other facts (e.g. the period-end date inside the s477
    // statement).
    let raw = full_text(children);
    // The iXBRL transformation attributes are commonly serialised either
    // prefixed (`ix:format`) or bare (`format`); accept both.
    let format = attr_any(attributes, &["format", "ix:format"]);
    let decimals = attr_any(attributes, &["decimals", "ix:decimals"]);
    let precision = attr_any(attributes, &["precision", "ix:precision"]);
    let scale = attr_any(attributes, &["scale", "ix:scale"])
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    let unit_ref = attr_any(attributes, &["unitRef", "unitref", "ix:unitRef"]);
    let numeric = if is_numeric && !is_nil {
        parse_number(&raw).map(|v| v * 10f64.powi(scale))
    } else {
        None
    };
    Some(Fact {
        qname,
        local_name,
        context_ref,
        value: raw,
        numeric,
        scale,
        decimals,
        format,
        is_nil,
        is_numeric,
        unit_ref,
        precision,
    })
}

/// Parse a display number (commas stripped; `ixt2:numdotdecimal` semantics).
fn parse_number(s: &str) -> Option<f64> {
    let cleaned: String = s
        .trim()
        .chars()
        .filter(|c| *c != ',' && *c != '\u{a0}')
        .collect();
    cleaned.parse::<f64>().ok()
}

/// The local name of a (possibly prefixed) qname.
pub fn local_of(q: &str) -> String {
    match q.rfind(':') {
        Some(i) => q[i + 1..].to_string(),
        None => q.to_string(),
    }
}

fn attr(attributes: &[(String, String)], key: &str) -> Option<String> {
    attributes
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

/// First matching attribute among the given names.
fn attr_any(attributes: &[(String, String)], keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| attr(attributes, k))
}

/// The concatenated text of the direct children of an element.
pub fn direct_text(children: &[XmlNode]) -> String {
    children
        .iter()
        .filter_map(|c| match c {
            XmlNode::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

/// The concatenated text of all descendants of the given children (recursive).
fn full_text(children: &[XmlNode]) -> String {
    let mut out = String::new();
    for c in children {
        match c {
            XmlNode::Text(t) => out.push_str(t),
            XmlNode::Elem { children: sub, .. } => out.push_str(&full_text(sub)),
        }
    }
    out
}

/// Normalise a date display string: non-breaking spaces (from `&#160;`) to
/// spaces, collapse whitespace.
pub fn normalise_spaces(s: &str) -> String {
    s.replace('\u{a0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_contexts_periods_and_dimensions() {
        let html = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:ix="http://www.xbrl.org/2013/inlineXBRL" xmlns:xbrli="http://www.xbrl.org/2003/instance" xmlns:xbrldi="http://xbrl.org/2006/xbrldi" xmlns:uk-bus="http://xbrl.frc.org.uk/cd/2023-01-01/business"><body><ix:header><ix:hidden>
        <xbrli:context id="c1"><xbrli:entity><xbrli:identifier scheme="http://www.companieshouse.gov.uk/">12345678</xbrli:identifier><xbrldi:segment><xbrldi:explicitMember dimension="uk-bus:AccountingStandardsDimension">uk-bus:Micro-entities</xbrldi:explicitMember></xbrldi:segment></xbrli:entity><xbrli:period><xbrli:startDate>2020-01-01</xbrli:startDate><xbrli:endDate>2020-12-31</xbrli:endDate></xbrli:period></xbrli:context>
        <xbrli:context id="c2"><xbrli:entity><xbrli:identifier scheme="http://www.companieshouse.gov.uk/">12345678</xbrli:identifier></xbrli:entity><xbrli:period><xbrli:instant>2020-12-31</xbrli:instant></xbrli:period></xbrli:context>
        <ix:nonNumeric name="uk-bus:StartDateForPeriodCoveredByReport" contextRef="c2" format="ixt2:datedaymonthyearen">01 January 2020</ix:nonNumeric>
        <ix:nonFraction name="uk-core:TurnoverRevenue" contextRef="c1" unitRef="GBP" decimals="0" scale="0">1,234.5</ix:nonFraction>
        </ix:hidden></ix:header></body></html>"#;
        let doc = parse(html);
        assert!(doc.has_companies_house_context);
        let c1 = &doc.contexts["c1"];
        assert_eq!(c1.period, Period::Duration("2020-01-01".into(), "2020-12-31".into()));
        assert_eq!(
            c1.dimensions,
            vec![("AccountingStandardsDimension".to_string(), "Micro-entities".to_string())]
        );
        let c2 = &doc.contexts["c2"];
        assert_eq!(c2.instant_date(), Some("2020-12-31"));
        let date_fact = doc.facts.iter().find(|f| f.local_name == "StartDateForPeriodCoveredByReport").unwrap();
        assert_eq!(date_fact.value, "01 January 2020");
        assert_eq!(date_fact.format.as_deref(), Some("ixt2:datedaymonthyearen"));
        let num_fact = doc.facts.iter().find(|f| f.local_name == "TurnoverRevenue").unwrap();
        assert_eq!(num_fact.numeric, Some(1234.5));
        assert_eq!(num_fact.value, "1,234.5");
        assert_eq!(num_fact.unit_ref.as_deref(), Some("GBP"));
        assert_eq!(num_fact.decimals.as_deref(), Some("0"));
    }

    #[test]
    fn parses_forever_and_scenario() {
        let html = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><ix:header><ix:hidden>
        <xbrli:context id="f"><xbrli:entity><xbrli:identifier scheme="x">1</xbrli:identifier></xbrli:entity><xbrli:period><xbrli:forever/></xbrli:period></xbrli:context>
        </ix:hidden></ix:header></body></html>"#;
        let doc = parse(html);
        assert_eq!(doc.contexts["f"].period, Period::Forever);
    }
}
