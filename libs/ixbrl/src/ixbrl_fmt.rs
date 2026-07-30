use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::writer::Writer;
use std::collections::HashMap;
use std::io::Cursor;

// ============================================================================
// XmlNode — generic XML node tree
// ============================================================================

/// A node in an XML / XHTML tree.
///
/// An `Elem` may have zero or more children; the `Text` variant holds
/// character data that quick-xml will XML-escape.
#[derive(Debug, Clone)]
pub enum XmlNode {
    Elem {
        name: String,
        attributes: Vec<(String, String)>,
        children: Vec<XmlNode>,
    },
    Text(String),
}

// -- Constructor helpers -----------------------------------------------------

/// Create an element with no attributes and no children.
pub fn el(name: &str) -> XmlNode {
    XmlNode::Elem {
        name: name.to_string(),
        attributes: Vec::new(),
        children: Vec::new(),
    }
}

/// Create an element with the given attributes.
pub fn elt(name: &str, attrs: &[(&str, &str)]) -> XmlNode {
    XmlNode::Elem {
        name: name.to_string(),
        attributes: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        children: Vec::new(),
    }
}

/// Create an element with attributes and a single text child.
pub fn elt_text(name: &str, attrs: &[(&str, &str)], text: &str) -> XmlNode {
    XmlNode::Elem {
        name: name.to_string(),
        attributes: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        children: vec![XmlNode::Text(text.to_string())],
    }
}

/// Create a text node (escaped by quick-xml).
pub fn text(s: &str) -> XmlNode {
    XmlNode::Text(s.to_string())
}

// -- Builder methods on XmlNode ----------------------------------------------

impl XmlNode {
    /// Chain a single child node.
    pub fn child(mut self, child: XmlNode) -> Self {
        match &mut self {
            XmlNode::Elem { children, .. } => children.push(child),
            _ => panic!("cannot add children to a non-Elem node"),
        }
        self
    }

    /// Chain multiple children.
    pub fn children(mut self, children: Vec<XmlNode>) -> Self {
        match &mut self {
            XmlNode::Elem {
                children: existing, ..
            } => existing.extend(children),
            _ => panic!("cannot add children to a non-Elem node"),
        }
        self
    }

    /// Push children in-place (non-consuming).
    pub fn push_children(&mut self, extra: Vec<XmlNode>) {
        if let XmlNode::Elem { children, .. } = self {
            children.extend(extra);
        }
    }

    // -- Serialization -------------------------------------------------------

    /// Serialise this node tree to an XML/HTML string.
    pub fn to_xml_string(&self) -> String {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        Self::write_node(self, &mut writer);
        let cursor = writer.into_inner();
        String::from_utf8(cursor.into_inner()).unwrap()
    }

    fn write_node(node: &XmlNode, writer: &mut Writer<Cursor<Vec<u8>>>) {
        match node {
            XmlNode::Elem {
                name,
                attributes,
                children,
            } => {
                if children.is_empty() {
                    // Self-closing element
                    let mut elem = BytesStart::new(name);
                    for (k, v) in attributes {
                        elem.push_attribute((k.as_str(), v.as_str()));
                    }
                    writer
                        .write_event(Event::Empty(elem))
                        .expect("write empty elem");
                } else {
                    let mut elem = BytesStart::new(name);
                    for (k, v) in attributes {
                        elem.push_attribute((k.as_str(), v.as_str()));
                    }
                    writer
                        .write_event(Event::Start(elem))
                        .expect("write start elem");
                    for child in children {
                        Self::write_node(child, writer);
                    }
                    writer
                        .write_event(Event::End(BytesEnd::new(name)))
                        .expect("write end elem");
                }
            }
            XmlNode::Text(t) => {
                writer
                    .write_event(Event::Text(BytesText::new(t)))
                    .expect("write text");
            }
        }
    }
}

// ============================================================================
// iXBRL / HTML helper elements
// ============================================================================

