// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn test_count() {
    let mut rights = BTreeMap::new();
    rights.insert("0", 1);
    rights.insert("1", 2);
    rights.insert("2", 3);
    let config = EpochConfiguration::new(rights);
    assert_eq!(config.total_votes, 6);

    assert_eq!(config.count_votes(vec![&"1"]), 2);
    assert_eq!(config.count_votes(vec![&"4"]), 0);
}

#[test]
fn test_pick_author() {
    let mut rights = BTreeMap::new();
    rights.insert("0", 1);
    rights.insert("1", 2);
    rights.insert("2", 5);
    let config = EpochConfiguration::new(rights);

    let mut hits = BTreeMap::new();
    for seed in 10..(10 + config.total_votes) {
        let author = config.pick_author(seed as u64);
        *hits.entry(author).or_insert(0) += 1;
    }
    let mut results = hits.iter().map(|x| *x.1).collect::<Vec<_>>();
    results.sort_unstable();
    assert_eq!(vec![1, 2, 5], results);
}

fn equal_configuration(num_nodes: usize) -> EpochConfiguration<usize> {
    let mut voting_rights = BTreeMap::new();
    for index in 0..num_nodes {
        voting_rights.insert(index, 1);
    }
    EpochConfiguration::new(voting_rights)
}

#[test]
fn test_quorum() {
    assert_eq!(equal_configuration(1).quorum_threshold(), 1);
    assert_eq!(equal_configuration(2).quorum_threshold(), 2);
    assert_eq!(equal_configuration(3).quorum_threshold(), 3);
    assert_eq!(equal_configuration(4).quorum_threshold(), 3);
    assert_eq!(equal_configuration(5).quorum_threshold(), 4);
    assert_eq!(equal_configuration(6).quorum_threshold(), 5);
}
