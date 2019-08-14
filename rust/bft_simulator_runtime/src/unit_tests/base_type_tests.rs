// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn test_round_plus_usize() {
    assert_eq!(Round(3) + 4, Round(7));
}

#[test]
fn test_signature() {
    let sig = Signature::sign(35, Author(2));
    assert!(sig.check(35, Author(2)).is_ok());
    assert!(sig.check(32, Author(2)).is_err());
    assert!(sig.check(35, Author(1)).is_err());
}
