use slugarch_tile_model::{
    generate_workload, HomeAgent, WorkloadKind, WorkloadTrace, WORKLOAD_SEED,
};
use std::collections::{BTreeMap, BTreeSet};

const TILE_COUNTS: [u16; 4] = [1, 2, 4, 8];
const WORKLOADS: [WorkloadKind; 4] = [
    WorkloadKind::PrivatePartitions,
    WorkloadKind::ReadSharedFanout,
    WorkloadKind::ProducerConsumer,
    WorkloadKind::HotLinePingPong,
];

fn counts_by_tile(trace: &WorkloadTrace, measured: bool) -> BTreeMap<u16, usize> {
    let events = if measured {
        &trace.measured
    } else {
        &trace.warmup
    };
    let mut counts = BTreeMap::new();
    for event in events {
        *counts.entry(event.tile_id).or_default() += 1;
    }
    counts
}

fn assert_legal(events: &[slugarch_tile_model::TileEvent]) {
    let mut agent = HomeAgent::default();
    for event in events {
        agent
            .apply(event.clone())
            .unwrap_or_else(|error| panic!("generated illegal event {event:?}: {error}"));
    }
}

#[test]
fn every_workload_has_the_exact_per_tile_event_budget() {
    for workload in WORKLOADS {
        for tiles in TILE_COUNTS {
            let trace =
                generate_workload(workload, tiles, 100, 10_000, WORKLOAD_SEED).expect("trace");
            assert_eq!(trace.warmup.len(), usize::from(tiles) * 100);
            assert_eq!(trace.measured.len(), usize::from(tiles) * 10_000);
            assert_eq!(
                counts_by_tile(&trace, false)
                    .values()
                    .copied()
                    .collect::<Vec<_>>(),
                vec![100; usize::from(tiles)]
            );
            assert_eq!(
                counts_by_tile(&trace, true)
                    .values()
                    .copied()
                    .collect::<Vec<_>>(),
                vec![10_000; usize::from(tiles)]
            );
        }
    }
}

#[test]
fn generated_identifiers_are_unique_and_deterministic() {
    let first = generate_workload(
        WorkloadKind::ProducerConsumer,
        8,
        100,
        10_000,
        WORKLOAD_SEED,
    )
    .expect("first trace");
    let second = generate_workload(
        WorkloadKind::ProducerConsumer,
        8,
        100,
        10_000,
        WORKLOAD_SEED,
    )
    .expect("second trace");
    assert_eq!(first, second);

    let mut identities = BTreeSet::new();
    for event in first.warmup.iter().chain(&first.measured) {
        assert!(identities.insert((event.tile_id, event.event_id, event.request_id)));
    }
}

#[test]
fn every_generated_phase_is_legal_under_the_home_agent() {
    for workload in WORKLOADS {
        for tiles in TILE_COUNTS {
            let trace =
                generate_workload(workload, tiles, 100, 10_000, WORKLOAD_SEED).expect("trace");
            assert_legal(&trace.warmup);
            assert_legal(&trace.measured);
        }
    }
}

#[test]
fn unsupported_tile_counts_are_rejected() {
    for tiles in [0, 3, 9, 64] {
        let error = generate_workload(
            WorkloadKind::PrivatePartitions,
            tiles,
            100,
            10_000,
            WORKLOAD_SEED,
        )
        .unwrap_err();
        assert_eq!(error.code, 0x0005);
    }
}
