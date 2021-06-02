// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::node::Config;
use bft_lib::{simulated_context::*, smr_context::CryptographicModule};

#[test]
fn test_block_signing() {
    let mut context = SimulatedContext::new(
        Author(2),
        Config::default(),
        /* not used */ 0,
        /* not used */ 0,
    );
    let b = SignedValue::make(
        &mut context,
        Block_::<SimulatedContext<Config>> {
            command: Command {
                proposer: Author(1),
                index: 2,
            },
            time: NodeTime(2),
            previous_quorum_certificate_hash: QuorumCertificateHash(47),
            round: Round(3),
            author: Author(2),
        },
    );
    assert!(b.verify(&context).is_ok());
    assert!(context
        .verify(Author(1), context.hash(&b.value), b.signature)
        .is_err());

    let b2 = SignedValue::make(
        &mut context,
        Block_::<SimulatedContext<Config>> {
            command: Command {
                proposer: Author(3),
                index: 2,
            },
            time: NodeTime(2),
            previous_quorum_certificate_hash: QuorumCertificateHash(47),
            round: Round(3),
            author: Author(2),
        },
    );
    assert!(b2.verify(&context).is_ok());
    assert!(context
        .verify(Author(2), context.hash(&b.value), b2.signature)
        .is_err());
}
