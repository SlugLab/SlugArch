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
    classes: Vec<Class>,
    address_spaces: &'a Vec<crate::config::AddressSpace>,
    attached_wrapper: &'a crate::config::AttachedWrapper,
}

#[derive(Serialize)]
struct FlitLayout {
    class_opcode_byte: u32,
    tag_bytes: [u32; 2],
    addr_bytes: [u32; 2],
    data_bytes: [u32; 2],
    reserved_bytes: [u32; 2],
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

pub fn emit(cfg: &CxlEndpointConfig) -> String {
    let runtime = Runtime {
        schema: "slugcxl.runtime.v1",
        name: &cfg.name,
        flit_bytes: 64,
        flit_layout: FlitLayout {
            class_opcode_byte: 0,
            tag_bytes: [1, 2],
            addr_bytes: [3, 10],
            data_bytes: [11, 42],
            reserved_bytes: [43, 63],
        },
        classes: vec![
            Class { name: "M2SReq", value: 0x1, opcodes: vec![
                Opcode { name: "MemRd", value: 0x0 },
                Opcode { name: "MemRdData", value: 0x1 },
                Opcode { name: "MemInv", value: 0x2 },
            ]},
            Class { name: "M2SRwD", value: 0x2, opcodes: vec![
                Opcode { name: "MemWr", value: 0x0 },
                Opcode { name: "MemWrPtl", value: 0x1 },
                Opcode { name: "MemClnEvct", value: 0x2 },
            ]},
            Class { name: "S2MDRS", value: 0x3, opcodes: vec![
                Opcode { name: "MemData", value: 0x0 },
                Opcode { name: "MemDataNxm", value: 0x1 },
            ]},
            Class { name: "S2MNDR", value: 0x4, opcodes: vec![
                Opcode { name: "Cmp", value: 0x0 },
                Opcode { name: "CmpS", value: 0x1 },
                Opcode { name: "CmpE", value: 0x2 },
                Opcode { name: "CmpI", value: 0x3 },
                Opcode { name: "MemPassDirty", value: 0x4 },
                Opcode { name: "DispatchFailed", value: 0xF },
            ]},
        ],
        address_spaces: &cfg.address_spaces,
        attached_wrapper: &cfg.attached_wrapper,
    };
    serde_json::to_string_pretty(&runtime).expect("serialize runtime")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_runtime_json() {
        let cfg = CxlEndpointConfig::slugcxl_4x4();
        let j = emit(&cfg);
        insta::assert_snapshot!(j);
    }
}
