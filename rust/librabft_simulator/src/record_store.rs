// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use base_types::*;
use pacemaker::{Pacemaker, PacemakerState};
use record::*;
use smr_context::SMRContext;
use std::{
    collections::{BTreeSet, HashMap},
    fmt::Debug,
};

#[cfg(test)]
#[path = "unit_tests/record_store_tests.rs"]
mod record_store_tests;

// -- BEGIN FILE record_store --
pub trait RecordStore: Debug {
    /// Return the hash of a QC at the highest round, or the initial hash.
    fn highest_quorum_certificate_hash(&self) -> QuorumCertificateHash;
    /// Query the round of the highest QC.
    fn highest_quorum_certificate_round(&self) -> Round;
    /// Query the highest QC.
    fn highest_quorum_certificate(&self) -> Option<&QuorumCertificate>;
    /// Query the round of the highest TC.
    fn highest_timeout_certificate_round(&self) -> Round;
    /// Query the round of the highest commit.
    fn highest_committed_round(&self) -> Round;
    /// Query the last QC of the highest commit rule.
    fn highest_commit_certificate(&self) -> Option<&QuorumCertificate>;
    /// Current round as seen by the record store.
    fn current_round(&self) -> Round;

    /// Iterate on the committed blocks starting after the round `after_round` and ending with the
    /// highest commit known so far.
    fn committed_states_after(&self, after_round: Round) -> Vec<(Round, State)>;

    /// Access the block proposed by the leader chosen by the Pacemaker (if any).
    fn proposed_block(&self, pacemaker: &Pacemaker) -> Option<(BlockHash, Round, Author)>;
    /// Check if a timeout already exists.
    fn has_timeout(&self, author: Author, round: Round) -> bool;

    /// Create a timeout.
    fn create_timeout(&mut self, author: Author, round: Round, smr_context: &mut SMRContext);
    /// Fetch a command from mempool and propose a block.
    fn propose_block(
        &mut self,
        local_author: Author,
        previous_qc_hash: QuorumCertificateHash,
        clock: NodeTime,
        smr_context: &mut SMRContext,
    );
    /// Execute the command contained in a block and vote for the resulting state.
    /// Return false if the execution failed.
    fn create_vote(
        &mut self,
        local_author: Author,
        block_hash: BlockHash,
        smr_context: &mut SMRContext,
    ) -> bool;
    /// Try to create a QC for the last block that we have proposed.
    fn check_for_new_quorum_certificate(
        &mut self,
        local_author: Author,
        smr_context: &mut SMRContext,
    ) -> bool;

    /// Compute the previous round and the second previous round of a block.
    fn previous_round(&self, block_hash: BlockHash) -> Round;
    fn second_previous_round(&self, block_hash: BlockHash) -> Round;
    /// Pick an author based on a seed, with chances proportional to voting rights.
    fn pick_author(&self, seed: u64) -> Author;

    /// APIs supporting data synchronization.
    fn timeouts(&self) -> Vec<Timeout>;
    fn current_vote(&self, local_author: Author) -> Option<&Vote>;
    fn block(&self, block_hash: BlockHash) -> Option<&Block>;
    fn known_quorum_certificate_rounds(&self) -> BTreeSet<Round>;
    fn unknown_records(&self, known_qc_rounds: BTreeSet<Round>) -> Vec<Record>;
    fn insert_network_record(&mut self, record: Record, smr_context: &mut SMRContext);
}
// -- END FILE --

// -- BEGIN FILE record_store_state --
#[derive(Debug)]
pub struct RecordStoreState {
    /// Epoch initialization.
    epoch_id: EpochId,
    configuration: EpochConfiguration,
    initial_hash: QuorumCertificateHash,
    initial_state: State,
    /// Storage of verified blocks and QCs.
    blocks: HashMap<BlockHash, Block>,
    quorum_certificates: HashMap<QuorumCertificateHash, QuorumCertificate>,
    current_proposed_block: Option<BlockHash>,
    /// Computed round values.
    highest_quorum_certificate_round: Round,
    highest_quorum_certificate_hash: QuorumCertificateHash,
    highest_timeout_certificate_round: Round,
    current_round: Round,
    highest_committed_round: Round,
    highest_commit_certificate_hash: Option<QuorumCertificateHash>,
    /// Storage of verified timeouts at the highest TC round.
    highest_timeout_certificate: Option<Vec<Timeout>>,
    /// Storage of verified votes and timeouts at the current round.
    current_timeouts: HashMap<Author, Timeout>,
    current_votes: HashMap<Author, Vote>,
    /// Computed weight values.
    current_timeouts_weight: usize,
    current_election: ElectionState,
}

