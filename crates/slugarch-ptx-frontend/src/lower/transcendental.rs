use super::Lowerer;
use crate::error::FrontendError;
use ptx_parser::Instruction;
use slugarch_ir::module::FunctionBuilder;
use slugarch_ir::op::{Op, OpMeta};

pub struct TranscendentalLowerer;

impl Lowerer for TranscendentalLowerer {
    fn try_lower(
        &self,
        inst: &Instruction<ptx_parser::ParsedOperand<&str>>,
        b: &mut FunctionBuilder<'_>,
        hint: &str,
    ) -> Result<bool, FrontendError> {
        let opcode = match inst {
            Instruction::Sqrt { .. } => 17,
            Instruction::Rsqrt { .. } => 18,
            Instruction::Sin { .. } => 19,
            Instruction::Cos { .. } => 20,
            Instruction::Tanh { .. } => 21,
            Instruction::Lg2 { .. } => 22,
            Instruction::Ex2 { .. } => 23,
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
