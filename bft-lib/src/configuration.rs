// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "unit_tests/configuration_tests.rs"]
mod configuration_tests;

#[derive(Eq, PartialEq, Clone, Debug)]
/// Hold voting rights for a give epoch.
pub struct EpochConfiguration {
    voting_rights: BTreeMap<Author, usize>,
    total_votes: usize,
}

impl EpochConfiguration {
    pub fn new(voting_rights: BTreeMap<Author, usize>) -> Self {
        let total_votes = voting_rights.iter().fold(0, |sum, (_, votes)| sum + *votes);
        EpochConfiguration {
            voting_rights,
            total_votes,
        }
    }

    pub fn weight(&self, author: &Author) -> usize {
        *self.voting_rights.get(author).unwrap_or(&0)
    }

    pub fn count_votes<'a, I>(&'a self, authors: I) -> usize
    where
        I: IntoIterator<Item = &'a Author>,
    {
        authors.into_iter().fold(0, |sum, author| {
            sum + self.voting_rights.get(author).unwrap_or(&0)
        })
    }

    pub fn quorum_threshold(&self) -> usize {
        // If N = 3f + 1 + k (0 <= k < 3)
        // then (2 N + 3) / 3 = 2f + 1 + (2k + 2)/3 = 2f + 1 + k = N - f
        2 * self.total_votes / 3 + 1
    }

    pub fn validity_threshold(&self) -> usize {
        // If N = 3f + 1 + k (0 <= k < 3)
        // then (N + 2) / 3 = f + 1 + k/3 = f + 1
        (self.total_votes + 2) / 3
    }

    pub fn pick_author(&self, seed: u64) -> Author {
        // TODO: this is linear-time.
        let mut target = seed as usize % self.total_votes;
        for (author, votes) in &self.voting_rights {
            if *votes > target {
                return *author;
            }
            target -= *votes;
        }
        unreachable!()
    }
}
