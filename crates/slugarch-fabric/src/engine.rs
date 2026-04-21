//! Fabric event loop: pokes VerilatedIps, drives their clocks, retires
//! completions in token order, dispatches CPU-emu commands immediately.

use crate::cpu_emu;
use crate::FabricError;
use serde::{Deserialize, Serialize};
use slugarch_backend::DispatchCmd;
use slugarch_ir::types::{IpId, TokenId};
use slugarch_verilator::{VerilatedIp, WireCmd};
use std::collections::HashMap;

const DEFAULT_IP_CYCLE_BUDGET: u64 = 4096;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunReport {
    pub total_cycles: u64,
    pub per_ip_cycles: HashMap<IpId, u64>,
    pub completions: u64,
}

pub struct Fabric {
    ips: HashMap<IpId, VerilatedIp>,
    host_mem: Vec<u8>,
}

impl Fabric {
    pub fn new(host_mem_size: usize) -> Self {
        Self {
            ips: HashMap::new(),
            host_mem: vec![0u8; host_mem_size],
        }
    }

    pub fn host_mem(&self) -> &[u8] {
        &self.host_mem
    }

    pub fn host_mem_mut(&mut self) -> &mut [u8] {
        &mut self.host_mem
    }

    pub fn set_host_mem(&mut self, bytes: &[u8]) {
        self.host_mem.clear();
        self.host_mem.extend_from_slice(bytes);
    }

    fn ensure_ip(&mut self, ip_id: IpId) -> &mut VerilatedIp {
        self.ips.entry(ip_id).or_insert_with(|| {
            let mut ip = VerilatedIp::new(ip_id);
            ip.reset();
            ip
        })
    }

    pub fn run(&mut self, stream: Vec<DispatchCmd>) -> Result<RunReport, FabricError> {
        let mut report = RunReport::default();

        let mut retired: std::collections::HashSet<TokenId> = std::collections::HashSet::new();
        retired.insert(TokenId(0));

        let mut pending: Vec<DispatchCmd> = stream;

        struct InFlight {
            ip: IpId,
            cmd: DispatchCmd,
            cycles_elapsed: u64,
        }
        let mut in_flight: Vec<InFlight> = Vec::new();

        // Global safety cap — 4 * IP budget * (1 + #ops). Loose bound.
        let global_budget: u64 = 4 * DEFAULT_IP_CYCLE_BUDGET * (1 + pending.len() as u64);

        while !pending.is_empty() || !in_flight.is_empty() {
            let mut launched_any = false;
            let mut i = 0;
            while i < pending.len() {
                let can_fire = retired.contains(&pending[i].token_in);
                if !can_fire {
                    i += 1;
                    continue;
                }
                let cmd = pending.swap_remove(i);

                if cmd.ip.is_cpu_backed() {
                    let cost = cpu_emu::execute(&cmd, &mut self.host_mem);
                    report.total_cycles += cost;
                    *report.per_ip_cycles.entry(cmd.ip).or_insert(0) += cost;
                    retired.insert(cmd.token_out);
                    report.completions += 1;
                    launched_any = true;
                } else {
                    let ip = self.ensure_ip(cmd.ip);
                    let wire = WireCmd::new(cmd.token);
                    ip.poke_cmd(&wire);
                    in_flight.push(InFlight {
                        ip: cmd.ip,
                        cmd,
                        cycles_elapsed: 0,
                    });
                    launched_any = true;
                }
            }

            if in_flight.is_empty() {
                if !launched_any && !pending.is_empty() {
                    return Err(FabricError::TokenResolutionStuck {
                        pending: pending.len(),
                    });
                }
                continue;
            }

            let unique_ips: std::collections::HashSet<IpId> =
                in_flight.iter().map(|f| f.ip).collect();
            for ip_id in unique_ips {
                let ip = self.ips.get_mut(&ip_id).unwrap();
                ip.tick();
            }
            report.total_cycles += 1;

            let mut still: Vec<InFlight> = Vec::new();
            for mut fl in in_flight.drain(..) {
                fl.cycles_elapsed += 1;
                *report.per_ip_cycles.entry(fl.ip).or_insert(0) += 1;
                let ip = self.ips.get_mut(&fl.ip).unwrap();
                if let Some(done) = ip.try_take_done() {
                    let _ = done;
                    retired.insert(fl.cmd.token_out);
                    report.completions += 1;
                } else if fl.cycles_elapsed > DEFAULT_IP_CYCLE_BUDGET {
                    return Err(FabricError::DispatchTimeout {
                        ip: fl.ip,
                        cycles: fl.cycles_elapsed,
                        cmd_op: None,
                    });
                } else {
                    still.push(fl);
                }
            }
            in_flight = still;

            if report.total_cycles > global_budget {
                return Err(FabricError::DispatchTimeout {
                    ip: in_flight.first().map(|f| f.ip).unwrap_or(IpId::NoCMesh),
                    cycles: report.total_cycles,
                    cmd_op: None,
                });
            }
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slugarch_backend::{DispatchCmd, DispatchMeta};
    use slugarch_ir::types::{IpId, TokenId};

    fn cpu_cmd(opcode: u32, token_in: u32, token_out: u32) -> DispatchCmd {
        DispatchCmd {
            ip: IpId::PtxEmulationCore,
            opcode,
            token: [0; 32],
            token_in: TokenId(token_in),
            token_out: TokenId(token_out),
            meta: DispatchMeta::default(),
        }
    }

    #[test]
    fn empty_stream_produces_zero_cycles() {
        let mut f = Fabric::new(64);
        let r = f.run(vec![]).unwrap();
        assert_eq!(r.total_cycles, 0);
        assert_eq!(r.completions, 0);
    }

    #[test]
    fn linear_cpu_chain_accumulates_cycles() {
        let mut f = Fabric::new(64);
        let cmds = vec![cpu_cmd(2, 0, 1), cpu_cmd(17, 1, 2), cpu_cmd(4, 2, 3)];
        let r = f.run(cmds).unwrap();
        assert_eq!(r.total_cycles, 6);
        assert_eq!(r.completions, 3);
        assert_eq!(r.per_ip_cycles[&IpId::PtxEmulationCore], 6);
    }

    #[test]
    fn stuck_dep_is_detected() {
        let mut f = Fabric::new(64);
        let r = f.run(vec![cpu_cmd(1, 42, 43)]);
        assert!(matches!(r, Err(FabricError::TokenResolutionStuck { .. })));
    }
}