/// Counting votes for a proposed block and its execution state.
#[derive(Debug)]
enum ElectionState {
    Ongoing {
        ballot: HashMap<(BlockHash, State), usize>,
    },
    Won {
        block_hash: BlockHash,
        state: State,
    },
    Closed,
}
// -- END FILE --

struct BackwardQuorumCertificateIterator<'a> {
    store: &'a RecordStoreState,
    current_hash: QuorumCertificateHash,
}

impl<'a> BackwardQuorumCertificateIterator<'a> {
    fn new(
        store: &'a RecordStoreState,
        qc_hash: QuorumCertificateHash,
    ) -> BackwardQuorumCertificateIterator<'a> {
        BackwardQuorumCertificateIterator {
            store,
            current_hash: qc_hash,
        }
    }
}

impl<'a> Iterator for BackwardQuorumCertificateIterator<'a> {
    type Item = &'a QuorumCertificate;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_hash == self.store.initial_hash {
            return None;
        }
        let qc = self.store.quorum_certificate(self.current_hash).unwrap();
        let block = self.store.block(qc.certified_block_hash).unwrap();
        self.current_hash = block.previous_quorum_certificate_hash;
        return Some(qc);
    }
}

impl RecordStoreState {
    pub fn new(
        initial_hash: QuorumCertificateHash,
        initial_state: State,
        epoch_id: EpochId,
        configuration: EpochConfiguration,
    ) -> RecordStoreState {
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

    fn ancestor_rounds<'a>(
        &'a self,
        qc_hash: QuorumCertificateHash,
    ) -> impl Iterator<Item = Round> + 'a {
        BackwardQuorumCertificateIterator::new(self, qc_hash).map(|qc| qc.round)
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

    fn update_commit_3chain_round(&mut self, qc_hash: QuorumCertificateHash) {
        let rounds = {
            let mut iter = self.ancestor_rounds(qc_hash);
            let r3 = iter.next();
            let r2 = iter.next();
            let r1 = iter.next();
            (r1, r2, r3)
        };
        if let (Some(r1), Some(r2), Some(r3)) = rounds {
            if r3 == r2 + 1 && r2 == r1 + 1 {
                if r1 > self.highest_committed_round {
                    self.highest_committed_round = r1;
                    self.highest_commit_certificate_hash = Some(qc_hash);
                }
            }
        }
    }

    fn vote_committed_state(&self, block_hash: BlockHash) -> Option<State> {
        let block = self.block(block_hash).unwrap();
        let r3 = block.round;
        let qc2_hash = block.previous_quorum_certificate_hash;
        let mut iter = BackwardQuorumCertificateIterator::new(&self, qc2_hash);
        let opt_qc2 = iter.next();
        let opt_qc1 = iter.next();
        if let (Some(qc1), Some(qc2)) = (opt_qc1, opt_qc2) {
            let r2 = qc2.round;
            let r1 = qc1.round;
            if r3 == r2 + 1 && r2 == r1 + 1 {
                return Some(qc1.state.clone());
            }
        }
        None
    }

