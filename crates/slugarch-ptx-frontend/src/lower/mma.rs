use super::Lowerer;
use crate::error::FrontendError;
use slugarch_ir::module::FunctionBuilder;

pub struct MmaLowerer;
impl Lowerer for MmaLowerer {
    fn try_lower(
        &self,
        _inst: &ptx_parser::Instruction<ptx_parser::ParsedOperand<&str>>,
        _b: &mut FunctionBuilder<'_>,
        _hint: &str,
    ) -> Result<bool, FrontendError> { Ok(false) }
}
