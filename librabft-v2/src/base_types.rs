// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "unit_tests/base_type_tests.rs"]
mod base_type_tests;

// The following types are simplified for simulation purposes.
#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Debug, Serialize, Deserialize)]
pub(crate) struct BlockHash<V>(pub V);
#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Debug, Serialize, Deserialize)]
pub(crate) struct QuorumCertificateHash<V>(pub V);

pub(crate) fn is_power2_minus1(x: usize) -> bool {
    (x & (x + 1)) == 0
}

pub(crate) fn merge_sort<T: Eq, I: IntoIterator<Item = T>, F: Fn(&T, &T) -> std::cmp::Ordering>(
    v1: I,
    v2: I,
    cmp: F,
) -> Vec<T> {
    let mut iter1 = v1.into_iter();
    let mut iter2 = v2.into_iter();
    let mut result = Vec::new();
    let mut head1 = iter1.next();
    let mut head2 = iter2.next();
    while let (Some(x1), Some(x2)) = (&head1, &head2) {
        match cmp(&x1, &x2) {
            std::cmp::Ordering::Less => {
                result.push(head1.unwrap());
                head1 = iter1.next();
            }
            std::cmp::Ordering::Equal => {
                if head1 == head2 {
                    result.push(head1.unwrap());
                } else {
                    result.push(head1.unwrap());
                    result.push(head2.unwrap());
                }
                head1 = iter1.next();
                head2 = iter2.next();
            }
            std::cmp::Ordering::Greater => {
                result.push(head2.unwrap());
                head2 = iter2.next();
            }
        }
    }
    while let Some(x1) = head1 {
        result.push(x1);
        head1 = iter1.next();
    }
    while let Some(x2) = head2 {
        result.push(x2);
        head2 = iter2.next();
    }
    result
}
