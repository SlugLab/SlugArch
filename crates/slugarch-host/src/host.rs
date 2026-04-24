//! CxlHost — drives the Verilator-compiled slugcxl_4x4 endpoint with a
//! GEMM dispatch stream, collects responses, decodes the result matrix.

use crate::dispatch::build_gemm_dispatch_stream;
use crate::result::decode_results;
use crate::{GemmJob, GemmResult, HostError};
use slugarch_cxl_wire::{decode, encode, CxlMsg, S2MNDROp};
use slugarch_ir::types::IpId;
use slugarch_verilator::VerilatedIp;

const FLIT_TIMEOUT_TICKS: u64 = 64;

pub struct CxlHost {
    ip: VerilatedIp,
    next_tag: u16,
}

impl Default for CxlHost {
    fn default() -> Self {
        Self::new()
    }
}

impl CxlHost {
    pub fn new() -> Self {
        let mut ip = VerilatedIp::new(IpId::SlugCxl4x4);
        ip.reset();
        Self { ip, next_tag: 0 }
    }

    pub fn run_gemm(&mut self, job: &GemmJob) -> Result<GemmResult, HostError> {
        let stream = build_gemm_dispatch_stream(job, self.next_tag);
        self.next_tag = self.next_tag.wrapping_add(49);

        let mut flits_sent = 0u64;
        let mut flits_received = 0u64;
        let mut read_responses: Vec<CxlMsg> = Vec::with_capacity(16);

        let start_cycles = self.ip.tick();
        let mut last_cycles = start_cycles;

        for (i, msg) in stream.iter().enumerate() {
            let out_flit = encode(msg);
            self.ip.send_flit(&out_flit);
            flits_sent += 1;

            let mut waited = 0u64;
            let resp = loop {
                last_cycles = self.ip.tick();
                if let Some(f) = self.ip.try_recv_flit() {
                    break f;
                }
                waited += 1;
                if waited > FLIT_TIMEOUT_TICKS {
                    return Err(HostError::FlitTimeout { ticks: waited });
                }
            };

            let resp_msg = decode(&resp)?;
            flits_received += 1;

            let expected_tag = msg.tag();
            if resp_msg.tag() != expected_tag {
                return Err(HostError::UnexpectedTag {
                    got: resp_msg.tag(),
                    outstanding: vec![expected_tag],
                });
            }

            match &resp_msg {
                CxlMsg::S2MNDR { opcode: S2MNDROp::Cmp, .. } => {}
                CxlMsg::S2MNDR { opcode: S2MNDROp::DispatchFailed, tag } => {
                    return Err(HostError::DispatchFailed {
                        tag: *tag,
                        reason: format!("endpoint rejected dispatch #{i}"),
                    });
                }
                CxlMsg::S2MNDR { opcode, tag } => {
                    return Err(HostError::DispatchFailed {
                        tag: *tag,
                        reason: format!("unexpected NDR opcode {:?}", opcode),
                    });
                }
                CxlMsg::S2MDRS { .. } => {
                    read_responses.push(resp_msg);
                }
                other => {
                    return Err(HostError::DispatchFailed {
                        tag: other.tag(),
                        reason: format!("unexpected response class {:?}", other.class()),
                    });
                }
            }
        }

        if read_responses.len() != 16 {
            return Err(HostError::DispatchFailed {
                tag: 0,
                reason: format!("expected 16 read responses, got {}", read_responses.len()),
            });
        }

        let c = decode_results(&read_responses)?;

        Ok(GemmResult {
            c,
            cycles: last_cycles.saturating_sub(start_cycles),
            flits_sent,
            flits_received,
        })
    }
}
