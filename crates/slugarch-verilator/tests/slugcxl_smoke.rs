use slugarch_ir::types::IpId;
use slugarch_verilator::VerilatedIp;

/// Smoke: reset the slugcxl_4x4 top, send one bogus FLIT (addr not matching
/// dispatch region), expect a FLIT back (class=0x4 S2MNDR, opcode=0xF
/// DispatchFailed). Proves: construct, reset, send, tick-to-respond, recv.
#[test]
fn slugcxl_4x4_dispatch_failed_for_bogus_addr() {
    let mut ip = VerilatedIp::new(IpId::SlugCxl4x4);
    ip.reset();

    // Craft an M2SRwD FLIT (class 0x2, opcode 0x0) at addr 0x9999 (not claimed).
    let mut flit = [0u8; 64];
    flit[0] = 0x20; // class=0x2, opcode=0x0
    flit[1..3].copy_from_slice(&0x1234u16.to_le_bytes()); // tag = 0x1234
    flit[3..11].copy_from_slice(&0x9999u64.to_le_bytes()); // addr = 0x9999

    ip.send_flit(&flit);

    // Tick up to 64 cycles looking for a response FLIT.
    let mut response: Option<[u8; 64]> = None;
    for _ in 0..64 {
        ip.tick();
        if let Some(f) = ip.try_recv_flit() {
            response = Some(f);
            break;
        }
    }
    let resp = response.expect("no response FLIT within 64 cycles");
    assert_eq!(resp[0], 0x4F, "expected S2MNDR::DispatchFailed (byte 0 = 0x4F), got {:#x}", resp[0]);
    assert_eq!(&resp[1..3], &0x1234u16.to_le_bytes());
}