/// Build a `span` with string content.
pub fn span_text(t: &str) -> XmlNode {
    elt_text("span", &[], t)
}

/// Build a `span` with child nodes.
pub fn span(children: Vec<XmlNode>) -> XmlNode {
    elt("span", &[]).children(children)
}

/// Build a `div`.
pub fn div(class: &str, children: Vec<XmlNode>) -> XmlNode {
    elt("div", &[("class", class)]).children(children)
}

/// Build a `div` with a custom id.
pub fn div_id(class: &str, id: &str, children: Vec<XmlNode>) -> XmlNode {
    elt("div", &[("class", class), ("id", id)]).children(children)
}

/// Build an `h2`.
pub fn h2(text: &str) -> XmlNode {
    elt_text("h2", &[], text)
}

/// Build a `td` with string content.
pub fn td_text(class: &str, text: &str) -> XmlNode {
    elt_text("td", &[("class", class)], text)
}

/// Build a `td` with child nodes.
pub fn td(class: &str, children: Vec<XmlNode>) -> XmlNode {
    elt("td", &[("class", class)]).children(children)
}

/// Build a `tr` with optional class and cell children.
pub fn tr(class: Option<&str>, cells: Vec<XmlNode>) -> XmlNode {
    match class {
        Some(c) => elt("tr", &[("class", c)]).children(cells),
        None => elt("tr", &[]).children(cells),
    }
}

/// Build a `table`.
pub fn table(class: &str, rows: Vec<XmlNode>) -> XmlNode {
    elt("table", &[("class", class)]).children(rows)
}

/// Non-breaking space `\u{00A0}`.
pub fn nbsp() -> XmlNode {
    text("\u{00A0}")
}

/// Pound sign `£`.
pub fn pound() -> XmlNode {
    text("\u{00A3}")
}

// ============================================================================
// iXBRL fact helpers
// ============================================================================

/// Build an `ix:nonFraction` fact.
pub fn non_fraction_fmt(
    name: &str,
    ctx: &str,
    unit: &str,
    value: &str,
    decimals: &str,
    scale: &str,
) -> XmlNode {
    elt_text(
        "ix:nonFraction",
        &[
            ("name", name),
            ("contextRef", ctx),
            ("unitRef", unit),
            ("format", "ixt2:numdotdecimal"),
            ("decimals", decimals),
            ("scale", scale),
        ],
        value,
    )
}

/// Build an `ix:nonFraction` fact with the standard GBP / decimals=2 / scale=0.
pub fn non_fraction(name: &str, ctx: &str, value: &str) -> XmlNode {
    non_fraction_fmt(name, ctx, "U-GBP", value, "2", "0")
}

/// Build an `ix:nonNumeric` fact.
pub fn non_numeric(name: &str, ctx: &str, value: &str) -> XmlNode {
    elt_text(
        "ix:nonNumeric",
        &[("name", name), ("contextRef", ctx)],
        value,
    )
}

/// Build an `ix:nonNumeric` fact with a format attribute.
pub fn non_numeric_fmt(name: &str, ctx: &str, value: &str, format: &str) -> XmlNode {
    elt_text(
        "ix:nonNumeric",
        &[("name", name), ("contextRef", ctx), ("format", format)],
        value,
    )
}

// ============================================================================
// Structured iXBRL pages and facts
// ============================================================================

/// Build a fact div: `.fact > .ref + .description + .factvalue`.
pub fn fact_wrapper(ref_num: &str, description: &str, fact_value: XmlNode) -> XmlNode {
    elt("div", &[("class", "fact")]).children(vec![
        elt_text("div", &[("class", "ref")], ref_num),
        elt_text(
            "div",
            &[("class", "description")],
            &format!("{}:", description),
        ),
        elt("div", &[("class", "factvalue")]).child(fact_value),
    ])
}

/// Build a page div.
pub fn page(children: Vec<XmlNode>) -> XmlNode {
    div("page", children)
}

