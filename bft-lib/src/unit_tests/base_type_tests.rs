// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn test_round_plus_usize() {
    assert_eq!(Round(3) + 4, Round(7));
}
