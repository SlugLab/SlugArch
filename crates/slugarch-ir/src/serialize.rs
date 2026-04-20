//! Serialization helpers for SlugIR. JSON is canonical (human-readable, diffable);
//! bincode is a compact form for replay artifacts.

use crate::module::Module;
use crate::IrError;

pub fn to_json(m: &Module) -> Result<String, IrError> {
    serde_json::to_string_pretty(m).map_err(|e| IrError::Serialize(e.to_string()))
}

pub fn from_json(text: &str) -> Result<Module, IrError> {
    serde_json::from_str(text).map_err(|e| IrError::Deserialize(e.to_string()))
}

pub fn to_bincode(m: &Module) -> Result<Vec<u8>, IrError> {
    bincode::serialize(m).map_err(|e| IrError::Serialize(e.to_string()))
}

pub fn from_bincode(bytes: &[u8]) -> Result<Module, IrError> {
    bincode::deserialize(bytes).map_err(|e| IrError::Deserialize(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Context, FunctionBuilder};
    use crate::op::{ArithKind, Op, OperandRef};
    use crate::types::{Dtype, OpId};

    fn sample_module() -> Module {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "sample");
        b.add_op(Op::Arith {
            kind: ArithKind::Add,
            operands: vec![OperandRef::Op(OpId(0)), OperandRef::ImmU64(7)],
            dtype: Dtype::I32,
        });
        let mut m = Module::default();
        m.functions.push(b.finish());
        m
    }

    #[test]
    fn json_round_trip() {
        let m = sample_module();
        let s = to_json(&m).unwrap();
        let back = from_json(&s).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn bincode_round_trip() {
        let m = sample_module();
        let bytes = to_bincode(&m).unwrap();
        let back = from_bincode(&bytes).unwrap();
        assert_eq!(back, m);
    }
}
