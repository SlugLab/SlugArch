//! ReplayArtifact — the fabric recording format.

use crate::FabricError;
use serde::{Deserialize, Serialize};
use slugarch_ir::module::Module;
use slugarch_ir::types::{BackendChoice, OpId};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayArtifact {
    pub slugir: Module,
    pub backend_choices: HashMap<OpId, BackendChoice>,
    pub host_mem: Vec<u8>,
    pub policy_name: String,
}

impl ReplayArtifact {
    pub fn from_module(slugir: &Module, host_mem: &[u8], policy_name: &str) -> Self {
        let mut backend_choices: HashMap<OpId, BackendChoice> = HashMap::new();
        for f in &slugir.functions {
            for (id, meta) in &f.meta {
                if let Some(bc) = meta.backend {
                    backend_choices.insert(*id, bc);
                }
            }
        }
        Self {
            slugir: slugir.clone(),
            backend_choices,
            host_mem: host_mem.to_vec(),
            policy_name: policy_name.to_string(),
        }
    }

    pub fn to_bincode(&self) -> Result<Vec<u8>, FabricError> {
        bincode::serialize(self).map_err(|e| FabricError::Serialize(e.to_string()))
    }

    pub fn from_bincode(bytes: &[u8]) -> Result<Self, FabricError> {
        bincode::deserialize(bytes).map_err(|e| FabricError::Serialize(e.to_string()))
    }

    pub fn write_to(&self, path: &std::path::Path) -> Result<(), FabricError> {
        let bytes = self.to_bincode()?;
        std::fs::write(path, bytes).map_err(|e| FabricError::Io(e.to_string()))
    }

    pub fn read_from(path: &std::path::Path) -> Result<Self, FabricError> {
        let bytes = std::fs::read(path).map_err(|e| FabricError::Io(e.to_string()))?;
        Self::from_bincode(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slugarch_ir::module::{Context, FunctionBuilder};
    use slugarch_ir::op::{ArithKind, Op, OpMeta};
    use slugarch_ir::types::{BackendChoice, Dtype, IpId};

    #[test]
    fn artifact_round_trips_through_bincode() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        let id = b.add_op(Op::Arith {
            kind: ArithKind::Add,
            operands: vec![],
            dtype: Dtype::I32,
        });
        b.finish_meta(
            id,
            OpMeta {
                backend: Some(BackendChoice(IpId::PtxEmulationCore)),
                ..OpMeta::default()
            },
        );
        let mut m = Module::default();
        m.functions.push(b.finish());

        let host_mem = vec![0u8, 1, 2, 3];
        let art = ReplayArtifact::from_module(&m, &host_mem, "default_v1");
        let bytes = art.to_bincode().unwrap();
        let back = ReplayArtifact::from_bincode(&bytes).unwrap();
        assert_eq!(back.host_mem, host_mem);
        assert_eq!(back.policy_name, "default_v1");
        assert_eq!(back.backend_choices.len(), 1);
    }
}
