//! FLIT encoder.

use crate::flit::{FlitBytes, DATA_BYTES, FLIT_BYTES, OFFSET_ADDR, OFFSET_DATA, OFFSET_TAG};
use crate::msg::CxlMsg;

pub fn encode(msg: &CxlMsg) -> FlitBytes {
    let mut flit = [0u8; FLIT_BYTES];
    let class = msg.class() as u8;

    let (opcode, tag, addr, data): (u8, u16, u64, Option<[u8; 32]>) = match msg {
        CxlMsg::M2SReq { tag, opcode, addr } => (*opcode as u8, *tag, *addr, None),
        CxlMsg::M2SRwD {
            tag,
            opcode,
            addr,
            data,
        } => (*opcode as u8, *tag, *addr, Some(*data)),
        CxlMsg::S2MDRS { tag, opcode, data } => (*opcode as u8, *tag, 0, Some(*data)),
        CxlMsg::S2MNDR { tag, opcode } => (*opcode as u8, *tag, 0, None),
        CxlMsg::D2HReq { tag, opcode, addr } => (*opcode as u8, *tag, *addr, None),
        CxlMsg::D2HResp { tag, opcode, data } => (*opcode as u8, *tag, 0, *data),
        CxlMsg::H2DReq { tag, opcode, addr } => (*opcode as u8, *tag, *addr, None),
        CxlMsg::H2DResp { tag, opcode } => (*opcode as u8, *tag, 0, None),
    };

    // Byte 0: class high nibble | opcode low nibble.
    flit[0] = (class << 4) | (opcode & 0x0F);
    // Bytes 1-2: tag LE.
    flit[OFFSET_TAG..OFFSET_TAG + 2].copy_from_slice(&tag.to_le_bytes());
    // Bytes 3-10: addr LE.
    flit[OFFSET_ADDR..OFFSET_ADDR + 8].copy_from_slice(&addr.to_le_bytes());
    // Bytes 11-42: data (if present).
    if let Some(d) = data {
        flit[OFFSET_DATA..OFFSET_DATA + DATA_BYTES].copy_from_slice(&d);
    }
    // Bytes 43-63: already zero (initialized).
    flit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::{M2SRwDOp, S2MNDROp};

    #[test]
    fn m2s_rwd_encodes_class_and_opcode_in_byte_0() {
        let msg = CxlMsg::M2SRwD {
            tag: 0,
            opcode: M2SRwDOp::MemWr,
            addr: 0,
            data: [0; 32],
        };
        let flit = encode(&msg);
        // class=0x2 (M2SRwD) in high nibble, opcode=0x0 (MemWr) in low.
        assert_eq!(flit[0], 0x20);
    }

    #[test]
    fn tag_encodes_little_endian_bytes_1_and_2() {
        let msg = CxlMsg::S2MNDR {
            tag: 0xABCD,
            opcode: S2MNDROp::Cmp,
        };
        let flit = encode(&msg);
        assert_eq!(flit[1], 0xCD);
        assert_eq!(flit[2], 0xAB);
    }

    #[test]
    fn addr_encodes_little_endian_bytes_3_to_10() {
        let msg = CxlMsg::M2SRwD {
            tag: 0,
            opcode: M2SRwDOp::MemWr,
            addr: 0x1122_3344_5566_7788,
            data: [0; 32],
        };
        let flit = encode(&msg);
        assert_eq!(flit[3], 0x88);
        assert_eq!(flit[10], 0x11);
    }

    #[test]
    fn data_encodes_bytes_11_to_42() {
        let mut data = [0u8; 32];
        data[0] = 0xDE;
        data[31] = 0xAD;
        let msg = CxlMsg::M2SRwD {
            tag: 0,
            opcode: M2SRwDOp::MemWr,
            addr: 0,
            data,
        };
        let flit = encode(&msg);
        assert_eq!(flit[11], 0xDE);
        assert_eq!(flit[42], 0xAD);
    }

    #[test]
    fn reserved_bytes_43_to_63_are_zero() {
        let msg = CxlMsg::M2SRwD {
            tag: 0xFFFF,
            opcode: M2SRwDOp::MemWr,
            addr: !0,
            data: [0xFF; 32],
        };
        let flit = encode(&msg);
        for b in &flit[43..64] {
            assert_eq!(*b, 0);
        }
    }
}
