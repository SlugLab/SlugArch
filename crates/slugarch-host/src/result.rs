//! Decode 16 S2MDRS::MemData responses into a 4x4 result matrix.

use crate::HostError;
use slugarch_cxl_wire::{CxlMsg, S2MDRSOp};

/// Given 16 read-path responses in address-order (addr 0..15), returns
/// the 4x4 result matrix (row-major).
pub fn decode_results(responses: &[CxlMsg]) -> Result<[[u32; 4]; 4], HostError> {
    if responses.len() != 16 {
        return Err(HostError::DispatchFailed {
            tag: 0,
            reason: format!("expected 16 responses, got {}", responses.len()),
        });
    }

    let mut c = [[0u32; 4]; 4];
    for (i, r) in responses.iter().enumerate() {
        let row = i / 4;
        let col = i % 4;
        match r {
            CxlMsg::S2MDRS { opcode: S2MDRSOp::MemData, data, .. } => {
                let b0 = data[0] as u32;
                let b1 = data[1] as u32;
                let b2 = data[2] as u32;
                c[row][col] = b0 | (b1 << 8) | (b2 << 16);
            }
            CxlMsg::S2MNDR { opcode, tag } => {
                return Err(HostError::DispatchFailed {
                    tag: *tag,
                    reason: format!("expected S2MDRS, got S2MNDR {:?}", opcode),
                });
            }
            other => {
                return Err(HostError::DispatchFailed {
                    tag: other.tag(),
                    reason: format!("unexpected response class: {:?}", other.class()),
                });
            }
        }
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drs(tag: u16, b0: u8, b1: u8, b2: u8) -> CxlMsg {
        let mut data = [0u8; 32];
        data[0] = b0;
        data[1] = b1;
        data[2] = b2;
        CxlMsg::S2MDRS { tag, opcode: S2MDRSOp::MemData, data }
    }

    #[test]
    fn decodes_identity_row_major() {
        let mut responses: Vec<CxlMsg> = Vec::new();
        for i in 0..16u8 {
            responses.push(drs(i as u16, i, 0, 0));
        }
        let c = decode_results(&responses).unwrap();
        assert_eq!(c[0], [0, 1, 2, 3]);
        assert_eq!(c[3], [12, 13, 14, 15]);
    }

    #[test]
    fn decodes_multi_byte_accumulators() {
        let mut responses = vec![drs(0, 0xFF, 0x7F, 0x00)];
        for i in 1..16u8 {
            responses.push(drs(i as u16, 0, 0, 0));
        }
        let c = decode_results(&responses).unwrap();
        assert_eq!(c[0][0], 0x00_7FFF);
    }

    #[test]
    fn non_drs_response_errors() {
        use slugarch_cxl_wire::S2MNDROp;
        let mut responses = vec![CxlMsg::S2MNDR { tag: 0, opcode: S2MNDROp::DispatchFailed }];
        for _ in 1..16 {
            responses.push(drs(0, 0, 0, 0));
        }
        let err = decode_results(&responses).unwrap_err();
        assert!(matches!(err, HostError::DispatchFailed { .. }));
    }
}
