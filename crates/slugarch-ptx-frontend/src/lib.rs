//! PTX → SlugIR frontend.

pub mod error;
pub mod lower;

pub use error::FrontendError;

use slugarch_ir::module::{Context, Module};

pub fn parse_ptx_raw(text: &str) -> Result<ptx_parser::Module<'_>, Vec<ptx_parser::PtxError<'_>>> {
    ptx_parser::parse_module_checked(text)
}

pub fn parse_ptx(text: &str) -> Result<ptx_parser::Module<'_>, FrontendError> {
    match ptx_parser::parse_module_checked(text) {
        Ok(m) => Ok(m),
        Err(errs) => {
            let first = errs.first().map(|e| format!("{:?}", e)).unwrap_or_default();
            Err(FrontendError::Parse(errs.len(), first))
        }
    }
}

/// Lower a parsed PTX module into SlugIR using the registered per-class lowerers.
///
/// v1 scope: produces one `Function` per `.entry` kernel; for each instruction
/// in the entry body, walks the registered lowerers in order and stops at the
/// first that claims it. Instructions nobody recognizes emit an `Op::Emu`
/// placeholder with opcode=255 ("unknown"), so lowering is never a silent
/// drop but also never a fatal error in Plan 1 — the validate_against_rtlmap
/// pass is the one that fails hard if the resulting IR doesn't match.
pub fn lower_to_slugir<'a>(
    module: &'a ptx_parser::Module<'a>,
    ctx: &mut Context,
) -> Result<Module, FrontendError> {
    use lower::{
        arith::ArithLowerer, bit_ops::BitOpsLowerer, control::ControlLowerer, ld_st::LdStLowerer,
        mma::MmaLowerer, transcendental::TranscendentalLowerer, Lowerer,
    };
    let lowerers: Vec<Box<dyn Lowerer>> = vec![
        Box::new(ArithLowerer),
        Box::new(BitOpsLowerer),
        Box::new(TranscendentalLowerer),
        Box::new(LdStLowerer),
        Box::new(MmaLowerer),
        Box::new(ControlLowerer),
    ];
    let mut out = Module::default();
    for directive in &module.directives {
        if let Some(entry) = extract_entry(directive) {
            let mut b = slugarch_ir::module::FunctionBuilder::new(ctx, entry.name.to_string());
            for (idx, inst) in entry.instructions.iter().enumerate() {
                let hint = format!("{}[{}]", entry.name, idx);
                let mut handled = false;
                // `inst` is `&&Instruction` (Vec of references); auto-deref
                // to the `&Instruction` the Lowerer trait expects.
                for l in &lowerers {
                    if l.try_lower(inst, &mut b, &hint)? {
                        handled = true;
                        break;
                    }
                }
                if !handled {
                    // Unknown op: emit Emu 255.
                    b.add_op(slugarch_ir::op::Op::Emu {
                        opcode: 255,
                        operands: vec![],
                    });
                }
            }
            out.functions.push(b.finish());
        }
    }
    Ok(out)
}

/// A pared-down view of a `.entry` kernel extracted from the ptx_parser AST.
///
/// AST shape notes (ptx_parser 0.x, vendored at
/// `vendor/concordia-ptx/ptx_parser/src/ast.rs`):
/// - `Directive` has two variants: `Variable(LinkingDirective, ...)` for
///   module-level globals and `Method(LinkingDirective, Function<...>)` for
///   functions/kernels.
/// - `Function { func_directive: MethodDeclaration, tuning, body: Option<Vec<S>> }`
///   where for the parsed module `S = Statement<ParsedOperand<&str>>`.
/// - `MethodDeclaration.name: MethodName<'input, ID>` discriminates kernel vs
///   function: `MethodName::Kernel(&str)` is a `.entry`, `MethodName::Func(ID)`
///   is a `.func` (internal, non-kernel).
/// - `Statement` is an enum — only `Statement::Instruction(_, Instruction)`
///   carries an actual instruction; other variants (`Label`, `Variable`,
///   `Block`) are skipped here.
///
/// Because the body is a `Vec<Statement>` (not a `Vec<Instruction>`), we can't
/// return a `&[Instruction]` slice without materializing a new collection —
/// so `EntryView::instructions` owns a `Vec<&'a Instruction<...>>` (option (a)
/// from the Task 18 plan). The dispatcher in `lower_to_slugir` iterates this
/// vec of references.
struct EntryView<'a> {
    name: &'a str,
    instructions: Vec<&'a ptx_parser::Instruction<ptx_parser::ParsedOperand<&'a str>>>,
}

fn extract_entry<'a>(
    dir: &'a ptx_parser::Directive<'a, ptx_parser::ParsedOperand<&'a str>>,
) -> Option<EntryView<'a>> {
    // Only `.entry` kernels produce a Function in v1; Variables and `.func`s
    // are skipped. `Statement::Block` is flattened recursively so nested
    // blocks still contribute instructions.
    let ptx_parser::Directive::Method(_linkage, func) = dir else {
        return None;
    };
    let name = match func.func_directive.name {
        ptx_parser::MethodName::Kernel(n) => n,
        ptx_parser::MethodName::Func(_) => return None,
    };
    let body = func.body.as_ref()?;
    let mut instructions = Vec::new();
    collect_instructions(body, &mut instructions);
    Some(EntryView { name, instructions })
}

fn collect_instructions<'a>(
    stmts: &'a [ptx_parser::Statement<ptx_parser::ParsedOperand<&'a str>>],
    out: &mut Vec<&'a ptx_parser::Instruction<ptx_parser::ParsedOperand<&'a str>>>,
) {
    for stmt in stmts {
        match stmt {
            ptx_parser::Statement::Instruction(_pred, inst) => out.push(inst),
            ptx_parser::Statement::Block(inner) => collect_instructions(inner, out),
            ptx_parser::Statement::Label(_) | ptx_parser::Statement::Variable(_) => {}
        }
    }
}
