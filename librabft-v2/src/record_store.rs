// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use crate::{
    base_types::*,
    pacemaker::{Pacemaker, PacemakerState},
    record::*,
};
use anyhow::{bail, ensure};
use bft_lib::{
    base_types::*,
    configuration::EpochConfiguration,
    smr_context::{SignedValue, SmrContext},
};
use log::{debug, info, warn};
use std::{
    collections::{BTreeSet, HashMap},
    fmt::Debug,
};

#[cfg(all(test, feature = "simulator"))]
#[path = "unit_tests/record_store_tests.rs"]
mod record_store_tests;

// -- BEGIN FILE record_store --
pub(crate) trait RecordStore<Context: SmrContext> {
    /// Return the hash of a QC at the highest round, or the initial hash.
    fn highest_quorum_certificate_hash(&self) -> QuorumCertificateHash<Context::HashValue>;
    /// Query the round of the highest QC.
    fn highest_quorum_certificate_round(&self) -> Round;
    /// Query the highest QC.
    fn highest_quorum_certificate(&self) -> Option<&QuorumCertificate<Context>>;
    /// Query the round of the highest TC.
    fn highest_timeout_certificate_round(&self) -> Round;
    /// Query the round of the highest commit.
    fn highest_committed_round(&self) -> Round;
    /// Query the last QC of the highest commit rule.
    fn highest_commit_certificate(&self) -> Option<&QuorumCertificate<Context>>;
    /// Current round as seen by the record store.
    fn current_round(&self) -> Round;

    /// Iterate on the committed blocks starting after the round `after_round` and ending with the
    /// highest commit known so far.
    fn committed_states_after(&self, after_round: Round) -> Vec<(Round, Context::State)>;

    /// Access the block proposed by the leader chosen by the Pacemaker (if any).
    fn proposed_block(
        &self,
        pacemaker: &dyn Pacemaker<Context>,
    ) -> Option<(BlockHash<Context::HashValue>, Round, Context::Author)>;
    /// Check if a timeout already exists.
    fn has_timeout(&self, author: Context::Author, round: Round) -> bool;

    /// Create a timeout.
    fn create_timeout(&mut self, author: Context::Author, round: Round, context: &mut Context);
    /// Fetch a command from mempool and propose a block.
    fn propose_block(
        &mut self,
        context: &mut Context,
        previous_qc_hash: QuorumCertificateHash<Context::HashValue>,
        clock: NodeTime,
    );
    /// Execute the command contained in a block and vote for the resulting state.
    /// Return false if the execution failed.
    fn create_vote(
        &mut self,
        context: &mut Context,
        block_hash: BlockHash<Context::HashValue>,
    ) -> bool;
    /// Try to create a QC for the last block that we have proposed.
    fn check_for_new_quorum_certificate(&mut self, context: &mut Context) -> bool;

    /// Compute the previous round and the second previous round of a block.
    fn previous_round(&self, block_hash: BlockHash<Context::HashValue>) -> Round;
    fn second_previous_round(&self, block_hash: BlockHash<Context::HashValue>) -> Round;
    /// Pick an author based on a seed, with chances proportional to voting rights.
    fn pick_author(&self, seed: u64) -> Context::Author;

    /// APIs supporting data synchronization.
    fn timeouts(&self) -> Vec<Timeout<Context>>;
    fn current_vote(&self, local_author: Context::Author) -> Option<&Vote<Context>>;
    fn block(&self, block_hash: BlockHash<Context::HashValue>) -> Option<&Block<Context>>;
    fn known_quorum_certificate_rounds(&self) -> BTreeSet<Round>;
    fn unknown_records(&self, known_qc_rounds: BTreeSet<Round>) -> Vec<Record<Context>>;
    fn insert_network_record(&mut self, record: Record<Context>, context: &mut Context);
}
// -- END FILE --

