//! slugarch CLI: `run | replay | validate`.

use anyhow::{anyhow, Context as _, Result};
use clap::{Parser, Subcommand};
use slugarch_backend::emit_dispatches;
use slugarch_fabric::{Fabric, ReplayArtifact};
use slugarch_ir::module::{Context, Module};
use slugarch_ir::op::Op;
use slugarch_ir::pass::Pass;
use slugarch_ir::passes::select_backend::BackendPolicy;
use slugarch_ir::passes::validate_against_rtlmap::{PipelineRtlmap, ValidateAgainstRtlmap};
use slugarch_ir::passes::{AssignTokens, FuseDecodeOps, SelectBackend};
use slugarch_ir::types::{BackendChoice, IpId};
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
    /// Run a 4x4 GEMM over real CXL FLITs through the slugcxl_4x4 endpoint.
    RunCxl {
        /// Path to a GemmJob JSON file: { "a": [[..]], "b": [[..]] }
        job: PathBuf,
    },
    /// Run the CXL GEMM path with boundary replay recording and validation.
    EvalCxl {
        /// Path to a GemmJob JSON file: { "a": [[..]], "b": [[..]] }
        job: PathBuf,
        /// Recording mode: validation, delta, or full.
        #[arg(long, default_value = "validation")]
        mode: String,
    },
    /// Export SlugCXL request FLITs for the CXLMemSim QEMU Type-2 BAR target.
    ExportCxlmemsim {
        /// Path to a GemmJob JSON file.
        job: PathBuf,
        /// Output directory for requests.bin and expected.json.
        #[arg(long)]
        out: PathBuf,
    },
    /// Validate CXLMemSim QEMU Type-2 BAR response FLITs.
    ValidateCxlmemsim {
        /// Path to a GemmJob JSON file.
        job: PathBuf,
        /// Path to responses.bin captured from the guest helper.
        #[arg(long)]
        responses: PathBuf,
        /// Output directory for summary.json and summary.csv.
        #[arg(long)]
        out: PathBuf,
    },
    /// Build the simulator-feasible benchmark report and claim ledger.
    MeasureSimFeasible {
        /// Path to a GemmJob JSON file.
        job: PathBuf,
        /// Output directory for sim-feasible-bench-20260702.json and .md.
        #[arg(long)]
        out: PathBuf,
        /// Existing qemu-type2 repeatability artifact directory.
        #[arg(long)]
        qemu_repeatability_dir: Option<PathBuf>,
        /// Device root to probe for dax* devices.
        #[arg(long, default_value = "/dev")]
        dev_root: PathBuf,
        /// Number of replay validation repetitions per mode.
        #[arg(long, default_value_t = 5)]
        replay_repeats: usize,
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
        Cmd::RunCxl { job } => run_cxl(&job),
        Cmd::EvalCxl { job, mode } => eval_cxl(&job, &mode),
        Cmd::ExportCxlmemsim { job, out } => export_cxlmemsim(&job, &out),
        Cmd::ValidateCxlmemsim {
            job,
            responses,
            out,
        } => validate_cxlmemsim(&job, &responses, &out),
        Cmd::MeasureSimFeasible {
            job,
            out,
            qemu_repeatability_dir,
            dev_root,
            replay_repeats,
        } => measure_sim_feasible(
            &job,
            &out,
            qemu_repeatability_dir.as_deref(),
            &dev_root,
            replay_repeats,
        ),
    }
}

fn read_gemm_job(job_path: &std::path::Path) -> Result<slugarch_host::GemmJob> {
    let text = std::fs::read_to_string(job_path)
        .with_context(|| format!("reading {}", job_path.display()))?;
    serde_json::from_str(&text).with_context(|| "parsing GemmJob JSON")
}

fn export_cxlmemsim(job_path: &std::path::Path, out: &std::path::Path) -> Result<()> {
    let job = read_gemm_job(job_path)?;
    let expected = slugarch_host::qemu_type2::export_requests(&job, out)
        .map_err(|e| anyhow!("export cxlmemsim: {}", e))?;
    println!("workload: {}", expected.workload);
    println!("requests: {}", expected.request_count);
    println!("flit_bytes: {}", expected.flit_bytes);
    println!("out: {}", out.display());
    Ok(())
}

