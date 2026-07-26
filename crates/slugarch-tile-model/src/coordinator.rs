use crate::{EventKind, FailureRecord, FaultCode, HomeAgent, ModelError, TileCounters};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileRecord {
    pub tile_id: u16,
    pub record_sequence: u64,
    pub event_id: u64,
    pub epoch: u64,
    pub policy_digest: [u8; 32],
    pub metadata_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpochStatus {
    Open,
    Complete,
    Failed(FailureRecord),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochCoordinator {
    pub epoch: u64,
    pub policy_digest: [u8; 32],
    pub participants: BTreeMap<u16, TileCounters>,
    status: EpochStatus,
    next_record_sequence: BTreeMap<u16, u64>,
    reconciled_tiles: BTreeSet<u16>,
}

impl EpochCoordinator {
    pub fn new(
        epoch: u64,
        policy_digest: [u8; 32],
        participants: impl IntoIterator<Item = u16>,
    ) -> Result<Self, ModelError> {
        let mut counters = BTreeMap::new();
        for tile_id in participants {
            if tile_id > 63 || counters.insert(tile_id, TileCounters::default()).is_some() {
                return Err(ModelError::new(
                    0x0007,
                    tile_id,
                    0,
                    epoch,
                    "participant tile IDs must be unique and in 0..=63",
                ));
            }
        }
        if counters.is_empty() {
            return Err(ModelError::new(
                0x0007,
                0,
                0,
                epoch,
                "an epoch requires at least one participant",
            ));
        }
        let next_record_sequence = counters.keys().map(|tile_id| (*tile_id, 0)).collect();
        Ok(Self {
            epoch,
            policy_digest,
            participants: counters,
            status: EpochStatus::Open,
            next_record_sequence,
            reconciled_tiles: BTreeSet::new(),
        })
    }

    pub fn status(&self) -> &EpochStatus {
        &self.status
    }

    pub fn observe_tile_record(&mut self, record: TileRecord) -> Result<(), FailureRecord> {
        if let EpochStatus::Failed(failure) = &self.status {
            return Err(failure.clone());
        }
        if self.status == EpochStatus::Complete {
            return Err(self.fail(evidence_failure(
                FaultCode::EvidenceIncomplete,
                record.tile_id,
                record.event_id,
                self.epoch,
            )));
        }
        if !self.participants.contains_key(&record.tile_id) || record.epoch != self.epoch {
            return Err(self.fail(evidence_failure(
                FaultCode::EvidenceIncomplete,
                record.tile_id,
                record.event_id,
                self.epoch,
            )));
        }
        if record.policy_digest != self.policy_digest {
            return Err(self.fail(evidence_failure(
                FaultCode::PolicyDigest,
                record.tile_id,
                record.event_id,
                self.epoch,
            )));
        }
        let expected_sequence = self.next_record_sequence[&record.tile_id];
        if record.record_sequence != expected_sequence {
            return Err(self.fail(evidence_failure(
                FaultCode::EvidenceSequence,
                record.tile_id,
                record.event_id,
                self.epoch,
            )));
        }

        let counters = self
            .participants
            .get_mut(&record.tile_id)
            .expect("participant checked above");
        counters.event_count += 1;
        counters.record_count += 1;
        counters.metadata_bytes += record.metadata_bytes;
        self.next_record_sequence
            .insert(record.tile_id, expected_sequence + 1);
        Ok(())
    }

    pub fn reconcile_tile_counters(
        &mut self,
        tile_id: u16,
        observed: TileCounters,
    ) -> Result<(), FailureRecord> {
        if let EpochStatus::Failed(failure) = &self.status {
            return Err(failure.clone());
        }
        let Some(expected) = self.participants.get(&tile_id) else {
            return Err(self.fail(evidence_failure(
                FaultCode::EvidenceIncomplete,
                tile_id,
                0,
                self.epoch,
            )));
        };
        if observed.drop_count != 0 {
            return Err(self.fail(evidence_failure(
                FaultCode::RecordDrop,
                tile_id,
                expected.event_count,
                self.epoch,
            )));
        }
        if expected != &observed {
            return Err(self.fail(evidence_failure(
                FaultCode::EvidenceCounters,
                tile_id,
                expected.event_count,
                self.epoch,
            )));
        }
        self.reconciled_tiles.insert(tile_id);
        Ok(())
    }

    pub fn record_required_drop(&mut self, tile_id: u16, event_id: u64) -> FailureRecord {
        if let Some(counters) = self.participants.get_mut(&tile_id) {
            counters.event_count += 1;
            counters.drop_count += 1;
        }
        self.fail(evidence_failure(
            FaultCode::RecordDrop,
            tile_id,
            event_id,
            self.epoch,
        ))
    }

    pub fn fail(&mut self, failure: FailureRecord) -> FailureRecord {
        match &self.status {
            EpochStatus::Failed(first) => first.clone(),
            EpochStatus::Open | EpochStatus::Complete => {
                self.status = EpochStatus::Failed(failure.clone());
                failure
            }
        }
    }

    pub fn seal_success(&mut self, model: &HomeAgent) -> Result<(), FailureRecord> {
        if let EpochStatus::Failed(failure) = &self.status {
            return Err(failure.clone());
        }
        if self.status == EpochStatus::Complete {
            return Ok(());
        }
        if self.reconciled_tiles.len() != self.participants.len()
            || self.participants.values().any(|counters| {
                counters.event_count == 0
                    || counters.event_count != counters.record_count
                    || counters.drop_count != 0
            })
        {
            return Err(self.fail(evidence_failure(
                FaultCode::EvidenceIncomplete,
                0,
                0,
                self.epoch,
            )));
        }
        let model_is_sealed = model.records().last().is_some_and(|record| {
            record.event.kind == EventKind::EpochSeal && record.event.epoch == self.epoch
        });
        if !model_is_sealed {
            return Err(self.fail(evidence_failure(
                FaultCode::EvidenceModelSeal,
                0,
                0,
                self.epoch,
            )));
        }
        self.status = EpochStatus::Complete;
        Ok(())
    }

    pub fn begin_recovery(&self, new_epoch: u64) -> Result<Self, ModelError> {
        if !matches!(self.status, EpochStatus::Failed(_)) || new_epoch == self.epoch {
            return Err(ModelError::new(
                0x0008,
                0,
                0,
                self.epoch,
                "recovery requires a failed epoch and a different epoch ID",
            ));
        }
        Self::new(
            new_epoch,
            self.policy_digest,
            self.participants.keys().copied(),
        )
    }
}

fn evidence_failure(code: FaultCode, tile_id: u16, event_id: u64, epoch: u64) -> FailureRecord {
    FailureRecord {
        code,
        tile_id,
        event_id,
        epoch,
    }
}
