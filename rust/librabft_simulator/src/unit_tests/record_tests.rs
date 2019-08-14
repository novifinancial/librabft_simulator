// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn test_block_signing() {
    let b = Record::make_block(
        Command {
            proposer: Author(1),
            index: 2,
        },
        NodeTime(2),
        QuorumCertificateHash(47),
        Round(3),
        Author(2),
    );
    assert!(b.signature().check(b.digest(), b.author()).is_ok());
    assert!(b.signature().check(b.digest(), Author(1)).is_err());
    let b2 = Record::make_block(
        Command {
            proposer: Author(3),
            index: 2,
        },
        NodeTime(2),
        QuorumCertificateHash(47),
        Round(3),
        Author(2),
    );
    assert!(b.signature().check(b2.digest(), b.author()).is_err());
}
