//! Emits the runtime-loadable hardware-JIT replay pipeline.

use crate::config::{CxlEndpointConfig, HardwareJitConfig};

pub fn emit(_cfg: &CxlEndpointConfig, _hj: &HardwareJitConfig) -> String {
    include_str!("templates/slugcxl_hj_pipeline.sv").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CxlEndpointConfig;

    #[test]
    fn snapshot_hj_pipeline_sv() {
        let cfg = CxlEndpointConfig::slugcxl_4x4();
        let sv = emit(&cfg, &cfg.hardware_jit);
        insta::assert_snapshot!(sv);
    }
}
