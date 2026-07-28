//! Reusable SlugCXL SystemVerilog and runtime-artifact generator.

mod config;
mod emit_endpoint;
mod emit_fit_top;
mod emit_hj_pipeline;
mod emit_hj_top;
mod emit_policy_image;
mod emit_quartus;
mod emit_runtime;
mod emit_top;
mod hj_overhead;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use config::CxlEndpointConfig;
use slugarch_jit::{
    AddressRange, EpochPolicy, EventClass, Policy, RecordMode, Rule, VerifiedPolicy,
    SLUG_JIT_ABI_VERSION,
};

pub use emit_policy_image::{
    decode_policy_image, encode_policy_image, policy_image_hex, policy_image_manifest,
    DecodedPolicyImage, PolicyImageError, POLICY_HEADER_BYTES, POLICY_IMAGE_BYTES,
    POLICY_INSTRUCTION_SLOTS, POLICY_RANGE_SLOTS,
};

#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub out: PathBuf,
    pub hardware_jit: bool,
    pub quartus_project: Option<PathBuf>,
    pub policy_path: Option<PathBuf>,
}

pub fn generate(options: &GenerateOptions) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(&options.out)
        .with_context(|| format!("creating {}", options.out.display()))?;
    let cfg = CxlEndpointConfig::slugcxl_4x4();
    cfg.validate()?;
    let policy = if options.hardware_jit {
        Some(load_policy(options.policy_path.as_deref())?)
    } else {
        if options.policy_path.is_some() {
            bail!("--policy requires --hj");
        }
        None
    };

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
        emit_runtime::emit(&cfg, policy.is_some()),
    )?;

    if options.hardware_jit {
        let policy = policy.as_ref().expect("HJ policy was created above");
        let image = encode_policy_image(policy)?;
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
            emit_fit_top::emit(&cfg, policy, &image),
        )?;
        write(
            &mut outputs,
            options.out.join("slugcxl_hj_overhead.json"),
            hj_overhead::emit_report_json(&cfg.hardware_jit),
        )?;
        write(
            &mut outputs,
            options.out.join("slugcxl_hj_policy.hex"),
            policy_image_hex(&image)?,
        )?;
        write(
            &mut outputs,
            options.out.join("slugcxl_hj_policy.json"),
            policy_image_manifest(policy, &image)?,
        )?;
    }

    if let Some(project_root) = &options.quartus_project {
        emit_quartus_project(&mut outputs, project_root, &cfg)?;
    }

    Ok(outputs)
}

fn load_policy(path: Option<&Path>) -> Result<VerifiedPolicy> {
    let policy = if let Some(path) = path {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        Policy::parse(&bytes)?
    } else {
        default_policy()
    };
    Ok(policy.verify()?)
}

pub(crate) fn default_policy() -> Policy {
    Policy {
        version: SLUG_JIT_ABI_VERSION,
        name: "validation-cxlmem".to_string(),
        allowed_classes: vec![
            EventClass::CxlMemRead,
            EventClass::CxlMemWrite,
            EventClass::CxlMemData,
            EventClass::Completion,
        ],
        ranges: vec![AddressRange {
            base: 80 * 1024 * 1024,
            length: 32 * 1024 * 1024,
        }],
        sample_stride: 1,
        record_mode: RecordMode::Validation,
        metadata_budget: 256,
        epoch_policy: EpochPolicy::Phase,
        rules: vec![
            Rule::Capture {
                mode: RecordMode::Validation,
            },
            Rule::Emit,
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    }
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
