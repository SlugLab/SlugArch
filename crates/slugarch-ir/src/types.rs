use serde::{Deserialize, Serialize};

/// Identifier for a backend IP. Matches the vendored Gemma IP names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IpId {
    SystolicArray4x4,
    SystolicArray16x16,
    SystolicArray32x32,
    NpuArrayV4SeedG,
    NpuClusterV4,
    NoCMesh,
    GemmIp,
    PtxEmulationCore,
}

impl IpId {
    pub fn catalog_name(self) -> &'static str {
        match self {
            IpId::SystolicArray4x4 => "systolic_array_4x4",
            IpId::SystolicArray16x16 => "systolic_array_16x16",
            IpId::SystolicArray32x32 => "systolic_array_32x32",
            IpId::NpuArrayV4SeedG => "npu_array_v4_seed_g",
            IpId::NpuClusterV4 => "npu_cluster_v4",
            IpId::NoCMesh => "noc_mesh",
            IpId::GemmIp => "gemm_ip",
            IpId::PtxEmulationCore => "ptx_emulation_core",
        }
    }

    pub fn all() -> &'static [IpId] {
        &[
            IpId::SystolicArray4x4,
            IpId::SystolicArray16x16,
            IpId::SystolicArray32x32,
            IpId::NpuArrayV4SeedG,
            IpId::NpuClusterV4,
            IpId::NoCMesh,
            IpId::GemmIp,
            IpId::PtxEmulationCore,
        ]
    }

    pub fn is_cpu_backed(self) -> bool {
        matches!(self, IpId::PtxEmulationCore)
    }
}

/// Backend choice produced by the `select_backend` pass. Always corresponds to an IP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BackendChoice(pub IpId);

/// Stable identifier for an operation node inside a Function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OpId(pub u32);

/// Stable identifier for a token (assigned by `assign_tokens`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TokenId(pub u32);

/// Byte offset into the host memory buffer.
pub type Addr = u64;

/// Primitive data type of a tensor/operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dtype {
    U8, I8, U16, I16, U32, I32, U64, I64,
    F16, BF16, F32, F64,
}

/// Shape of a tensor tile, up to 4 dims in v1.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Shape(pub Vec<u32>);

impl Shape {
    pub fn rank(&self) -> usize { self.0.len() }
    pub fn num_elements(&self) -> u64 {
        self.0.iter().map(|d| *d as u64).product()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_catalog_names_are_unique_and_expected() {
        let names: Vec<_> = IpId::all().iter().map(|ip| ip.catalog_name()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "catalog names must be unique");
        assert!(names.contains(&"ptx_emulation_core"));
        assert!(names.contains(&"systolic_array_16x16"));
    }

    #[test]
    fn ptx_emulation_is_the_only_cpu_backend() {
        let cpu_ips: Vec<_> = IpId::all().iter().filter(|ip| ip.is_cpu_backed()).collect();
        assert_eq!(cpu_ips.len(), 1);
        assert_eq!(cpu_ips[0], &IpId::PtxEmulationCore);
    }

    #[test]
    fn shape_num_elements_matches_product() {
        let s = Shape(vec![16, 16, 4]);
        assert_eq!(s.rank(), 3);
        assert_eq!(s.num_elements(), 16 * 16 * 4);
    }

    #[test]
    fn ipid_serializes_round_trip() {
        let ip = IpId::SystolicArray16x16;
        let json = serde_json::to_string(&ip).unwrap();
        assert_eq!(json, "\"SystolicArray16x16\"");
        let back: IpId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ip);
    }
}
