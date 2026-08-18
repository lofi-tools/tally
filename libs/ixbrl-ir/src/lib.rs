//! The iXBRL intermediate representation (IR).
//!
//! The generic XML tree ([`XmlNode`]) and the parsed-facts model
//! ([`ParsedIxBrlFacts`], [`xbrl_context_dimensions`]) that the reports,
//! the CT600 return serializer and the Companies House filing parse all
//! build on, plus the iXBRL formatting helpers (contexts, units, fact and
//! table builders) and the display formatting ([`format_f64`]).
//!
//! This crate is a leaf: it depends only on quick-xml and the standard
//! library.

pub mod ixbrl_fmt;
pub use ixbrl_fmt::{
    HTML_ATTRS, ParsedIxBrlFacts, XmlNode, context_duration, context_duration_full,
    context_instant, data_cell, data_cell_ix, data_cell_neg_ix, data_cell_total,
    data_cell_total_ix, data_cell_total_n_ix, data_cell_total_neg_ix, div, div_id, el, elt,
    elt_text, fact_wrapper, facts, format_f64, h2, nbsp, non_fraction, non_fraction_fmt,
    non_numeric, non_numeric_fmt, page, pound, spacer_row, span, span_text, space2,
    span_space2, table, table_row_ix, table_row_ix_neg, table_row_total, table_total_row_ix,
    table_total_row_ix_neg, td, td_text, text, tr, unit, worksheet, worksheet_currency_row,
    worksheet_header_row, worksheet_header_row_pl, xbrl_context_dimensions,
};
