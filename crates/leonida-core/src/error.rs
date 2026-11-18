use thiserror::Error;

/// Errors returned by `leonida-core` operations.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("unsupported gtamaplib-vc schema: {0}")]
    UnsupportedSchema(String),

    #[error("cannot build IRL-to-IG model from empty anchors")]
    NoAnchors,

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}