    fn verify_network_record(&self, record: &Record) -> Result<u64> {
        let hash = record.digest();
        match record {
            Record::Block(block) => {
                ensure!(
                    !self.blocks.contains_key(&BlockHash(hash)),
                    "Block was already inserted."
                );
                block.signature.check(hash, block.author)?;
                ensure!(
                    block.previous_quorum_certificate_hash == self.initial_hash
                        || self
                            .quorum_certificates
                            .contains_key(&block.previous_quorum_certificate_hash),
                    "The previous QC (if any) must be verified first."
                );
                if self.initial_hash == block.previous_quorum_certificate_hash {
                    ensure!(block.round > Round(0), "Rounds must start at 1");
                } else {
                    let previous_qc = self
                        .quorum_certificate(block.previous_quorum_certificate_hash)
                        .unwrap();
                    let previous_block = self.block(previous_qc.certified_block_hash).unwrap();
                    ensure!(
                        block.round > previous_block.round,
                        "Rounds must be increasing"
                    );
                }
            }
            Record::Vote(vote) => {
                ensure!(
                    vote.epoch_id == self.epoch_id,
                    "Epoch identifier of vote ({:?}) must match the current epoch ({:?}).",
                    vote.epoch_id,
                    self.epoch_id
                );
                ensure!(
                    self.blocks.contains_key(&vote.certified_block_hash),
                    "The certified block hash of a vote must be verified first."
                );
                ensure!(
                    self.block(vote.certified_block_hash).unwrap().round == vote.round,
                    "The round of the vote must match the certified block."
                );
                ensure!(
                    self.vote_committed_state(vote.certified_block_hash) == vote.committed_state,
                    "The committed_state value of a vote must follow the commit rule."
                );
                ensure!(
                    vote.round == self.current_round,
                    "Only accepting votes for a proposal at the current {:?}. This one was at {:?}",
                    self.current_round,
                    vote.round
                );
                ensure!(
                    !self.current_votes.contains_key(&vote.author),
                    "We insert votes only for authors who haven't voted yet."
                );
                vote.signature.check(hash, vote.author)?
            }
            Record::QuorumCertificate(qc) => {
                ensure!(
                    qc.epoch_id == self.epoch_id,
                    "Epoch identifier of QC ({:?}) must match the current epoch ({:?}).",
                    qc.epoch_id,
                    self.epoch_id
                );
                ensure!(
                    !self
                        .quorum_certificates
                        .contains_key(&QuorumCertificateHash(hash)),
                    "QuorumCertificate was already inserted."
                );
                ensure!(
                    self.blocks.contains_key(&qc.certified_block_hash),
                    "The certified block hash of a QC must be verified first."
                );
                ensure!(
                    self.block(qc.certified_block_hash).unwrap().round == qc.round,
                    "The round of the QC must match the certified block."
                );
                ensure!(
                    qc.author == self.block(qc.certified_block_hash).unwrap().author,
                    "QCs must be created by the author of the certified block"
                );
                ensure!(
                    self.vote_committed_state(qc.certified_block_hash) == qc.committed_state,
                    "The committed_state value of a QC must follow the commit rule."
                );
                let mut weight = 0;
                for (author, signature) in &qc.votes {
                    let original_vote_digest = Record::digest(&Record::Vote(Vote {
                        epoch_id: self.epoch_id,
                        round: qc.round,
                        certified_block_hash: qc.certified_block_hash,
                        state: qc.state.clone(),
                        committed_state: qc.committed_state.clone(),
                        author: author.clone(),
                        signature: Signature(0), // ignored
                    }));
                    signature.check(original_vote_digest, *author)?;
                    weight += self.configuration.weight(author);
                }
                ensure!(
                    weight >= self.configuration.quorum_threshold(),
                    "Votes in QCs must form a quorum"
                );
                // TODO: do not recompute hash
                qc.signature
                    .check(Record::QuorumCertificate(qc.clone()).digest(), qc.author)?;
            }
            Record::Timeout(timeout) => {
                ensure!(
                    timeout.epoch_id == self.epoch_id,
                    "Epoch identifier of timeout ({:?}) must match the current epoch ({:?}).",
                    timeout.epoch_id,
                    self.epoch_id
                );
                ensure!(
                    timeout.highest_certified_block_round
                        <= self.highest_quorum_certificate_round(),
                    "Timeouts must refer to a known certified block round."
                );
                ensure!(
                    timeout.round == self.current_round,
                    "Accepting only timeouts at the current {:?}. This one was at {:?}",
                    self.current_round,
                    timeout.round
                );
                ensure!(
                    !self.current_timeouts.contains_key(&timeout.author),
                    "A timeout is already known for the same round and the same author"
                );
                timeout.signature.check(hash, timeout.author)?;
            }
        }
        Ok(hash)
    }

    fn quorum_certificate(&self, qc_hash: QuorumCertificateHash) -> Option<&QuorumCertificate> {
        self.quorum_certificates.get(&qc_hash)
    }

    fn compute_state(&self, block_hash: BlockHash, smr_context: &mut SMRContext) -> Option<State> {
        let block = self.block(block_hash).unwrap();
        let (previous_state, previous_voters, previous_author) = {
            if block.previous_quorum_certificate_hash == self.initial_hash {
                (&self.initial_state, None, Vec::new())
            } else {
                let previous_qc = self
                    .quorum_certificate(block.previous_quorum_certificate_hash)
                    .unwrap();
                let voters = previous_qc.votes.iter().map(|x| x.0).collect();
                (&previous_qc.state, Some(previous_qc.author), voters)
            }
        };
        smr_context.compute(
            previous_state,
            block.command.clone(),
            block.time,
            previous_voters,
            previous_author,
        )
    }

