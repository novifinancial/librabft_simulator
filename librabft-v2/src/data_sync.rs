// Copyright (c) Facebook, Inc. and its affiliates.
// SPDX-License-Identifier: Apache-2.0

use crate::{node::*, record::*};
use bft_lib::{base_types::*, interfaces::DataSyncNode, smr_context::SmrContext};
use futures::future;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[cfg(all(test, feature = "simulator"))]
#[path = "unit_tests/data_sync_tests.rs"]
mod data_sync_tests;

// -- BEGIN FILE data_sync --
#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct DataSyncNotification<Context: SmrContext> {
    /// Current epoch identifier.
    current_epoch: EpochId,
    /// Tail QC of the highest commit rule.
    #[serde(bound(serialize = "Context: SmrContext"))]
    #[serde(bound(deserialize = "Context: SmrContext"))]
    highest_commit_certificate: Option<QuorumCertificate<Context>>,
    /// Highest QC.
    #[serde(bound(serialize = "Context: SmrContext"))]
    #[serde(bound(deserialize = "Context: SmrContext"))]
    highest_quorum_certificate: Option<QuorumCertificate<Context>>,
    /// Timeouts in the highest TC, then at the current round, if any.
    #[serde(bound(serialize = "Context: SmrContext"))]
    #[serde(bound(deserialize = "Context: SmrContext"))]
    timeouts: Vec<Timeout<Context>>,
    /// Sender's vote at the current round, if any (meant for the proposer).
    #[serde(bound(serialize = "Context: SmrContext"))]
    #[serde(bound(deserialize = "Context: SmrContext"))]
    current_vote: Option<Vote<Context>>,
    /// Known proposed block at the current round, if any.
    #[serde(bound(serialize = "Context: SmrContext"))]
    #[serde(bound(deserialize = "Context: SmrContext"))]
    proposed_block: Option<Block<Context>>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct DataSyncRequest {
    /// Current epoch identifier.
    current_epoch: EpochId,
    /// Selection of rounds for which the receiver already knows a QC.
    known_quorum_certificates: BTreeSet<Round>,
}

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize, Clone)]
pub struct DataSyncResponse<Context: SmrContext> {
    /// Current epoch identifier.
    current_epoch: EpochId,
    /// Records for the receiver to insert, for each epoch, in the given order.
    /// Epochs older than the receiver's current epoch will be skipped, as well as chains
    /// of records ending with QC known to the receiver.
    #[serde(bound(serialize = "Context: SmrContext"))]
    #[serde(bound(deserialize = "Context: SmrContext"))]
    records: Vec<(EpochId, Vec<Record<Context>>)>,
}
// -- END FILE --

impl<Context> NodeState<Context>
where
    Context: SmrContext,
{
    fn create_request_internal(&self) -> DataSyncRequest {
        DataSyncRequest {
            current_epoch: self.epoch_id(),
            known_quorum_certificates: self.record_store().known_quorum_certificate_rounds(),
        }
    }
}

impl<Context> DataSyncNode<Context> for NodeState<Context>
where
    Context: SmrContext,
{
    type Notification = DataSyncNotification<Context>;
    type Request = DataSyncRequest;
    type Response = DataSyncResponse<Context>;

    fn create_notification(&self, context: &Context) -> Self::Notification {
        // Pass the latest (non-empty) commit certificate across epochs.
        let highest_commit_certificate = match self.record_store().highest_commit_certificate() {
            Some(hqc) => Some(hqc.clone()),
            None => self.epoch_id().previous().and_then(|previous_epoch| {
                self.record_store_at(previous_epoch)
                    .expect("The record store of the previous epoch should exist.")
                    .highest_commit_certificate()
                    .cloned()
            }),
        };
        DataSyncNotification {
            current_epoch: self.epoch_id(),
            highest_commit_certificate,
            highest_quorum_certificate: self.record_store().highest_quorum_certificate().cloned(),
            timeouts: self.record_store().timeouts(),
            current_vote: self.record_store().current_vote(context.author()).cloned(),
            proposed_block: match self.record_store().proposed_block(self.pacemaker()) {
                Some((hash, _, author)) => {
                    // Do not reshare other leaders' proposals.
                    if author == context.author() {
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
        smr_context: &mut Context,
        notification: Self::Notification,
    ) -> Async<Option<Self::Request>> {
        // Whether we should request more data because of a new epoch or missings records.
        let mut should_sync = false;
        // Note that malicious nodes can always lie to make us send a request, but they may as
        // well send us a lengthy and slow `DataSyncResponse` directly. (DoS prevention is out of
        // scope for this simulator.)
        should_sync |= notification.current_epoch > self.epoch_id();

        if let Some(highest_commit_certificate) = &notification.highest_commit_certificate {
            // Try to insert the QC just in case.
            self.insert_network_record(
                highest_commit_certificate.value.epoch_id,
                Record::QuorumCertificate(highest_commit_certificate.clone()),
                smr_context,
            );
            should_sync |= (highest_commit_certificate.value.epoch_id > self.epoch_id())
                || (highest_commit_certificate.value.epoch_id == self.epoch_id()
                    && highest_commit_certificate.value.round
                        > self.record_store().highest_committed_round() + 2);
        }
        if let Some(highest_quorum_certificate) = &notification.highest_quorum_certificate {
            // Try to insert the QC.
            self.insert_network_record(
                highest_quorum_certificate.value.epoch_id,
                Record::QuorumCertificate(highest_quorum_certificate.clone()),
                smr_context,
            );
            // Check if we should request more data.
            should_sync |= (highest_quorum_certificate.value.epoch_id > self.epoch_id())
                || (highest_quorum_certificate.value.epoch_id == self.epoch_id()
                    && highest_quorum_certificate.value.round
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
        let value = if should_sync {
            Some(self.create_request_internal())
        } else {
            None
        };
        Box::pin(future::ready(value))
    }

    fn create_request(&self, _context: &Context) -> Self::Request {
        self.create_request_internal()
    }

    fn handle_request(
        &self,
        _smr_context: &mut Context,
        request: Self::Request,
    ) -> Async<Self::Response> {
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
        let value = DataSyncResponse {
            current_epoch: self.epoch_id(),
            records,
        };
        Box::pin(future::ready(value))
    }

    fn handle_response(
        &mut self,
        smr_context: &mut Context,
        response: Self::Response,
        clock: NodeTime,
    ) -> Async<()> {
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
        Box::pin(future::ready(()))
    }
}
