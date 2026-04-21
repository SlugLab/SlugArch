use super::Lowerer;
use crate::error::FrontendError;
use ptx_parser::Instruction;
use slugarch_ir::module::FunctionBuilder;
use slugarch_ir::op::{Op, OpMeta, TileKind};
use slugarch_ir::types::{Dtype, Shape};

pub struct MmaLowerer;

impl Lowerer for MmaLowerer {
    fn try_lower(
        &self,
        inst: &Instruction<ptx_parser::ParsedOperand<&str>>,
        b: &mut FunctionBuilder<'_>,
        hint: &str,
    ) -> Result<bool, FrontendError> {
        match inst {
            Instruction::Mma { data, .. } => {
                let shape = parse_mma_shape_via_debug(data);
                let dtype = parse_mma_dtype_via_debug(data);
                let id = b.add_op(Op::TensorTile {
                    kind: TileKind::Gemm,
                    shape,
                    dtype,
                    operands: vec![],
                });
                b.finish_meta(
                    id,
                    OpMeta {
                        source_hint: Some(hint.to_string()),
                        ..OpMeta::default()
                    },
                );
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

/// Extract up to 3 positive integers from the Debug rendering of the mma data
/// field. The typical render includes "Shape16x8x16" or "m16n8k16" — either
/// way, consecutive decimal runs give the three dims.
fn parse_mma_shape_via_debug<T: core::fmt::Debug>(data: &T) -> Shape {
    let s = format!("{:?}", data);
    let mut dims = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            if let Ok(n) = cur.parse::<u32>() {
                dims.push(n);
            }
            cur.clear();
            if dims.len() >= 3 {
                break;
            }
        }
    }
    if !cur.is_empty() && dims.len() < 3 {
        if let Ok(n) = cur.parse::<u32>() {
            dims.push(n);
        }
    }
    if dims.is_empty() {
        dims = vec![16, 16, 16];
    } // fallback
    Shape(dims)
}

fn parse_mma_dtype_via_debug<T: core::fmt::Debug>(data: &T) -> Dtype {
    let s = format!("{:?}", data);
    if s.contains("F16") {
        Dtype::F16
    } else if s.contains("BF16") {
        Dtype::BF16
    } else if s.contains("F32") {
        Dtype::F32
    } else {
        Dtype::F16
    }
}