// -- BEGIN FILE record_store_state --
#[derive(Debug)]
pub struct RecordStoreState<Context: SmrContext> {
    /// Epoch initialization.
    epoch_id: EpochId,
    configuration: EpochConfiguration<Context::Author>,
    initial_hash: QuorumCertificateHash<Context::HashValue>,
    initial_state: Context::State,
    /// Storage of verified blocks and QCs.
    blocks: HashMap<BlockHash<Context::HashValue>, Block<Context>>,
    quorum_certificates:
        HashMap<QuorumCertificateHash<Context::HashValue>, QuorumCertificate<Context>>,
    current_proposed_block: Option<BlockHash<Context::HashValue>>,
    /// Computed round values.
    highest_quorum_certificate_round: Round,
    highest_quorum_certificate_hash: QuorumCertificateHash<Context::HashValue>,
    highest_timeout_certificate_round: Round,
    current_round: Round,
    highest_committed_round: Round,
    highest_commit_certificate_hash: Option<QuorumCertificateHash<Context::HashValue>>,
    /// Storage of verified timeouts at the highest TC round.
    highest_timeout_certificate: Option<Vec<Timeout<Context>>>,
    /// Storage of verified votes and timeouts at the current round.
    current_timeouts: HashMap<Context::Author, Timeout<Context>>,
    current_votes: HashMap<Context::Author, Vote<Context>>,
    /// Computed weight values.
    current_timeouts_weight: usize,
    current_election: ElectionState<Context>,
}

/// Counting votes for a proposed block and its execution state.
#[derive(Debug)]
enum ElectionState<Context: SmrContext> {
    Ongoing {
        ballot: HashMap<(BlockHash<Context::HashValue>, Context::State), usize>,
    },
    Won {
        block_hash: BlockHash<Context::HashValue>,
        state: Context::State,
    },
    Closed,
}
// -- END FILE --

struct BackwardQuorumCertificateIterator<'a, Context: SmrContext> {
    store: &'a RecordStoreState<Context>,
    current_hash: QuorumCertificateHash<Context::HashValue>,
}

impl<'a, Context: SmrContext> BackwardQuorumCertificateIterator<'a, Context> {
    fn new(
        store: &'a RecordStoreState<Context>,
        qc_hash: QuorumCertificateHash<Context::HashValue>,
    ) -> Self {
        BackwardQuorumCertificateIterator {
            store,
            current_hash: qc_hash,
        }
    }
}

impl<'a, Context: SmrContext> Iterator for BackwardQuorumCertificateIterator<'a, Context> {
    type Item = &'a QuorumCertificate<Context>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_hash == self.store.initial_hash {
            return None;
        }
        let qc = self.store.quorum_certificate(self.current_hash).unwrap();
        let block = self.store.block(qc.value.certified_block_hash).unwrap();
        self.current_hash = block.value.previous_quorum_certificate_hash;
        Some(qc)
    }
}

impl<Context: SmrContext> RecordStoreState<Context> {
    pub(crate) fn new(
        initial_hash: QuorumCertificateHash<Context::HashValue>,
        initial_state: Context::State,
        epoch_id: EpochId,
        configuration: EpochConfiguration<Context::Author>,
    ) -> Self {
        warn!("Creating new record store for epoch: {:?}, initial_hash: {:?}, initial_state: {:?}, configuration: {:?}", epoch_id, initial_hash, initial_state, configuration);
        RecordStoreState {
            configuration,
            initial_hash,
            initial_state,
            epoch_id,
            blocks: HashMap::new(),
            quorum_certificates: HashMap::new(),
            current_proposed_block: None,
            highest_quorum_certificate_round: Round(0),
            highest_quorum_certificate_hash: initial_hash,
            highest_timeout_certificate_round: Round(0),
            current_round: Round(1),
            highest_committed_round: Round(0),
            highest_commit_certificate_hash: None,
            highest_timeout_certificate: None,
            current_timeouts: HashMap::new(),
            current_votes: HashMap::new(),
            current_timeouts_weight: 0,
            current_election: ElectionState::Ongoing {
                ballot: HashMap::new(),
            },
        }
    }

