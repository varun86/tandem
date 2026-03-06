use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireProviderCatalog {
    pub all: Vec<WireProviderEntry>,
    pub connected: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireProviderEntry {
    pub id: String,
    pub name: Option<String>,
    pub models: HashMap<String, WireProviderModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireProviderModel {
    pub name: Option<String>,
    pub limit: Option<WireProviderModelLimit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireProviderModelLimit {
    pub context: Option<u32>,
}
