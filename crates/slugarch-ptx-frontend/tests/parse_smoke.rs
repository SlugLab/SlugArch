const TRIVIAL_PTX: &str = r#"
.version 7.0
.target sm_70
.address_size 64

.visible .entry trivial() {
    ret;
}
"#;

#[test]
fn trivial_ptx_parses() {
    let module =
        slugarch_ptx_frontend::parse_ptx_raw(TRIVIAL_PTX).expect("trivial PTX should parse");
    assert_eq!(module.version, (7, 0));
    assert_eq!(module.invalid_directives, 0);
    assert!(!module.directives.is_empty());
}
