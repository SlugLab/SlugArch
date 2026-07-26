use crate::{EventKind, ModelError, TileEvent, LINE_BYTES};
use serde::{Deserialize, Serialize};

pub const WORKLOAD_SEED: u64 = 0x534c_5547_5449_4c45;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkloadKind {
    PrivatePartitions,
    ReadSharedFanout,
    ProducerConsumer,
    HotLinePingPong,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadTrace {
    pub warmup: Vec<TileEvent>,
    pub measured: Vec<TileEvent>,
    pub seed: u64,
}

pub fn generate_workload(
    kind: WorkloadKind,
    tiles: u16,
    warmup_per_tile: u64,
    measured_per_tile: u64,
    seed: u64,
) -> Result<WorkloadTrace, ModelError> {
    if !matches!(tiles, 1 | 2 | 4 | 8) {
        return Err(ModelError::new(
            0x0005,
            tiles,
            0,
            0,
            "tile count must be one of 1, 2, 4, or 8",
        ));
    }

    let mut next_event_id = 0;
    let warmup = generate_phase(
        kind,
        tiles,
        warmup_per_tile,
        1,
        0x1000_0000,
        &mut next_event_id,
    )?;
    let measured = generate_phase(
        kind,
        tiles,
        measured_per_tile,
        2,
        0x2000_0000,
        &mut next_event_id,
    )?;
    Ok(WorkloadTrace {
        warmup,
        measured,
        seed,
    })
}

fn generate_phase(
    kind: WorkloadKind,
    tiles: u16,
    events_per_tile: u64,
    epoch: u64,
    base: u64,
    next_event_id: &mut u64,
) -> Result<Vec<TileEvent>, ModelError> {
    match kind {
        WorkloadKind::PrivatePartitions => {
            private_partitions(tiles, events_per_tile, epoch, base, next_event_id)
        }
        WorkloadKind::ReadSharedFanout => {
            read_shared_fanout(tiles, events_per_tile, epoch, base, next_event_id)
        }
        WorkloadKind::ProducerConsumer => {
            producer_consumer(tiles, events_per_tile, epoch, base, next_event_id)
        }
        WorkloadKind::HotLinePingPong => {
            hot_line_ping_pong(tiles, events_per_tile, epoch, base, next_event_id)
        }
    }
}

fn private_partitions(
    tiles: u16,
    target: u64,
    epoch: u64,
    base: u64,
    next_event_id: &mut u64,
) -> Result<Vec<TileEvent>, ModelError> {
    let mut events = Vec::with_capacity(usize::from(tiles) * target as usize);
    let complete_publications = target / 4;
    for operation in 0..complete_publications {
        for tile in 0..tiles {
            let line = base + u64::from(tile) * 0x10_0000 + operation * LINE_BYTES;
            push(
                &mut events,
                tile,
                EventKind::ReadExclusive,
                line,
                0,
                epoch,
                next_event_id,
            )?;
            push(
                &mut events,
                tile,
                EventKind::Writeback,
                line,
                1,
                epoch,
                next_event_id,
            )?;
            push(
                &mut events,
                tile,
                EventKind::Fence,
                line,
                1,
                epoch,
                next_event_id,
            )?;
            push(
                &mut events,
                tile,
                EventKind::Completion,
                line,
                1,
                epoch,
                next_event_id,
            )?;
        }
    }
    pad_shared_reads(
        &mut events,
        tiles,
        target % 4,
        epoch,
        base + 0x0f00_0000,
        0,
        next_event_id,
    )?;
    Ok(events)
}

fn read_shared_fanout(
    tiles: u16,
    target: u64,
    epoch: u64,
    base: u64,
    next_event_id: &mut u64,
) -> Result<Vec<TileEvent>, ModelError> {
    let mut events = Vec::with_capacity(usize::from(tiles) * target as usize);
    for _ in 0..target {
        for tile in 0..tiles {
            push(
                &mut events,
                tile,
                EventKind::ReadShared,
                base,
                0,
                epoch,
                next_event_id,
            )?;
        }
    }
    Ok(events)
}

fn producer_consumer(
    tiles: u16,
    target: u64,
    epoch: u64,
    base: u64,
    next_event_id: &mut u64,
) -> Result<Vec<TileEvent>, ModelError> {
    let mut events = Vec::with_capacity(usize::from(tiles) * target as usize);
    let complete_rounds = target / 5;
    for round in 0..complete_rounds {
        for producer in 0..tiles {
            let consumer = (producer + 1) % tiles;
            let line = base + u64::from(producer) * 0x10_0000 + round * LINE_BYTES;
            push(
                &mut events,
                producer,
                EventKind::ReadExclusive,
                line,
                0,
                epoch,
                next_event_id,
            )?;
            push(
                &mut events,
                producer,
                EventKind::Writeback,
                line,
                1,
                epoch,
                next_event_id,
            )?;
            push(
                &mut events,
                producer,
                EventKind::Fence,
                line,
                1,
                epoch,
                next_event_id,
            )?;
            push(
                &mut events,
                producer,
                EventKind::Completion,
                line,
                1,
                epoch,
                next_event_id,
            )?;
            push(
                &mut events,
                consumer,
                EventKind::ReadShared,
                line,
                1,
                epoch,
                next_event_id,
            )?;
        }
    }
    pad_shared_reads(
        &mut events,
        tiles,
        target % 5,
        epoch,
        base + 0x0f00_0000,
        0,
        next_event_id,
    )?;
    Ok(events)
}

fn hot_line_ping_pong(
    tiles: u16,
    target: u64,
    epoch: u64,
    line: u64,
    next_event_id: &mut u64,
) -> Result<Vec<TileEvent>, ModelError> {
    let mut events = Vec::with_capacity(usize::from(tiles) * target as usize);
    let mut counts = vec![0u64; usize::from(tiles)];
    let mut owner = None;
    let mut version = 0;

    loop {
        let writer = owner.map_or(0, |current| (current + 1) % tiles);
        let mut additions = vec![0u64; usize::from(tiles)];
        additions[usize::from(writer)] += 4;
        if let Some(current) = owner {
            if current != writer {
                additions[usize::from(current)] += 2;
            }
        }
        if counts
            .iter()
            .zip(&additions)
            .any(|(count, addition)| count + addition > target)
        {
            break;
        }

        push(
            &mut events,
            writer,
            EventKind::ReadExclusive,
            line,
            version,
            epoch,
            next_event_id,
        )?;
        if let Some(current) = owner {
            if current != writer {
                push(
                    &mut events,
                    current,
                    EventKind::Invalidate,
                    line,
                    version,
                    epoch,
                    next_event_id,
                )?;
                push(
                    &mut events,
                    current,
                    EventKind::InvalidateAck,
                    line,
                    version,
                    epoch,
                    next_event_id,
                )?;
            }
        }
        version += 1;
        push(
            &mut events,
            writer,
            EventKind::Writeback,
            line,
            version,
            epoch,
            next_event_id,
        )?;
        push(
            &mut events,
            writer,
            EventKind::Fence,
            line,
            version,
            epoch,
            next_event_id,
        )?;
        push(
            &mut events,
            writer,
            EventKind::Completion,
            line,
            version,
            epoch,
            next_event_id,
        )?;

        for (count, addition) in counts.iter_mut().zip(additions) {
            *count += addition;
        }
        owner = Some(writer);
    }

    for tile in 0..tiles {
        while counts[usize::from(tile)] < target {
            push(
                &mut events,
                tile,
                EventKind::ReadShared,
                line,
                version,
                epoch,
                next_event_id,
            )?;
            counts[usize::from(tile)] += 1;
        }
    }
    Ok(events)
}

#[allow(clippy::too_many_arguments)]
fn pad_shared_reads(
    events: &mut Vec<TileEvent>,
    tiles: u16,
    reads_per_tile: u64,
    epoch: u64,
    line: u64,
    version: u64,
    next_event_id: &mut u64,
) -> Result<(), ModelError> {
    for _ in 0..reads_per_tile {
        for tile in 0..tiles {
            push(
                events,
                tile,
                EventKind::ReadShared,
                line,
                version,
                epoch,
                next_event_id,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push(
    events: &mut Vec<TileEvent>,
    tile_id: u16,
    kind: EventKind,
    line_address: u64,
    version: u64,
    epoch: u64,
    next_event_id: &mut u64,
) -> Result<(), ModelError> {
    let event_id = *next_event_id;
    *next_event_id += 1;
    events.push(TileEvent::new(
        tile_id,
        event_id,
        event_id + 1,
        epoch,
        line_address,
        version,
        kind,
    )?);
    Ok(())
}
