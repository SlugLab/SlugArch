use crate::types::OpId;
use serde::{Deserialize, Serialize};

/// A dependency edge between two ops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Edge {
    /// The dst op consumes the src op's value (SSA data dependency).
    Data(OpId, OpId),
    /// The dst op must wait for the src op's token retirement (ordering dependency).
    Token(OpId, OpId),
}

impl Edge {
    pub fn src(&self) -> OpId {
        match self {
            Edge::Data(s, _) | Edge::Token(s, _) => *s,
        }
    }
    pub fn dst(&self) -> OpId {
        match self {
            Edge::Data(_, d) | Edge::Token(_, d) => *d,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edge_accessors() {
        let e = Edge::Data(OpId(3), OpId(5));
        assert_eq!(e.src(), OpId(3));
        assert_eq!(e.dst(), OpId(5));
    }
}
