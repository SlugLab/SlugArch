use crate::pass::Pass;
use crate::Module;
use crate::IrError;

pub struct ValidateAgainstRtlmap;
impl Pass for ValidateAgainstRtlmap {
    fn name(&self) -> &'static str { "validate_against_rtlmap" }
    fn run(&mut self, _m: &mut Module) -> Result<(), IrError> { Ok(()) }
}
