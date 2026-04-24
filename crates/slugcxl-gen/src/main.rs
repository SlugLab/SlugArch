//! slugcxl-gen: emit SystemVerilog + runtime JSON for a CxlEndpointConfig.

mod config;
mod emit_endpoint;
mod emit_runtime;
mod emit_top;

use anyhow::{Context, Result};
use clap::Parser;
use config::CxlEndpointConfig;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "slugcxl-gen", about = "Emit CXL endpoint SystemVerilog")]
struct Cli {
    #[arg(long)]
    out: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    std::fs::create_dir_all(&cli.out)
        .with_context(|| format!("creating {}", cli.out.display()))?;

    let cfg = CxlEndpointConfig::slugcxl_4x4();
    cfg.validate()?;

    write(&cli.out.join("slugcxl_endpoint.sv"), emit_endpoint::emit(&cfg))?;
    write(&cli.out.join("slugcxl_4x4_top.sv"), emit_top::emit(&cfg))?;
    write(&cli.out.join("slugcxl_endpoint_runtime.json"), emit_runtime::emit(&cfg))?;
    println!("emitted 3 files to {}", cli.out.display());
    Ok(())
}

fn write(path: &std::path::Path, content: String) -> Result<()> {
    std::fs::write(path, content).with_context(|| format!("writing {}", path.display()))
}
