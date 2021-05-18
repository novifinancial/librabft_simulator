// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use bft_lib::{
    simulated_context::*,
    smr_context::{Config, CryptographicModule},
};

#[test]
fn test_block_signing() {
    let mut context = SimulatedContext::new(
        Config::new(Author(2)),
        /* not used */ 0,
        /* not used */ 0,
    );
    let b = Record::make_block(
        &mut context,
        Command {
            proposer: Author(1),
            index: 2,
        },
        NodeTime(2),
        QuorumCertificateHash(47),
        Round(3),
        Author(2),
    );
    let hash = context.hash(&b);
    assert!(context.verify(b.author(), hash, b.signature()).is_ok());
    assert!(context.verify(Author(1), hash, b.signature()).is_err());
    let mut context = SimulatedContext::new(
        Config::new(Author(2)),
        /* not used */ 0,
        /* not used */ 0,
    );
    let b2 = Record::make_block(
        &mut context,
        Command {
            proposer: Author(3),
            index: 2,
        },
        NodeTime(2),
        QuorumCertificateHash(47),
        Round(3),
        Author(2),
    );
    let hash = context.hash(&b2);
    assert!(context.verify(b.author(), hash, b.signature()).is_err());
}
