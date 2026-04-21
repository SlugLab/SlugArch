//! Parser for Gemma runtime descriptors under
//! vendor/gemma-generated/generated/<ip>/runtime/<ip>.json.

use serde::Deserialize;
use slugarch_ir::types::IpId;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct IpRuntime {
    pub name: String,
    /// Free-form opcode table (e.g. ptx_emulation_core has a map of str -> str).
    /// Not all IPs carry opcodes; preserve as raw Value for now.
    #[serde(default)]
    pub opcodes: serde_json::Map<String, serde_json::Value>,
}

impl IpRuntime {
    pub fn load(ip_id: IpId) -> Result<Self, DescriptorError> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("vendor/gemma-generated/generated")
            .join(ip_id.catalog_name())
            .join("runtime")
            .join(format!("{}.json", ip_id.catalog_name()));
        let text = std::fs::read_to_string(&root)
            .map_err(|e| DescriptorError::Io(format!("reading {}: {}", root.display(), e)))?;
        serde_json::from_str(&text)
            .map_err(|e| DescriptorError::Parse(format!("{}: {}", root.display(), e)))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DescriptorError {
    #[error("runtime descriptor i/o: {0}")]
    Io(String),
    #[error("runtime descriptor parse: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ip_runtime_loads() {
        for ip in IpId::all() {
            let r = IpRuntime::load(*ip).unwrap_or_else(|e| panic!("load {:?}: {}", ip, e));
            assert_eq!(r.name, ip.catalog_name());
        }
    }

    #[test]
    fn ptx_emulation_has_opcodes() {
        let r = IpRuntime::load(IpId::PtxEmulationCore).unwrap();
        assert!(
            r.opcodes.len() > 5,
            "expected opcode table, got {}",
            r.opcodes.len()
        );
        assert!(
            r.opcodes.contains_key("2"),
            "opcode 2 (and) should be present"
        );
    }
}
