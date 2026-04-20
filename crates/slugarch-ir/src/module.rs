use crate::graph::Edge;
use crate::op::{Op, OpMeta};
use crate::types::OpId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single SlugIR function (typically one PTX kernel entry).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub ops: HashMap<OpId, Op>,
    pub meta: HashMap<OpId, OpMeta>,
    pub edges: Vec<Edge>,
    /// Topologically ordered op ids; the source of truth for walk order.
    pub order: Vec<OpId>,
}

impl Function {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Default::default() }
    }

    pub fn successors(&self, id: OpId) -> impl Iterator<Item = OpId> + '_ {
        self.edges.iter().filter(move |e| e.src() == id).map(|e| e.dst())
    }

    pub fn predecessors(&self, id: OpId) -> impl Iterator<Item = OpId> + '_ {
        self.edges.iter().filter(move |e| e.dst() == id).map(|e| e.src())
    }
}

/// A SlugIR module: one or more functions + a symbol table.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub functions: Vec<Function>,
    pub symbols: HashMap<String, String>,
}

/// Build-time context for assembling SlugIR. Hands out fresh OpIds.
#[derive(Debug, Default)]
pub struct Context {
    next_op: u32,
}

impl Context {
    pub fn new() -> Self { Self::default() }
    pub fn fresh_op(&mut self) -> OpId {
        let id = OpId(self.next_op);
        self.next_op += 1;
        id
    }
}

/// Builder for a single Function. Wraps a Context to hand out OpIds and
/// records the insertion order in `order`.
pub struct FunctionBuilder<'ctx> {
    ctx: &'ctx mut Context,
    f: Function,
}

impl<'ctx> FunctionBuilder<'ctx> {
    pub fn new(ctx: &'ctx mut Context, name: impl Into<String>) -> Self {
        Self { ctx, f: Function::new(name) }
    }

    pub fn add_op(&mut self, op: Op) -> OpId {
        let id = self.ctx.fresh_op();
        self.f.ops.insert(id, op);
        self.f.meta.insert(id, OpMeta::default());
        self.f.order.push(id);
        id
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.f.edges.push(edge);
    }

    pub fn finish(self) -> Function { self.f }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::{ArithKind, OperandRef};
    use crate::types::Dtype;

    #[test]
    fn builder_assigns_unique_ids_in_order() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        let a = b.add_op(Op::Arith { kind: ArithKind::Add, operands: vec![], dtype: Dtype::I32 });
        let c = b.add_op(Op::Arith { kind: ArithKind::Mul, operands: vec![], dtype: Dtype::I32 });
        let f = b.finish();
        assert_eq!(f.order, vec![a, c]);
        assert!(a < c);
        assert_eq!(f.ops.len(), 2);
        assert_eq!(f.meta.len(), 2);
    }

    #[test]
    fn edges_and_successors() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        let a = b.add_op(Op::Arith { kind: ArithKind::Add, operands: vec![], dtype: Dtype::I32 });
        let c = b.add_op(Op::Arith { kind: ArithKind::Mul, operands: vec![OperandRef::Op(a)], dtype: Dtype::I32 });
        b.add_edge(Edge::Data(a, c));
        let f = b.finish();
        let succs: Vec<_> = f.successors(a).collect();
        assert_eq!(succs, vec![c]);
        let preds: Vec<_> = f.predecessors(c).collect();
        assert_eq!(preds, vec![a]);
    }

    #[test]
    fn function_round_trips_through_json() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        b.add_op(Op::Arith { kind: ArithKind::Add, operands: vec![], dtype: Dtype::I32 });
        let original = b.finish();
        let s = serde_json::to_string(&original).unwrap();
        let back: Function = serde_json::from_str(&s).unwrap();
        assert_eq!(back, original);
    }
}
