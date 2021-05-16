// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn test_is_power2_minus1() {
    assert!(is_power2_minus1(1));
    assert!(is_power2_minus1(3));
    assert!(is_power2_minus1(7));
    assert!(!is_power2_minus1(8));
    assert!(!is_power2_minus1(2));
}

#[test]
fn test_merge_sort() {
    assert_eq!(
        vec![0, 2, 5, 6, 9],
        merge_sort(vec![0, 2, 6, 9], vec![2, 5, 6], u64::cmp),
    );
}
