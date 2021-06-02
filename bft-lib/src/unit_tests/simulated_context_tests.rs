// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Foo(u32);

#[derive(Serialize, Deserialize)]
struct Bar(u32);

impl BcsSignable for Foo {}
impl BcsSignable for Bar {}

#[test]
fn test_hashing_and_signing() {
    let mut context = SimulatedContext::new(
        Author(0),
        (),
        /* num_nodes */ 2,
        /* max commands per epoch */ 2,
    );
    let h1 = context.hash(&Foo(35));
    let h2 = context.hash(&Bar(35));

    let sig1 = context.sign(h1).unwrap();
    assert!(context.verify(Author(0), h1, sig1).is_ok());
    assert!(context.verify(Author(1), h1, sig1).is_err());
    assert!(context.verify(Author(0), h2, sig1).is_err());

    let bytes = bcs::to_bytes(&Foo(35)).unwrap();
    let mut hasher = DefaultHasher::default();
    hasher.write(b"Foo::");
    hasher.write(&bytes);
    let h = hasher.finish();
    assert_eq!(h1, h);
}

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

#[derive(Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
struct DummyCertificate;

impl CommitCertificate<State> for DummyCertificate {
    fn committed_state(&self) -> Option<&State> {
        None
    }
}

#[test]
fn test_simulated_context() {
    let mut context = SimulatedContext::new(
        Author(0),
        (),
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

    StateFinalizer::<State>::commit(&mut context, &s1, None);
    StateFinalizer::<State>::commit(&mut context, &s2, Some(&DummyCertificate));
    StateFinalizer::<State>::discard(&mut context, &s3);

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