fn validate_cxlmemsim(
    job_path: &std::path::Path,
    responses: &std::path::Path,
    out: &std::path::Path,
) -> Result<()> {
    let job = read_gemm_job(job_path)?;
    let summary = slugarch_host::qemu_type2::validate_responses(&job, responses, out)
        .map_err(|e| anyhow!("validate cxlmemsim: {}", e))?;
    println!("status: {}", summary.status);
    println!("requests: {}", summary.request_count);
    println!("responses: {}", summary.response_count);
    println!("tag_mismatches: {}", summary.tag_mismatches);
    println!("dispatch_failures: {}", summary.dispatch_failures);
    if summary.status != "pass" {
        return Err(anyhow!("CXLMemSim Type-2 validation failed"));
    }
    Ok(())
}

fn measure_sim_feasible(
    job_path: &std::path::Path,
    out: &std::path::Path,
    qemu_repeatability_dir: Option<&std::path::Path>,
    dev_root: &std::path::Path,
    replay_repeats: usize,
) -> Result<()> {
    let job = read_gemm_job(job_path)?;
    let report = slugarch_host::sim_feasible::build_sim_feasible_report(
        slugarch_host::sim_feasible::SimFeasibleInput {
            job: &job,
            replay_repeats,
            qemu_repeatability_dir,
            dev_root,
        },
    )
    .map_err(|e| anyhow!("measure sim feasible: {}", e))?;
    slugarch_host::sim_feasible::write_sim_feasible_report(&report, out)
        .map_err(|e| anyhow!("write sim feasible report: {}", e))?;
    println!("workload: {}", report.workload);
    println!("claims: {}", report.claims.len());
    println!("out: {}", out.display());
    Ok(())
}

fn run_cxl(job_path: &std::path::Path) -> Result<()> {
    use slugarch_host::{CxlHost, GemmJob};
    let text = std::fs::read_to_string(job_path)
        .with_context(|| format!("reading {}", job_path.display()))?;
    let job: GemmJob = serde_json::from_str(&text).with_context(|| "parsing GemmJob JSON")?;
    let mut host = CxlHost::new();
    let result = host.run_gemm(&job).map_err(|e| anyhow!("cxl run: {}", e))?;
    println!("cycles: {}", result.cycles);
    println!("flits_sent: {}", result.flits_sent);
    println!("flits_received: {}", result.flits_received);
    println!("result:");
    for row in &result.c {
        println!("  {:?}", row);
    }
    Ok(())
}

fn eval_cxl(job_path: &std::path::Path, mode: &str) -> Result<()> {
    use slugarch_host::{CxlHost, CxlRecordPolicy, GemmJob};
    let text = std::fs::read_to_string(job_path)
        .with_context(|| format!("reading {}", job_path.display()))?;
    let job: GemmJob = serde_json::from_str(&text).with_context(|| "parsing GemmJob JSON")?;
    let mode = parse_cxl_record_mode(mode)?;
    let policy = CxlRecordPolicy::gemm(mode);

    let run_a = CxlHost::new()
        .run_gemm_recorded(&job, policy.clone())
        .map_err(|e| anyhow!("cxl eval run A: {}", e))?;
    let run_b = CxlHost::new()
        .run_gemm_recorded(&job, policy)
        .map_err(|e| anyhow!("cxl eval run B: {}", e))?;
    let validation = run_a.artifact.validate_equivalent(&run_b.artifact);
    let summary = &run_a.artifact.summary;

    println!("mode: {:?}", mode);
    println!("cycles: {}", run_a.result.cycles);
    println!("flits_sent: {}", run_a.result.flits_sent);
    println!("flits_received: {}", run_a.result.flits_received);
    println!("records: {}", summary.record_count);
    println!("epochs: {}", summary.epoch_count);
    println!("application_flit_bytes: {}", summary.application_flit_bytes);
    println!("replay_record_bytes: {}", summary.replay_record_bytes);
    println!("payload_capture_bytes: {}", summary.payload_capture_bytes);
    println!(
        "replay_bytes_per_app_gib: {:.2}",
        summary.replay_bytes_per_app_gib()
    );
    println!("records_compared: {}", validation.records_compared);
    println!("record_mismatches: {}", validation.record_mismatches);
    println!(
        "final_commitment_matches: {}",
        validation.final_commitment_matches
    );
    println!("replay_equivalent: {}", validation.is_equivalent());

    Ok(())
}

