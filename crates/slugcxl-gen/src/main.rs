//! slugcxl-gen: emit SystemVerilog + runtime JSON for a CxlEndpointConfig.
//!
//! Tasks 7-10 populate the emission. For now this is a compile-only stub.

mod config;
mod emit_endpoint;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "slugcxl-gen", about = "Emit CXL endpoint SystemVerilog")]
struct Cli {
    #[arg(long)]
    out: PathBuf,
}

fn main() -> Result<()> {
    let _cli = Cli::parse();
    anyhow::bail!("slugcxl-gen: emission not yet implemented (Tasks 7-10)");
}
