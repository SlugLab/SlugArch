//! CxlEndpointConfig — the input the generator consumes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CxlEndpointConfig {
    pub name: String,
    pub protocol: CxlProtocol,
    pub address_spaces: Vec<AddressSpace>,
    pub attached_wrapper: AttachedWrapper,
    pub outstanding_reqs: u32,
    pub dispatch_table: Vec<DispatchRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CxlProtocol {
    pub mem: bool,
    pub cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddressSpace {
    pub name: String,
    pub base: u64,
    pub length: u64,
    pub readable: bool,
    pub writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachedWrapper {
    pub module: String,
    pub token_width: u32,
    pub dispatch_base: u64,
    pub result_base: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchRoute {
    pub match_mask: u64,
    pub match_value: u64,
    pub target: RouteTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouteTarget {
    AttachedIp,
    Dram,
    Drop,
}

impl CxlEndpointConfig {
    /// The v1 hardcoded config for slugcxl_4x4.
    pub fn slugcxl_4x4() -> Self {
        Self {
            name: "slugcxl_4x4".into(),
            protocol: CxlProtocol { mem: true, cache: true },
            address_spaces: vec![
                AddressSpace {
                    name: "dispatch".into(),
                    base: 0x2000,
                    length: 0x1000,
                    readable: true,
                    writable: true,
                },
                AddressSpace {
                    name: "result".into(),
                    base: 0x3000,
                    length: 0x1000,
                    readable: true,
                    writable: false,
                },
            ],
            attached_wrapper: AttachedWrapper {
                module: "gemma_codegen_systolic_array_4x4_df".into(),
                token_width: 256,
                dispatch_base: 0x2000,
                result_base: 0x3000,
            },
            outstanding_reqs: 1,
            dispatch_table: vec![DispatchRoute {
                match_mask: 0xFFFF_F000,
                match_value: 0x0000_2000,
                target: RouteTarget::AttachedIp,
            }],
        }
    }

    pub fn validate(&self) -> Result<(), GenError> {
        if self.attached_wrapper.module != "gemma_codegen_systolic_array_4x4_df" {
            return Err(GenError::UnknownWrapper(
                self.attached_wrapper.module.clone(),
            ));
        }
        for (i, a) in self.address_spaces.iter().enumerate() {
            for b in &self.address_spaces[i + 1..] {
                let a_end = a.base.saturating_add(a.length);
                let b_end = b.base.saturating_add(b.length);
                if a.base < b_end && b.base < a_end {
                    return Err(GenError::OverlappingAddressSpace {
                        a: a.clone(),
                        b: b.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GenError {
    #[error("unknown attached wrapper: {0}")]
    UnknownWrapper(String),
    #[error("overlapping address spaces: {a:?} vs {b:?}")]
    OverlappingAddressSpace { a: AddressSpace, b: AddressSpace },
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        CxlEndpointConfig::slugcxl_4x4().validate().unwrap();
    }

    #[test]
    fn overlapping_address_spaces_rejected() {
        let mut cfg = CxlEndpointConfig::slugcxl_4x4();
        cfg.address_spaces.push(AddressSpace {
            name: "bogus".into(),
            base: 0x2500,
            length: 0x100,
            readable: true,
            writable: true,
        });
        assert!(matches!(
            cfg.validate(),
            Err(GenError::OverlappingAddressSpace { .. })
        ));
    }

    #[test]
    fn unknown_wrapper_rejected() {
        let mut cfg = CxlEndpointConfig::slugcxl_4x4();
        cfg.attached_wrapper.module = "not_a_real_module".into();
        assert!(matches!(cfg.validate(), Err(GenError::UnknownWrapper(_))));
    }
}
