use super::Lowerer;
use crate::error::FrontendError;
use ptx_parser::Instruction;
use slugarch_ir::module::FunctionBuilder;
use slugarch_ir::op::{Op, OpMeta};

pub struct ControlLowerer;

impl Lowerer for ControlLowerer {
    fn try_lower(
        &self,
        inst: &Instruction<ptx_parser::ParsedOperand<&str>>,
        b: &mut FunctionBuilder<'_>,
        hint: &str,
    ) -> Result<bool, FrontendError> {
        let matched = matches!(inst,
            Instruction::Bra { .. }
            | Instruction::Call { .. }
            | Instruction::Ret { .. }
            | Instruction::Bar { .. }
            | Instruction::BarWarp { .. }
            | Instruction::BarRed { .. }
        );
        if !matched { return Ok(false); }
        let id = b.add_op(Op::Emu { opcode: 254, operands: vec![] });
        b.finish_meta(id, OpMeta { source_hint: Some(hint.to_string()), ..OpMeta::default() });
        Ok(true)
    }
}
