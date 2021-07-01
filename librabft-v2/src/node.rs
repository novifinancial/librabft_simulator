// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::too_many_arguments)]

use crate::{pacemaker::*, record::*, record_store::*};
use anyhow::anyhow;
use bft_lib::{
    base_types::*,
    interfaces::{ConsensusNode, NodeUpdateActions},
    smr_context::SmrContext,
};
use log::debug;
use serde::{Deserialize, Serialize};
use std::{
    cmp::{max, min},
    collections::HashMap,
};

#[cfg(all(test, feature = "simulator"))]
#[path = "unit_tests/node_tests.rs"]
mod node_tests;

// -- BEGIN FILE node_state --
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(bound(serialize = "Context: SmrContext"))]
#[serde(bound(deserialize = "Context: SmrContext"))]
pub struct NodeState<Context: SmrContext> {
    /// Module dedicated to storing records for the current epoch.
    record_store: RecordStoreState<Context>,
    /// Module dedicated to leader election.
    pacemaker: PacemakerState<Context>,
    /// Current epoch.
    epoch_id: EpochId,
    /// Highest round voted so far.
    latest_voted_round: Round,
    /// Current locked round.
    locked_round: Round,
    /// Time of the latest query-all operation.
    latest_query_all_time: NodeTime,
    /// Track data to which the main handler has already reacted.
    tracker: CommitTracker,
    /// Record stores from previous epochs.
    past_record_stores: HashMap<EpochId, RecordStoreState<Context>>,
}
// -- END FILE --

// -- BEGIN FILE commit_tracker --
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct CommitTracker {
    /// Latest epoch identifier that was processed.
    epoch_id: EpochId,
    /// Round of the latest commit that was processed.
    highest_committed_round: Round,
    /// Time of the latest commit that was processed.
    latest_commit_time: NodeTime,
    /// Minimal interval between query-all actions when no commit happens.
    target_commit_interval: Duration,
}
// -- END FILE --

impl CommitTracker {
    fn new(epoch_id: EpochId, node_time: NodeTime, target_commit_interval: Duration) -> Self {
        CommitTracker {
            epoch_id,
            highest_committed_round: Round(0),
            latest_commit_time: node_time,
            target_commit_interval,
        }
    }
}

/// Initial configuration of LibraBFTv2 node.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(Default))]
pub struct NodeConfig {
    pub target_commit_interval: Duration,
    pub delta: Duration,
    pub gamma: f64,
    pub lambda: f64,
}

impl<Context> NodeState<Context>
where
    Context: SmrContext,
{
    pub fn make_initial_state(context: &Context, config: NodeConfig, node_time: NodeTime) -> Self {
        let initial_state = context.last_committed_state();
        let epoch_id = context.read_epoch_id(&initial_state);
        let tracker = CommitTracker::new(epoch_id, node_time, config.target_commit_interval);
        let record_store = RecordStoreState::new(
            Self::initial_hash(context, epoch_id),
            initial_state.clone(),
            epoch_id,
            context.configuration(&initial_state),
        );
        let pacemaker = PacemakerState::new(
            epoch_id,
            node_time,
            config.delta,
            config.gamma,
            config.lambda,
        );
        NodeState {
            record_store,
            pacemaker,
            epoch_id,
            latest_voted_round: Round(0),
            locked_round: Round(0),
            latest_query_all_time: node_time,
            tracker,
            past_record_stores: HashMap::new(),
        }
    }

    fn initial_hash(context: &Context, id: EpochId) -> QuorumCertificateHash<Context::HashValue> {
        QuorumCertificateHash(context.hash(&id))
    }

    pub(crate) fn epoch_id(&self) -> EpochId {
        self.epoch_id
    }

    pub(crate) fn record_store(&self) -> &dyn RecordStore<Context> {
        &self.record_store
    }

    pub(crate) fn record_store_at(&self, epoch_id: EpochId) -> Option<&dyn RecordStore<Context>> {
        if epoch_id == self.epoch_id {
            return Some(&self.record_store);
        }
        self.past_record_stores
            .get(&epoch_id)
            .map(|store| &*store as &dyn RecordStore<Context>)
    }

    pub(crate) fn pacemaker(&self) -> &dyn Pacemaker<Context> {
        &self.pacemaker
    }

    pub(crate) fn update_tracker(&mut self, clock: NodeTime) {
        // Ignore actions
        self.tracker.update_tracker(
            self.latest_query_all_time,
            clock,
            self.epoch_id,
            &self.record_store,
        );
    }

    pub(crate) fn insert_network_record(
        &mut self,
        epoch_id: EpochId,
        record: Record<Context>,
        context: &mut Context,
    ) {
        if epoch_id == self.epoch_id {
            self.record_store.insert_network_record(record, context);
        } else {
            debug!(
                "{:?} Skipped records outside the current epoch ({:?} instead of {:?})",
                context.author(),
                epoch_id,
                self.epoch_id
            );
        }
    }
}

