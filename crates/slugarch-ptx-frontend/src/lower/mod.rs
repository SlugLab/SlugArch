pub mod arith;
pub mod bit_ops;
pub mod control;
pub mod ld_st;
pub mod mma;
pub mod transcendental;

use crate::error::FrontendError;
use slugarch_ir::module::FunctionBuilder;

/// Trait implemented by each per-op-class lowering module. Returns `true` if
/// the lowering handled the instruction, `false` if it didn't recognize it
/// and the dispatcher should try the next module.
pub trait Lowerer {
    fn try_lower(
        &self,
        inst: &ptx_parser::Instruction<ptx_parser::ParsedOperand<&str>>,
        b: &mut FunctionBuilder<'_>,
        hint: &str,
    ) -> Result<bool, FrontendError>;
}
