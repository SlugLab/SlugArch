use slugarch_cxl_wire::{encode, CxlMsg, S2MNDROp, FLIT_BYTES};
use slugarch_host::qemu_type2::{export_requests, validate_responses};
use slugarch_host::GemmJob;
use std::fs;

fn job() -> GemmJob {
    GemmJob {
        a: [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
        b: [
            [2, 3, 4, 5],
            [6, 7, 8, 9],
            [10, 11, 12, 13],
            [14, 15, 16, 17],
        ],
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "slugarch-qemu-type2-{}-{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn export_writes_49_request_flits_and_expected_json() {
    let dir = temp_dir("export");
    let expected = export_requests(&job(), &dir).unwrap();
    let requests = fs::read(dir.join("requests.bin")).unwrap();
    assert_eq!(requests.len(), 49 * FLIT_BYTES);
    assert_eq!(expected.request_count, 49);
    assert_eq!(expected.expected_c[0], [2, 3, 4, 5]);
    assert!(dir.join("expected.json").exists());
}

#[test]
fn validate_accepts_known_good_response_stream() {
    let dir = temp_dir("validate-good");
    export_requests(&job(), &dir).unwrap();
    let response_bytes = known_good_response_stream();
    fs::write(dir.join("responses.bin"), response_bytes).unwrap();
    let summary = validate_responses(&job(), &dir.join("responses.bin"), &dir).unwrap();
    assert_eq!(summary.status, "pass");
    assert_eq!(summary.response_count, 49);
    assert_eq!(summary.tag_mismatches, 0);
    assert_eq!(summary.result_c[3], [14, 15, 16, 17]);
}

#[test]
fn validate_rejects_truncated_responses() {
    let dir = temp_dir("truncated");
    fs::write(dir.join("responses.bin"), vec![0u8; FLIT_BYTES - 1]).unwrap();
    let err = validate_responses(&job(), &dir.join("responses.bin"), &dir).unwrap_err();
    assert!(format!("{err}").contains("multiple of 64"));
}

fn known_good_response_stream() -> Vec<u8> {
    let mut bytes = Vec::new();
    for tag in 0..33u16 {
        bytes.extend_from_slice(&encode(&CxlMsg::S2MNDR {
            tag,
            opcode: S2MNDROp::Cmp,
        }));
    }
    let expected = [
        [2u32, 3, 4, 5],
        [6, 7, 8, 9],
        [10, 11, 12, 13],
        [14, 15, 16, 17],
    ];
    for (i, value) in expected.iter().flatten().enumerate() {
        let mut data = [0u8; 32];
        data[0] = (*value & 0xff) as u8;
        data[1] = ((*value >> 8) & 0xff) as u8;
        data[2] = ((*value >> 16) & 0xff) as u8;
        bytes.extend_from_slice(&encode(&CxlMsg::S2MDRS {
            tag: 33 + i as u16,
            opcode: slugarch_cxl_wire::S2MDRSOp::MemData,
            data,
        }));
    }
    bytes
}
