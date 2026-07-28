#[path = "../build_support.rs"]
mod build_support;

#[test]
fn environment_root_overrides_stale_compiled_default() {
    let report = r#"
Compiled in defaults if not in environment:
    VERILATOR_ROOT     = /home/blaise/tools/verilator/share/verilator
Environment:
    VERILATOR_ROOT     = /home/victoryang00/tools/verilator/share/verilator
"#;

    assert_eq!(
        build_support::parse_verilator_root(report),
        Some("/home/victoryang00/tools/verilator/share/verilator")
    );
}
