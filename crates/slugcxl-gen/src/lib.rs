//! Reusable SlugCXL SystemVerilog and runtime-artifact generator.

mod config;
mod emit_endpoint;
mod emit_fit_top;
mod emit_hj_pipeline;
mod emit_hj_top;
mod emit_quartus;
mod emit_runtime;
mod emit_top;
mod hj_overhead;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use config::CxlEndpointConfig;

#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub out: PathBuf,
    pub hardware_jit: bool,
    pub quartus_project: Option<PathBuf>,
    pub policy_path: Option<PathBuf>,
}

pub fn generate(options: &GenerateOptions) -> Result<Vec<PathBuf>> {
    if options.policy_path.is_some() {
        bail!("runtime policy images are not implemented yet");
    }

    std::fs::create_dir_all(&options.out)
        .with_context(|| format!("creating {}", options.out.display()))?;
    let cfg = CxlEndpointConfig::slugcxl_4x4();
    cfg.validate()?;

    let mut outputs = Vec::new();
    write(
        &mut outputs,
        options.out.join("slugcxl_endpoint.sv"),
        emit_endpoint::emit(&cfg),
    )?;
    write(
        &mut outputs,
        options.out.join("slugcxl_4x4_top.sv"),
        emit_top::emit(&cfg),
    )?;
    write(
        &mut outputs,
        options.out.join("slugcxl_endpoint_runtime.json"),
        emit_runtime::emit(&cfg),
    )?;

    if options.hardware_jit {
        write(
            &mut outputs,
            options.out.join("slugcxl_hj_pipeline.sv"),
            emit_hj_pipeline::emit(&cfg, &cfg.hardware_jit),
        )?;
        write(
            &mut outputs,
            options.out.join("slugcxl_4x4_hj_top.sv"),
            emit_hj_top::emit(&cfg),
        )?;
        write(
            &mut outputs,
            options.out.join("slugcxl_hj_fit_top.sv"),
            emit_fit_top::emit(&cfg),
        )?;
        write(
            &mut outputs,
            options.out.join("slugcxl_hj_overhead.json"),
            hj_overhead::emit_report_json(&cfg.hardware_jit),
        )?;
    }

    if let Some(project_root) = &options.quartus_project {
        emit_quartus_project(&mut outputs, project_root, &cfg)?;
    }

    Ok(outputs)
}

fn write(outputs: &mut Vec<PathBuf>, path: PathBuf, content: String) -> Result<()> {
    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    outputs.push(path);
    Ok(())
}

fn emit_quartus_project(
    outputs: &mut Vec<PathBuf>,
    project_root: &Path,
    cfg: &CxlEndpointConfig,
) -> Result<()> {
    let quartus = project_root.join("quartus");
    let scripts = project_root.join("scripts");
    std::fs::create_dir_all(&quartus).with_context(|| format!("creating {}", quartus.display()))?;
    std::fs::create_dir_all(&scripts).with_context(|| format!("creating {}", scripts.display()))?;

    write(
        outputs,
        project_root.join("slugcxl_hj_agilex.qpf"),
        emit_quartus::emit_qpf(),
    )?;
    write(
        outputs,
        project_root.join("slugcxl_hj_agilex.qsf"),
        emit_quartus::emit_qsf(cfg),
    )?;
    write(
        outputs,
        quartus.join("slugcxl_hj_agilex.sdc"),
        emit_quartus::emit_sdc(),
    )?;
    write(
        outputs,
        quartus.join("build_slugcxl_hj_sof.tcl"),
        emit_quartus::emit_build_tcl(),
    )?;
    let script = scripts.join("build_slugcxl_hj_sof.sh");
    write(outputs, script.clone(), emit_quartus::emit_build_script())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script)
            .with_context(|| format!("stat {}", script.display()))?
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions)
            .with_context(|| format!("chmod {}", script.display()))?;
    }

    Ok(())
}
