//! slugarch-bench — runs the Verilator-compiled smoke_tb for each RTL IP
//! and reports pass/fail. Used as a Tier 3 regression harness before
//! changes to the vendored RTL.

use std::process::Command;

const IPS: &[(&str, &str)] = &[
    ("systolic_array_4x4", env!("BENCH_BIN_SYSTOLIC_ARRAY_4X4")),
    (
        "systolic_array_16x16",
        env!("BENCH_BIN_SYSTOLIC_ARRAY_16X16"),
    ),
    (
        "systolic_array_32x32",
        env!("BENCH_BIN_SYSTOLIC_ARRAY_32X32"),
    ),
    ("npu_array_v4_seed_g", env!("BENCH_BIN_NPU_ARRAY_V4_SEED_G")),
    ("npu_cluster_v4", env!("BENCH_BIN_NPU_CLUSTER_V4")),
    ("noc_mesh", env!("BENCH_BIN_NOC_MESH")),
    ("gemm_ip", env!("BENCH_BIN_GEMM_IP")),
];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let filter: Option<&str> = args.first().map(|s| s.as_str());

    let mut pass = 0usize;
    let mut fail = 0usize;
    for (ip, bin) in IPS {
        if filter.map_or(false, |f| !ip.contains(f)) {
            continue;
        }
        eprint!("  {:25} ... ", ip);
        let out = Command::new(bin).output()?;
        let status = out.status;
        let last_line = String::from_utf8_lossy(&out.stdout)
            .lines()
            .last()
            .unwrap_or("")
            .to_string();
        if status.success() {
            eprintln!("OK  ({})", last_line);
            pass += 1;
        } else {
            eprintln!("FAIL (code {})", status.code().unwrap_or(-1));
            eprintln!("    stdout: {}", String::from_utf8_lossy(&out.stdout));
            eprintln!("    stderr: {}", String::from_utf8_lossy(&out.stderr));
            fail += 1;
        }
    }
    println!("bench: {} pass, {} fail", pass, fail);
    if fail > 0 {
        std::process::exit(1);
    }
    Ok(())
}