/// Build a facts container div.
pub fn facts(children: Vec<XmlNode>) -> XmlNode {
    div("facts", children)
}

/// Build a worksheet container div.
pub fn worksheet(children: Vec<XmlNode>) -> XmlNode {
    div("worksheet", children)
}

// ============================================================================
// Data cells (value cells for worksheet tables)
// ============================================================================

/// Two non-breaking spaces.
pub fn space2() -> XmlNode {
    text("\u{00A0}\u{00A0}")
}

/// Span wrapping two non-breaking spaces.
pub fn span_space2() -> XmlNode {
    elt("span", &[]).child(text("\u{00A0}\u{00A0}"))
}

/// Build a `td` with a plain numeric value.
///
/// Trailing non-breaking spaces are included **in the text content**
/// (not as a separate child element) for consistent visual alignment
/// with iXBRL-tagged cells.  Negative values are never followed by NBSP
/// — matching the reference output.
pub fn data_cell(value: f64) -> XmlNode {
    let formatted = format_f64(value);
    if value == 0.0 {
        td("data value nil cell", vec![spannbsp("0.00")])
    } else if value < 0.0 {
        td(
            "data value negative cell",
            vec![span(vec![
                span_text("( "),
                span_text(&format_f64(value.abs())),
                span_text(" )"),
            ])],
        )
    } else {
        td("data value cell", vec![spannbsp(&formatted)])
    }
}

/// Build a `td` with a plain total-value (breakdown total style).
///
/// Trailing NBSP is baked into the text content for non-negative values.
pub fn data_cell_total(value: f64) -> XmlNode {
    let formatted = format_f64(value);
    if value == 0.0 {
        td(
            "data value breakdown total nil cell",
            vec![spannbsp("0.00")],
        )
    } else if value < 0.0 {
        td(
            "data value breakdown total negative cell",
            vec![span(vec![
                span_text("( "),
                span_text(&format_f64(value.abs())),
                span_text(" )"),
            ])],
        )
    } else {
        td(
            "data value breakdown total cell",
            vec![spannbsp(&formatted)],
        )
    }
}

/// `span_text` with two trailing non-breaking spaces baked in.
fn spannbsp(s: &str) -> XmlNode {
    span_text(&format!("{}\u{00A0}\u{00A0}", s))
}

/// Build a `td` with an iXBRL-tagged value (breakdown total style).
pub fn data_cell_total_ix(name: &str, ctx: &str, value: f64) -> XmlNode {
    let formatted = format_f64(value.abs());
    if value < 0.0 {
        td(
            "data value breakdown total negative cell",
            vec![span(vec![
                span_text("( "),
                non_fraction(name, ctx, &formatted),
                span_text(" )"),
            ])],
        )
    } else if value == 0.0 {
        td(
            "data value breakdown total nil cell",
            vec![span(vec![non_fraction(name, ctx, "0.00"), span_space2()])],
        )
    } else {
        td(
            "data value breakdown total cell",
            vec![span(vec![
                non_fraction(name, ctx, &formatted),
                span_space2(),
            ])],
        )
    }
}

/// Build a `td` with an iXBRL-tagged value (plain data cell style).
pub fn data_cell_ix(value: f64, name: &str, ctx: &str) -> XmlNode {
    let formatted = format_f64(value);
    if value == 0.0 {
        td(
            "data value nil cell",
            vec![span(vec![
                el("span"),
                non_fraction(name, ctx, "0.00"),
                span_space2(),
            ])],
        )
    } else {
        td(
            "data value cell",
            vec![span(vec![
                el("span"),
                non_fraction(name, ctx, &formatted),
                span_space2(),
            ])],
        )
    }
}

