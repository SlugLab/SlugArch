use slugarch_verilator_sys as sys;

fn tick_cycle_then_free(new_fn: unsafe extern "C" fn() -> *mut sys::SlugarchIp, name: &str) {
    unsafe {
        let ip = new_fn();
        assert!(!ip.is_null(), "{} constructor returned null", name);
        sys::slugarch_ip_reset(ip);
        let token = [0u8; sys::SLUGARCH_TOKEN_BYTES as usize];
        sys::slugarch_ip_poke_cmd(ip, 1, token.as_ptr());
        let before = sys::slugarch_ip_tick(ip);
        for _ in 0..16 {
            sys::slugarch_ip_tick(ip);
        }
        let after = sys::slugarch_ip_tick(ip);
        assert!(after > before, "{} cycle count did not advance", name);
        let mut out = [0u8; sys::SLUGARCH_TOKEN_BYTES as usize];
        let _ = sys::slugarch_ip_peek_done(ip, out.as_mut_ptr());
        sys::slugarch_ip_free(ip);
    }
}

#[test]
fn smoke_systolic_4x4() {
    tick_cycle_then_free(sys::slugarch_ip_new_systolic_4x4, "systolic_4x4");
}
#[test]
fn smoke_systolic_16x16() {
    tick_cycle_then_free(sys::slugarch_ip_new_systolic_16x16, "systolic_16x16");
}
#[test]
fn smoke_systolic_32x32() {
    tick_cycle_then_free(sys::slugarch_ip_new_systolic_32x32, "systolic_32x32");
}
#[test]
fn smoke_npu_seed_g() {
    tick_cycle_then_free(sys::slugarch_ip_new_npu_seed_g, "npu_seed_g");
}
#[test]
fn smoke_npu_cluster() {
    tick_cycle_then_free(sys::slugarch_ip_new_npu_cluster, "npu_cluster");
}
#[test]
fn smoke_noc_mesh() {
    tick_cycle_then_free(sys::slugarch_ip_new_noc_mesh, "noc_mesh");
}
#[test]
fn smoke_gemm_ip() {
    tick_cycle_then_free(sys::slugarch_ip_new_gemm_ip, "gemm_ip");
}