#[cfg(feature = "simulator")]
impl<Context: SmrContext> bft_lib::simulator::ActiveRound for NodeState<Context> {
    fn active_round(&self) -> Round {
        self.pacemaker.active_round()
    }
}

// -- BEGIN FILE process_pacemaker_actions --
impl<Context: SmrContext> NodeState<Context> {
    fn process_pacemaker_actions(
        &mut self,
        pacemaker_actions: PacemakerUpdateActions<Context>,
        clock: NodeTime,
        context: &mut Context,
    ) -> NodeUpdateActions<Context> {
        let actions = NodeUpdateActions {
            next_scheduled_update: pacemaker_actions.next_scheduled_update,
            should_broadcast: pacemaker_actions.should_broadcast,
            should_query_all: pacemaker_actions.should_query_all,
            should_send: pacemaker_actions.should_send,
        };
        if let Some(round) = pacemaker_actions.should_create_timeout {
            self.record_store
                .create_timeout(context.author(), round, context);
            // Prevent voting at a round for which we have created a timeout already.
            self.latest_voted_round.max_update(round);
        }
        if let Some(previous_qc_hash) = pacemaker_actions.should_propose_block {
            self.record_store
                .propose_block(context, previous_qc_hash, clock);
        }
        actions
    }
}
// -- END FILE --

// -- BEGIN FILE consensus_node_impl --
impl<Context> ConsensusNode<Context> for NodeState<Context>
where
    Context: SmrContext,
{
    fn load_node(context: &mut Context, node_time: NodeTime) -> AsyncResult<Self> {
        Box::pin(async move {
            let value = context
                .read_value("node_state".to_string())
                .await?
                .ok_or(anyhow!("missing state value"))?;
            let node: Self = bincode::deserialize(&value)?;
            let previous_time = std::cmp::max(
                node.latest_query_all_time,
                std::cmp::max(
                    node.tracker.latest_commit_time,
                    node.pacemaker.active_round_start_time,
                ),
            );
            anyhow::ensure!(
                node_time >= previous_time,
                "refusing to restore saved state from the future"
            );
            Ok(node)
        })
    }

    fn save_node<'a>(&'a mut self, context: &'a mut Context) -> AsyncResult<()> {
        Box::pin(async move {
            let value = bincode::serialize(&*self)?;
            context.store_value("node_state".to_string(), value).await
        })
    }

    fn update_node(
        &mut self,
        context: &mut Context,
        clock: NodeTime,
    ) -> NodeUpdateActions<Context> {
        // Update pacemaker state and process pacemaker actions (e.g., creating a timeout, proposing
        // a block).
        let pacemaker_actions = self.pacemaker.update_pacemaker(
            context.author(),
            self.epoch_id,
            &self.record_store,
            self.latest_query_all_time,
            clock,
        );
        let mut actions = self.process_pacemaker_actions(pacemaker_actions, clock, context);
        // Vote on a valid proposal block designated by the pacemaker, if any.
        if let Some((block_hash, block_round, proposer)) =
            self.record_store.proposed_block(&self.pacemaker)
        {
            // Enforce voting constraints.
            if block_round > self.latest_voted_round
                && self.record_store.previous_round(block_hash) >= self.locked_round
            {
                // Update the latest voted round.
                self.latest_voted_round = block_round;
                // Update the locked round.
                self.locked_round = max(
                    self.locked_round,
                    self.record_store.second_previous_round(block_hash),
                );
                // Try to execute the command contained the a block and create a vote.
                if self.record_store.create_vote(context, block_hash) {
                    // Ask to notify and send our vote to the author of the block.
                    actions.should_send = vec![proposer];
                }
            }
        }
        // Check if our last proposal has reached a quorum of votes and create a QC.
        if self.record_store.check_for_new_quorum_certificate(context) {
            // Broadcast the QC to finish our work as a leader.
            actions.should_broadcast = true;
            // Schedule a new run now to process the new QC.
            actions.next_scheduled_update = clock;
        }
        // Check for new commits and verify if we should start a new epoch.
        self.process_commits(context);
        // Update the commit tracker and ask that we query all nodes if needed.
        let tracker_actions = self.tracker.update_tracker(
            self.latest_query_all_time,
            clock,
            self.epoch_id,
            &self.record_store,
        );
        actions.should_query_all = actions.should_query_all || tracker_actions.should_query_all;
        actions.next_scheduled_update = min(
            actions.next_scheduled_update,
            tracker_actions.next_scheduled_update,
        );
        // Update the time of the latest query-all action.
        if actions.should_query_all {
            self.latest_query_all_time = clock;
        }
        // Return desired actions to main handler.
        actions
    }
}
// -- END FILE --

