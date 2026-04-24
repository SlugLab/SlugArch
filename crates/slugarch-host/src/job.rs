//! GemmJob + GemmResult types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmJob {
    pub a: [[u8; 4]; 4],
    pub b: [[u8; 4]; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GemmResult {
    pub c: [[u32; 4]; 4],
    pub cycles: u64,
    pub flits_sent: u64,
    pub flits_received: u64,
}
