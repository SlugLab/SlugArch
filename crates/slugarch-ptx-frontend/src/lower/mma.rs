use super::Lowerer;
use crate::error::FrontendError;
use ptx_parser::{Instruction, MmaDetails, ScalarType};
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
                let id = b.add_op(Op::TensorTile {
                    kind: TileKind::Gemm,
                    shape: mma_shape(),
                    dtype: mma_dtype(data),
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

// ptx_parser's grammar (vendor/concordia-ptx/ptx_parser/src/lib.rs ~3979) only
// accepts `mma.sync.aligned.m16n8k16` today, and MmaDetails discards the
// shape dims. The canonical SM 80 MMA tile is M=16, N=8, K=16.
fn mma_shape() -> Shape {
    Shape(vec![16, 8, 16])
}

fn mma_dtype(data: &MmaDetails) -> Dtype {
    match data.dtype_scalar {
        ScalarType::F16 => Dtype::F16,
        ScalarType::BF16 => Dtype::BF16,
        ScalarType::F32 => Dtype::F32,
        _ => Dtype::F16,
    }
}
