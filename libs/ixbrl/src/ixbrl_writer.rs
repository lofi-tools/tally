use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::writer::Writer;
use std::io::{Cursor, Write};

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