fn parse_cxl_record_mode(mode: &str) -> Result<slugarch_host::CxlRecordMode> {
    match mode {
        "validation" => Ok(slugarch_host::CxlRecordMode::Validation),
        "delta" => Ok(slugarch_host::CxlRecordMode::Delta),
        "full" => Ok(slugarch_host::CxlRecordMode::Full),
        other => Err(anyhow!(
            "unknown CXL record mode `{}`; expected validation, delta, or full",
            other
        )),
    }
}

fn lower(path: &std::path::Path) -> Result<Module> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let parsed =
        slugarch_ptx_frontend::parse_ptx(&text).map_err(|e| anyhow!("parse failed: {:?}", e))?;
    let mut ctx = Context::new();
    let mut m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx)
        .map_err(|e| anyhow!("lower failed: {:?}", e))?;
    FuseDecodeOps
        .run(&mut m)
        .map_err(|e| anyhow!("fuse: {}", e))?;
    SelectBackend::new(AllEmuPolicy)
        .run(&mut m)
        .map_err(|e| anyhow!("select: {}", e))?;
    AssignTokens
        .run(&mut m)
        .map_err(|e| anyhow!("tokens: {}", e))?;
    Ok(m)
}

fn run(kernel: &std::path::Path, record: Option<&std::path::Path>, mem_size: usize) -> Result<()> {
    let m = lower(kernel)?;
    let stream = emit_dispatches(&m, "all_emu_v1").map_err(|e| anyhow!("bind: {}", e))?;
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
    let art =
        ReplayArtifact::read_from(artifact_path).map_err(|e| anyhow!("read artifact: {}", e))?;
    let stream =
        emit_dispatches(&art.slugir, &art.policy_name).map_err(|e| anyhow!("bind: {}", e))?;
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
    let oracle_rtlmap =
        PipelineRtlmap::from_json_file(oracle).map_err(|e| anyhow!("load oracle: {}", e))?;

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

#[cfg(test)]
mod tests {
    use super::{export_cxlmemsim, read_gemm_job, Cli, Cmd};
    use clap::Parser;
    use slugarch_host::GemmJob;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("slugarch-cli-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_export_cxlmemsim_subcommand() {
        let cli = Cli::parse_from([
            "slugarch",
            "export-cxlmemsim",
            "job.json",
            "--out",
            "out-dir",
        ]);
        match cli.cmd {
            Cmd::ExportCxlmemsim { job, out } => {
                assert_eq!(job, PathBuf::from("job.json"));
                assert_eq!(out, PathBuf::from("out-dir"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn parses_validate_cxlmemsim_subcommand() {
        let cli = Cli::parse_from([
            "slugarch",
            "validate-cxlmemsim",
            "job.json",
            "--responses",
            "responses.bin",
            "--out",
            "out-dir",
        ]);
        match cli.cmd {
            Cmd::ValidateCxlmemsim {
                job,
                responses,
                out,
            } => {
                assert_eq!(job, PathBuf::from("job.json"));
                assert_eq!(responses, PathBuf::from("responses.bin"));
                assert_eq!(out, PathBuf::from("out-dir"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn export_cxlmemsim_writes_expected_artifacts() {
        let dir = temp_dir("export-cxlmemsim");
        let job_path = dir.join("job.json");
        let out_dir = dir.join("out");
        fs::write(
            &job_path,
            r#"{
  "a": [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
  "b": [[2, 3, 4, 5], [6, 7, 8, 9], [10, 11, 12, 13], [14, 15, 16, 17]]
}"#,
        )
        .unwrap();

        export_cxlmemsim(&job_path, &out_dir).unwrap();

        let job: GemmJob = read_gemm_job(&job_path).unwrap();
        assert_eq!(job.b[3], [14, 15, 16, 17]);
        assert!(out_dir.join("requests.bin").exists());
        assert!(out_dir.join("expected.json").exists());
    }
}
