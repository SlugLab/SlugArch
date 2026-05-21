//! slugcxl-gen: emit SystemVerilog + runtime JSON for a CxlEndpointConfig.

mod config;
mod emit_endpoint;
mod emit_fit_top;
mod emit_hj_pipeline;
mod emit_hj_top;
mod emit_quartus;
mod emit_runtime;
mod emit_top;
mod hj_overhead;

use anyhow::{Context, Result};
use clap::Parser;
use config::CxlEndpointConfig;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "slugcxl-gen", about = "Emit CXL endpoint SystemVerilog")]
struct Cli {
    #[arg(long)]
    out: PathBuf,
    /// Also emit hardware-JIT pipeline, HJ top, and overhead report.
    #[arg(long)]
    hj: bool,
    /// Emit Quartus project scaffolding rooted at this target directory.
    #[arg(long)]
    quartus_project: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    std::fs::create_dir_all(&cli.out).with_context(|| format!("creating {}", cli.out.display()))?;

    let cfg = CxlEndpointConfig::slugcxl_4x4();
    cfg.validate()?;

    write(
        &cli.out.join("slugcxl_endpoint.sv"),
        emit_endpoint::emit(&cfg),
    )?;
    write(&cli.out.join("slugcxl_4x4_top.sv"), emit_top::emit(&cfg))?;
    write(
        &cli.out.join("slugcxl_endpoint_runtime.json"),
        emit_runtime::emit(&cfg),
    )?;
    let mut emitted = 3usize;

    if cli.hj {
        write(
            &cli.out.join("slugcxl_hj_pipeline.sv"),
            emit_hj_pipeline::emit(&cfg, &cfg.hardware_jit),
        )?;
        write(
            &cli.out.join("slugcxl_4x4_hj_top.sv"),
            emit_hj_top::emit(&cfg),
        )?;
        write(
            &cli.out.join("slugcxl_hj_fit_top.sv"),
            emit_fit_top::emit(&cfg),
        )?;
        write(
            &cli.out.join("slugcxl_hj_overhead.json"),
            hj_overhead::emit_report_json(&cfg.hardware_jit),
        )?;
        emitted += 4;
    }

    if let Some(project_root) = &cli.quartus_project {
        emit_quartus_project(project_root, &cfg)?;
        emitted += 5;
    }

    println!("emitted {emitted} files to {}", cli.out.display());
    Ok(())
}

fn write(path: &std::path::Path, content: String) -> Result<()> {
    std::fs::write(path, content).with_context(|| format!("writing {}", path.display()))
}

fn emit_quartus_project(project_root: &std::path::Path, cfg: &CxlEndpointConfig) -> Result<()> {
    let quartus = project_root.join("quartus");
    let scripts = project_root.join("scripts");
    std::fs::create_dir_all(&quartus).with_context(|| format!("creating {}", quartus.display()))?;
    std::fs::create_dir_all(&scripts).with_context(|| format!("creating {}", scripts.display()))?;

    write(
        &project_root.join("slugcxl_hj_agilex.qpf"),
        emit_quartus::emit_qpf(),
    )?;
    write(
        &project_root.join("slugcxl_hj_agilex.qsf"),
        emit_quartus::emit_qsf(cfg),
    )?;
    write(
        &quartus.join("slugcxl_hj_agilex.sdc"),
        emit_quartus::emit_sdc(),
    )?;
    write(
        &quartus.join("build_slugcxl_hj_sof.tcl"),
        emit_quartus::emit_build_tcl(),
    )?;
    let script = scripts.join("build_slugcxl_hj_sof.sh");
    write(&script, emit_quartus::emit_build_script())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script)
            .with_context(|| format!("stat {}", script.display()))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms)
            .with_context(|| format!("chmod {}", script.display()))?;
    }

    Ok(())
}
