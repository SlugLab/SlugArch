//! Emits slugcxl_endpoint_runtime.json — documents the FLIT layout and
//! opcode encoding so Rust and RTL have a single source of truth.

use crate::config::CxlEndpointConfig;
use serde::Serialize;

#[derive(Serialize)]
struct Runtime<'a> {
    schema: &'static str,
    name: &'a str,
    flit_bytes: u32,
    flit_layout: FlitLayout,
    record_layout: RecordLayout,
    transport_limits: TransportLimits,
    classes: Vec<Class>,
    address_spaces: &'a Vec<crate::config::AddressSpace>,
    attached_wrapper: &'a crate::config::AttachedWrapper,
    hardware_jit: &'a crate::config::HardwareJitConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    policy_hex: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    policy_json: Option<&'a str>,
}

#[derive(Serialize)]
struct FlitLayout {
    class_opcode_byte: u32,
    tag_bytes: [u32; 2],
    addr_bytes: [u32; 2],
    data_bytes: [u32; 2],
    event_id_bytes: [u32; 2],
    phase_id_bytes: [u32; 2],
    status_bytes: [u32; 2],
    control_byte: u32,
    payload_len_bits: [u32; 2],
    reserved_bit: u32,
    direction_bit: u32,
}

#[derive(Serialize)]
struct RecordLayout {
    record_bytes: u32,
    header_bytes: u32,
    version_bytes: [u32; 2],
    sequence_bytes: [u32; 2],
    event_id_bytes: [u32; 2],
    policy_digest_bytes: [u32; 2],
    epoch_bytes: [u32; 2],
    event_fields_bytes: [u32; 2],
    address_bytes: [u32; 2],
    tag_bytes: [u32; 2],
    status_bytes: [u32; 2],
    capture_header_bytes: [u32; 2],
    reserved_bytes: [u32; 2],
    capture_data_bytes: [u32; 2],
}

#[derive(Serialize)]
struct TransportLimits {
    local_debug_transport: bool,
    standards_compliant_cxl_flit: bool,
    max_payload_bytes: u32,
    max_delta_pairs: u32,
    max_tag_bits: u32,
    max_opcode_bits: u32,
    fixed_policy_event_classes: [&'static str; 4],
    match_opcode_supported: bool,
}

#[derive(Serialize)]
struct Class {
    name: &'static str,
    value: u8,
    opcodes: Vec<Opcode>,
}

#[derive(Serialize)]
struct Opcode {
    name: &'static str,
    value: u8,
}

pub fn emit(cfg: &CxlEndpointConfig, policy_artifacts: bool) -> String {
    let runtime = Runtime {
        schema: "slugcxl.runtime.v1",
        name: &cfg.name,
        flit_bytes: 64,
        flit_layout: FlitLayout {
            class_opcode_byte: 0,
            tag_bytes: [1, 2],
            addr_bytes: [3, 10],
            data_bytes: [11, 42],
            event_id_bytes: [43, 50],
            phase_id_bytes: [51, 58],
            status_bytes: [59, 62],
            control_byte: 63,
            payload_len_bits: [0, 5],
            reserved_bit: 6,
            direction_bit: 7,
        },
        record_layout: RecordLayout {
            record_bytes: 128,
            header_bytes: 96,
            version_bytes: [0, 3],
            sequence_bytes: [4, 11],
            event_id_bytes: [12, 19],
            policy_digest_bytes: [20, 51],
            epoch_bytes: [52, 59],
            event_fields_bytes: [60, 63],
            address_bytes: [64, 71],
            tag_bytes: [72, 79],
            status_bytes: [80, 83],
            capture_header_bytes: [84, 87],
            reserved_bytes: [88, 95],
            capture_data_bytes: [96, 127],
        },
        transport_limits: TransportLimits {
            local_debug_transport: true,
            standards_compliant_cxl_flit: false,
            max_payload_bytes: 32,
            max_delta_pairs: 16,
            max_tag_bits: 16,
            max_opcode_bits: 4,
            fixed_policy_event_classes: [
                "cxl_mem_read",
                "cxl_mem_write",
                "cxl_mem_data",
                "completion",
            ],
            match_opcode_supported: false,
        },
        classes: vec![
            Class {
                name: "M2SReq",
                value: 0x1,
                opcodes: vec![
                    Opcode {
                        name: "MemRd",
                        value: 0x0,
                    },
                    Opcode {
                        name: "MemRdData",
                        value: 0x1,
                    },
                    Opcode {
                        name: "MemInv",
                        value: 0x2,
                    },
                ],
            },
            Class {
                name: "M2SRwD",
                value: 0x2,
                opcodes: vec![
                    Opcode {
                        name: "MemWr",
                        value: 0x0,
                    },
                    Opcode {
                        name: "MemWrPtl",
                        value: 0x1,
                    },
                    Opcode {
                        name: "MemClnEvct",
                        value: 0x2,
                    },
                ],
            },
            Class {
                name: "S2MDRS",
                value: 0x3,
                opcodes: vec![
                    Opcode {
                        name: "MemData",
                        value: 0x0,
                    },
                    Opcode {
                        name: "MemDataNxm",
                        value: 0x1,
                    },
                ],
            },
            Class {
                name: "S2MNDR",
                value: 0x4,
                opcodes: vec![
                    Opcode {
                        name: "Cmp",
                        value: 0x0,
                    },
                    Opcode {
                        name: "CmpS",
                        value: 0x1,
                    },
                    Opcode {
                        name: "CmpE",
                        value: 0x2,
                    },
                    Opcode {
                        name: "CmpI",
                        value: 0x3,
                    },
                    Opcode {
                        name: "MemPassDirty",
                        value: 0x4,
                    },
                    Opcode {
                        name: "DispatchFailed",
                        value: 0xF,
                    },
                ],
            },
        ],
        address_spaces: &cfg.address_spaces,
        attached_wrapper: &cfg.attached_wrapper,
        hardware_jit: &cfg.hardware_jit,
        policy_hex: policy_artifacts.then_some("slugcxl_hj_policy.hex"),
        policy_json: policy_artifacts.then_some("slugcxl_hj_policy.json"),
    };
    serde_json::to_string_pretty(&runtime).expect("serialize runtime")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_runtime_json() {
        let cfg = CxlEndpointConfig::slugcxl_4x4();
        let j = emit(&cfg, false);
        insta::assert_snapshot!(j);
    }
}
