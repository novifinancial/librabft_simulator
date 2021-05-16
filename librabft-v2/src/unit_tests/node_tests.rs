// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::{simulated_context::*, smr_context::*};
use futures::executor::block_on;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

#[test]
fn test_node() {
    let mut context = SimulatedContext::new(
        Config::new(Author(0)),
        /* num_nodes */ 1,
        /* max commands per epoch */ 2,
    );
    let initial_hash = QuorumCertificateHash(0);
    let initial_state = context.last_committed_state();
    let epoch_id = EpochId(0);
    let mut node1 = block_on(NodeState::load_node(&mut context, NodeTime(0)));

    // Make a sequence of blocks / QCs
    let cmd = context.fetch().unwrap();
    let b0 = Record::make_block(cmd.clone(), NodeTime(1), initial_hash, Round(1), Author(0));

    let mut hasher = DefaultHasher::new();
    b0.hash(&mut hasher);
    let block_hash = BlockHash(hasher.finish());

    let state = context
        .compute(&initial_state, cmd, NodeTime(1), None, Vec::new())
        .unwrap();

    let v0 = match Record::make_vote(
        epoch_id,
        Round(1),
        block_hash,
        state.clone(),
        Author(0),
        /* commitment */ None,
    ) {
        Record::Vote(x) => x,
        _ => unreachable!(),
    };
    let qc0 = Record::make_quorum_certificate(
        epoch_id,
        Round(1),
        block_hash,
        state,
        /* votes */ vec![(Author(0), v0.signature)],
        /* commitment */ None,
        Author(0),
    );
    let qc_hash = QuorumCertificateHash(qc0.digest());

    node1.insert_network_record(epoch_id, b0, &mut context);
    node1.insert_network_record(epoch_id, qc0, &mut context);
    assert_eq!(
        node1.record_store.highest_quorum_certificate_hash(),
        qc_hash
    );
}
