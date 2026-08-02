use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum FontFormat {
    Ttf,
    Otf,
    Ttc,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontFace {
    pub family: String,
    pub style: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontEntry {
    pub canonical_path: String,
    pub file_name: String,
    pub format: FontFormat,
    pub faces: Vec<FontFace>,
    pub loaded_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationError {
    pub path: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub fonts: Vec<FontEntry>,
    pub restore_candidates: Vec<String>,
    pub exit_errors: Vec<OperationError>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub snapshot: AppSnapshot,
    pub changed: Vec<String>,
    pub skipped: Vec<String>,
    pub errors: Vec<OperationError>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgress {
    pub operation: String,
    pub completed: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExitResult {
    pub exiting: bool,
    pub errors: Vec<OperationError>,
}
