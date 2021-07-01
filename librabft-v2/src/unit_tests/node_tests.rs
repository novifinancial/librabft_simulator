// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::{node::NodeConfig, record::BlockHash};
use bft_lib::{simulated_context::*, smr_context::*};
use futures::executor::block_on;

#[test]
fn test_node() {
    let mut context = SimulatedContext::new(
        Author(0),
        /* num_nodes */ 1,
        /* max commands per epoch */ 2,
    );
    let mut node0 = NodeState::make_initial_state(&context, NodeConfig::default(), NodeTime(0));
    block_on(node0.save_node(&mut context)).unwrap();

    let mut node1 = block_on(NodeState::load_node(&mut context, NodeTime(0))).unwrap();
    assert_eq!(node0, node1);

    let epoch_id = EpochId(0);
    let initial_hash = QuorumCertificateHash(context.hash(&epoch_id));
    let initial_state = context.last_committed_state();

    // Make a sequence of blocks / QCs
    let cmd = context.fetch().unwrap();
    let b0 = SignedValue::make(
        &mut context,
        Block_ {
            command: cmd.clone(),
            time: NodeTime(1),
            previous_quorum_certificate_hash: initial_hash,
            round: Round(1),
            author: Author(0),
        },
    );

    let block_hash = BlockHash(context.hash(&b0.value));

    let state = context
        .compute(&initial_state, cmd, NodeTime(1), None, Vec::new())
        .unwrap();

    let v0 = SignedValue::make(
        &mut context,
        Vote_::<SimulatedContext> {
            epoch_id,
            round: Round(1),
            certified_block_hash: block_hash,
            state: state.clone(),
            author: Author(0),
            committed_state: None,
        },
    );
    let qc0 = SignedValue::make(
        &mut context,
        QuorumCertificate_ {
            epoch_id,
            round: Round(1),
            certified_block_hash: block_hash,
            state,
            votes: vec![(Author(0), v0.signature)],
            committed_state: None,
            author: Author(0),
        },
    );
    let qc_hash = QuorumCertificateHash(context.hash(&qc0.value));

    node1.insert_network_record(epoch_id, Record::Block(b0), &mut context);
    node1.insert_network_record(epoch_id, Record::QuorumCertificate(qc0), &mut context);
    assert_eq!(
        node1.record_store.highest_quorum_certificate_hash(),
        qc_hash
    );
}
