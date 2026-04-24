use slugarch_host::{CxlHost, GemmJob};

#[test]
fn identity_times_constant_matches_expected() {
    let job = GemmJob {
        a: [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
        b: [
            [2, 3, 4, 5],
            [6, 7, 8, 9],
            [10, 11, 12, 13],
            [14, 15, 16, 17],
        ],
    };
    let mut host = CxlHost::new();
    let result = host.run_gemm(&job).unwrap();

    // I * B = B
    assert_eq!(
        result.c,
        [
            [2, 3, 4, 5],
            [6, 7, 8, 9],
            [10, 11, 12, 13],
            [14, 15, 16, 17],
        ],
        "expected I*B=B, got {:?}",
        result.c
    );
    assert!(result.cycles > 50, "cycles too low: {}", result.cycles);
    assert!(result.cycles < 5000, "cycles too high: {}", result.cycles);
    assert_eq!(result.flits_sent, 49);
    assert_eq!(result.flits_received, 49);
}
