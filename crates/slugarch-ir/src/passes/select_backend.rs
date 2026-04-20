use crate::pass::Pass;
use crate::Module;
use crate::IrError;

pub struct SelectBackend;
pub struct BackendPolicy;
impl Pass for SelectBackend {
    fn name(&self) -> &'static str { "select_backend" }
    fn run(&mut self, _m: &mut Module) -> Result<(), IrError> { Ok(()) }
}
