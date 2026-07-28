use std::path::{Path, PathBuf};
use std::process::Command;

mod build_support;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_support.rs");
    println!("cargo:rerun-if-changed=shim/ip_shim.h");
    println!("cargo:rerun-if-changed=shim/ip_shim.cpp");
    println!("cargo:rerun-if-env-changed=VERILATOR");
    println!("cargo:rerun-if-env-changed=VERILATOR_ROOT");
    println!("cargo:rerun-if-env-changed=VERILATOR_INCLUDE");

    let vendor_root = slugarch_path::gemma_generated_root();
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let verilator_bin = std::env::var("VERILATOR").unwrap_or_else(|_| "verilator".to_string());
    let verilator_include = find_verilator_include(&verilator_bin);

    for ip in IPS {
        verilate_ip(&verilator_bin, &vendor_root, &out_dir, ip);
    }
    verilate_slugcxl(&verilator_bin, &vendor_root, &out_dir);
    verilate_slugcxl_hj(&verilator_bin, &vendor_root, &out_dir);
    compile_shim(&out_dir, &verilator_include);
    generate_bindings(&out_dir);
}

const IPS: &[&str] = &[
    "systolic_array_4x4",
    "systolic_array_16x16",
    "systolic_array_32x32",
    "npu_array_v4_seed_g",
    "npu_cluster_v4",
    "noc_mesh",
    "gemm_ip",
];

const SLUGCXL_TOP: &str = "slugcxl_4x4_top";
const SLUGCXL_HJ_TOP: &str = "slugcxl_4x4_hj_top";

fn verilate_ip(verilator_bin: &str, vendor_root: &Path, out_dir: &Path, ip: &str) {
    let obj_dir = out_dir.join(format!("obj_dir_{}", ip));
    let wrapper_top = format!("gemma_codegen_{}_df", ip);

    let original_path = vendor_root.join(format!("generated/{}/hardware/{}.f", ip, ip));
    let original = std::fs::read_to_string(&original_path)
        .unwrap_or_else(|e| panic!("reading {}: {}", original_path.display(), e));
    println!("cargo:rerun-if-changed={}", original_path.display());
    for line in original.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        println!(
            "cargo:rerun-if-changed={}",
            vendor_root.join(line).display()
        );
    }

    let filtered: String = original
        .lines()
        .filter(|line| !line.contains("smoke_tb"))
        .collect::<Vec<_>>()
        .join("\n");
    let filelist_path = out_dir.join(format!("{}.verilator.f", ip));
    std::fs::write(&filelist_path, &filtered).unwrap();

    let status = Command::new(verilator_bin)
        .args([
            "--cc",
            "--build",
            "--no-timing",
            "-O1",
            "-CFLAGS",
            "-fPIC",
            "--Mdir",
            obj_dir.to_str().unwrap(),
            "-Irtl/designs", // NPU baseline uses `include of companion files (no space after -I)
            "-f",
            filelist_path.to_str().unwrap(),
            "--top-module",
            &wrapper_top,
            "-Wno-UNUSED",
            "-Wno-UNUSEDSIGNAL",
            "-Wno-WIDTH",
            "-Wno-TIMESCALEMOD",
            "-Wno-MODDUP", // NPU baseline `includes companion files also listed in the .f
            "-Wno-IMPORTSTAR",
            "-Wno-CASEINCOMPLETE",
            "-Wno-INITIALDLY",
        ])
        .current_dir(vendor_root)
        .status()
        .expect("failed to invoke verilator");
    if !status.success() {
        panic!("verilator failed on {}", ip);
    }

    // Verilator 5.x emits two archives in obj_dir:
    //   libV<top>.a      — the model, lib-prefixed and Rust-linkable as `V<top>`
    //   libverilated.a   — the Verilator runtime
    let libpath = obj_dir.join(format!("libV{}.a", wrapper_top));
    if !libpath.exists() {
        panic!("expected Verilator output not found: {}", libpath.display());
    }
    println!("cargo:rustc-link-search=native={}", obj_dir.display());
    println!("cargo:rustc-link-lib=static=V{}", wrapper_top);
    println!("cargo:rustc-link-lib=static=verilated");
}

fn verilate_slugcxl(verilator_bin: &str, vendor_root: &Path, out_dir: &Path) {
    let obj_dir = out_dir.join(format!("obj_dir_{}", SLUGCXL_TOP));
    let wrapper_top = SLUGCXL_TOP;

    // Filelist: the existing systolic_array_16x16 Verilog + the generated
    // slugcxl endpoint + slugcxl_4x4_top. Paths are relative to vendor_root.
    // Note: "slugcxl_4x4" in the name refers to the 4x4 GEMM matrix size
    // the host runtime operates on; internally the attached IP is the 16x16
    // systolic because its df_wrapper uses the per-cell load/compute/read
    // protocol the endpoint drives.
    let filelist_content = "rtl/designs/sovryn_pan_stem_systolic_array_16x16_baseline.v\n\
         generated/systolic_array_16x16/rtl/systolic_array_16x16_df_wrapper.sv\n\
         generated/slugcxl/slugcxl_endpoint.sv\n\
         generated/slugcxl/slugcxl_4x4_top.sv\n";
    let filelist_path = out_dir.join(format!("{}.verilator.f", SLUGCXL_TOP));
    std::fs::write(&filelist_path, filelist_content).unwrap();

    for line in filelist_content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        println!(
            "cargo:rerun-if-changed={}",
            vendor_root.join(line).display()
        );
    }

    let status = Command::new(verilator_bin)
        .args([
            "--cc",
            "--build",
            "--no-timing",
            "-O1",
            "-CFLAGS",
            "-fPIC",
            "--Mdir",
            obj_dir.to_str().unwrap(),
            "-Irtl/designs",
            "-f",
            filelist_path.to_str().unwrap(),
            "--top-module",
            wrapper_top,
            "-Wno-UNUSED",
            "-Wno-UNUSEDSIGNAL",
            "-Wno-WIDTH",
            "-Wno-TIMESCALEMOD",
            "-Wno-MODDUP",
            "-Wno-IMPORTSTAR",
            "-Wno-CASEINCOMPLETE",
            "-Wno-INITIALDLY",
        ])
        .current_dir(vendor_root)
        .status()
        .expect("failed to invoke verilator");
    if !status.success() {
        panic!("verilator failed on {}", wrapper_top);
    }

    let libpath = obj_dir.join(format!("libV{}.a", wrapper_top));
    if !libpath.exists() {
        panic!("expected Verilator output not found: {}", libpath.display());
    }
    println!("cargo:rustc-link-search=native={}", obj_dir.display());
    println!("cargo:rustc-link-lib=static=V{}", wrapper_top);
    // libverilated.a is already linked by the Gemma IPs above.
}

