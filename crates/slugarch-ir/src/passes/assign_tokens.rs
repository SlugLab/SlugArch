use crate::pass::Pass;
use crate::Module;
use crate::IrError;

pub struct AssignTokens;
impl Pass for AssignTokens {
    fn name(&self) -> &'static str { "assign_tokens" }
    fn run(&mut self, _m: &mut Module) -> Result<(), IrError> { Ok(()) }
}
