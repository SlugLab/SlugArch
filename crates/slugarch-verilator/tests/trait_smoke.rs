use slugarch_ir::types::IpId;
use slugarch_verilator::{VerilatedIp, WireCmd};

fn round_trip(ip_id: IpId) {
    let mut ip = VerilatedIp::new(ip_id);
    ip.reset();
    let cmd = WireCmd::new([0; 32]);
    ip.poke_cmd(&cmd);
    let before = ip.tick();
    for _ in 0..16 {
        ip.tick();
    }
    let after = ip.tick();
    assert!(after > before);
    let _maybe = ip.try_take_done();
    assert_eq!(ip.ip_id(), ip_id);
}

#[test]
fn systolic_4x4() {
    round_trip(IpId::SystolicArray4x4);
}
#[test]
fn systolic_16x16() {
    round_trip(IpId::SystolicArray16x16);
}
#[test]
fn systolic_32x32() {
    round_trip(IpId::SystolicArray32x32);
}
#[test]
fn npu_seed_g() {
    round_trip(IpId::NpuArrayV4SeedG);
}
#[test]
fn npu_cluster() {
    round_trip(IpId::NpuClusterV4);
}
#[test]
fn noc_mesh() {
    round_trip(IpId::NoCMesh);
}
#[test]
fn gemm_ip() {
    round_trip(IpId::GemmIp);
}

#[test]
#[should_panic(expected = "CPU-backed")]
fn ptx_emulation_panics() {
    let _ = VerilatedIp::new(IpId::PtxEmulationCore);
}