/// Build a `td` with an iXBRL-tagged value displayed as a negative (wrapped in parens).
pub fn data_cell_neg_ix(name: &str, ctx: &str, value: f64) -> XmlNode {
    let formatted = format_f64(value);
    if value == 0.0 {
        td(
            "data value nil cell",
            vec![span(vec![
                el("span"),
                non_fraction(name, ctx, "0.00"),
                span_space2(),
            ])],
        )
    } else {
        td(
            "data value negative cell",
            vec![span(vec![
                span_text("( "),
                non_fraction(name, ctx, &formatted),
                span_text(" )"),
            ])],
        )
    }
}

/// Build a `td` with a heading/total cell with iXBRL (data value total style).
pub fn data_cell_total_n_ix(name: &str, ctx: &str, value: f64) -> XmlNode {
    let formatted = format_f64(value);
    if value == 0.0 {
        td(
            "data value total nil cell",
            vec![span(vec![
                el("span"),
                non_fraction(name, ctx, "0.00"),
                span_space2(),
            ])],
        )
    } else {
        td(
            "data value total cell",
            vec![span(vec![
                el("span"),
                non_fraction(name, ctx, &formatted),
                span_space2(),
            ])],
        )
    }
}

/// Build a `td` with a heading/total cell with iXBRL (data value total negative style).
pub fn data_cell_total_neg_ix(name: &str, ctx: &str, value: f64) -> XmlNode {
    let formatted = format_f64(value);
    if value == 0.0 {
        td(
            "data value total nil cell",
            vec![span(vec![
                el("span"),
                non_fraction(name, ctx, "0.00"),
                span_space2(),
            ])],
        )
    } else {
        td(
            "data value total negative cell",
            vec![span(vec![
                span_text("( "),
                non_fraction(name, ctx, &formatted),
                span_text(" )"),
            ])],
        )
    }
}

// ============================================================================
// Table row helpers for worksheets
// ============================================================================

/// A standard worksheet row: label + current-year value + prev-year value.
pub fn table_row_ix(
    label: &str,
    name: &str,
    ctx_cur: &str,
    ctx_prev: &str,
    val_cur: f64,
    val_prev: f64,
) -> XmlNode {
    tr(
        Some("row"),
        vec![
            td("label breakdown item cell", vec![span_text(label)]),
            data_cell_ix(val_cur, name, ctx_cur),
            data_cell_ix(val_prev, name, ctx_prev),
        ],
    )
}

/// A worksheet row with negative (parens) values.
pub fn table_row_ix_neg(
    label: &str,
    name: &str,
    ctx_cur: &str,
    ctx_prev: &str,
    val_cur: f64,
    val_prev: f64,
) -> XmlNode {
    tr(
        Some("row"),
        vec![
            td("label breakdown item cell", vec![span_text(label)]),
            data_cell_neg_ix(name, ctx_cur, val_cur),
            data_cell_neg_ix(name, ctx_prev, val_prev),
        ],
    )
}

/// A total row (with separator line) using iXBRL tags.
pub fn table_total_row_ix(
    label: &str,
    name: &str,
    ctx_cur: &str,
    ctx_prev: &str,
    val_cur: f64,
    val_prev: f64,
) -> XmlNode {
    tr(
        Some("row"),
        vec![
            td_text("label breakdown total cell", label),
            data_cell_total_ix(name, ctx_cur, val_cur),
            data_cell_total_ix(name, ctx_prev, val_prev),
        ],
    )
}

/// A total row (with separator line) with negated values (costs shown in parens).
pub fn table_total_row_ix_neg(
    label: &str,
    name: &str,
    ctx_cur: &str,
    ctx_prev: &str,
    val_cur: f64,
    val_prev: f64,
) -> XmlNode {
    tr(
        Some("row"),
        vec![
            td_text("label breakdown total cell", label),
            data_cell_total_ix(name, ctx_cur, -val_cur),
            data_cell_total_ix(name, ctx_prev, -val_prev),
        ],
    )
}

