use crate::wire::{WireCmd, WireDone, TOKEN_BYTES};
use slugarch_ir::types::IpId;
use slugarch_verilator_sys as sys;

pub struct VerilatedIp {
    raw: *mut sys::SlugarchIp,
    ip_id: IpId,
}

impl VerilatedIp {
    pub fn new(ip_id: IpId) -> Self {
        let raw = unsafe {
            match ip_id {
                IpId::SystolicArray4x4 => sys::slugarch_ip_new_systolic_4x4(),
                IpId::SystolicArray16x16 => sys::slugarch_ip_new_systolic_16x16(),
                IpId::SystolicArray32x32 => sys::slugarch_ip_new_systolic_32x32(),
                IpId::NpuArrayV4SeedG => sys::slugarch_ip_new_npu_seed_g(),
                IpId::NpuClusterV4 => sys::slugarch_ip_new_npu_cluster(),
                IpId::NoCMesh => sys::slugarch_ip_new_noc_mesh(),
                IpId::GemmIp => sys::slugarch_ip_new_gemm_ip(),
                IpId::PtxEmulationCore => {
                    panic!("ptx_emulation_core is CPU-backed; use the slugarch-backend CPU impl")
                }
            }
        };
        assert!(!raw.is_null(), "null IP constructor for {:?}", ip_id);
        Self { raw, ip_id }
    }

    pub fn ip_id(&self) -> IpId {
        self.ip_id
    }

    pub fn reset(&mut self) {
        unsafe {
            sys::slugarch_ip_reset(self.raw);
        }
    }

    pub fn tick(&mut self) -> u64 {
        unsafe { sys::slugarch_ip_tick(self.raw) }
    }

    pub fn poke_cmd(&mut self, cmd: &WireCmd) {
        unsafe {
            sys::slugarch_ip_poke_cmd(
                self.raw,
                if cmd.valid { 1 } else { 0 },
                cmd.token_in.as_ptr(),
            );
        }
    }

    pub fn try_take_done(&mut self) -> Option<WireDone> {
        let mut token_out = [0u8; TOKEN_BYTES];
        let done = unsafe { sys::slugarch_ip_peek_done(self.raw, token_out.as_mut_ptr()) };
        if done != 0 {
            Some(WireDone {
                valid: true,
                token_out,
            })
        } else {
            None
        }
    }

    pub fn cmd_ready(&self) -> bool {
        unsafe { sys::slugarch_ip_peek_cmd_ready(self.raw) != 0 }
    }
}

impl Drop for VerilatedIp {
    fn drop(&mut self) {
        unsafe {
            sys::slugarch_ip_free(self.raw);
        }
        self.raw = std::ptr::null_mut();
    }
}

// SAFETY: Each VerilatedIp owns its own VerilatedContext. Not Sync (shared
// access would race), but Send — a single thread can hand one off.
unsafe impl Send for VerilatedIp {}
