//! slugarch CLI: `run | replay | validate`.

use anyhow::{anyhow, Context as _, Result};
use clap::{Parser, Subcommand};
use slugarch_backend::bindings::PtxEmulationBinding;
use slugarch_backend::{BackendBinding, BindCtx, DispatchCmd};
use slugarch_fabric::{Fabric, ReplayArtifact};
use slugarch_ir::module::{Context, Module};
use slugarch_ir::op::Op;
use slugarch_ir::pass::Pass;
use slugarch_ir::passes::select_backend::BackendPolicy;
use slugarch_ir::passes::validate_against_rtlmap::{PipelineRtlmap, ValidateAgainstRtlmap};
use slugarch_ir::passes::{AssignTokens, FuseDecodeOps, SelectBackend};
use slugarch_ir::types::{BackendChoice, IpId, TokenId};
use std::path::PathBuf;

/// v1 policy: route everything to PtxEmulationCore. Real per-IP routing
/// requires token encodings derived from each wrapper's port_bindings
/// (post-v1).
struct AllEmuPolicy;
impl BackendPolicy for AllEmuPolicy {
    fn name(&self) -> &'static str {
        "all_emu_v1"
    }
    fn pick(&self, _op: &Op) -> BackendChoice {
        BackendChoice(IpId::PtxEmulationCore)
    }
}

#[derive(Parser)]
#[command(name = "slugarch", about = "PTX-in / cycles-out simulator")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Lower a PTX kernel, drive the fabric, report cycles.
    Run {
        /// Path to the .ptx file.
        kernel: PathBuf,
        /// Write a ReplayArtifact (.slug) to this path.
        #[arg(long)]
        record: Option<PathBuf>,
        /// Host-memory buffer size (bytes).
        #[arg(long, default_value_t = 4096)]
        mem: usize,
    },
    /// Replay a previously recorded run.
    Replay { artifact: PathBuf },
    /// Structurally validate a PTX kernel's SlugIR against a pipeline
    /// rtlmap.json (e.g., the qwen_decode_token oracle from the vendored
    /// Gemma mappings).
    Validate {
        kernel: PathBuf,
        #[arg(long)]
        oracle: PathBuf,
        /// Optional JSON object mapping source_hint -> node_id.
        #[arg(long)]
        hints: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run {
            kernel,
            record,
            mem,
        } => run(&kernel, record.as_deref(), mem),
        Cmd::Replay { artifact } => replay(&artifact),
        Cmd::Validate {
            kernel,
            oracle,
            hints,
        } => validate(&kernel, &oracle, hints.as_deref()),
    }
}

fn lower(path: &std::path::Path) -> Result<Module> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let parsed = slugarch_ptx_frontend::parse_ptx(&text)
        .map_err(|e| anyhow!("parse failed: {:?}", e))?;
    let mut ctx = Context::new();
    let mut m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx)
        .map_err(|e| anyhow!("lower failed: {:?}", e))?;
    FuseDecodeOps.run(&mut m).map_err(|e| anyhow!("fuse: {}", e))?;
    SelectBackend::new(AllEmuPolicy)
        .run(&mut m)
        .map_err(|e| anyhow!("select: {}", e))?;
    AssignTokens
        .run(&mut m)
        .map_err(|e| anyhow!("tokens: {}", e))?;
    Ok(m)
}

fn emit_dispatches(m: &Module) -> Vec<DispatchCmd> {
    let mut out: Vec<DispatchCmd> = Vec::new();
    for f in &m.functions {
        for id in &f.order {
            let op = f.ops.get(id).unwrap();
            let meta = f.meta.get(id).unwrap();
            let ctx = BindCtx {
                token_in: meta.token_in.unwrap_or(TokenId(0)),
                token_out: meta.token_out.unwrap_or(TokenId(0)),
                source_hint: meta.source_hint.as_deref(),
                policy: Some("all_emu_v1"),
            };
            let opcode = match op {
                Op::Emu { opcode, .. } => *opcode,
                _ => 253,
            };
            let cmds = PtxEmulationBinding
                .bind(
                    &Op::Emu {
                        opcode,
                        operands: vec![],
                    },
                    &ctx,
                )
                .unwrap();
            out.extend(cmds);
        }
    }
    out
}

fn run(kernel: &std::path::Path, record: Option<&std::path::Path>, mem_size: usize) -> Result<()> {
    let m = lower(kernel)?;
    let stream = emit_dispatches(&m);
    let initial_mem = vec![0u8; mem_size];
    let mut fabric = Fabric::new(mem_size);
    fabric.set_host_mem(&initial_mem);
    let report = fabric.run(stream).map_err(|e| anyhow!("fabric: {}", e))?;
    println!("total_cycles: {}", report.total_cycles);
    println!("completions:  {}", report.completions);
    for (ip, cycles) in &report.per_ip_cycles {
        println!("  {:?}: {} cycles", ip, cycles);
    }
    if let Some(path) = record {
        let art = ReplayArtifact::from_module(&m, &initial_mem, "all_emu_v1");
        art.write_to(path)
            .map_err(|e| anyhow!("write artifact: {}", e))?;
        println!("recorded: {}", path.display());
    }
    Ok(())
}

fn replay(artifact_path: &std::path::Path) -> Result<()> {
    let art = ReplayArtifact::read_from(artifact_path)
        .map_err(|e| anyhow!("read artifact: {}", e))?;
    let stream = emit_dispatches(&art.slugir);
    let mut fabric = Fabric::new(art.host_mem.len());
    fabric.set_host_mem(&art.host_mem);
    let report = fabric.run(stream).map_err(|e| anyhow!("fabric: {}", e))?;
    println!("replay_total_cycles: {}", report.total_cycles);
    println!("replay_completions:  {}", report.completions);
    println!("policy: {}", art.policy_name);
    Ok(())
}

fn validate(
    kernel: &std::path::Path,
    oracle: &std::path::Path,
    hints: Option<&std::path::Path>,
) -> Result<()> {
    let mut m = lower(kernel)?;
    let oracle_rtlmap = PipelineRtlmap::from_json_file(oracle)
        .map_err(|e| anyhow!("load oracle: {}", e))?;

    let hint_map: std::collections::HashMap<String, String> = if let Some(h) = hints {
        let text = std::fs::read_to_string(h)?;
        serde_json::from_str(&text)?
    } else {
        std::collections::HashMap::new()
    };

    let function_name = m
        .functions
        .first()
        .map(|f| f.name.clone())
        .ok_or_else(|| anyhow!("no functions in module"))?;

    let mut pass = ValidateAgainstRtlmap {
        oracle: oracle_rtlmap,
        function_name,
        node_id_of_hint: hint_map,
    };
    pass.run(&mut m).map_err(|e| anyhow!("validate: {}", e))?;
    println!("oracle match: OK");
    Ok(())
}
