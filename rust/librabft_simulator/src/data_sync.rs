// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use base_types::*;
use node::*;
use record::*;
use smr_context::SMRContext;
use std::collections::BTreeSet;

#[cfg(test)]
#[path = "unit_tests/data_sync_tests.rs"]
mod data_sync_tests;

// -- BEGIN FILE data_sync --
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone)]
pub struct DataSyncNotification {
    /// Current epoch identifier.
    current_epoch: EpochId,
    /// Tail QC of the highest commit rule.
    highest_commit_certificate: Option<QuorumCertificate>,
    /// Highest QC.
    highest_quorum_certificate: Option<QuorumCertificate>,
    /// Timeouts in the highest TC, then at the current round, if any.
    timeouts: Vec<Timeout>,
    /// Sender's vote at the current round, if any (meant for the proposer).
    current_vote: Option<Vote>,
    /// Known proposed block at the current round, if any.
    proposed_block: Option<Block>,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone)]
pub struct DataSyncRequest {
    /// Current epoch identifier.
    current_epoch: EpochId,
    /// Selection of rounds for which the receiver already knows a QC.
    known_quorum_certificates: BTreeSet<Round>,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct DataSyncResponse {
    /// Current epoch identifier.
    current_epoch: EpochId,
    /// Records for the receiver to insert, for each epoch, in the given order.
    /// Epochs older than the receiver's current epoch will be skipped, as well as chains
    /// of records ending with QC known to the receiver.
    records: Vec<(EpochId, Vec<Record>)>,
}
// -- END FILE --

impl NodeState {
    fn create_request_internal(&self) -> DataSyncRequest {
        DataSyncRequest {
            current_epoch: self.epoch_id(),
            known_quorum_certificates: self.record_store().known_quorum_certificate_rounds(),
        }
    }
}

impl<Context> DataSyncNode<Context> for NodeState
where
    Context: SMRContext,
{
    type Notification = DataSyncNotification;
    type Request = DataSyncRequest;
    type Response = DataSyncResponse;

    fn create_notification(&self) -> DataSyncNotification {
        // Pass the latest (non-empty) commit certificate across epochs.
        let highest_commit_certificate = match self.record_store().highest_commit_certificate() {
            Some(hqc) => Some(hqc.clone()),
            None => match self.epoch_id().previous() {
                Some(previous_epoch) => self
                    .record_store_at(previous_epoch)
                    .expect("The record store of the previous epoch should exist.")
                    .highest_commit_certificate()
                    .cloned(),
                None => None,
            },
        };
        DataSyncNotification {
            current_epoch: self.epoch_id(),
            highest_commit_certificate,
            highest_quorum_certificate: self.record_store().highest_quorum_certificate().cloned(),
            timeouts: self.record_store().timeouts(),
            current_vote: self
                .record_store()
                .current_vote(self.local_author())
                .cloned(),
            proposed_block: match self.record_store().proposed_block(self.pacemaker()) {
                Some((hash, _, author)) => {
                    // Do not reshare other leaders' proposals.
                    if author == self.local_author() {
                        Some(self.record_store().block(hash).unwrap().clone())
                    } else {
                        None
                    }
                }
                None => None,
            },
        }
    }

    fn handle_notification(
        &mut self,
        notification: DataSyncNotification,
        smr_context: &mut Context,
    ) -> Option<DataSyncRequest> {
        // Whether we should request more data because of a new epoch or missings records.
        let mut should_sync = false;
        // Note that malicious nodes can always lie to make us send a request, but they may as
        // well send us a lengthy and slow `DataSyncResponse` directly. (DoS prevention is out of
        // scope for this simulator.)
        should_sync |= notification.current_epoch > self.epoch_id();

        if let Some(highest_commit_certificate) = &notification.highest_commit_certificate {
            // Try to insert the QC just in case.
            self.insert_network_record(
                highest_commit_certificate.epoch_id,
                Record::QuorumCertificate(highest_commit_certificate.clone()),
                smr_context,
            );
            should_sync |= (highest_commit_certificate.epoch_id > self.epoch_id())
                || (highest_commit_certificate.epoch_id == self.epoch_id()
                    && highest_commit_certificate.round
                        > self.record_store().highest_committed_round() + 2);
        }
        if let Some(highest_quorum_certificate) = &notification.highest_quorum_certificate {
            // Try to insert the QC.
            self.insert_network_record(
                highest_quorum_certificate.epoch_id,
                Record::QuorumCertificate(highest_quorum_certificate.clone()),
                smr_context,
            );
            // Check if we should request more data.
            should_sync |= (highest_quorum_certificate.epoch_id > self.epoch_id())
                || (highest_quorum_certificate.epoch_id == self.epoch_id()
                    && highest_quorum_certificate.round
                        > self.record_store().highest_quorum_certificate_round());
        }
        // Try to insert the proposed block right away.
        if let Some(block) = notification.proposed_block {
            self.insert_network_record(
                notification.current_epoch,
                Record::Block(block),
                smr_context,
            );
        }
        // Try to insert timeouts right away.
        for timeout in notification.timeouts {
            self.insert_network_record(
                notification.current_epoch,
                Record::Timeout(timeout),
                smr_context,
            );
        }
        // Try to insert votes right away.
        if let Some(vote) = notification.current_vote {
            self.insert_network_record(notification.current_epoch, Record::Vote(vote), smr_context);
        }
        // Create a follow-up request if needed.
        if should_sync {
            Some(self.create_request_internal())
        } else {
            None
        }
    }

    fn create_request(&self) -> DataSyncRequest {
        self.create_request_internal()
    }

    fn handle_request(&self, request: DataSyncRequest) -> DataSyncResponse {
        let mut records = Vec::new();
        if let Some(store) = self.record_store_at(request.current_epoch) {
            records.push((
                request.current_epoch,
                store.unknown_records(request.known_quorum_certificates),
            ));
        }
        for i in (request.current_epoch.0 + 1)..(self.epoch_id().0 + 1) {
            let epoch_id = EpochId(i);
            let store = self
                .record_store_at(epoch_id)
                .expect("All record stores up to the current epoch should exist.");
            records.push((epoch_id, store.unknown_records(BTreeSet::new())));
        }
        DataSyncResponse {
            current_epoch: self.epoch_id(),
            records,
        }
    }

    fn handle_response(
        &mut self,
        response: DataSyncResponse,
        smr_context: &mut Context,
        clock: NodeTime,
    ) {
        let num_records = response.records.len();
        // Insert all the records in order.
        // Process the commits so that new epochs are created along the way.
        // No need to call a full handler `update_node` because past epochs are stopped.
        for (i, (epoch_id, records)) in response.records.into_iter().enumerate() {
            if epoch_id < self.epoch_id() {
                // Looks like we have stopped this epoch in the meantime.
                continue;
            }
            if epoch_id > self.epoch_id() {
                // This should not happen. Abort.
                break;
            }
            for record in records {
                self.insert_network_record(epoch_id, record, smr_context);
            }
            if i == num_records - 1 {
                // Leave the latest epoch for the main handler to process.
                break;
            }
            // Deliver commits and start the next epochs.
            self.process_commits(smr_context);
            self.update_tracker(clock);
        }
    }
}