fn verilate_slugcxl_hj(verilator_bin: &str, vendor_root: &Path, out_dir: &Path) {
    let obj_dir = out_dir.join(format!("obj_dir_{}", SLUGCXL_HJ_TOP));
    let filelist_content = "rtl/designs/sovryn_pan_stem_systolic_array_16x16_baseline.v\n\
         generated/systolic_array_16x16/rtl/systolic_array_16x16_df_wrapper.sv\n\
         generated/slugcxl/slugcxl_endpoint.sv\n\
         generated/slugcxl/slugcxl_4x4_top.sv\n\
         generated/slugcxl/slugcxl_hj_pipeline.sv\n\
         generated/slugcxl/slugcxl_4x4_hj_top.sv\n";
    let filelist_path = out_dir.join(format!("{}.verilator.f", SLUGCXL_HJ_TOP));
    std::fs::write(&filelist_path, filelist_content).unwrap();

    for line in filelist_content.lines() {
        let line = line.trim();
        if !line.is_empty() {
            println!(
                "cargo:rerun-if-changed={}",
                vendor_root.join(line).display()
            );
        }
    }

    let status = Command::new(verilator_bin)
        .args([
            "--cc",
            "--build",
            "--no-timing",
            "-O1",
            "-CFLAGS",
            "-fPIC",
            "--Mdir",
            obj_dir.to_str().unwrap(),
            "-Irtl/designs",
            "-f",
            filelist_path.to_str().unwrap(),
            "--top-module",
            SLUGCXL_HJ_TOP,
            "-GMODEL_OBSERVE_ONLY=1",
            "-Wno-UNUSED",
            "-Wno-UNUSEDSIGNAL",
            "-Wno-WIDTH",
            "-Wno-TIMESCALEMOD",
            "-Wno-MODDUP",
            "-Wno-IMPORTSTAR",
            "-Wno-CASEINCOMPLETE",
            "-Wno-INITIALDLY",
        ])
        .current_dir(vendor_root)
        .status()
        .expect("failed to invoke verilator");
    if !status.success() {
        panic!("verilator failed on {}", SLUGCXL_HJ_TOP);
    }

    let libpath = obj_dir.join(format!("libV{}.a", SLUGCXL_HJ_TOP));
    if !libpath.exists() {
        panic!("expected Verilator output not found: {}", libpath.display());
    }
    println!("cargo:rustc-link-search=native={}", obj_dir.display());
    println!("cargo:rustc-link-lib=static=V{}", SLUGCXL_HJ_TOP);
}

fn compile_shim(out_dir: &Path, verilator_include: &str) {
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .pic(true)
        .file("shim/ip_shim.cpp")
        .include("shim")
        .include(verilator_include)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-sign-compare");
    for ip in IPS {
        build.include(out_dir.join(format!("obj_dir_{}", ip)));
    }
    build.include(out_dir.join(format!("obj_dir_{}", SLUGCXL_TOP)));
    build.include(out_dir.join(format!("obj_dir_{}", SLUGCXL_HJ_TOP)));
    build.compile("slugarch_verilator_shim");
    println!("cargo:rustc-link-lib=stdc++");
}

fn find_verilator_include(verilator_bin: &str) -> String {
    if let Ok(include) = std::env::var("VERILATOR_INCLUDE") {
        return include;
    }
    if let Ok(root) = std::env::var("VERILATOR_ROOT") {
        return PathBuf::from(root).join("include").display().to_string();
    }

    let output = Command::new(verilator_bin)
        .arg("-V")
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "failed to invoke `{}` while locating Verilator headers: {}. \
             Set VERILATOR, VERILATOR_ROOT, or VERILATOR_INCLUDE.",
                verilator_bin, e
            )
        });
    if !output.status.success() {
        panic!(
            "`{} -V` failed while locating Verilator headers. \
             Set VERILATOR_ROOT or VERILATOR_INCLUDE.",
            verilator_bin
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(root) = build_support::parse_verilator_root(&stdout) {
        return PathBuf::from(root).join("include").display().to_string();
    }

    panic!(
        "could not parse VERILATOR_ROOT from `{} -V`; set VERILATOR_INCLUDE",
        verilator_bin
    );
}

fn generate_bindings(out_dir: &Path) {
    let bindings = bindgen::Builder::default()
        .header("shim/ip_shim.h")
        .clang_arg("-x")
        .clang_arg("c")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen failed");
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("failed to write bindings.rs");
}
