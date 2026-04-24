//! Wire-layer confirmation that tags flow unchanged through encode/decode.
//! CxlHost::run_gemm checks response tags against outgoing tags, so as long
//! as the wire preserves them, the in-host check has reliable inputs.

use slugarch_cxl_wire::{decode, encode, CxlMsg, M2SRwDOp, S2MNDROp};

#[test]
fn wire_decode_preserves_mismatched_tags_distinctly() {
    let out = CxlMsg::M2SRwD {
        tag: 0x42,
        opcode: M2SRwDOp::MemWr,
        addr: 0x2000,
        data: [0; 32],
    };
    let back = CxlMsg::S2MNDR {
        tag: 0x99,
        opcode: S2MNDROp::Cmp,
    };

    let out_flit = encode(&out);
    let back_flit = encode(&back);

    let out_dec = decode(&out_flit).unwrap();
    let back_dec = decode(&back_flit).unwrap();
    assert_ne!(out_dec.tag(), back_dec.tag());
    assert_eq!(out_dec.tag(), 0x42);
    assert_eq!(back_dec.tag(), 0x99);
}
