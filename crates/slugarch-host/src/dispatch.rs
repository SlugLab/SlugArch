//! Build the 49-dispatch CXL message stream for a 4x4 GEMM.
//!
//! systolic_array_4x4 wrapper token layout (from its df_wrapper.sv):
//!   token_in[2]     = load_valid
//!   token_in[3]     = load_matrix_sel  (0=A, 1=B)
//!   token_in[11:4]  = load_addr         (0..15)
//!   token_in[19:12] = load_data         (one byte of A or B)
//!   token_in[20]    = compute_valid
//!   token_in[21]    = read_valid
//!   token_in[29:22] = read_addr         (0..15)

use crate::GemmJob;
use slugarch_cxl_wire::{CxlMsg, M2SReqOp, M2SRwDOp};

pub const DISPATCH_ADDR: u64 = 0x2000;

fn make_load_token(sel: u8, addr: u8, data: u8) -> u32 {
    let mut t = 0u32;
    t |= 1u32 << 2;
    t |= (sel as u32 & 1) << 3;
    t |= (addr as u32 & 0xFF) << 4;
    t |= (data as u32 & 0xFF) << 12;
    t
}

fn make_compute_token() -> u32 {
    1u32 << 20
}

fn make_read_token(addr: u8) -> u32 {
    let mut t = 0u32;
    t |= 1u32 << 21;
    t |= (addr as u32 & 0xFF) << 22;
    t
}

fn pack_token_into_data(token: u32) -> [u8; 32] {
    let mut data = [0u8; 32];
    data[..4].copy_from_slice(&token.to_le_bytes());
    data
}

/// Build the 49-message dispatch stream. Tags are assigned sequentially
/// starting at `first_tag`.
pub fn build_gemm_dispatch_stream(job: &GemmJob, first_tag: u16) -> Vec<CxlMsg> {
    let mut out = Vec::with_capacity(49);
    let mut tag = first_tag;

    // Phase 1: load A.
    for row in 0..4 {
        for col in 0..4 {
            let addr = (row * 4 + col) as u8;
            let token = make_load_token(0, addr, job.a[row][col]);
            out.push(CxlMsg::M2SRwD {
                tag,
                opcode: M2SRwDOp::MemWr,
                addr: DISPATCH_ADDR,
                data: pack_token_into_data(token),
            });
            tag = tag.wrapping_add(1);
        }
    }

    // Phase 2: load B.
    for row in 0..4 {
        for col in 0..4 {
            let addr = (row * 4 + col) as u8;
            let token = make_load_token(1, addr, job.b[row][col]);
            out.push(CxlMsg::M2SRwD {
                tag,
                opcode: M2SRwDOp::MemWr,
                addr: DISPATCH_ADDR,
                data: pack_token_into_data(token),
            });
            tag = tag.wrapping_add(1);
        }
    }

    // Phase 3: compute.
    out.push(CxlMsg::M2SRwD {
        tag,
        opcode: M2SRwDOp::MemWr,
        addr: DISPATCH_ADDR,
        data: pack_token_into_data(make_compute_token()),
    });
    tag = tag.wrapping_add(1);

    // Phase 4: reads. v1 host encodes reads as M2SReq; the read token is
    // packed into the high 32 bits of the addr field (bits [63:32]).
    // Endpoint extracts from flit_in_data[87:56] for reads.
    for addr in 0..16 {
        let token = make_read_token(addr);
        let combined_addr = DISPATCH_ADDR | ((token as u64) << 32);
        out.push(CxlMsg::M2SReq {
            tag,
            opcode: M2SReqOp::MemRd,
            addr: combined_addr,
        });
        tag = tag.wrapping_add(1);
    }

    assert_eq!(out.len(), 49);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_job() -> GemmJob {
        GemmJob {
            a: [[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]],
            b: [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
        }
    }

    #[test]
    fn stream_has_49_messages() {
        let s = build_gemm_dispatch_stream(&sample_job(), 0);
        assert_eq!(s.len(), 49);
    }

    #[test]
    fn first_16_are_load_a() {
        let s = build_gemm_dispatch_stream(&sample_job(), 100);
        for (i, m) in s[..16].iter().enumerate() {
            match m {
                CxlMsg::M2SRwD { tag, opcode, addr, data } => {
                    assert_eq!(*tag, 100 + i as u16);
                    assert_eq!(*opcode, M2SRwDOp::MemWr);
                    assert_eq!(*addr, DISPATCH_ADDR);
                    let token = u32::from_le_bytes(data[..4].try_into().unwrap());
                    assert_eq!((token >> 2) & 1, 1, "load_valid bit");
                    assert_eq!((token >> 3) & 1, 0, "load_matrix_sel = 0 (A)");
                    assert_eq!((token >> 4) & 0xFF, i as u32, "load_addr");
                }
                _ => panic!("expected M2SRwD for load A"),
            }
        }
    }

    #[test]
    fn middle_16_are_load_b() {
        let s = build_gemm_dispatch_stream(&sample_job(), 0);
        for m in &s[16..32] {
            match m {
                CxlMsg::M2SRwD { data, .. } => {
                    let token = u32::from_le_bytes(data[..4].try_into().unwrap());
                    assert_eq!((token >> 3) & 1, 1, "load_matrix_sel = 1 (B)");
                }
                _ => panic!("expected M2SRwD for load B"),
            }
        }
    }

    #[test]
    fn message_32_is_compute() {
        let s = build_gemm_dispatch_stream(&sample_job(), 0);
        match &s[32] {
            CxlMsg::M2SRwD { data, .. } => {
                let token = u32::from_le_bytes(data[..4].try_into().unwrap());
                assert_eq!((token >> 20) & 1, 1, "compute_valid bit");
            }
            _ => panic!("expected M2SRwD for compute"),
        }
    }

    #[test]
    fn last_16_are_reads_with_token_in_addr_high() {
        let s = build_gemm_dispatch_stream(&sample_job(), 0);
        for (i, m) in s[33..].iter().enumerate() {
            match m {
                CxlMsg::M2SReq { opcode, addr, .. } => {
                    assert_eq!(*opcode, M2SReqOp::MemRd);
                    assert_eq!(*addr & 0xFFFF, DISPATCH_ADDR & 0xFFFF);
                    let token = (*addr >> 32) as u32;
                    assert_eq!((token >> 21) & 1, 1, "read_valid");
                    assert_eq!((token >> 22) & 0xFF, i as u32, "read_addr = {}", i);
                }
                _ => panic!("expected M2SReq for read"),
            }
        }
    }
}
