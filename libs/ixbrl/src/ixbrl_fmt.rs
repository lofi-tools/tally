use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::writer::Writer;
use std::collections::HashMap;
use std::io::{Cursor, Write};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedIxBrlFacts {
    pub numeric: HashMap<String, f64>,
    pub non_numeric: HashMap<String, String>,
    pub numeric_by_ctx: HashMap<(String, String), f64>,
    pub non_numeric_by_ctx: HashMap<(String, String), String>,
}

impl ParsedIxBrlFacts {
    /// Render these facts as an ixbrl HTML document with CSS envelope
    pub fn to_html(&self) -> String {
        let mut w = IxbrlWriter::new();

        w.write_raw("<?xml version='1.0' encoding='ASCII'?>\n");
        w.write_raw("<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:ix=\"http://www.xbrl.org/2013/inlineXBRL\" xmlns:link=\"http://www.xbrl.org/2003/linkbase\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" xmlns:xbrli=\"http://www.xbrl.org/2003/instance\" xmlns:xbrldi=\"http://xbrl.org/2006/xbrldi\" xmlns:ixt2=\"http://www.xbrl.org/inlineXBRL/transformation/2011-07-31\" xmlns:iso4217=\"http://www.xbrl.org/2003/iso4217\" xmlns:ct-comp=\"http://www.hmrc.gov.uk/schemas/ct/comp/2023-01-01\" xmlns:dpl=\"http://xbrl.frc.org.uk/dpl/2023-01-01\" xmlns:uk-bus=\"http://xbrl.frc.org.uk/cd/2023-01-01/business\" xmlns:uk-core=\"http://xbrl.frc.org.uk/fr/2023-01-01/core\" xmlns:uk-geo=\"http://xbrl.frc.org.uk/cd/2023-01-01/countries\">");

        w.write_raw("<head><title>Corporation Tax Statement</title><style type=\"text/css\">\n");
        w.write_raw(include_str!("taxonomy_mappings/ct_return_style.css"));
        w.write_raw("</style></head><body>");

        self.write_header(&mut w);
        self.write_resources(&mut w);

        w.write_raw("</body></html>");
        w.into_string()
    }

    fn write_header(&self, w: &mut IxbrlWriter) {
        w.write_raw("<div style=\"display:none\"><ix:header>");
        w.write_raw("<ix:hidden>");

        // Write non-numeric facts with ctxt-0 context
        for (name, value) in &self.non_numeric {
            if let Some(val) = self
                .non_numeric_by_ctx
                .get(&(name.clone(), "ctxt-0".to_string()))
            {
                w.open_element("ix:nonNumeric", &[("name", name), ("contextRef", "ctxt-0")]);
                w.write_raw(value);
                w.close_element("ix:nonNumeric");
            }
        }

        w.write_raw("</ix:hidden>");

        w.write_raw("<ix:references>");
        w.write_raw("<link:schemaRef xlink:type=\"simple\" xlink:href=\"http://www.hmrc.gov.uk/schemas/ct/comp/2023-01-01/ct-comp-2023.xsd\"></link:schemaRef>");
        w.write_raw("<link:schemaRef xlink:type=\"simple\" xlink:href=\"https://xbrl.frc.org.uk/dpl/2023-01-01/dpl-2023-01-01.xsd\"></link:schemaRef>");
        w.write_raw("</ix:references>");
    }

    fn write_resources(&self, w: &mut IxbrlWriter) {
        w.write_raw("<ix:resources>");

        // Write contexts based on what we find in the facts
        // ctxt-0: instant context for company
        // ctxt-1, ctxt-2, etc: duration contexts

        // Write unit
        w.open_element("xbrli:unit", &[("id", "U-GBP")]);
        w.write_element("xbrli:measure", &[], "iso4217:GBP");
        w.close_element("xbrli:unit");

        w.write_raw("</ix:resources></ix:header></div>");

        w.write_raw("<div id=\"report\" class=\"report\">");
        self.write_report(w);
        w.write_raw("</div>");
    }

    fn write_report(&self, w: &mut IxbrlWriter) {
        // Write report content based on facts
        // This is a simplified version - just output the facts
        w.write_raw("<div class=\"page\"><h2>Corporation Tax Facts</h2>");

        for (name, value) in &self.non_numeric {
            w.open_element("div", &[("class", "fact")]);
            w.write_element("div", &[("class", "ref")], "-");
            w.write_element("div", &[("class", "description")], &format!("{}:", name));
            w.open_element("div", &[("class", "factvalue")]);
            w.open_element("ix:nonNumeric", &[("name", name), ("contextRef", "ctxt-0")]);
            w.write_raw(value);
            w.close_element("ix:nonNumeric");
            w.close_element("div");
            w.close_element("div");
        }

        for (name, value) in &self.numeric {
            w.open_element("div", &[("class", "fact")]);
            w.write_element("div", &[("class", "ref")], "-");
            w.write_element("div", &[("class", "description")], &format!("{}:", name));
            w.open_element("div", &[("class", "factvalue")]);
            w.open_element(
                "ix:nonFraction",
                &[
                    ("name", name),
                    ("contextRef", "ctxt-0"),
                    ("unitRef", "U-GBP"),
                    ("format", "ixt2:numdotdecimal"),
                    ("decimals", "2"),
                    ("scale", "0"),
                ],
            );
            w.write_raw(&format!("{:.2}", value));
            w.close_element("ix:nonFraction");
            w.close_element("div");
            w.close_element("div");
        }

        w.write_raw("</div>");
    }
}

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
                                if let Ok(v) = val.parse::<f64>() {
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

pub struct IxbrlWriter {
    writer: Writer<Cursor<Vec<u8>>>,
}

impl IxbrlWriter {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(Cursor::new(Vec::new())),
        }
    }

    pub fn write_declaration(&mut self) {
        self.writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("ASCII"), None)))
            .unwrap();
    }

    pub fn write_raw(&mut self, raw: &str) {
        self.writer.get_mut().write_all(raw.as_bytes()).unwrap();
    }

    pub fn write_text(&mut self, text: &str) {
        self.writer
            .write_event(Event::Text(BytesText::new(text)))
            .unwrap();
    }

    pub fn write_element(&mut self, tag: &str, attrs: &[(&str, &str)], text: &str) {
        let mut elem = BytesStart::new(tag);
        for (k, v) in attrs {
            elem.push_attribute((*k, *v));
        }
        self.writer.write_event(Event::Start(elem)).unwrap();
        self.writer
            .write_event(Event::Text(BytesText::new(text)))
            .unwrap();
        self.writer
            .write_event(Event::End(BytesEnd::new(tag)))
            .unwrap();
    }

    pub fn open_element(&mut self, tag: &str, attrs: &[(&str, &str)]) {
        let mut elem = BytesStart::new(tag);
        for (k, v) in attrs {
            elem.push_attribute((*k, *v));
        }
        self.writer.write_event(Event::Start(elem)).unwrap();
    }

    pub fn close_element(&mut self, tag: &str) {
        self.writer
            .write_event(Event::End(BytesEnd::new(tag)))
            .unwrap();
    }

    pub fn into_string(self) -> String {
        let cursor = self.writer.into_inner();
        String::from_utf8(cursor.into_inner()).unwrap()
    }
}
