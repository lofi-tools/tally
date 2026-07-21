use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Context, DimensionMap, Document, Period, Segment, ValueKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericTaxonomy {
    pub title: String,
    pub style: Option<String>,
    pub contexts: Vec<Context>,
    pub computations: Vec<GenericComputation>,
    pub metadata: Vec<GenericMetadata>,
    pub tags: HashMap<String, String>,
    pub schema: Vec<String>,
    pub namespaces: HashMap<String, String>,
    pub document: Option<Document>,
    pub sign_reversed: HashMap<String, bool>,
    pub segment: DimensionMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericComputation {
    pub id: String,
    pub description: String,
    pub kind: String,
    pub period: Period,
    pub inputs: Option<Vec<String>>,
    pub accounts: Option<Vec<String>>,
    pub segments: Option<Vec<Segment>>,
    pub reverse_sign: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericMetadata {
    pub id: String,
    pub config: Option<String>,
    pub context: String,
    pub kind: Option<ValueKind>,
    pub value: Option<String>,
}
