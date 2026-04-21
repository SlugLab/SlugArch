//! assign_tokens: derives TokenId values for each op from the edge graph.

use crate::module::Module;
use crate::pass::Pass;
use crate::types::{OpId, TokenId};
use crate::IrError;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct AssignTokens;

impl Pass for AssignTokens {
    fn name(&self) -> &'static str {
        "assign_tokens"
    }

    fn run(&mut self, module: &mut Module) -> Result<(), IrError> {
        for f in module.functions.iter_mut() {
            // Build adjacency + in-degree from edges.
            let mut indeg: HashMap<OpId, usize> = f.ops.keys().map(|&k| (k, 0)).collect();
            let mut adj: HashMap<OpId, Vec<OpId>> = HashMap::new();
            for e in &f.edges {
                adj.entry(e.src()).or_default().push(e.dst());
                *indeg.entry(e.dst()).or_insert(0) += 1;
            }
            // Kahn's topo sort; if the graph has a cycle we will fail to
            // visit all ops.
            let mut queue: VecDeque<OpId> = indeg
                .iter()
                .filter(|(_, d)| **d == 0)
                .map(|(k, _)| *k)
                .collect();
            // Deterministic ordering: sort the initial queue and each expansion by OpId.
            let mut q_vec: Vec<_> = queue.drain(..).collect();
            q_vec.sort();
            queue.extend(q_vec);

            let mut visited: HashSet<OpId> = HashSet::new();
            let mut topo: Vec<OpId> = Vec::new();
            while let Some(id) = queue.pop_front() {
                if !visited.insert(id) {
                    continue;
                }
                topo.push(id);
                let mut succ = adj.remove(&id).unwrap_or_default();
                succ.sort();
                for s in succ {
                    let d = indeg.get_mut(&s).unwrap();
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(s);
                    }
                }
            }
            if topo.len() != f.ops.len() {
                let cycle: Vec<_> = f
                    .ops
                    .keys()
                    .copied()
                    .filter(|id| !visited.contains(id))
                    .collect();
                return Err(IrError::TokenGraphCycle { cycle });
            }
            f.order = topo.clone();

            // Assign token_out per op in topo order; token_in = max predecessor token_out.
            let mut tok_out: HashMap<OpId, u32> = HashMap::new();
            for (i, id) in topo.iter().enumerate() {
                let mut max_pred_tok = 0u32;
                for pred in f.predecessors(*id) {
                    if let Some(t) = tok_out.get(&pred) {
                        if *t > max_pred_tok {
                            max_pred_tok = *t;
                        }
                    }
                }
                let this_tok = (i as u32) + 1;
                tok_out.insert(*id, this_tok);
                let meta = f.meta.entry(*id).or_default();
                meta.token_in = Some(TokenId(max_pred_tok));
                meta.token_out = Some(TokenId(this_tok));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Edge;
    use crate::module::{Context, FunctionBuilder};
    use crate::op::{ArithKind, Op};
    use crate::types::Dtype;

    #[test]
    fn linear_chain_tokens_increase_monotonically() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        let a = b.add_op(Op::Arith {
            kind: ArithKind::Add,
            operands: vec![],
            dtype: Dtype::I32,
        });
        let c = b.add_op(Op::Arith {
            kind: ArithKind::Mul,
            operands: vec![],
            dtype: Dtype::I32,
        });
        let d = b.add_op(Op::Arith {
            kind: ArithKind::Sub,
            operands: vec![],
            dtype: Dtype::I32,
        });
        b.add_edge(Edge::Data(a, c));
        b.add_edge(Edge::Data(c, d));
        let mut m = Module::default();
        m.functions.push(b.finish());
        AssignTokens.run(&mut m).unwrap();
        let f = &m.functions[0];
        let ma = f.meta.get(&a).unwrap();
        let mc = f.meta.get(&c).unwrap();
        let md = f.meta.get(&d).unwrap();
        assert_eq!(ma.token_in, Some(TokenId(0)));
        assert_eq!(ma.token_out, Some(TokenId(1)));
        assert_eq!(mc.token_in, Some(TokenId(1)));
        assert_eq!(mc.token_out, Some(TokenId(2)));
        assert_eq!(md.token_in, Some(TokenId(2)));
        assert_eq!(md.token_out, Some(TokenId(3)));
    }

    #[test]
    fn cycle_is_detected() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        let a = b.add_op(Op::Arith {
            kind: ArithKind::Add,
            operands: vec![],
            dtype: Dtype::I32,
        });
        let c = b.add_op(Op::Arith {
            kind: ArithKind::Mul,
            operands: vec![],
            dtype: Dtype::I32,
        });
        b.add_edge(Edge::Data(a, c));
        b.add_edge(Edge::Data(c, a));
        let mut m = Module::default();
        m.functions.push(b.finish());
        let err = AssignTokens.run(&mut m).unwrap_err();
        assert!(matches!(err, IrError::TokenGraphCycle { .. }));
    }

    #[test]
    fn independent_ops_get_distinct_tokens() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        let a = b.add_op(Op::Arith {
            kind: ArithKind::Add,
            operands: vec![],
            dtype: Dtype::I32,
        });
        let c = b.add_op(Op::Arith {
            kind: ArithKind::Mul,
            operands: vec![],
            dtype: Dtype::I32,
        });
        let mut m = Module::default();
        m.functions.push(b.finish());
        AssignTokens.run(&mut m).unwrap();
        let f = &m.functions[0];
        let ta = f.meta[&a].token_out.unwrap();
        let tc = f.meta[&c].token_out.unwrap();
        assert_ne!(ta, tc);
    }
}