    fn ancestor_rounds(
        &self,
        qc_hash: QuorumCertificateHash<Context::HashValue>,
    ) -> impl Iterator<Item = Round> + '_ {
        BackwardQuorumCertificateIterator::new(self, qc_hash).map(|qc| qc.value.round)
    }

    fn update_current_round(&mut self, round: Round) {
        if round <= self.current_round {
            return;
        }
        self.current_round = round;
        self.current_proposed_block = None;
        self.current_timeouts = HashMap::new();
        self.current_votes = HashMap::new();
        self.current_timeouts_weight = 0;
        self.current_election = ElectionState::Ongoing {
            ballot: HashMap::new(),
        };
    }

    fn update_commit_3chain_round(&mut self, qc_hash: QuorumCertificateHash<Context::HashValue>) {
        let rounds = {
            let mut iter = self.ancestor_rounds(qc_hash);
            let r3 = iter.next();
            let r2 = iter.next();
            let r1 = iter.next();
            (r1, r2, r3)
        };
        if let (Some(r1), Some(r2), Some(r3)) = rounds {
            if r3 == r2 + 1 && r2 == r1 + 1 && r1 > self.highest_committed_round {
                self.highest_committed_round = r1;
                self.highest_commit_certificate_hash = Some(qc_hash);
            }
        }
    }

    fn vote_committed_state(
        &self,
        block_hash: BlockHash<Context::HashValue>,
    ) -> Option<Context::State> {
        let block = self.block(block_hash).unwrap();
        let r3 = block.value.round;
        let qc2_hash = block.value.previous_quorum_certificate_hash;
        let mut iter = BackwardQuorumCertificateIterator::new(&self, qc2_hash);
        let opt_qc2 = iter.next();
        let opt_qc1 = iter.next();
        if let (Some(qc1), Some(qc2)) = (opt_qc1, opt_qc2) {
            let r2 = qc2.value.round;
            let r1 = qc1.value.round;
            if r3 == r2 + 1 && r2 == r1 + 1 {
                return Some(qc1.value.state.clone());
            }
        }
        None
    }

    fn verify_network_record(
        &self,
        context: &Context,
        record: &Record<Context>,
    ) -> Result<Context::HashValue> {
        match record {
            Record::Block(block) => {
                let hash = context.hash(&block.value);
                ensure!(
                    !self.blocks.contains_key(&BlockHash(hash)),
                    "Block was already inserted."
                );
                context.verify(block.value.author, hash, block.signature)?;
                ensure!(
                    block.value.previous_quorum_certificate_hash == self.initial_hash
                        || self
                            .quorum_certificates
                            .contains_key(&block.value.previous_quorum_certificate_hash),
                    "The previous QC (if any) must be verified first."
                );
                if self.initial_hash == block.value.previous_quorum_certificate_hash {
                    ensure!(block.value.round > Round(0), "Rounds must start at 1");
                } else {
                    let previous_qc = self
                        .quorum_certificate(block.value.previous_quorum_certificate_hash)
                        .unwrap();
                    let previous_block =
                        self.block(previous_qc.value.certified_block_hash).unwrap();
                    ensure!(
                        block.value.round > previous_block.value.round,
                        "Rounds must be increasing"
                    );
                }
                Ok(hash)
            }
            Record::Vote(vote) => {
                let hash = context.hash(&vote.value);
                ensure!(
                    vote.value.epoch_id == self.epoch_id,
                    "Epoch identifier of vote ({:?}) must match the current epoch ({:?}).",
                    vote.value.epoch_id,
                    self.epoch_id
                );
                ensure!(
                    self.blocks.contains_key(&vote.value.certified_block_hash),
                    "The certified block hash of a vote must be verified first."
                );
                ensure!(
                    self.block(vote.value.certified_block_hash)
                        .unwrap()
                        .value
                        .round
                        == vote.value.round,
                    "The round of the vote must match the certified block."
                );
                ensure!(
                    self.vote_committed_state(vote.value.certified_block_hash)
                        == vote.value.committed_state,
                    "The committed_state value of a vote must follow the commit rule."
                );
                ensure!(
                    vote.value.round == self.current_round,
                    "Only accepting votes for a proposal at the current {:?}. This one was at {:?}",
                    self.current_round,
                    vote.value.round
                );
                ensure!(
                    !self.current_votes.contains_key(&vote.value.author),
                    "We insert votes only for authors who haven't voted yet."
                );
                context.verify(vote.value.author, hash, vote.signature)?;
                Ok(hash)
            }
            Record::QuorumCertificate(qc) => {
                let hash = context.hash(&qc.value);
                ensure!(
                    qc.value.epoch_id == self.epoch_id,
                    "Epoch identifier of QC ({:?}) must match the current epoch ({:?}).",
                    qc.value.epoch_id,
                    self.epoch_id
                );
                ensure!(
                    !self
                        .quorum_certificates
                        .contains_key(&QuorumCertificateHash(hash)),
                    "QuorumCertificate was already inserted."
                );
                ensure!(
                    self.blocks.contains_key(&qc.value.certified_block_hash),
                    "The certified block hash of a QC must be verified first."
                );
                ensure!(
                    self.block(qc.value.certified_block_hash)
                        .unwrap()
                        .value
                        .round
                        == qc.value.round,
                    "The round of the QC must match the certified block."
                );
                ensure!(
                    qc.value.author
                        == self
                            .block(qc.value.certified_block_hash)
                            .unwrap()
                            .value
                            .author,
                    "QCs must be created by the author of the certified block"
                );
                ensure!(
                    self.vote_committed_state(qc.value.certified_block_hash)
                        == qc.value.committed_state,
                    "The committed_state value of a QC must follow the commit rule."
                );
                let mut weight = 0;
                for (author, signature) in &qc.value.votes {
                    let original_vote_hash = context.hash(&Vote_::<Context> {
                        epoch_id: self.epoch_id,
                        round: qc.value.round,
                        certified_block_hash: qc.value.certified_block_hash,
                        state: qc.value.state.clone(),
                        committed_state: qc.value.committed_state.clone(),
                        author: *author,
                    });
                    context.verify(*author, original_vote_hash, *signature)?;
                    weight += self.configuration.weight(author);
                }
                ensure!(
                    weight >= self.configuration.quorum_threshold(),
                    "Votes in QCs must form a quorum"
                );
                context.verify(qc.value.author, hash, qc.signature)?;
                Ok(hash)
            }
            Record::Timeout(timeout) => {
                let hash = context.hash(&timeout.value);
                ensure!(
                    timeout.value.epoch_id == self.epoch_id,
                    "Epoch identifier of timeout ({:?}) must match the current epoch ({:?}).",
                    timeout.value.epoch_id,
                    self.epoch_id
                );
                ensure!(
                    timeout.value.highest_certified_block_round
                        <= self.highest_quorum_certificate_round(),
                    "Timeouts must refer to a known certified block round."
                );
                ensure!(
                    timeout.value.round == self.current_round,
                    "Accepting only timeouts at the current {:?}. This one was at {:?}",
                    self.current_round,
                    timeout.value.round
                );
                ensure!(
                    !self.current_timeouts.contains_key(&timeout.value.author),
                    "A timeout is already known for the same round and the same author"
                );
                context.verify(timeout.value.author, hash, timeout.signature)?;
                Ok(hash)
            }
        }
    }

    fn quorum_certificate(
        &self,
        qc_hash: QuorumCertificateHash<Context::HashValue>,
    ) -> Option<&QuorumCertificate<Context>> {
        self.quorum_certificates.get(&qc_hash)
    }

    fn compute_state(
        &self,
        block_hash: BlockHash<Context::HashValue>,
        context: &mut Context,
    ) -> Option<Context::State> {
        let block = self.block(block_hash).unwrap();
        let (previous_state, previous_voters, previous_author) = {
            if block.value.previous_quorum_certificate_hash == self.initial_hash {
                (&self.initial_state, None, Vec::new())
            } else {
                let previous_qc = self
                    .quorum_certificate(block.value.previous_quorum_certificate_hash)
                    .unwrap();
                let voters = previous_qc.value.votes.iter().map(|x| x.0).collect();
                (
                    &previous_qc.value.state,
                    Some(previous_qc.value.author),
                    voters,
                )
            }
        };
        context.compute(
            previous_state,
            block.value.command.clone(),
            block.value.time,
            previous_voters,
            previous_author,
        )
    }

    fn try_insert_network_record(
        &mut self,
        record: Record<Context>,
        context: &mut Context,
    ) -> Result<()> {
        // First, check that the record is "relevant" and that invariants of "verified records",
        // such as chaining, are respected.
        let hash = self.verify_network_record(&*context, &record)?;
        // Second, insert the record. In the case of QC, this is where check execution states.
        match record {
            Record::Block(block) => {
                let block_hash = BlockHash(hash);
                if block.value.round == self.current_round
                    && PacemakerState::leader(&*self, block.value.round) == block.value.author
                {
                    // If we use a VRF, this assumes that we have inserted the highest commit rule
                    // beforehand.
                    self.current_proposed_block = Some(block_hash);
                }
                self.blocks.insert(block_hash, block);
            }
            Record::Vote(vote) => {
                self.current_votes.insert(vote.value.author, vote.clone());
                let has_newly_won_election = match &mut self.current_election {
                    ElectionState::Ongoing { ballot } => {
                        let entry = ballot
                            .entry((vote.value.certified_block_hash, vote.value.state.clone()))
                            .or_insert(0);
                        *entry += self.configuration.weight(&vote.value.author);
                        if *entry >= self.configuration.quorum_threshold() {
                            Some(ElectionState::Won {
                                block_hash: vote.value.certified_block_hash,
                                state: vote.value.state,
                            })
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(won_election) = has_newly_won_election {
                    self.current_election = won_election;
                }
            }
            Record::QuorumCertificate(qc) => {
                let block_hash = qc.value.certified_block_hash;
                let qc_hash = QuorumCertificateHash(hash);
                let qc_round = qc.value.round;
                let qc_state = qc.value.state.clone();
                self.quorum_certificates.insert(qc_hash, qc);
                // Make sure that the state in the QC is known to execution.
                match self.compute_state(block_hash, context) {
                    Some(state) => {
                        ensure!(
                            state == qc_state,
                            "I computed a different state for a QC. This is very bad: {:?}",
                            qc_state
                        );
                    }
                    None => {
                        bail!("I failed to execute a block with a QC at {:?} while my last commit is at {:?}", qc_round, self.highest_committed_round);
                    }
                }
                // Update computed values.
                if qc_round > self.highest_quorum_certificate_round {
                    self.highest_quorum_certificate_round = qc_round;
                    self.highest_quorum_certificate_hash = qc_hash;
                }
                self.update_current_round(qc_round + 1);
                self.update_commit_3chain_round(qc_hash);
            }
            Record::Timeout(timeout) => {
                self.current_timeouts
                    .insert(timeout.value.author, timeout.clone());
                self.current_timeouts_weight += self.configuration.weight(&timeout.value.author);
                if self.current_timeouts_weight >= self.configuration.quorum_threshold() {
                    let timeout_certificate =
                        self.current_timeouts.iter().map(|x| x.1.clone()).collect();
                    self.highest_timeout_certificate = Some(timeout_certificate);
                    self.highest_timeout_certificate_round = self.current_round;
                    self.update_current_round(self.current_round + 1);
                }
            }
        }
        Ok(())
    }
}

impl<Context: SmrContext> RecordStore<Context> for RecordStoreState<Context> {
    fn current_round(&self) -> Round {
        self.current_round
    }

    fn pick_author(&self, seed: u64) -> Context::Author {
        self.configuration.pick_author(seed)
    }

    fn highest_quorum_certificate_hash(&self) -> QuorumCertificateHash<Context::HashValue> {
        self.highest_quorum_certificate_hash
    }

    fn committed_states_after(&self, after_round: Round) -> Vec<(Round, Context::State)> {
        let cc_hash = self
            .highest_commit_certificate_hash
            .unwrap_or(self.initial_hash);
        let mut iter = BackwardQuorumCertificateIterator::new(self, cc_hash);
        iter.next();
        iter.next();
        let mut commits = Vec::new();
        for qc in iter {
            if qc.value.round <= after_round {
                break;
            }
            info!("Delivering committed state for round {:?}", qc.value.round);
            commits.push((qc.value.round, qc.value.state.clone()));
        }
        commits.reverse();
        commits
    }

    fn highest_quorum_certificate_round(&self) -> Round {
        self.highest_quorum_certificate_round
    }

    fn highest_timeout_certificate_round(&self) -> Round {
        self.highest_timeout_certificate_round
    }

    fn highest_committed_round(&self) -> Round {
        self.highest_committed_round
    }

    fn previous_round(&self, block_hash: BlockHash<Context::HashValue>) -> Round {
        let block = self.block(block_hash).unwrap();
        let hash = block.value.previous_quorum_certificate_hash;
        if hash == self.initial_hash {
            Round(0)
        } else {
            let qc = self.quorum_certificate(hash).unwrap();
            let block = self.block(qc.value.certified_block_hash).unwrap();
            block.value.round
        }
    }

    fn second_previous_round(&self, block_hash: BlockHash<Context::HashValue>) -> Round {
        let block = self.block(block_hash).unwrap();
        let hash = block.value.previous_quorum_certificate_hash;
        if hash == self.initial_hash {
            Round(0)
        } else {
            let qc = self.quorum_certificate(hash).unwrap();
            self.previous_round(qc.value.certified_block_hash)
        }
    }

    fn proposed_block(
        &self,
        pacemaker: &dyn Pacemaker<Context>,
    ) -> Option<(BlockHash<Context::HashValue>, Round, Context::Author)> {
        if self.epoch_id != pacemaker.active_epoch()
            || self.current_round != pacemaker.active_round()
        {
            // Pacemaker is behind. We have already cleaned up proposals at this round.
            return None;
        }
        if let Some(leader) = pacemaker.active_leader() {
            match &self.current_proposed_block {
                None => None,
                Some(hash) => {
                    let block = self.block(*hash).unwrap();
                    assert_eq!(block.value.round, self.current_round);
                    assert_eq!(block.value.author, leader);
                    Some((*hash, block.value.round, block.value.author))
                }
            }
        } else {
            None
        }
    }

    fn create_timeout(&mut self, author: Context::Author, round: Round, context: &mut Context) {
        self.insert_network_record(
            Record::Timeout(SignedValue::make(
                context,
                Timeout_ {
                    epoch_id: self.epoch_id,
                    round,
                    highest_certified_block_round: self.highest_quorum_certificate_round(),
                    author,
                },
            )),
            context,
        );
    }

    fn has_timeout(&self, author: Context::Author, round: Round) -> bool {
        round == self.current_round && self.current_timeouts.contains_key(&author)
    }

    fn propose_block(
        &mut self,
        context: &mut Context,
        previous_quorum_certificate_hash: QuorumCertificateHash<Context::HashValue>,
        time: NodeTime,
    ) {
        if let Some(command) = context.fetch() {
            let block = Record::Block(SignedValue::make(
                context,
                Block_ {
                    command,
                    time,
                    previous_quorum_certificate_hash,
                    round: self.current_round,
                    author: context.author(),
                },
            ));
            self.insert_network_record(block, context)
        }
    }

    fn create_vote(
        &mut self,
        context: &mut Context,
        certified_block_hash: BlockHash<Context::HashValue>,
    ) -> bool {
        let committed_state = self.vote_committed_state(certified_block_hash);
        match self.compute_state(certified_block_hash, context) {
            Some(state) => {
                let vote = Record::Vote(SignedValue::make(
                    context,
                    Vote_ {
                        epoch_id: self.epoch_id,
                        round: self.block(certified_block_hash).unwrap().value.round,
                        certified_block_hash,
                        state,
                        author: context.author(),
                        committed_state,
                    },
                ));
                self.insert_network_record(vote, context);
                true
            }
            None => false,
        }
    }

    fn check_for_new_quorum_certificate(&mut self, context: &mut Context) -> bool {
        match &self.current_election {
            ElectionState::Won { block_hash, state } => {
                if self.block(*block_hash).unwrap().value.author != context.author() {
                    return false;
                }
                let committed_state = self.vote_committed_state(*block_hash);
                let authors_and_signatures = self
                    .current_votes
                    .iter()
                    .filter_map(|(_, vote)| {
                        if vote.value.state == *state {
                            Some((vote.value.author, vote.signature))
                        } else {
                            None
                        }
                    })
                    .collect();
                let quorum_certificate = Record::QuorumCertificate(SignedValue::make(
                    context,
                    QuorumCertificate_ {
                        epoch_id: self.epoch_id,
                        round: self.current_round,
                        certified_block_hash: *block_hash,
                        state: state.clone(),
                        votes: authors_and_signatures,
                        committed_state,
                        author: context.author(),
                    },
                ));
                self.current_election = ElectionState::Closed;
                self.insert_network_record(quorum_certificate, context);
                true
            }
            _ => false,
        }
    }

    fn highest_commit_certificate(&self) -> Option<&QuorumCertificate<Context>> {
        self.highest_commit_certificate_hash
            .map(|hash| self.quorum_certificate(hash).unwrap())
    }

    fn highest_quorum_certificate(&self) -> Option<&QuorumCertificate<Context>> {
        self.quorum_certificate(self.highest_quorum_certificate_hash)
    }

    fn timeouts(&self) -> Vec<Timeout<Context>> {
        let mut timeouts = Vec::new();
        if let Some(highest_tc) = &self.highest_timeout_certificate {
            timeouts.extend(highest_tc.iter().cloned());
        }
        timeouts.extend(self.current_timeouts.iter().map(|(_, tc)| tc.clone()));
        timeouts
    }

    fn block(&self, block_hash: BlockHash<Context::HashValue>) -> Option<&Block<Context>> {
        self.blocks.get(&block_hash)
    }

    fn current_vote(&self, local_author: Context::Author) -> Option<&Vote<Context>> {
        self.current_votes.get(&local_author)
    }

    fn known_quorum_certificate_rounds(&self) -> BTreeSet<Round> {
        let highest_qc_hash = self.highest_quorum_certificate_hash;
        let highest_cc_hash = self
            .highest_commit_certificate_hash
            .unwrap_or(self.initial_hash);
        let mut result = BTreeSet::new();
        for n in self
            .ancestor_rounds(highest_qc_hash)
            .enumerate()
            .filter_map(|(i, x)| if is_power2_minus1(i) { Some(x) } else { None })
        {
            result.insert(n);
        }
        for n in self
            .ancestor_rounds(highest_cc_hash)
            .enumerate()
            .filter_map(|(i, x)| if is_power2_minus1(i) { Some(x) } else { None })
        {
            result.insert(n);
        }
        result
    }

    fn unknown_records(&self, known_qc_rounds: BTreeSet<Round>) -> Vec<Record<Context>> {
        let highest_qc_hash = self.highest_quorum_certificate_hash;
        let highest_cc_hash = self
            .highest_commit_certificate_hash
            .unwrap_or(self.initial_hash);
        let chain1: Vec<_> = BackwardQuorumCertificateIterator::new(self, highest_qc_hash)
            .take_while(|qc| !known_qc_rounds.contains(&qc.value.round))
            .collect();
        let chain2: Vec<_> = BackwardQuorumCertificateIterator::new(self, highest_cc_hash)
            .take_while(|qc| !known_qc_rounds.contains(&qc.value.round))
            .collect();
        let qcs = merge_sort(chain1.into_iter(), chain2.into_iter(), |qc1, qc2| {
            qc2.value.round.cmp(&qc1.value.round)
        });
        let mut result = Vec::new();
        for n in (0..qcs.len()).rev() {
            let qc = qcs[n];
            let block = self.block(qc.value.certified_block_hash).unwrap();
            result.push(Record::Block(block.clone()));
            result.push(Record::QuorumCertificate(qc.clone()));
        }
        // Copying timeouts again.
        for timeout in self.timeouts() {
            result.push(Record::Timeout(timeout.clone()));
        }
        // Skipping votes intentionally.
        if let Some(block_hash) = &self.current_proposed_block {
            result.push(Record::Block(self.block(*block_hash).unwrap().clone()));
        }
        result
    }

    fn insert_network_record(&mut self, record: Record<Context>, context: &mut Context) {
        debug!("Inserting {:?}", record);
        match self.try_insert_network_record(record, context) {
            Err(err) => {
                debug!("=> Skipped: {}", err);
            }
            Ok(()) => (),
        };
        // TODO: discard unneeded records from self.blocks and self.quorum_certificates
    }
}
