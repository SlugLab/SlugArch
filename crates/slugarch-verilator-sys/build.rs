fn main() {
    // Task 3 adds the real Verilator invocation. For now build.rs is a no-op
    // so the workspace compiles.
    println!("cargo:rerun-if-changed=build.rs");
}