// -- BEGIN FILE process_commits --
impl<Context> NodeState<Context>
where
    Context: SmrContext,
{
    pub(crate) fn process_commits(&mut self, context: &mut Context) {
        // For all commits that have not been processed yet, according to the commit tracker..
        for (round, state) in self
            .record_store
            .committed_states_after(self.tracker.highest_committed_round)
        {
            // .. deliver the committed state to the SMR layer, together with a commit certificate,
            // if any.
            if round == self.record_store.highest_committed_round() {
                match self.record_store.highest_commit_certificate() {
                    None => context.commit(&state, None),
                    Some(x) => context.commit(&state, Some(&x.value)),
                };
            } else {
                context.commit(&state, None);
            };
            // .. check if the current epoch just ended. If it did..
            let new_epoch_id = context.read_epoch_id(&state);
            if new_epoch_id > self.epoch_id {
                // .. create a new record store and switch to the new epoch.
                let new_record_store = RecordStoreState::new(
                    Self::initial_hash(context, new_epoch_id),
                    state.clone(),
                    new_epoch_id,
                    context.configuration(&state),
                );
                let old_record_store = std::mem::replace(&mut self.record_store, new_record_store);
                self.past_record_stores
                    .insert(self.epoch_id, old_record_store);
                self.epoch_id = new_epoch_id;
                // .. initialize voting constraints.
                self.latest_voted_round = Round(0);
                self.locked_round = Round(0);
                // .. stop delivering commits after an epoch change.
                break;
            }
        }
    }
}
// -- END FILE --

// -- BEGIN FILE commit_tracker_impl --
#[derive(Debug)]
struct CommitTrackerUpdateActions {
    /// Time at which to call `update_node` again, at the latest.
    next_scheduled_update: NodeTime,
    /// Whether we need to query all other nodes.
    should_query_all: bool,
}

impl CommitTracker {
    fn update_tracker<Context: SmrContext>(
        &mut self,
        latest_query_all_time: NodeTime,
        clock: NodeTime,
        current_epoch_id: EpochId,
        current_record_store: &dyn RecordStore<Context>,
    ) -> CommitTrackerUpdateActions {
        let mut actions = CommitTrackerUpdateActions::new();
        // Update tracked values: epoch, round, and time of the latest commit.
        if current_epoch_id > self.epoch_id {
            self.epoch_id = current_epoch_id;
            self.highest_committed_round = current_record_store.highest_committed_round();
            self.latest_commit_time = clock;
        } else {
            let highest_committed_round = current_record_store.highest_committed_round();
            if highest_committed_round > self.highest_committed_round {
                self.highest_committed_round = highest_committed_round;
                self.latest_commit_time = clock;
            }
        }
        // Decide if too much time passed since the latest commit or the latest query-all action.
        let mut deadline =
            max(self.latest_commit_time, latest_query_all_time) + self.target_commit_interval;
        if clock >= deadline {
            // If yes, trigger a query-all action.
            actions.should_query_all = true;
            deadline = clock + self.target_commit_interval;
        }
        // Schedule the next update.
        actions.next_scheduled_update = deadline;
        // Return desired actions to main handler.
        actions
    }
}
// -- END FILE --

impl CommitTrackerUpdateActions {
    fn new() -> Self {
        CommitTrackerUpdateActions {
            should_query_all: false,
            next_scheduled_update: NodeTime::never(),
        }
    }
}
