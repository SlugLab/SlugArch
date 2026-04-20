use crate::types::{Addr, BackendChoice, Dtype, OpId, Shape, TokenId};
use serde::{Deserialize, Serialize};

/// Reference to an operand value inside a Function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OperandRef {
    /// SSA-style reference to another op's output.
    Op(OpId),
    /// Host-memory address (input/output buffer).
    Addr(Addr),
    /// Immediate scalar (serialized as u64 for simplicity; interpret per Dtype).
    ImmU64(u64),
    ImmF32(f32),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArithKind {
    Add, Sub, Mul, Mad, Div, Rem, Fma,
    Min, Max, Abs,
    Cvt, Mov,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TileKind {
    /// Dense matmul tile.
    Gemm,
    /// Elementwise over a tile.
    Elementwise,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StateKind {
    /// RMS normalization step.
    RmsNorm,
    /// Attention decode (per-token state advance).
    AttnDecode,
    /// MLP gate/up/down fused step.
    MlpStep,
    /// Generic decode-step fallback.
    DecodeGeneric,
}

/// SlugIR operation. One node in a Function's graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    /// Scalar/vector arithmetic. No backend selection affinity on its own.
    Arith {
        kind: ArithKind,
        operands: Vec<OperandRef>,
        dtype: Dtype,
    },
    /// Tensor-tile op (lowers to a GEMM-class IP).
    TensorTile {
        kind: TileKind,
        shape: Shape,
        dtype: Dtype,
        operands: Vec<OperandRef>,
    },
    /// Stateful/decode-step op (lowers to NPU array).
    StateStep {
        kind: StateKind,
        operands: Vec<OperandRef>,
    },
    /// Memory movement (lowers to NoC / DMA path).
    Dma { src: Addr, dst: Addr, bytes: u64 },
    /// Fallback emulation op (lowers to ptx_emulation_core).
    /// Opcode values match those listed in
    /// `vendor/gemma-generated/generated/ptx_emulation_core/runtime/ptx_emulation_core.json`.
    Emu { opcode: u32, operands: Vec<OperandRef> },
}

/// Metadata attached to an Op once passes have annotated it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpMeta {
    /// Assigned by `select_backend`.
    pub backend: Option<BackendChoice>,
    /// Assigned by `assign_tokens`.
    pub token_in: Option<TokenId>,
    pub token_out: Option<TokenId>,
    /// Free-form source hint for diagnostics (PTX line, span, etc.).
    pub source_hint: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Dtype, IpId, Shape};

    #[test]
    fn arith_serializes_as_tagged_variant() {
        let op = Op::Arith {
            kind: ArithKind::Add,
            operands: vec![OperandRef::Op(crate::types::OpId(0)), OperandRef::ImmU64(3)],
            dtype: Dtype::I32,
        };
        let v: serde_json::Value = serde_json::to_value(&op).unwrap();
        assert_eq!(v["Arith"]["kind"], "Add");
        assert_eq!(v["Arith"]["dtype"], "I32");
    }

    #[test]
    fn tensor_tile_carries_shape_and_dtype() {
        let op = Op::TensorTile {
            kind: TileKind::Gemm,
            shape: Shape(vec![16, 16]),
            dtype: Dtype::F16,
            operands: vec![],
        };
        if let Op::TensorTile { shape, dtype, .. } = &op {
            assert_eq!(shape.0, vec![16, 16]);
            assert_eq!(*dtype, Dtype::F16);
        } else {
            panic!("unexpected Op variant");
        }
    }

    #[test]
    fn op_meta_backend_round_trips() {
        let meta = OpMeta {
            backend: Some(BackendChoice(IpId::SystolicArray16x16)),
            token_in: Some(crate::types::TokenId(0)),
            token_out: Some(crate::types::TokenId(1)),
            source_hint: Some("ptx:42".into()),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: OpMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(back, meta);
    }
}