/// A total row WITHOUT iXBRL tags (plain numbers).
pub fn table_row_total(label: &str, val_cur: f64, val_prev: f64) -> XmlNode {
    tr(
        Some("row"),
        vec![
            td_text("label breakdown total cell", label),
            data_cell_total(val_cur),
            data_cell_total(val_prev),
        ],
    )
}

// ============================================================================
// Worksheet header / currency rows
// ============================================================================

/// Build a header row for a P&amp;L worksheet table (with colspan="1" on each header).
pub fn worksheet_header_row_pl(fy2: i32, fy1: i32) -> XmlNode {
    tr(
        Some("row"),
        vec![
            td("label cell", vec![nbsp()]),
            elt_text(
                "td",
                &[("class", "column header cell"), ("colspan", "1")],
                &fy2.to_string(),
            ),
            elt_text(
                "td",
                &[("class", "column header cell"), ("colspan", "1")],
                &fy1.to_string(),
            ),
        ],
    )
}

/// Build a header row for a worksheet table: empty label + two column headers.
pub fn worksheet_header_row(fy2: i32, fy1: i32) -> XmlNode {
    tr(
        Some("row"),
        vec![
            td("label cell", vec![nbsp()]), // &#160;
            elt_text("td", &[("class", "column header cell")], &fy2.to_string()),
            elt_text("td", &[("class", "column header cell")], &fy1.to_string()),
        ],
    )
}

/// Build a currency symbol row for a worksheet table.
pub fn worksheet_currency_row() -> XmlNode {
    tr(
        Some("row"),
        vec![
            td("label cell", vec![nbsp()]),
            td("column currency cell", vec![pound()]),
            td("column currency cell", vec![pound()]),
        ],
    )
}

/// Build a blank spacer row.
pub fn spacer_row() -> XmlNode {
    tr(Some("row"), vec![td("label cell", vec![nbsp()])])
}

// ============================================================================
// XBRL infrastructure elements
// ============================================================================

/// Build a `xbrli:context` (instant).
pub fn context_instant(
    id: &str,
    scheme_id: &str,
    date: &chrono::NaiveDate,
    dim: Option<&str>,
    val: Option<&str>,
) -> XmlNode {
    let mut entity = elt("xbrli:entity", &[]).child(elt_text(
        "xbrli:identifier",
        &[("scheme", "http://www.companieshouse.gov.uk/")],
        scheme_id,
    ));
    if let Some(d) = dim {
        entity = entity.child(elt("xbrli:segment", &[]).child(elt_text(
            "xbrldi:explicitMember",
            &[("dimension", d)],
            val.unwrap_or(""),
        )));
    }
    elt("xbrli:context", &[("id", id)]).children(vec![
        entity,
        elt("xbrli:period", &[]).child(elt_text("xbrli:instant", &[], &date.to_string())),
    ])
}

/// Build a `xbrli:context` (duration) with explicit dimensions.
pub fn context_duration_full(
    id: &str,
    scheme_id: &str,
    start: &chrono::NaiveDate,
    end: &chrono::NaiveDate,
    typed_dim: Option<&str>,
    typed_val: Option<&str>,
    explicit_dims: &[(&str, &str)],
) -> XmlNode {
    let mut entity = elt("xbrli:entity", &[]).child(elt_text(
        "xbrli:identifier",
        &[("scheme", "http://www.companieshouse.gov.uk/")],
        scheme_id,
    ));
    if typed_dim.is_some() || !explicit_dims.is_empty() {
        let mut segment = elt("xbrli:segment", &[]);
        if let Some(d) = typed_dim {
            segment = segment.child(elt("xbrldi:typedMember", &[("dimension", d)]).child(
                elt_text("ct-comp:BusinessNameDomain", &[], typed_val.unwrap_or("")),
            ));
        }
        for (dim, val) in explicit_dims {
            segment = segment.child(elt_text(
                "xbrldi:explicitMember",
                &[("dimension", dim)],
                val,
            ));
        }
        entity = entity.child(segment);
    }
    elt("xbrli:context", &[("id", id)]).children(vec![
        entity,
        elt("xbrli:period", &[]).children(vec![
            elt_text("xbrli:startDate", &[], &start.to_string()),
            elt_text("xbrli:endDate", &[], &end.to_string()),
        ]),
    ])
}

