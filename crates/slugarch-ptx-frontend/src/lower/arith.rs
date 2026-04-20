use super::Lowerer;
use crate::error::FrontendError;
use ptx_parser::Instruction;
use slugarch_ir::module::FunctionBuilder;
use slugarch_ir::op::{ArithKind, Op, OpMeta};
use slugarch_ir::types::Dtype;

pub struct ArithLowerer;

impl Lowerer for ArithLowerer {
    fn try_lower(
        &self,
        inst: &Instruction<ptx_parser::ParsedOperand<&str>>,
        b: &mut FunctionBuilder<'_>,
        hint: &str,
    ) -> Result<bool, FrontendError> {
        let (kind, dtype) = match inst {
            Instruction::Add { data, .. } => (ArithKind::Add, dtype_via_debug(data)),
            Instruction::Sub { data, .. } => (ArithKind::Sub, dtype_via_debug(data)),
            Instruction::Mul { data, .. } => (ArithKind::Mul, dtype_via_debug(data)),
            Instruction::Mad { data, .. } => (ArithKind::Mad, dtype_via_debug(data)),
            Instruction::Div { data, .. } => (ArithKind::Div, dtype_via_debug(data)),
            Instruction::Rem { data, .. } => (ArithKind::Rem, dtype_via_debug(data)),
            Instruction::Fma { data, .. } => (ArithKind::Fma, dtype_via_debug(data)),
            Instruction::Min { data, .. } => (ArithKind::Min, dtype_via_debug(data)),
            Instruction::Max { data, .. } => (ArithKind::Max, dtype_via_debug(data)),
            Instruction::Abs { data, .. } => (ArithKind::Abs, dtype_via_debug(data)),
            Instruction::Cvt { data, .. } => (ArithKind::Cvt, dtype_via_debug(data)),
            Instruction::Mov { data, .. } => (ArithKind::Mov, dtype_via_debug(data)),
            _ => return Ok(false),
        };
        let id = b.add_op(Op::Arith { kind, operands: vec![], dtype });
        b.finish_meta(id, OpMeta { source_hint: Some(hint.to_string()), ..OpMeta::default() });
        Ok(true)
    }
}

/// Best-effort Dtype from an instruction-data Debug string. v1 only; Plan 3
/// replaces this with real typed-field reads once captured PTX is in hand
/// and we know exactly which types and field names appear. Crude substring
/// matches avoid coupling to the exact ptx_parser AST enum shape.
fn dtype_via_debug<T: core::fmt::Debug>(data: &T) -> Dtype {
    let s = format!("{:?}", data);
    if s.contains("F16") || s.contains("f16") { Dtype::F16 }
    else if s.contains("BF16") || s.contains("bf16") { Dtype::BF16 }
    else if s.contains("F32") || s.contains("f32") { Dtype::F32 }
    else if s.contains("F64") || s.contains("f64") { Dtype::F64 }
    else if s.contains("U8") || s.contains("u8") { Dtype::U8 }
    else if s.contains("I8") || s.contains("i8") { Dtype::I8 }
    else if s.contains("U16") || s.contains("u16") { Dtype::U16 }
    else if s.contains("I16") || s.contains("i16") { Dtype::I16 }
    else if s.contains("U32") || s.contains("u32") { Dtype::U32 }
    else if s.contains("I32") || s.contains("i32") { Dtype::I32 }
    else if s.contains("U64") || s.contains("u64") { Dtype::U64 }
    else if s.contains("I64") || s.contains("i64") { Dtype::I64 }
    else { Dtype::I32 }
}
