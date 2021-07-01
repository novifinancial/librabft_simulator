// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, hash::Hash};

#[cfg(test)]
#[path = "unit_tests/configuration_tests.rs"]
mod configuration_tests;

/// Represent BFT permissions during an epoch. NOTE: The order of the nodes is recorded
/// and will influence leader selections based on `pick_author`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "Author: Hash + Eq + Serialize"))]
#[serde(bound(deserialize = "Author: Hash + Eq + Deserialize<'de>"))]
pub struct EpochConfiguration<Author: Hash> {
    authors: Vec<(Author, usize)>,
    voting_rights: HashMap<Author, usize>,
    total_votes: usize,
}

impl<Author> EpochConfiguration<Author>
where
    Author: Hash + Eq + Clone,
{
    /// Create a new epoch.
    pub fn new(authors: Vec<(Author, usize)>) -> Self {
        let voting_rights = authors.iter().cloned().collect();
        let total_votes = authors.iter().map(|(_, v)| *v).sum();
        EpochConfiguration {
            authors,
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

    // TODO: this function is linear-time in the number of nodes.
    pub fn pick_author(&self, seed: u64) -> Author {
        let mut rng = Xoshiro256StarStar::seed_from_u64(seed);
        let mut target = rng.gen_range(0..self.total_votes);
        for (author, votes) in &self.authors {
            if *votes > target {
                return author.clone();
            }
            target -= *votes;
        }
        unreachable!()
    }
}

impl<Author> PartialEq for EpochConfiguration<Author>
where
    Author: Hash + Eq + Clone,
{
    fn eq(&self, other: &Self) -> bool {
        if self.authors != other.authors {
            return false;
        }
        for (author, rights) in &self.authors {
            if other.voting_rights.get(author) != Some(rights) {
                return false;
            }
        }
        assert_eq!(self.total_votes, other.total_votes);
        true
    }
}
