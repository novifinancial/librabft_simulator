// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use simulated_context::SimulatedContext;
use smr_context::*;

struct SharedRecordStore {
    store: RecordStoreState,
    contexts: HashMap<Author, SimulatedContext>,
}

impl SharedRecordStore {
    fn new(num_nodes: usize, epoch_ttl: usize) -> Self {
        let epoch_id = EpochId(0);
        let initial_hash = QuorumCertificateHash(0);
        let mut contexts = HashMap::new();
        for i in 0..num_nodes {
            contexts.insert(
                Author(i),
                SimulatedContext::new(Author(i), num_nodes, epoch_ttl),
            );
        }
        let state = contexts
            .get(&Author(0))
            .unwrap()
            .last_committed_state()
            ;
        SharedRecordStore {
            store: RecordStoreState::new(
                initial_hash,
                state.clone(),
                epoch_id,
                contexts.get(&Author(0)).unwrap().configuration(&state),
            ),
            contexts,
        }
    }

    fn create_timeout(&mut self, author_id: usize, round: Round) {
        let author = Author(author_id);
        self.store
            .create_timeout(author, round, self.contexts.get_mut(&author).unwrap())
    }

    fn propose_block(
        &mut self,
        author_id: usize,
        previous_qc_hash: QuorumCertificateHash,
        clock: NodeTime,
    ) {
        let author = Author(author_id);
        self.store.propose_block(
            author,
            previous_qc_hash,
            clock,
            self.contexts.get_mut(&author).unwrap(),
        );
    }

    fn create_vote(&mut self, author_id: usize, block_hash: BlockHash) -> bool {
        let author = Author(author_id);
        self.store
            .create_vote(author, block_hash, self.contexts.get_mut(&author).unwrap())
    }

    fn check_for_new_quorum_certificate(&mut self) -> bool {
        let author = self.leader(self.store.current_round());
        self.store
            .check_for_new_quorum_certificate(author, self.contexts.get_mut(&author).unwrap())
    }

    fn leader(&self, round: Round) -> Author {
        PacemakerState::leader(&self.store, round)
    }

    fn make_round(&mut self, clock: NodeTime) {
        let author = self.leader(self.store.current_round());
        let previous_qc_hash = self.store.highest_quorum_certificate_hash();
        self.store.propose_block(
            author,
            previous_qc_hash,
            clock,
            self.contexts.get_mut(&author).unwrap(),
        );
        let proposed_hash = self.store.current_proposed_block.unwrap();
        let threshold = self
            .contexts
            .get(&Author(0))
            .unwrap()
            .configuration(&self.store.initial_state)
            .quorum_threshold();
        for i in 0..threshold {
            assert!(self.create_vote(i, proposed_hash));
        }
        assert!(self.check_for_new_quorum_certificate());
    }

    fn make_tc(&mut self) {
        let threshold = self
            .contexts
            .get(&Author(0))
            .unwrap()
            .configuration(&self.store.initial_state)
            .quorum_threshold();
        let round = self.store.current_round();
        for i in 0..threshold {
            self.create_timeout(i, round);
        }
    }
}

#[test]
fn test_initial_store() {
    let shared_store = SharedRecordStore::new(2, 20);
    let store = &shared_store.store;
    assert_eq!(store.blocks.len(), 0);
    assert_eq!(store.quorum_certificates.len(), 0);
    assert_eq!(
        store.highest_quorum_certificate_hash(),
        QuorumCertificateHash(0)
    );
    assert_eq!(store.highest_quorum_certificate_round(), Round(0));
    assert_eq!(store.highest_timeout_certificate_round(), Round(0));
    assert_eq!(store.highest_committed_round(), Round(0));
    assert_eq!(store.current_round(), Round(1));
    assert_eq!(store.current_timeouts.len(), 0);
}

#[test]
fn test_propose_and_vote_no_qc() {
    let mut shared_store = SharedRecordStore::new(2, 20);
    shared_store.propose_block(0, QuorumCertificateHash(0), NodeTime(1));
    shared_store.propose_block(1, QuorumCertificateHash(0), NodeTime(2));
    let block_hashes: Vec<_> = shared_store.store.blocks.keys().cloned().collect();
    assert!(shared_store.create_vote(0, block_hashes[0]));
    assert!(shared_store.create_vote(0, block_hashes[0]));
    assert!(shared_store.create_vote(1, block_hashes[1]));
    assert!(!shared_store.check_for_new_quorum_certificate());
    // We should count only one vote per author, hence no QC.
    let store = &shared_store.store;
    assert_eq!(store.blocks.len(), 2);
    assert_eq!(store.quorum_certificates.len(), 0);
    assert_eq!(
        store.highest_quorum_certificate_hash(),
        QuorumCertificateHash(0)
    );
    assert_eq!(store.highest_quorum_certificate_round(), Round(0));
    assert_eq!(store.highest_timeout_certificate_round(), Round(0));
    assert_eq!(store.highest_committed_round(), Round(0));
    assert_eq!(store.current_round(), Round(1));
    assert_eq!(store.current_timeouts.len(), 0);
}

