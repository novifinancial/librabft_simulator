// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn test_happened_before() {
    let mut s1 = SimulatedLedgerState::new();
    let mut s2 = SimulatedLedgerState::new();
    assert!(!s1.happened_just_before(&s2));
    s1.execute(
        Command {
            proposer: Author(0),
            index: 0,
        },
        NodeTime(1),
    );
    assert!(!s1.happened_just_before(&s2));
    assert!(s2.happened_just_before(&s1));
    s1.execute(
        Command {
            proposer: Author(1),
            index: 0,
        },
        NodeTime(1),
    );
    s2.execute(
        Command {
            proposer: Author(1),
            index: 0,
        },
        NodeTime(1),
    );
    assert!(!s2.happened_just_before(&s1));
}

struct DummyCertificate;

impl CommitCertificate for DummyCertificate {
    fn committed_state(&self) -> Option<&State> {
        None
    }
}

#[test]
fn test_simulated_context() {
    let mut context = SimulatedContext::new(
        Config::new(Author(0)),
        /* num_nodes */ 2,
        /* max commands per epoch */ 2,
    );
    let s0 = context.last_committed_state();
    let c1 = context.fetch().unwrap();
    let c2 = context.fetch().unwrap();
    let c3 = context.fetch().unwrap();

    let s1 = context
        .compute(&s0, c1, NodeTime(1), None, Vec::new())
        .unwrap();
    assert_eq!(context.read_epoch_id(&s1), EpochId(0));

    let s2 = context
        .compute(&s1, c2, NodeTime(4), None, Vec::new())
        .unwrap();
    assert_eq!(context.read_epoch_id(&s2), EpochId(1));

    let s3 = context
        .compute(&s0, c3, NodeTime(3), None, Vec::new())
        .unwrap();
    assert_eq!(context.read_epoch_id(&s3), EpochId(0));

    StateFinalizer::<DummyCertificate>::commit(&mut context, &s1, None);
    StateFinalizer::<DummyCertificate>::commit(&mut context, &s2, None);
    StateFinalizer::<DummyCertificate>::discard(&mut context, &s3);

    assert_eq!(
        context.last_committed_ledger_state.execution_history,
        vec![
            (
                Command {
                    proposer: Author(0),
                    index: 0,
                },
                NodeTime(1)
            ),
            (
                Command {
                    proposer: Author(0),
                    index: 1,
                },
                NodeTime(4)
            ),
        ]
    )
}
