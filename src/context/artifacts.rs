use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectIndex {
    pub version: String,
    pub root: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexSummary {
    pub file_count: u32,
    pub code_count: u32,
}

#[derive(Debug)]
pub struct Artifacts;
