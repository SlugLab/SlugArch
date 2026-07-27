use slugarch_jit::{Policy, RecordMode, SLUG_JIT_ABI_VERSION};

const POLICY: &str = r#"{
  "version":1,
  "name":"validation-cxlmem",
  "allowed_classes":["cxl_mem_read","cxl_mem_write","cxl_mem_data","completion"],
  "ranges":[{"base":83886080,"length":33554432}],
  "sample_stride":1,
  "record_mode":"validation",
  "metadata_budget":256,
  "epoch_policy":"phase",
  "rules":[
    {"op":"capture","mode":"validation"},
    {"op":"emit"},
    {"op":"epoch_from_phase"},
    {"op":"halt"}
  ]
}"#;

#[test]
fn strict_v1_policy_parses() {
    let policy = Policy::parse(POLICY.as_bytes()).unwrap();
    assert_eq!(policy.version, SLUG_JIT_ABI_VERSION);
    assert_eq!(policy.record_mode, RecordMode::Validation);
}

#[test]
fn unknown_field_is_rejected() {
    let bad = POLICY.replace("\"version\":1,", "\"version\":1,\"surprise\":7,");
    assert!(Policy::parse(bad.as_bytes()).is_err());
}

#[test]
fn zero_length_range_and_trailing_data_are_rejected() {
    let zero = POLICY.replace("\"length\":33554432", "\"length\":0");
    assert!(Policy::parse(zero.as_bytes()).is_err());

    let trailing = format!("{POLICY}x");
    assert!(Policy::parse(trailing.as_bytes()).is_err());
}
