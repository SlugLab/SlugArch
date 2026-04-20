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
        arith::ArithLowerer, bit_ops::BitOpsLowerer, control::ControlLowerer,
        ld_st::LdStLowerer, mma::MmaLowerer, transcendental::TranscendentalLowerer, Lowerer,
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

struct EntryView<'a> {
    name: &'a str,
    instructions: &'a [ptx_parser::Instruction<ptx_parser::ParsedOperand<&'a str>>],
}

fn extract_entry<'a>(
    _dir: &'a ptx_parser::Directive<'a, ptx_parser::ParsedOperand<&'a str>>,
) -> Option<EntryView<'a>> {
    // Actual pattern-matching on Directive enum variants depends on the
    // exact ptx_parser ast shape; this helper is the single place to
    // adapt to its structure. Task 18 fills this in against the real AST.
    None
}
