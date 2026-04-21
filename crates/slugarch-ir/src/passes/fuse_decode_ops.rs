use crate::module::{Function, Module};
use crate::op::{Op, StateKind};
use crate::pass::Pass;
use crate::types::OpId;
use crate::IrError;

/// Canonicalization pass. v1 behavior: merge back-to-back MlpStep StateSteps
/// into a single MlpStep whose operand list is the concatenation. This is
/// deliberately narrow — we want real fusion logic driven by captured PTX,
/// which lands in Plan 3. This pass exists so the four-pass pipeline is
/// plumbed end-to-end now.
pub struct FuseDecodeOps;

impl Pass for FuseDecodeOps {
    fn name(&self) -> &'static str {
        "fuse_decode_ops"
    }

    fn run(&mut self, module: &mut Module) -> Result<(), IrError> {
        for f in module.functions.iter_mut() {
            fuse_mlp_chain(f)?;
        }
        Ok(())
    }
}

fn fuse_mlp_chain(f: &mut Function) -> Result<(), IrError> {
    let mut to_remove: Vec<OpId> = Vec::new();
    let mut i = 0;
    while i + 1 < f.order.len() {
        let a = f.order[i];
        let b = f.order[i + 1];
        let a_is_mlp = matches!(
            f.ops.get(&a),
            Some(Op::StateStep {
                kind: StateKind::MlpStep,
                ..
            })
        );
        let b_is_mlp = matches!(
            f.ops.get(&b),
            Some(Op::StateStep {
                kind: StateKind::MlpStep,
                ..
            })
        );
        let b_depends_on_a = f.edges.iter().any(|e| e.src() == a && e.dst() == b);
        if a_is_mlp && b_is_mlp && b_depends_on_a {
            // Merge b's operands into a; mark b for removal.
            let b_ops = match f.ops.remove(&b) {
                Some(Op::StateStep { operands, .. }) => operands,
                _ => unreachable!(),
            };
            if let Some(Op::StateStep { operands, .. }) = f.ops.get_mut(&a) {
                operands.extend(b_ops);
            }
            f.meta.remove(&b);
            to_remove.push(b);
            // Don't advance i — next op is now at i+1.
        } else {
            i += 1;
        }
    }
    if !to_remove.is_empty() {
        f.order.retain(|id| !to_remove.contains(id));
        f.edges
            .retain(|e| !to_remove.contains(&e.src()) && !to_remove.contains(&e.dst()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Edge;
    use crate::module::{Context, FunctionBuilder};
    use crate::op::StateKind;

    fn mlp(operands: Vec<crate::op::OperandRef>) -> Op {
        Op::StateStep {
            kind: StateKind::MlpStep,
            operands,
        }
    }

    #[test]
    fn chained_mlp_steps_fuse_into_one() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        let a = b.add_op(mlp(vec![crate::op::OperandRef::ImmU64(1)]));
        let c = b.add_op(mlp(vec![crate::op::OperandRef::ImmU64(2)]));
        b.add_edge(Edge::Data(a, c));
        let mut m = Module::default();
        m.functions.push(b.finish());

        FuseDecodeOps.run(&mut m).unwrap();

        let f = &m.functions[0];
        assert_eq!(f.order.len(), 1);
        let fused = f.ops.get(&f.order[0]).unwrap();
        match fused {
            Op::StateStep { operands, .. } => assert_eq!(operands.len(), 2),
            _ => panic!("expected StateStep"),
        }
    }

    #[test]
    fn non_mlp_steps_are_not_fused() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        let a = b.add_op(Op::StateStep {
            kind: StateKind::RmsNorm,
            operands: vec![],
        });
        let c = b.add_op(Op::StateStep {
            kind: StateKind::AttnDecode,
            operands: vec![],
        });
        b.add_edge(Edge::Data(a, c));
        let mut m = Module::default();
        m.functions.push(b.finish());

        FuseDecodeOps.run(&mut m).unwrap();
        assert_eq!(m.functions[0].order.len(), 2);
    }

    #[test]
    fn independent_mlp_steps_are_not_fused() {
        // Two MlpSteps but NO edge between them -> not chained.
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        b.add_op(mlp(vec![]));
        b.add_op(mlp(vec![]));
        let mut m = Module::default();
        m.functions.push(b.finish());

        FuseDecodeOps.run(&mut m).unwrap();
        assert_eq!(m.functions[0].order.len(), 2);
    }
}