    fn try_insert_network_record(
        &mut self,
        record: Record,
        smr_context: &mut SMRContext,
    ) -> Result<()> {
        // First, check that the record is "relevant" and that invariants of "verified records",
        // such as chaining, are respected.
        let hash = self.verify_network_record(&record)?;
        // Second, insert the record. In the case of QC, this is where check execution states.
        match record {
            Record::Block(block) => {
                let block_hash = BlockHash(hash);
                if block.round == self.current_round
                    && PacemakerState::leader(&*self, block.round) == block.author
                {
                    // If we use a VRF, this assumes that we have inserted the highest commit rule
                    // beforehand.
                    self.current_proposed_block = Some(block_hash);
                }
                self.blocks.insert(block_hash, block);
            }
            Record::Vote(vote) => {
                self.current_votes.insert(vote.author, vote.clone());
                let has_newly_won_election = match &mut self.current_election {
                    ElectionState::Ongoing { ballot } => {
                        let entry = ballot
                            .entry((vote.certified_block_hash, vote.state.clone()))
                            .or_insert(0);
                        *entry += self.configuration.weight(&vote.author);
                        if *entry >= self.configuration.quorum_threshold() {
                            Some(ElectionState::Won {
                                block_hash: vote.certified_block_hash,
                                state: vote.state,
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
                let block_hash = qc.certified_block_hash;
                let qc_hash = QuorumCertificateHash(hash);
                let qc_round = qc.round;
                let qc_state = qc.state.clone();
                self.quorum_certificates.insert(qc_hash, qc);
                // Make sure that the state in the QC is known to execution.
                match self.compute_state(block_hash, smr_context) {
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
                    .insert(timeout.author, timeout.clone());
                self.current_timeouts_weight += self.configuration.weight(&timeout.author);
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

impl RecordStore for RecordStoreState {
    fn current_round(&self) -> Round {
        self.current_round
    }

    fn pick_author(&self, seed: u64) -> Author {
        self.configuration.pick_author(seed)
    }

    fn highest_quorum_certificate_hash(&self) -> QuorumCertificateHash {
        self.highest_quorum_certificate_hash
    }

    fn committed_states_after(&self, after_round: Round) -> Vec<(Round, State)> {
        let cc_hash = self
            .highest_commit_certificate_hash
            .unwrap_or(self.initial_hash);
        let mut iter = BackwardQuorumCertificateIterator::new(self, cc_hash);
        iter.next();
        iter.next();
        let mut commits = Vec::new();
        while let Some(qc) = iter.next() {
            if qc.round <= after_round {
                break;
            }
            info!("Delivering committed state for round {:?}", qc.round);
            commits.push((qc.round, qc.state.clone()));
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

    fn previous_round(&self, block_hash: BlockHash) -> Round {
        let block = self.block(block_hash).unwrap();
        let hash = block.previous_quorum_certificate_hash;
        if hash == self.initial_hash {
            Round(0)
        } else {
            let qc = self.quorum_certificate(hash).unwrap();
            let block = self.block(qc.certified_block_hash).unwrap();
            block.round
        }
    }

    fn second_previous_round(&self, block_hash: BlockHash) -> Round {
        let block = self.block(block_hash).unwrap();
        let hash = block.previous_quorum_certificate_hash;
        if hash == self.initial_hash {
            Round(0)
        } else {
            let qc = self.quorum_certificate(hash).unwrap();
            self.previous_round(qc.certified_block_hash)
        }
    }

    fn proposed_block(&self, pacemaker: &Pacemaker) -> Option<(BlockHash, Round, Author)> {
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
                    assert_eq!(block.round, self.current_round);
                    assert_eq!(block.author, leader);
                    Some((*hash, block.round, block.author))
                }
            }
        } else {
            None
        }
    }

    fn create_timeout(&mut self, author: Author, round: Round, smr_context: &mut SMRContext) {
        self.insert_network_record(
            Record::make_timeout(
                self.epoch_id,
                round,
                self.highest_quorum_certificate_round(),
                author,
            ),
            smr_context,
        );
    }

    fn has_timeout(&self, author: Author, round: Round) -> bool {
        round == self.current_round && self.current_timeouts.contains_key(&author)
    }

    fn propose_block(
        &mut self,
        local_author: Author,
        previous_qc_hash: QuorumCertificateHash,
        clock: NodeTime,
        smr_context: &mut SMRContext,
    ) {
        if let Some(command) = smr_context.fetch() {
            let block = Record::make_block(
                command,
                clock,
                previous_qc_hash,
                self.current_round,
                local_author,
            );
            self.insert_network_record(block, smr_context)
        }
    }

    fn create_vote(
        &mut self,
        local_author: Author,
        block_hash: BlockHash,
        smr_context: &mut SMRContext,
    ) -> bool {
        let committed_state = self.vote_committed_state(block_hash);
        match self.compute_state(block_hash, smr_context) {
            Some(state) => {
                let vote = Record::make_vote(
                    self.epoch_id,
                    self.block(block_hash).unwrap().round,
                    block_hash,
                    state,
                    local_author,
                    committed_state,
                );
                self.insert_network_record(vote, smr_context);
                true
            }
            None => false,
        }
    }

    fn check_for_new_quorum_certificate(
        &mut self,
        local_author: Author,
        smr_context: &mut SMRContext,
    ) -> bool {
        let quorum_certificate = match &self.current_election {
            ElectionState::Won { block_hash, state } => {
                if self.block(*block_hash).unwrap().author != local_author {
                    return false;
                }
                let committed_state = self.vote_committed_state(*block_hash);
                let authors_and_signatures = self
                    .current_votes
                    .iter()
                    .filter_map(|(_, vote)| {
                        if vote.state == *state {
                            Some((vote.author, vote.signature))
                        } else {
                            None
                        }
                    })
                    .collect();
                let quorum_certificate = Record::make_quorum_certificate(
                    self.epoch_id,
                    self.current_round,
                    *block_hash,
                    state.clone(),
                    authors_and_signatures,
                    committed_state,
                    local_author,
                );
                quorum_certificate
            }
            _ => {
                return false;
            }
        };
        self.current_election = ElectionState::Closed;
        self.insert_network_record(quorum_certificate, smr_context);
        true
    }

    fn highest_commit_certificate(&self) -> Option<&QuorumCertificate> {
        match self.highest_commit_certificate_hash {
            Some(hash) => Some(self.quorum_certificate(hash).unwrap()),
            None => None,
        }
    }

    fn highest_quorum_certificate(&self) -> Option<&QuorumCertificate> {
        self.quorum_certificate(self.highest_quorum_certificate_hash)
    }

    fn timeouts(&self) -> Vec<Timeout> {
        let mut timeouts = Vec::new();
        if let Some(highest_tc) = &self.highest_timeout_certificate {
            timeouts.extend(highest_tc.iter().cloned());
        }
        timeouts.extend(self.current_timeouts.iter().map(|(_, tc)| tc.clone()));
        timeouts
    }

    fn block(&self, block_hash: BlockHash) -> Option<&Block> {
        self.blocks.get(&block_hash)
    }

    fn current_vote(&self, local_author: Author) -> Option<&Vote> {
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

    fn unknown_records(&self, known_qc_rounds: BTreeSet<Round>) -> Vec<Record> {
        let highest_qc_hash = self.highest_quorum_certificate_hash;
        let highest_cc_hash = self
            .highest_commit_certificate_hash
            .unwrap_or(self.initial_hash);
        let chain1: Vec<_> = BackwardQuorumCertificateIterator::new(self, highest_qc_hash)
            .take_while(|qc| !known_qc_rounds.contains(&qc.round))
            .collect();
        let chain2: Vec<_> = BackwardQuorumCertificateIterator::new(self, highest_cc_hash)
            .take_while(|qc| !known_qc_rounds.contains(&qc.round))
            .collect();
        let qcs = merge_sort(chain1.into_iter(), chain2.into_iter(), |qc1, qc2| {
            qc2.round.cmp(&qc1.round)
        });
        let mut result = Vec::new();
        for n in (0..qcs.len()).rev() {
            let qc = qcs[n];
            let block = self.block(qc.certified_block_hash).unwrap();
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

    fn insert_network_record(&mut self, record: Record, smr_context: &mut SMRContext) {
        debug!("Inserting {:?}", record);
        match self.try_insert_network_record(record, smr_context) {
            Err(err) => {
                debug!("=> Skipped: {}", err);
            }
            Ok(()) => (),
        };
        // TODO: discard unneeded records from self.blocks and self.quorum_certificates
    }
}
