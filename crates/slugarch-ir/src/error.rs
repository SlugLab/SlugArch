use serde::Serialize;

/// Errors that can arise inside the IR layer — construction, passes, or serialization.
#[derive(Debug, thiserror::Error)]
pub enum IrError {
    #[error("serialization failed: {0}")]
    Serialize(String),

    #[error("deserialization failed: {0}")]
    Deserialize(String),

    #[error("pass {pass} failed on op {node:?}: {msg}")]
    PassFailed {
        pass: &'static str,
        node: Option<crate::types::OpId>,
        msg: String,
    },

    #[error("token graph has a cycle: {cycle:?}")]
    TokenGraphCycle { cycle: Vec<crate::types::OpId> },

    #[error("oracle mismatch: {diff}")]
    OracleMismatch { diff: String },
}

/// Structural diff between a SlugIR function and an rtlmap pipeline.
/// Populated by `validate_against_rtlmap`; rendered into IrError::OracleMismatch.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RtlmapDiff {
    pub missing_nodes: Vec<String>,
    pub extra_nodes: Vec<String>,
    pub wrong_ip: Vec<(String, String, String)>, // (node_id, expected, actual)
    pub edge_diff: Vec<String>,
}

impl RtlmapDiff {
    pub fn is_empty(&self) -> bool {
        self.missing_nodes.is_empty()
            && self.extra_nodes.is_empty()
            && self.wrong_ip.is_empty()
            && self.edge_diff.is_empty()
    }
}
