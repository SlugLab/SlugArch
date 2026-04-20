//! Pass trait + registry. Passes mutate a Module in place.

use crate::module::Module;
use crate::IrError;

/// A SlugIR pass. Named, deterministic, runs to completion or errors.
pub trait Pass {
    fn name(&self) -> &'static str;
    fn run(&mut self, module: &mut Module) -> Result<(), IrError>;
}

/// Runs a sequence of passes on a module, in order.
pub fn run_passes(module: &mut Module, passes: &mut [&mut dyn Pass]) -> Result<(), IrError> {
    for p in passes.iter_mut() {
        p.run(module)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Context, FunctionBuilder};
    use crate::op::{ArithKind, Op};
    use crate::types::Dtype;

    struct RenamerPass;
    impl Pass for RenamerPass {
        fn name(&self) -> &'static str { "rename" }
        fn run(&mut self, module: &mut Module) -> Result<(), IrError> {
            for f in module.functions.iter_mut() {
                f.name = format!("renamed_{}", f.name);
            }
            Ok(())
        }
    }

    #[test]
    fn run_passes_applies_each_pass_in_order() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "original");
        b.add_op(Op::Arith { kind: ArithKind::Add, operands: vec![], dtype: Dtype::I32 });
        let mut m = Module::default();
        m.functions.push(b.finish());

        let mut p1 = RenamerPass;
        let mut p2 = RenamerPass;
        run_passes(&mut m, &mut [&mut p1, &mut p2]).unwrap();
        assert_eq!(m.functions[0].name, "renamed_renamed_original");
    }
}
