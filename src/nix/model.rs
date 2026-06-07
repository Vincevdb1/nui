use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Input {
    pub name: String,
    pub url: String,
    pub branch: Option<String>,
    pub rev: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Output {
    pub path: String,
    pub name: Option<String>,
    pub config_type: String,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Package {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub is_unfree: bool,
    pub source_input: Option<String>,
}
