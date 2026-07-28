//! Command-line wrapper around the reusable SlugCXL generator.

use anyhow::Result;
use clap::Parser;
use slugcxl_gen::{generate, GenerateOptions};
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
    /// Load a strict SlugArch policy JSON instead of the default.
    #[arg(long)]
    policy: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let outputs = generate(&GenerateOptions {
        out: cli.out.clone(),
        hardware_jit: cli.hj,
        quartus_project: cli.quartus_project,
        policy_path: cli.policy,
    })?;
    println!("emitted {} files to {}", outputs.len(), cli.out.display());
    Ok(())
}
