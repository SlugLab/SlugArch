fn main() {
    println!("cargo:rerun-if-changed=include/slugarch_jit.h");
    println!("cargo:rerun-if-changed=tests/c_smoke.c");

    cc::Build::new()
        .file("tests/c_smoke.c")
        .include("include")
        .flag_if_supported("-std=c11")
        .warnings(true)
        .warnings_into_errors(true)
        .compile("slugarch_jit_c_smoke_compile");
}