/// Build a `xbrli:context` (duration) with a single explicit dimension.
pub fn context_duration(
    id: &str,
    scheme_id: &str,
    start: &chrono::NaiveDate,
    end: &chrono::NaiveDate,
    dim: Option<&str>,
    val: Option<&str>,
) -> XmlNode {
    let dims = match (dim, val) {
        (Some(d), Some(v)) => vec![(d, v)],
        _ => vec![],
    };
    context_duration_full(id, scheme_id, start, end, None, None, &dims)
}

/// Build a `xbrli:unit`.
pub fn unit(measure: &str) -> XmlNode {
    elt("xbrli:unit", &[("id", "U-GBP")]).child(elt_text("xbrli:measure", &[], measure))
}

// ============================================================================
// ParsedIxBrlFacts (round-trip support) — unchanged except where needed
// ============================================================================

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedIxBrlFacts {
    pub numeric: HashMap<String, f64>,
    pub non_numeric: HashMap<String, String>,
    pub numeric_by_ctx: HashMap<(String, String), f64>,
    pub non_numeric_by_ctx: HashMap<(String, String), String>,
}

impl ParsedIxBrlFacts {
    /// Render these facts as an ixbrl HTML document with CSS envelope.
    pub fn to_html(&self) -> String {
        // Build the document structure
        let mut hidden = Vec::new();

        // Write non-numeric facts with ctxt-0 context
        for (name, value) in &self.non_numeric {
            if self
                .non_numeric_by_ctx
                .contains_key(&(name.clone(), "ctxt-0".to_string()))
            {
                hidden.push(non_numeric(name, "ctxt-0", value));
            }
        }

        let header = elt("ix:header", &[]).children(vec![
            elt("ix:hidden", &[]).children(hidden),
            elt("ix:references", &[]).children(vec![
                elt_text(
                    "link:schemaRef",
                    &[
                        ("xlink:type", "simple"),
                        (
                            "xlink:href",
                            "http://www.hmrc.gov.uk/schemas/ct/comp/2023-01-01/ct-comp-2023.xsd",
                        ),
                    ],
                    "",
                ),
                elt_text(
                    "link:schemaRef",
                    &[
                        ("xlink:type", "simple"),
                        (
                            "xlink:href",
                            "https://xbrl.frc.org.uk/dpl/2023-01-01/dpl-2023-01-01.xsd",
                        ),
                    ],
                    "",
                ),
            ]),
        ]);

        let mut report_facts = Vec::new();
        for (name, value) in &self.non_numeric {
            report_facts.push(fact_wrapper("-", name, non_numeric(name, "ctxt-0", value)));
        }
        let mut num_map: HashMap<String, f64> = HashMap::new();
        for ((name, _ctx), value) in &self.numeric_by_ctx {
            num_map.insert(name.clone(), *value);
        }
        for (name, value) in &num_map {
            report_facts.push(fact_wrapper(
                "-",
                name,
                non_fraction(name, "ctxt-0", &format_f64(*value)),
            ));
        }

        let doc = elt("html", HTML_ATTRS).children(vec![
            elt("head", &[]).children(vec![
                elt_text("title", &[], "Corporation Tax Statement"),
                elt_text(
                    "style",
                    &[("type", "text/css")],
                    include_str!("reports/uk_frs105_corp_tax.css"),
                ),
            ]),
            elt("body", &[]).children(vec![
                elt("div", &[("style", "display:none")]).child(header),
                elt("div", &[("id", "report"), ("class", "report")]).children(vec![div(
                    "page",
                    vec![facts(vec![h2("Corporation Tax Facts")])],
                )]),
            ]),
        ]);

        let body = doc.to_xml_string();

        let decl = "<?xml version='1.0' encoding='UTF-8'?>\n".to_string();
        decl + &body
    }
}

