//! Per-IP BackendBinding implementations.

pub mod gemm_ip;
pub mod noc_mesh;
pub mod npu_cluster;
pub mod npu_seed_g;
pub mod ptx_emulation;
pub mod systolic;

pub use gemm_ip::GemmIpBinding;
pub use noc_mesh::NoCMeshBinding;
pub use npu_cluster::NpuClusterBinding;
pub use npu_seed_g::NpuSeedGBinding;
pub use ptx_emulation::PtxEmulationBinding;
pub use systolic::SystolicBinding;
