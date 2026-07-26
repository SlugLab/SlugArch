use slugarch_tile_model::{EventKind, FaultCode, TileEvent, LINE_BYTES};

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
