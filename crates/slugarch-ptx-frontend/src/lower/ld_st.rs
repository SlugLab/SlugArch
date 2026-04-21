use super::Lowerer;
use crate::error::FrontendError;
use ptx_parser::Instruction;
use slugarch_ir::module::FunctionBuilder;
use slugarch_ir::op::{Op, OpMeta};

pub struct LdStLowerer;

impl Lowerer for LdStLowerer {
    fn try_lower(
        &self,
        inst: &Instruction<ptx_parser::ParsedOperand<&str>>,
        b: &mut FunctionBuilder<'_>,
        hint: &str,
    ) -> Result<bool, FrontendError> {
        let bytes = match inst {
            Instruction::Ld { data, .. } => bytes_via_debug(data),
            Instruction::St { data, .. } => bytes_via_debug(data),
            _ => return Ok(false),
        };
        let id = b.add_op(Op::Dma {
            src: 0,
            dst: 0,
            bytes,
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
}

/// Best-effort byte-width via Debug string. v1 only; Plan 3 replaces this
/// with a structured field read once the captured Qwen PTX is in hand and
/// we know which widths are actually used.
fn bytes_via_debug<T: core::fmt::Debug>(data: &T) -> u64 {
    let s = format!("{:?}", data);
    if s.contains("B64") || s.contains("64") {
        8
    } else if s.contains("B32") || s.contains("32") {
        4
    } else if s.contains("B16") || s.contains("16") {
        2
    } else if s.contains("B8") || s.contains("8") {
        1
    } else {
        4
    }
}
