use slugarch_verilator_sys as sys;

#[test]
fn systolic_4x4_resets_and_ticks() {
    unsafe {
        let ip = sys::slugarch_ip_new_systolic_4x4();
        assert!(!ip.is_null());

        sys::slugarch_ip_reset(ip);

        let token = [0u8; sys::SLUGARCH_TOKEN_BYTES as usize];
        sys::slugarch_ip_poke_cmd(ip, 1, token.as_ptr());

        let cycles_before = sys::slugarch_ip_tick(ip);
        for _ in 0..32 {
            sys::slugarch_ip_tick(ip);
        }
        let cycles_after = sys::slugarch_ip_tick(ip);
        assert!(cycles_after > cycles_before);

        let mut out = [0u8; sys::SLUGARCH_TOKEN_BYTES as usize];
        let _done = sys::slugarch_ip_peek_done(ip, out.as_mut_ptr());

        sys::slugarch_ip_free(ip);
    }
}