#[rustfmt::skip]
pub const HTML_ATTRS: &[(&str, &str)] = &[
    ("xmlns", "http://www.w3.org/1999/xhtml"),
    ("xmlns:ix", "http://www.xbrl.org/2013/inlineXBRL"),
    ("xmlns:link", "http://www.xbrl.org/2003/linkbase"),
    ("xmlns:xlink", "http://www.w3.org/1999/xlink"),
    ("xmlns:xbrli", "http://www.xbrl.org/2003/instance"),
    ("xmlns:xbrldi", "http://xbrl.org/2006/xbrldi"),
    ("xmlns:ixt2", "http://www.xbrl.org/inlineXBRL/transformation/2011-07-31"),
    ("xmlns:iso4217", "http://www.xbrl.org/2003/iso4217"),
    ("xmlns:ct-comp", "http://www.hmrc.gov.uk/schemas/ct/comp/2023-01-01"),
    ("xmlns:dpl", "http://xbrl.frc.org.uk/dpl/2023-01-01"),
    ("xmlns:uk-bus", "http://xbrl.frc.org.uk/cd/2023-01-01/business"),
    ("xmlns:uk-core", "http://xbrl.frc.org.uk/fr/2023-01-01/core"),
    ("xmlns:uk-geo", "http://xbrl.frc.org.uk/cd/2023-01-01/countries"),
];

impl ParsedIxBrlFacts {
    pub fn from_html(html: &str) -> ParsedIxBrlFacts {
        use quick_xml::Reader;
        use quick_xml::events::Event;

        let mut facts = ParsedIxBrlFacts::default();
        let mut reader = Reader::from_str(html);
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let attrs: HashMap<String, String> = e
                        .attributes()
                        .filter_map(|a| a.ok())
                        .map(|a| {
                            (
                                String::from_utf8_lossy(a.key.as_ref()).to_string(),
                                String::from_utf8_lossy(&a.value).to_string(),
                            )
                        })
                        .collect();

                    let name = attrs.get("name").cloned();
                    let ctx = attrs.get("contextRef").cloned();

                    if let (Some(name), Some(ctx)) = (name, ctx)
                        && (tag == "ix:nonFraction" || tag == "ix:nonNumeric")
                    {
                        let mut text_buf = Vec::new();
                        if let Ok(Event::Text(text)) = reader.read_event_into(&mut text_buf) {
                            let raw = text.unescape().unwrap_or_default().to_string();
                            let val = raw.trim();
                            if tag == "ix:nonFraction" {
                                let cleaned = val.replace(',', "");
                                if let Ok(v) = cleaned.parse::<f64>() {
                                    facts.numeric.insert(name.clone(), v);
                                    facts.numeric_by_ctx.insert((name, ctx), v);
                                }
                            } else {
                                facts.non_numeric.insert(name.clone(), val.to_string());
                                facts
                                    .non_numeric_by_ctx
                                    .insert((name, ctx), val.to_string());
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        facts
    }
}

// ============================================================================
// Utility: number formatting
// ============================================================================

pub fn format_f64(v: f64) -> String {
    // Format the absolute value first so comma-insertion works correctly
    // regardless of whether the number is negative.  The minus sign is
    // prepended at the end if needed.
    let abs_v = v.abs();
    let formatted = format!("{:.2}", abs_v);
    let parts: Vec<&str> = formatted.split('.').collect();
    let int_part = parts[0];
    let dec_part = parts.get(1).unwrap_or(&"00");
    let mut result = String::new();
    let bytes = int_part.as_bytes();
    let len = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(*b as char);
    }
    result.push('.');
    result.push_str(dec_part);
    if v < 0.0 {
        result.insert(0, '-');
    }
    result
}
