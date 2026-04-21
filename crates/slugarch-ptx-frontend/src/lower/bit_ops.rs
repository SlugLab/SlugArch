use super::Lowerer;
use crate::error::FrontendError;
use ptx_parser::Instruction;
use slugarch_ir::module::FunctionBuilder;
use slugarch_ir::op::{Op, OpMeta};

pub struct BitOpsLowerer;

impl Lowerer for BitOpsLowerer {
    fn try_lower(
        &self,
        inst: &Instruction<ptx_parser::ParsedOperand<&str>>,
        b: &mut FunctionBuilder<'_>,
        hint: &str,
    ) -> Result<bool, FrontendError> {
        let opcode = match inst {
            Instruction::Not { .. } => 1,
            Instruction::And { .. } => 2,
            Instruction::Or { .. } => 3,
            Instruction::Xor { .. } => 4,
            Instruction::Shl { .. } => 5,
            Instruction::Shr { .. } => 6,
            Instruction::Popc { .. } => 7,
            Instruction::Clz { .. } => 8,
            Instruction::Brev { .. } => 9,
            Instruction::Bfe { .. } => 10,
            Instruction::Bfi { .. } => 11,
            Instruction::Prmt { .. } => 12,
            Instruction::Shf { .. } => 13,
            _ => return Ok(false),
        };
        let id = b.add_op(Op::Emu {
            opcode,
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
}
