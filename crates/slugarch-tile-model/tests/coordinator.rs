use slugarch_tile_model::{
    EpochCoordinator, EpochStatus, EventKind, FailureRecord, FaultCode, HomeAgent, TileCounters,
    TileEvent, TileRecord,
};

const DIGEST: [u8; 32] = [0x5a; 32];

fn record(tile_id: u16, sequence: u64, event_id: u64) -> TileRecord {
    TileRecord {
        tile_id,
        record_sequence: sequence,
        event_id,
        epoch: 9,
        policy_digest: DIGEST,
        metadata_bytes: 48,
    }
}

fn event(tile_id: u16, event_id: u64, kind: EventKind) -> TileEvent {
    TileEvent::new(tile_id, event_id, event_id + 100, 9, 0x4000, 0, kind)
        .expect("valid coordinator event")
}

#[test]
fn participant_ids_must_be_unique() {
    let error = EpochCoordinator::new(9, DIGEST, [0, 1, 1]).unwrap_err();
    assert_eq!(error.code, 0x0007);
}

#[test]
fn mismatched_digest_fails_at_the_recording_tile() {
    let mut coordinator = EpochCoordinator::new(9, DIGEST, [0, 1]).expect("coordinator");
    let mut mismatched = record(1, 0, 44);
    mismatched.policy_digest = [0xa5; 32];

    let failure = coordinator
        .observe_tile_record(mismatched)
        .expect_err("digest mismatch");
    assert_eq!(
        failure,
        FailureRecord {
            code: FaultCode::PolicyDigest,
            tile_id: 1,
            event_id: 44,
            epoch: 9,
        }
    );
    assert_eq!(coordinator.status(), &EpochStatus::Failed(failure));
}

#[test]
fn successful_epoch_requires_exact_reconciled_counters_and_model_seal() {
    let mut coordinator = EpochCoordinator::new(9, DIGEST, [0, 1]).expect("coordinator");
    coordinator
        .observe_tile_record(record(0, 0, 10))
        .expect("tile 0 record");
    coordinator
        .observe_tile_record(record(1, 0, 11))
        .expect("tile 1 record");
    for tile_id in [0, 1] {
        coordinator
            .reconcile_tile_counters(
                tile_id,
                TileCounters {
                    event_count: 1,
                    record_count: 1,
                    metadata_bytes: 48,
                    reject_count: 0,
                    drop_count: 0,
                },
            )
            .expect("matching counters");
    }

    let mut model = HomeAgent::default();
    model
        .apply(event(0, 10, EventKind::ReadShared))
        .expect("tile 0 model event");
    model
        .apply(event(1, 11, EventKind::ReadShared))
        .expect("tile 1 model event");
    model
        .apply(event(0, 12, EventKind::EpochSeal))
        .expect("model seal");

    coordinator
        .seal_success(&model)
        .expect("complete global epoch");
    assert_eq!(coordinator.status(), &EpochStatus::Complete);
}

#[test]
fn required_record_drop_prevents_partial_success() {
    let mut coordinator = EpochCoordinator::new(9, DIGEST, [0, 1]).expect("coordinator");
    coordinator
        .observe_tile_record(record(0, 0, 10))
        .expect("tile 0 record");
    let failure = coordinator.record_required_drop(1, 20);

    let mut model = HomeAgent::default();
    model
        .apply(event(0, 10, EventKind::ReadShared))
        .expect("model event");
    model
        .apply(event(0, 12, EventKind::EpochSeal))
        .expect("model seal");

    assert_eq!(failure.code, FaultCode::RecordDrop);
    assert_eq!(coordinator.seal_success(&model).unwrap_err(), failure);
    assert_eq!(coordinator.status(), &EpochStatus::Failed(failure));
}

#[test]
fn first_failure_is_preserved_and_recovery_zeros_state() {
    let mut coordinator = EpochCoordinator::new(9, DIGEST, [0, 1]).expect("coordinator");
    let first = FailureRecord {
        code: FaultCode::PolicyDigest,
        tile_id: 0,
        event_id: 7,
        epoch: 9,
    };
    let second = FailureRecord {
        code: FaultCode::RecordDrop,
        tile_id: 1,
        event_id: 8,
        epoch: 9,
    };
    assert_eq!(coordinator.fail(first.clone()), first);
    assert_eq!(coordinator.fail(second), first);

    let recovered = coordinator.begin_recovery(10).expect("new epoch");
    assert_eq!(recovered.epoch, 10);
    assert_eq!(recovered.policy_digest, DIGEST);
    assert_eq!(recovered.status(), &EpochStatus::Open);
    assert!(recovered
        .participants
        .values()
        .all(|counters| counters == &TileCounters::default()));
}

#[test]
fn record_sequences_are_strictly_increasing_per_tile() {
    let mut coordinator = EpochCoordinator::new(9, DIGEST, [0]).expect("coordinator");
    coordinator
        .observe_tile_record(record(0, 0, 10))
        .expect("first record");
    let failure = coordinator
        .observe_tile_record(record(0, 2, 11))
        .expect_err("sequence gap");
    assert_eq!(failure.code as u32, 0x3001);
    assert_eq!(failure.tile_id, 0);
    assert_eq!(failure.event_id, 11);
}
