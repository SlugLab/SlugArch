use slugarch_tile_model::{EventKind, FaultCode, HomeAgent, TileEvent, LINE_BYTES};

#[test]
fn stable_fault_codes_and_line_size_match_the_external_contract() {
    assert_eq!(FaultCode::CohInvalidatePending as u32, 0x1001);
    assert_eq!(FaultCode::CohStaleVersion as u32, 0x1002);
    assert_eq!(FaultCode::CohCompletionOrder as u32, 0x1003);
    assert_eq!(FaultCode::CohFenceMissing as u32, 0x1004);
    assert_eq!(FaultCode::PolicyDigest as u32, 0x2001);
    assert_eq!(FaultCode::RecordDrop as u32, 0x2002);
    assert_eq!(LINE_BYTES, 64);
}

#[test]
fn tile_events_accept_only_bounded_tiles_and_aligned_lines() {
    let kinds = [
        EventKind::ReadShared,
        EventKind::ReadExclusive,
        EventKind::Writeback,
        EventKind::Invalidate,
        EventKind::InvalidateAck,
        EventKind::Fence,
        EventKind::Completion,
        EventKind::EpochSeal,
    ];

    for (index, kind) in kinds.into_iter().enumerate() {
        let event = TileEvent::new(7, index as u64, 100 + index as u64, 3, 0x4000, 9, kind)
            .expect("valid tile event");
        assert_eq!(event.tile_id, 7);
        assert_eq!(event.kind, kind);
    }

    let invalid_tile = TileEvent::new(64, 1, 1, 1, 0x4000, 1, EventKind::ReadShared).unwrap_err();
    assert_eq!(invalid_tile.code, 0x0001);

    let invalid_line = TileEvent::new(1, 1, 1, 1, 0x4001, 1, EventKind::ReadShared).unwrap_err();
    assert_eq!(invalid_line.code, 0x0002);
}

fn event(tile: u16, id: u64, kind: EventKind, line: u64, version: u64) -> TileEvent {
    TileEvent::new(tile, id, id + 1000, 7, line, version, kind).expect("valid test event")
}

fn publish(agent: &mut HomeAgent, tile: u16, first_id: u64, line: u64, version: u64) {
    for (offset, kind) in [
        EventKind::ReadExclusive,
        EventKind::Writeback,
        EventKind::Fence,
        EventKind::Completion,
    ]
    .into_iter()
    .enumerate()
    {
        agent
            .apply(event(tile, first_id + offset as u64, kind, line, version))
            .expect("legal publication");
    }
}

#[test]
fn private_publication_sets_visible_owner_and_version() {
    let mut agent = HomeAgent::default();
    publish(&mut agent, 2, 1, 0x8000, 1);

    let line = agent.line(0x8000).expect("published line");
    assert_eq!(line.version, 1);
    assert_eq!(line.owner_tile, Some(2));
    assert_eq!(line.last_writer_tile, Some(2));
    assert_eq!(line.visible_epoch, 7);
    assert_eq!(line.outstanding_invalidations, 0);
    assert_eq!(agent.records().len(), 4);
}

#[test]
fn read_shared_fanout_tracks_each_reader() {
    let mut agent = HomeAgent::default();
    publish(&mut agent, 0, 1, 0x9000, 4);

    agent
        .apply(event(1, 5, EventKind::ReadShared, 0x9000, 4))
        .expect("tile 1 shared read");
    agent
        .apply(event(3, 6, EventKind::ReadShared, 0x9000, 4))
        .expect("tile 3 shared read");

    let line = agent.line(0x9000).expect("shared line");
    assert_eq!(line.sharers, (1 << 0) | (1 << 1) | (1 << 3));
    assert_eq!(line.owner_tile, Some(0));
}

#[test]
fn ownership_transfer_requires_all_invalidation_acknowledgements() {
    let mut agent = HomeAgent::default();
    publish(&mut agent, 0, 1, 0xa000, 1);
    agent
        .apply(event(1, 5, EventKind::ReadShared, 0xa000, 1))
        .expect("tile 1 shared read");
    agent
        .apply(event(2, 6, EventKind::ReadExclusive, 0xa000, 1))
        .expect("tile 2 requests ownership");

    assert_eq!(
        agent
            .line(0xa000)
            .expect("line awaiting invalidations")
            .outstanding_invalidations,
        (1 << 0) | (1 << 1)
    );

    agent
        .apply(event(0, 7, EventKind::Invalidate, 0xa000, 1))
        .expect("invalidate tile 0");
    agent
        .apply(event(0, 8, EventKind::InvalidateAck, 0xa000, 1))
        .expect("tile 0 acknowledgement");
    agent
        .apply(event(1, 9, EventKind::Invalidate, 0xa000, 1))
        .expect("invalidate tile 1");
    agent
        .apply(event(1, 10, EventKind::InvalidateAck, 0xa000, 1))
        .expect("tile 1 acknowledgement");

    for (id, kind) in [
        (11, EventKind::Writeback),
        (12, EventKind::Fence),
        (13, EventKind::Completion),
    ] {
        agent
            .apply(event(2, id, kind, 0xa000, 2))
            .expect("legal ownership transfer");
    }

    let line = agent.line(0xa000).expect("transferred line");
    assert_eq!(line.version, 2);
    assert_eq!(line.owner_tile, Some(2));
    assert_eq!(line.sharers, 1 << 2);
    assert_eq!(line.outstanding_invalidations, 0);
}

#[test]
fn identical_legal_traces_serialize_to_identical_records() {
    let mut first = HomeAgent::default();
    let mut second = HomeAgent::default();
    publish(&mut first, 4, 1, 0xb000, 8);
    publish(&mut second, 4, 1, 0xb000, 8);

    assert_eq!(
        serde_json::to_vec(first.records()).expect("serialize first trace"),
        serde_json::to_vec(second.records()).expect("serialize second trace")
    );
}
