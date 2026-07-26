use slugarch_tile_model::{
    first_fault, generate_workload, inject_one, EventKind, FaultCode, FaultKind, TileEvent,
    WorkloadKind, WorkloadTrace, WORKLOAD_SEED,
};

fn trace_for(kind: FaultKind) -> (WorkloadTrace, TileEvent) {
    let workload = match kind {
        FaultKind::MissingInvalidateAck | FaultKind::ReorderedCompletion => {
            WorkloadKind::HotLinePingPong
        }
        FaultKind::StaleLineVersion | FaultKind::FenceOmission => WorkloadKind::ProducerConsumer,
        FaultKind::PolicyDigestMismatch | FaultKind::RequiredRecordDrop => {
            WorkloadKind::PrivatePartitions
        }
    };
    let trace = generate_workload(workload, 2, 100, 100, WORKLOAD_SEED).expect("legal trace");
    let target = trace
        .measured
        .iter()
        .find(|event| match kind {
            FaultKind::MissingInvalidateAck => event.kind == EventKind::InvalidateAck,
            FaultKind::StaleLineVersion => event.kind == EventKind::ReadShared && event.version > 0,
            FaultKind::ReorderedCompletion => event.kind == EventKind::Completion,
            FaultKind::FenceOmission => event.kind == EventKind::Fence,
            FaultKind::PolicyDigestMismatch | FaultKind::RequiredRecordDrop => true,
        })
        .expect("eligible injection event")
        .clone();
    (trace, target)
}

fn expected_code(kind: FaultKind) -> FaultCode {
    match kind {
        FaultKind::MissingInvalidateAck => FaultCode::CohInvalidatePending,
        FaultKind::StaleLineVersion => FaultCode::CohStaleVersion,
        FaultKind::ReorderedCompletion => FaultCode::CohCompletionOrder,
        FaultKind::FenceOmission => FaultCode::CohFenceMissing,
        FaultKind::PolicyDigestMismatch => FaultCode::PolicyDigest,
        FaultKind::RequiredRecordDrop => FaultCode::RecordDrop,
    }
}

#[test]
fn every_declared_fault_has_the_exact_first_failure() {
    for kind in FaultKind::ALL {
        let (trace, target) = trace_for(kind);
        let faulted =
            inject_one(&trace, kind, target.tile_id, target.event_id).expect("valid injection");
        let observed = first_fault(&faulted).expect("fault must be detected");

        assert_eq!(observed.code, expected_code(kind));
        assert_eq!(observed, faulted.expected_failure);
        assert_eq!(faulted.injected_tile_id, target.tile_id);
        assert_eq!(faulted.injected_event_id, target.event_id);
        assert_ne!(
            faulted.original_event_sha256,
            faulted.transformed_event_sha256
        );
    }
}

#[test]
fn first_failure_is_stable_across_five_fresh_evaluations() {
    for kind in FaultKind::ALL {
        let (trace, target) = trace_for(kind);
        let faulted =
            inject_one(&trace, kind, target.tile_id, target.event_id).expect("valid injection");
        let expected = faulted.expected_failure.clone();
        for _ in 0..5 {
            assert_eq!(first_fault(&faulted), Some(expected.clone()));
        }
    }
}

#[test]
fn injection_rejects_an_event_that_cannot_express_the_fault() {
    let trace = generate_workload(WorkloadKind::ReadSharedFanout, 2, 100, 100, WORKLOAD_SEED)
        .expect("legal trace");
    let target = trace.measured.first().expect("first event");
    let error = inject_one(
        &trace,
        FaultKind::MissingInvalidateAck,
        target.tile_id,
        target.event_id,
    )
    .unwrap_err();
    assert_eq!(error.code, 0x0006);
}
