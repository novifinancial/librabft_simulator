// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn test_time_conversion() {
    let x = GlobalTime(15);
    let start = GlobalTime(3);
    assert_eq!(x.to_node_time(start), NodeTime(12));
    assert_eq!(GlobalTime::from_node_time(NodeTime(12), start), x);
}