#[test]
fn test_vote_with_quorum() {
    let mut shared_store = SharedRecordStore::new(2, 20);
    shared_store.propose_block(0, QuorumCertificateHash(0), NodeTime(1));
    shared_store.propose_block(1, QuorumCertificateHash(0), NodeTime(2));
    let proposed_hash = shared_store.store.current_proposed_block.unwrap();
    assert!(shared_store.create_vote(0, proposed_hash));
    assert!(shared_store.create_vote(1, proposed_hash));
    assert!(shared_store.check_for_new_quorum_certificate());
    let store = &shared_store.store;
    assert_eq!(store.blocks.len(), 2);
    assert_eq!(store.quorum_certificates.len(), 1);
    assert_eq!(store.highest_quorum_certificate_round(), Round(1));
    assert_eq!(store.highest_timeout_certificate_round(), Round(0));
    assert_eq!(store.highest_committed_round(), Round(0));
    assert_eq!(store.current_round(), Round(2));
    assert_eq!(store.current_timeouts.len(), 0);
}

#[test]
fn test_timeouts_no_tc() {
    let mut shared_store = SharedRecordStore::new(2, 20);
    shared_store.propose_block(1, QuorumCertificateHash(0), NodeTime(2));
    shared_store.create_timeout(0, Round(1));
    shared_store.create_timeout(0, Round(1));
    shared_store.create_timeout(1, Round(0));
    // We should count only one timeout per author, at the current round, hence no TC.
    let store = &shared_store.store;
    assert_eq!(store.blocks.len(), 1);
    assert_eq!(store.quorum_certificates.len(), 0);
    assert_eq!(
        store.highest_quorum_certificate_hash(),
        QuorumCertificateHash(0)
    );
    assert_eq!(store.highest_quorum_certificate_round(), Round(0));
    assert_eq!(store.highest_timeout_certificate_round(), Round(0));
    assert_eq!(store.highest_committed_round(), Round(0));
    assert_eq!(store.current_round(), Round(1));
    assert_eq!(store.current_timeouts.len(), 1);
}

#[test]
fn test_timeouts_with_tc() {
    let mut shared_store = SharedRecordStore::new(2, 20);
    shared_store.propose_block(1, QuorumCertificateHash(0), NodeTime(2));
    shared_store.create_timeout(1, Round(0)); // should be ignored
    shared_store.create_timeout(0, Round(1));
    shared_store.create_timeout(1, Round(1)); // complete TC
    shared_store.create_timeout(1, Round(2)); // single timeout
    {
        let store = &shared_store.store;
        assert_eq!(store.blocks.len(), 1);
        assert_eq!(store.quorum_certificates.len(), 0);
        assert_eq!(
            store.highest_quorum_certificate_hash(),
            QuorumCertificateHash(0)
        );
        assert_eq!(store.highest_quorum_certificate_round(), Round(0));
        assert_eq!(store.highest_timeout_certificate_round(), Round(1));
        assert_eq!(store.highest_committed_round(), Round(0));
        assert_eq!(store.current_round(), Round(2));
        assert_eq!(store.current_timeouts.len(), 1);
    }
    shared_store.create_timeout(0, Round(2)); // complete TC
    let store = &shared_store.store;
    assert_eq!(store.blocks.len(), 1);
    assert_eq!(store.highest_timeout_certificate_round(), Round(2));
    assert_eq!(store.current_round(), Round(3));
    assert_eq!(store.current_timeouts.len(), 0);
}

#[test]
fn test_non_contiguous_qcs() {
    let mut shared_store = SharedRecordStore::new(2, 20);
    shared_store.make_round(NodeTime(10));
    shared_store.make_round(NodeTime(20));
    shared_store.make_tc();
    shared_store.make_round(NodeTime(40));
    let store = &shared_store.store;
    assert_eq!(store.blocks.len(), 3);
    assert_eq!(store.quorum_certificates.len(), 3);
    assert_eq!(store.highest_quorum_certificate_round(), Round(4));
    assert_eq!(store.highest_timeout_certificate_round(), Round(3));
    assert_eq!(store.highest_committed_round(), Round(0));
    assert_eq!(store.current_round(), Round(5));
    assert_eq!(store.current_timeouts.len(), 0);
}

#[test]
fn test_commit() {
    let mut shared_store = SharedRecordStore::new(2, 20);
    shared_store.make_round(NodeTime(10));
    shared_store.make_tc();
    shared_store.make_round(NodeTime(30));
    shared_store.make_round(NodeTime(40));
    shared_store.make_round(NodeTime(50));
    shared_store.make_tc();
    let store = &shared_store.store;
    assert_eq!(store.blocks.len(), 4);
    assert_eq!(store.quorum_certificates.len(), 4);
    assert_eq!(store.highest_quorum_certificate_round(), Round(5));
    assert_eq!(store.highest_timeout_certificate_round(), Round(6));
    assert_eq!(store.highest_committed_round(), Round(3));
    assert_eq!(store.current_round(), Round(7));
    assert_eq!(store.current_timeouts.len(), 0);

    assert_eq!(store.highest_commit_certificate().unwrap().round, Round(5));
    assert_eq!(
        store.previous_round(
            store
                .highest_commit_certificate()
                .unwrap()
                .certified_block_hash
        ),
        Round(4)
    );
    assert_eq!(
        store.second_previous_round(
            store
                .highest_commit_certificate()
                .unwrap()
                .certified_block_hash
        ),
        Round(3)
    );

    let commits = store.committed_states_after(Round(0));
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].0, Round(1));
    assert_eq!(commits[1].0, Round(3));
    assert_eq!(
        Some(&commits[1].1),
        store
            .highest_commit_certificate()
            .unwrap()
            .committed_state
            .as_ref()
    );
}
