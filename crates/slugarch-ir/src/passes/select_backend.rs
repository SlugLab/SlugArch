//! select_backend: annotates each op with a BackendChoice.

use crate::module::Module;
use crate::op::{Op, TileKind};
use crate::pass::Pass;
use crate::types::{BackendChoice, IpId, Shape};
use crate::IrError;

/// Policy for picking a backend. Implementations decide per-op.
/// Having it as a trait (not a closure) lets us serialize named policies in
/// replay artifacts and reproduce them deterministically.
pub trait BackendPolicy {
    fn name(&self) -> &'static str;
    fn pick(&self, op: &Op) -> BackendChoice;
}

/// The default v1 policy:
///   - TensorTile(Gemm, shape): 32x32 if max(shape) >= 128; 16x16 if >= 32; else 4x4.
///   - TensorTile(Elementwise, _): NpuSeedG (tile elementwise = state update).
///   - StateStep(_): NpuSeedG (routed via NpuCluster is a fabric-layer detail).
///   - Dma: NoCMesh.
///   - Emu: PtxEmulationCore.
///   - Arith: PtxEmulationCore (pure scalar op; real PTX rarely emits scalar Arith
///     for tensor kernels, but the IR must annotate every op with a backend).
pub struct DefaultPolicy;

impl BackendPolicy for DefaultPolicy {
    fn name(&self) -> &'static str {
        "default_v1"
    }

    fn pick(&self, op: &Op) -> BackendChoice {
        let ip = match op {
            Op::TensorTile {
                kind: TileKind::Gemm,
                shape,
                ..
            } => gemm_tile_ip(shape),
            Op::TensorTile {
                kind: TileKind::Elementwise,
                ..
            } => IpId::NpuArrayV4SeedG,
            Op::StateStep { .. } => IpId::NpuArrayV4SeedG,
            Op::Dma { .. } => IpId::NoCMesh,
            Op::Emu { .. } => IpId::PtxEmulationCore,
            Op::Arith { .. } => IpId::PtxEmulationCore,
        };
        BackendChoice(ip)
    }
}

fn gemm_tile_ip(shape: &Shape) -> IpId {
    let max_dim = shape.0.iter().copied().max().unwrap_or(0);
    if max_dim >= 128 {
        IpId::SystolicArray32x32
    } else if max_dim >= 32 {
        IpId::SystolicArray16x16
    } else {
        IpId::SystolicArray4x4
    }
}

/// Force-override policy: every op maps to a fixed IP (used by value-preservation tests).
pub struct ForceIp(pub IpId, pub &'static str);

impl BackendPolicy for ForceIp {
    fn name(&self) -> &'static str {
        self.1
    }
    fn pick(&self, _op: &Op) -> BackendChoice {
        BackendChoice(self.0)
    }
}

pub struct SelectBackend<P: BackendPolicy> {
    policy: P,
}

impl<P: BackendPolicy> SelectBackend<P> {
    pub fn new(policy: P) -> Self {
        Self { policy }
    }
}

impl SelectBackend<DefaultPolicy> {
    pub fn default_policy() -> Self {
        Self::new(DefaultPolicy)
    }
}

impl<P: BackendPolicy> Pass for SelectBackend<P> {
    fn name(&self) -> &'static str {
        "select_backend"
    }

    fn run(&mut self, module: &mut Module) -> Result<(), IrError> {
        for f in module.functions.iter_mut() {
            for (id, op) in &f.ops {
                let choice = self.policy.pick(op);
                let meta = f.meta.entry(*id).or_default();
                meta.backend = Some(choice);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Context, FunctionBuilder};
    use crate::op::{OperandRef, StateKind, TileKind};
    use crate::types::{Dtype, Shape};

    #[test]
    fn large_gemm_picks_32x32() {
        let op = Op::TensorTile {
            kind: TileKind::Gemm,
            shape: Shape(vec![256, 256]),
            dtype: Dtype::F16,
            operands: vec![],
        };
        assert_eq!(DefaultPolicy.pick(&op).0, IpId::SystolicArray32x32);
    }

    #[test]
    fn small_gemm_picks_4x4() {
        let op = Op::TensorTile {
            kind: TileKind::Gemm,
            shape: Shape(vec![8, 8]),
            dtype: Dtype::F16,
            operands: vec![],
        };
        assert_eq!(DefaultPolicy.pick(&op).0, IpId::SystolicArray4x4);
    }

    #[test]
    fn state_step_picks_npu() {
        let op = Op::StateStep {
            kind: StateKind::RmsNorm,
            operands: vec![],
        };
        assert_eq!(DefaultPolicy.pick(&op).0, IpId::NpuArrayV4SeedG);
    }

    #[test]
    fn dma_picks_noc() {
        let op = Op::Dma {
            src: 0,
            dst: 64,
            bytes: 32,
        };
        assert_eq!(DefaultPolicy.pick(&op).0, IpId::NoCMesh);
    }

    #[test]
    fn emu_picks_ptx_emulation() {
        let op = Op::Emu {
            opcode: 1,
            operands: vec![OperandRef::ImmU64(0)],
        };
        assert_eq!(DefaultPolicy.pick(&op).0, IpId::PtxEmulationCore);
    }

    #[test]
    fn force_ip_policy_overrides_everything() {
        let force = ForceIp(IpId::SystolicArray16x16, "all_16x16");
        let op = Op::Dma {
            src: 0,
            dst: 8,
            bytes: 8,
        };
        assert_eq!(force.pick(&op).0, IpId::SystolicArray16x16);
    }

    #[test]
    fn pass_annotates_every_op() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "f");
        b.add_op(Op::TensorTile {
            kind: TileKind::Gemm,
            shape: Shape(vec![64, 64]),
            dtype: Dtype::F16,
            operands: vec![],
        });
        b.add_op(Op::Dma {
            src: 0,
            dst: 8,
            bytes: 8,
        });
        let mut m = Module::default();
        m.functions.push(b.finish());
        SelectBackend::default_policy().run(&mut m).unwrap();
        for meta in m.functions[0].meta.values() {
            assert!(meta.backend.is_some());
        }
    }
}
