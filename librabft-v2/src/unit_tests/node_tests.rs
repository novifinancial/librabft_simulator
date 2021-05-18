// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::base_types::BlockHash;
use bft_lib::{simulated_context::*, smr_context, smr_context::*};
use futures::executor::block_on;

#[test]
fn test_node() {
    let mut context = SimulatedContext::new(
        smr_context::Config::new(Author(0)),
        /* num_nodes */ 1,
        /* max commands per epoch */ 2,
    );
    let epoch_id = EpochId(0);
    let initial_hash = QuorumCertificateHash(context.hash(&epoch_id));
    let initial_state = context.last_committed_state();
    let mut node1 = block_on(NodeState::load_node(&mut context, NodeTime(0)));

    // Make a sequence of blocks / QCs
    let cmd = context.fetch().unwrap();
    let b0 = Record::make_block(
        &mut context,
        cmd.clone(),
        NodeTime(1),
        initial_hash,
        Round(1),
        Author(0),
    );

    let block_hash = BlockHash(context.hash(&b0));

    let state = context
        .compute(&initial_state, cmd, NodeTime(1), None, Vec::new())
        .unwrap();

    let v0 = match Record::make_vote(
        &mut context,
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
        &mut context,
        epoch_id,
        Round(1),
        block_hash,
        state,
        /* votes */ vec![(Author(0), v0.signature)],
        /* commitment */ None,
        Author(0),
    );
    let qc_hash = QuorumCertificateHash(context.hash(&qc0));

    node1.insert_network_record(epoch_id, b0, &mut context);
    node1.insert_network_record(epoch_id, qc0, &mut context);
    assert_eq!(
        node1.record_store.highest_quorum_certificate_hash(),
        qc_hash
    );
}
