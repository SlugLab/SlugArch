use std::path::PathBuf;
use std::process::Command;

const IPS: &[&str] = &[
    "systolic_array_4x4",
    "systolic_array_16x16",
    "systolic_array_32x32",
    "npu_array_v4_seed_g",
    "npu_cluster_v4",
    "noc_mesh",
    "gemm_ip",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let vendor_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("vendor")
        .join("gemma-generated");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let verilator = std::env::var("VERILATOR")
        .unwrap_or_else(|_| "/home/victoryang00/tools/verilator/bin/verilator".to_string());

    for ip in IPS {
        let obj = out_dir.join(format!("bench_obj_{}", ip));
        let top = format!("gemma_codegen_{}_df_tb", ip);
        let filelist = vendor_root.join(format!("generated/{}/hardware/{}.f", ip, ip));
        let status = Command::new(&verilator)
            .args([
                "--binary",
                "--timing",
                "-O1",
                "--Mdir",
                obj.to_str().unwrap(),
                "-Irtl/designs",
                "-f",
                filelist.to_str().unwrap(),
                "--top-module",
                &top,
                "-Wno-UNUSED",
                "-Wno-UNUSEDSIGNAL",
                "-Wno-WIDTH",
                "-Wno-TIMESCALEMOD",
                "-Wno-MODDUP",
                "-Wno-IMPORTSTAR",
                "-Wno-CASEINCOMPLETE",
                "-Wno-INITIALDLY",
            ])
            .current_dir(&vendor_root)
            .status()
            .expect("verilator invocation failed");
        if !status.success() {
            panic!("verilator --binary failed for {}", ip);
        }
        let bin = obj.join(format!("V{}", top));
        if !bin.exists() {
            panic!("expected binary not found: {}", bin.display());
        }
        // cargo:rustc-env keys must be valid env var names, so uppercase the IP.
        println!(
            "cargo:rustc-env=BENCH_BIN_{}={}",
            ip.to_uppercase(),
            bin.display()
        );
    }
}
