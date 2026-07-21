use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRS101Taxonomy {
    pub title: String,
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FRSSETaxonomy {
    pub title: String,
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IFRSTaxonomy {
    pub title: String,
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct USGAAPTaxonomy {
    pub title: String,
    pub style: Option<String>,
}
