//! CPU-backed ptx_emulation_core stub. Does not execute the opcodes
//! semantically — returns a fixed cycle cost per opcode kind and leaves
//! host memory unchanged. Real computation is post-v1.

use slugarch_backend::DispatchCmd;

/// Returns the simulated cycle cost of executing this emu dispatch.
/// Matches the opcode table in
/// vendor/gemma-generated/generated/ptx_emulation_core/runtime/ptx_emulation_core.json:
///   1..=13: bit ops (1 cycle)
///   14..=16: abs/min/max (1 cycle)
///   17..=23: transcendentals (4 cycles — reasonable SPU cost)
///   254: control-flow sentinel added by ControlLowerer (0 cycles)
///   anything else: 1 cycle
pub fn cycle_cost(opcode: u32) -> u64 {
    match opcode {
        17..=23 => 4,
        254 => 0,
        _ => 1,
    }
}

/// Execute a DispatchCmd on the CPU emulation core. v1: no-op except for
/// cycle accounting — host memory is left unchanged.
pub fn execute(cmd: &DispatchCmd, _host_mem: &mut [u8]) -> u64 {
    cycle_cost(cmd.opcode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcendental_opcodes_cost_4_cycles() {
        assert_eq!(cycle_cost(17), 4); // sqrt
        assert_eq!(cycle_cost(23), 4); // ex2
    }

    #[test]
    fn bit_op_opcodes_cost_1_cycle() {
        assert_eq!(cycle_cost(2), 1); // and
        assert_eq!(cycle_cost(4), 1); // xor
    }

    #[test]
    fn control_flow_sentinel_is_free() {
        assert_eq!(cycle_cost(254), 0);
    }

    #[test]
    fn unknown_opcode_costs_1_cycle() {
        assert_eq!(cycle_cost(255), 1);
        assert_eq!(cycle_cost(99_999), 1);
    }
}
