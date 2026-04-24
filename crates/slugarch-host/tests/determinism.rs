use slugarch_host::{CxlHost, GemmJob};

#[test]
fn running_same_job_twice_is_byte_identical() {
    let job = GemmJob {
        a: [[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12], [13, 14, 15, 16]],
        b: [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
    };

    let mut host1 = CxlHost::new();
    let r1 = host1.run_gemm(&job).unwrap();

    let mut host2 = CxlHost::new();
    let r2 = host2.run_gemm(&job).unwrap();

    assert_eq!(r1, r2, "GemmResult must be byte-identical across runs");
}
