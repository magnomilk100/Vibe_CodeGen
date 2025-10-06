use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// ========================================
/// Request/Response wire protocol
/// ========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Plan,
    Codegen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Plan,
    Answer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tx {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    pub max_actions: usize,
    pub max_patch_bytes: usize,
    pub allowed_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Safety {
    pub path_allowlist: Vec<String>,
    pub command_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instruction {
    pub system: String,
    pub user: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
}

/// A snapshot of current file content we want the model to see.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBlob {
    pub path: String,
    pub bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    pub truncated: bool,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSlice {
    /// Free-form summary or flags about the project
    pub summary: Value,
    /// Optional index (unused for now)
    pub files_index: Vec<Value>,
    /// Optional routes (unused for now)
    pub routes: Vec<Value>,
    /// Optional symbol data (unused for now)
    pub symbols: Value,
    /// Optional diagnostics (unused for now)
    pub diagnostics: Vec<Value>,
    /// NEW: actual file contents provided to the model
    #[serde(default)]
    pub files_snapshot: Vec<FileBlob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub schema_version: String,
    pub mode: Mode,
    pub transaction: Tx,
    pub limits: Limits,
    pub task: String,
    pub context: ContextSlice,
    pub capabilities: Vec<String>,
    pub safety: Safety,
    pub instruction: Instruction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub summary: String,
    pub steps: Vec<Step>,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            summary: String::new(),
            steps: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Create {
        id: String,
        title: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    },
    Update {
        id: String,
        title: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        patch: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    },
    Delete {
        id: String,
        title: String,
        path: String,
    },
    Command {
        id: String,
        title: String,
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
    Test {
        id: String,
        title: String,
        command: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub schema_version: String,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<Plan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<Answer>,
}
