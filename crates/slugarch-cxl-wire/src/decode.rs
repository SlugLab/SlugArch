//! FLIT decoder.

use crate::error::WireError;
use crate::flit::{FlitBytes, DATA_BYTES, OFFSET_ADDR, OFFSET_DATA, OFFSET_RESERVED, OFFSET_TAG};
use crate::msg::{
    CxlMsg, D2HReqOp, D2HRespOp, H2DReqOp, H2DRespOp, M2SReqOp, M2SRwDOp, MsgClass, S2MDRSOp,
    S2MNDROp,
};

pub fn decode(flit: &FlitBytes) -> Result<CxlMsg, WireError> {
    // Reject any flit with nonzero reserved bytes.
    for b in &flit[OFFSET_RESERVED..] {
        if *b != 0 {
            return Err(WireError::ReservedBitsSet);
        }
    }

    let class_nibble = flit[0] >> 4;
    let opcode_nibble = flit[0] & 0x0F;
    let class = MsgClass::from_nibble(class_nibble).ok_or(WireError::UnknownClass(class_nibble))?;

    let mut tag_bytes = [0u8; 2];
    tag_bytes.copy_from_slice(&flit[OFFSET_TAG..OFFSET_TAG + 2]);
    let tag = u16::from_le_bytes(tag_bytes);

    let mut addr_bytes = [0u8; 8];
    addr_bytes.copy_from_slice(&flit[OFFSET_ADDR..OFFSET_ADDR + 8]);
    let addr = u64::from_le_bytes(addr_bytes);

    let mut data = [0u8; DATA_BYTES];
    data.copy_from_slice(&flit[OFFSET_DATA..OFFSET_DATA + DATA_BYTES]);

    match class {
        MsgClass::M2SReq => {
            let opcode = match opcode_nibble {
                0x0 => M2SReqOp::MemRd,
                0x1 => M2SReqOp::MemRdData,
                0x2 => M2SReqOp::MemInv,
                op => return Err(WireError::UnknownOpcode(op, class)),
            };
            Ok(CxlMsg::M2SReq { tag, opcode, addr })
        }
        MsgClass::M2SRwD => {
            let opcode = match opcode_nibble {
                0x0 => M2SRwDOp::MemWr,
                0x1 => M2SRwDOp::MemWrPtl,
                0x2 => M2SRwDOp::MemClnEvct,
                op => return Err(WireError::UnknownOpcode(op, class)),
            };
            Ok(CxlMsg::M2SRwD {
                tag,
                opcode,
                addr,
                data,
            })
        }
        MsgClass::S2MDRS => {
            let opcode = match opcode_nibble {
                0x0 => S2MDRSOp::MemData,
                0x1 => S2MDRSOp::MemDataNxm,
                op => return Err(WireError::UnknownOpcode(op, class)),
            };
            Ok(CxlMsg::S2MDRS { tag, opcode, data })
        }
        MsgClass::S2MNDR => {
            let opcode = match opcode_nibble {
                0x0 => S2MNDROp::Cmp,
                0x1 => S2MNDROp::CmpS,
                0x2 => S2MNDROp::CmpE,
                0x3 => S2MNDROp::CmpI,
                0x4 => S2MNDROp::MemPassDirty,
                0xF => S2MNDROp::DispatchFailed,
                op => return Err(WireError::UnknownOpcode(op, class)),
            };
            Ok(CxlMsg::S2MNDR { tag, opcode })
        }
        MsgClass::D2HReq => {
            let opcode = match opcode_nibble {
                0x0 => D2HReqOp::RdShared,
                0x1 => D2HReqOp::RdOwn,
                0x2 => D2HReqOp::Inval,
                op => return Err(WireError::UnknownOpcode(op, class)),
            };
            Ok(CxlMsg::D2HReq { tag, opcode, addr })
        }
        MsgClass::D2HResp => {
            let opcode = match opcode_nibble {
                0x0 => D2HRespOp::RspIFwdM,
                0x1 => D2HRespOp::RspSFwdM,
                0x2 => D2HRespOp::RspV,
                op => return Err(WireError::UnknownOpcode(op, class)),
            };
            // v1 quirk: we can't cleanly detect "data present" from a flat
            // byte layout, so D2HResp always decodes with Some(data). Caller
            // consults the opcode semantics.
            Ok(CxlMsg::D2HResp {
                tag,
                opcode,
                data: Some(data),
            })
        }
        MsgClass::H2DReq => {
            let opcode = match opcode_nibble {
                0x0 => H2DReqOp::SnpData,
                0x1 => H2DReqOp::SnpInv,
                0x2 => H2DReqOp::SnpCur,
                op => return Err(WireError::UnknownOpcode(op, class)),
            };
            Ok(CxlMsg::H2DReq { tag, opcode, addr })
        }
        MsgClass::H2DResp => {
            let opcode = match opcode_nibble {
                0x0 => H2DRespOp::GoWritePull,
                0x1 => H2DRespOp::GoErr,
                0x2 => H2DRespOp::Go,
                op => return Err(WireError::UnknownOpcode(op, class)),
            };
            Ok(CxlMsg::H2DResp { tag, opcode })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::encode;
    use crate::msg::{M2SReqOp, M2SRwDOp, S2MDRSOp, S2MNDROp};

    #[test]
    fn round_trip_m2s_req() {
        let msg = CxlMsg::M2SReq {
            tag: 0x1234,
            opcode: M2SReqOp::MemRd,
            addr: 0x2000,
        };
        assert_eq!(decode(&encode(&msg)).unwrap(), msg);
    }

    #[test]
    fn round_trip_m2s_rwd() {
        let mut data = [0u8; 32];
        data[0] = 0xDE;
        data[1] = 0xAD;
        data[2] = 0xBE;
        data[3] = 0xEF;
        let msg = CxlMsg::M2SRwD {
            tag: 0xABCD,
            opcode: M2SRwDOp::MemWr,
            addr: 0x2000,
            data,
        };
        assert_eq!(decode(&encode(&msg)).unwrap(), msg);
    }

    #[test]
    fn round_trip_s2m_drs() {
        let mut data = [0u8; 32];
        data[0] = 0x11;
        data[1] = 0x22;
        data[2] = 0x33;
        let msg = CxlMsg::S2MDRS {
            tag: 0x5555,
            opcode: S2MDRSOp::MemData,
            data,
        };
        assert_eq!(decode(&encode(&msg)).unwrap(), msg);
    }

    #[test]
    fn round_trip_s2m_ndr_dispatch_failed() {
        let msg = CxlMsg::S2MNDR {
            tag: 0x42,
            opcode: S2MNDROp::DispatchFailed,
        };
        assert_eq!(decode(&encode(&msg)).unwrap(), msg);
    }

    #[test]
    fn reserved_bits_rejected() {
        let msg = CxlMsg::S2MNDR {
            tag: 0,
            opcode: S2MNDROp::Cmp,
        };
        let mut flit = encode(&msg);
        flit[50] = 0xFF;
        assert_eq!(decode(&flit), Err(WireError::ReservedBitsSet));
    }

    #[test]
    fn unknown_class_rejected() {
        let mut flit = [0u8; 64];
        flit[0] = 0xF0; // class 0xF not defined
        assert_eq!(decode(&flit), Err(WireError::UnknownClass(0xF)));
    }

    #[test]
    fn unknown_opcode_rejected() {
        let mut flit = [0u8; 64];
        flit[0] = 0x1E; // class 0x1 (M2SReq), opcode 0xE not defined
        match decode(&flit) {
            Err(WireError::UnknownOpcode(0xE, MsgClass::M2SReq)) => {}
            other => panic!("unexpected: {:?}", other),
        }
    }
}
