use slugarch_cxl_wire::{encode, CxlMsg, S2MDRSOp, S2MNDROp, FLIT_BYTES};
use slugarch_host::qemu_type2::{
    export_requests, validate_responses, QemuType2Expected, QemuType2Summary,
};
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
    let expected_json: QemuType2Expected =
        serde_json::from_slice(&fs::read(dir.join("expected.json")).unwrap()).unwrap();
    assert_eq!(requests.len(), 49 * FLIT_BYTES);
    assert_eq!(expected.request_count, 49);
    assert_eq!(expected.expected_c[0], [2, 3, 4, 5]);
    assert_eq!(expected_json.workload, expected.workload);
    assert_eq!(expected_json.flit_bytes, expected.flit_bytes);
    assert_eq!(expected_json.request_count, expected.request_count);
    assert_eq!(expected_json.request_tags, expected.request_tags);
    assert_eq!(expected_json.expected_c, expected.expected_c);
    assert!(dir.join("expected.json").exists());
}

#[test]
fn validate_accepts_known_good_response_stream() {
    let dir = temp_dir("validate-good");
    export_requests(&job(), &dir).unwrap();
    let response_bytes = known_good_response_stream();
    fs::write(dir.join("responses.bin"), response_bytes).unwrap();
    let summary = validate_responses(&job(), &dir.join("responses.bin"), &dir).unwrap();
    let summary_json: QemuType2Summary =
        serde_json::from_slice(&fs::read(dir.join("summary.json")).unwrap()).unwrap();
    assert_eq!(summary.status, "pass");
    assert_eq!(summary.response_count, 49);
    assert_eq!(summary.tag_mismatches, 0);
    assert_eq!(summary.result_c[3], [14, 15, 16, 17]);
    assert_eq!(summary_json.status, summary.status);
    assert_eq!(summary_json.request_count, summary.request_count);
    assert_eq!(summary_json.response_count, summary.response_count);
    assert_eq!(summary_json.tag_mismatches, summary.tag_mismatches);
    assert_eq!(summary_json.dispatch_failures, summary.dispatch_failures);
    assert_eq!(summary_json.result_c, summary.result_c);
    assert_eq!(summary_json.expected_c, summary.expected_c);
    assert_eq!(
        summary_json.requests_path,
        dir.join("requests.bin").display().to_string()
    );
    assert_eq!(
        summary_json.responses_path,
        dir.join("responses.bin").display().to_string()
    );
    assert!(dir.join("summary.json").exists());
    assert!(dir.join("summary.csv").exists());
}

#[test]
fn validate_rejects_truncated_responses() {
    let dir = temp_dir("truncated");
    fs::write(dir.join("responses.bin"), vec![0u8; FLIT_BYTES - 1]).unwrap();
    let err = validate_responses(&job(), &dir.join("responses.bin"), &dir).unwrap_err();
    assert!(format!("{err}").contains("multiple of 64"));
}

#[test]
fn validate_fails_on_bad_tag_response() {
    let summary = validate_fault_case("bad-tag", |responses| {
        responses[10] = CxlMsg::S2MNDR {
            tag: 99,
            opcode: S2MNDROp::Cmp,
        };
    });

    assert_eq!(summary.status, "fail");
    assert_eq!(summary.tag_mismatches, 1);
    assert_eq!(summary.dispatch_failures, 0);
}

#[test]
fn validate_fails_on_missing_response() {
    let summary = validate_fault_case("missing-response", |responses| {
        responses.pop();
    });

    assert_eq!(summary.status, "fail");
    assert_eq!(summary.response_count, 48);
}

#[test]
fn validate_fails_on_extra_duplicate_response() {
    let summary = validate_fault_case("extra-duplicate", |responses| {
        let duplicate = responses.last().unwrap().clone();
        responses.push(duplicate);
    });

    assert_eq!(summary.status, "fail");
    assert_eq!(summary.response_count, 50);
}

#[test]
fn validate_fails_on_dispatch_failed_response() {
    let summary = validate_fault_case("dispatch-failed", |responses| {
        responses[3] = CxlMsg::S2MNDR {
            tag: 3,
            opcode: S2MNDROp::DispatchFailed,
        };
    });

    assert_eq!(summary.status, "fail");
    assert_eq!(summary.dispatch_failures, 1);
}

#[test]
fn validate_fails_on_wrong_read_data() {
    let summary = validate_fault_case("wrong-read-data", |responses| {
        if let CxlMsg::S2MDRS { data, .. } = &mut responses[33] {
            data[0] = data[0].wrapping_add(1);
        }
    });

    assert_eq!(summary.status, "fail");
    assert_eq!(summary.tag_mismatches, 0);
    assert_eq!(summary.dispatch_failures, 0);
    assert_ne!(summary.result_c, summary.expected_c);
}

#[test]
fn validate_fails_on_wrong_response_phase() {
    let summary = validate_fault_case("wrong-response-phase", |responses| {
        responses[33] = CxlMsg::S2MNDR {
            tag: 33,
            opcode: S2MNDROp::Cmp,
        };
    });

    assert_eq!(summary.status, "fail");
    assert_eq!(summary.dispatch_failures, 1);
}

fn known_good_response_stream() -> Vec<u8> {
    encode_messages(&known_good_responses())
}

fn validate_fault_case(name: &str, mutate: impl FnOnce(&mut Vec<CxlMsg>)) -> QemuType2Summary {
    let dir = temp_dir(name);
    export_requests(&job(), &dir).unwrap();
    let mut responses = known_good_responses();
    mutate(&mut responses);
    fs::write(dir.join("responses.bin"), encode_messages(&responses)).unwrap();
    validate_responses(&job(), &dir.join("responses.bin"), &dir).unwrap()
}

fn encode_messages(messages: &[CxlMsg]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for msg in messages {
        bytes.extend_from_slice(&encode(msg));
    }
    bytes
}

fn known_good_responses() -> Vec<CxlMsg> {
    let mut responses = Vec::new();
    for tag in 0..33u16 {
        responses.push(CxlMsg::S2MNDR {
            tag,
            opcode: S2MNDROp::Cmp,
        });
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
        responses.push(CxlMsg::S2MDRS {
            tag: 33 + i as u16,
            opcode: S2MDRSOp::MemData,
            data,
        });
    }
    responses
}
