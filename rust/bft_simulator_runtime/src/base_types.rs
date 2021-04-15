// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use failure::Error;
use std::{
    collections::hash_map::DefaultHasher,
    fmt,
    hash::{Hash, Hasher},
};

#[cfg(test)]
#[path = "unit_tests/base_type_tests.rs"]
mod base_type_tests;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Debug)]
pub struct Round(pub usize);
#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub struct NodeTime(pub i64);
pub type Duration = i64;

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Debug)]
pub struct Author(pub usize);
#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Debug)]
pub struct Signature(pub u64);

impl fmt::Debug for NodeTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl std::ops::Add<usize> for Round {
    type Output = Round;

    fn add(self, rhs: usize) -> Self::Output {
        Round(self.0 + rhs)
    }
}

impl NodeTime {
    pub fn never() -> Self {
        NodeTime(std::i64::MAX)
    }
}

impl Default for NodeTime {
    fn default() -> Self {
        Self::never()
    }
}

impl std::ops::Add<Duration> for NodeTime {
    type Output = NodeTime;

    fn add(self, rhs: Duration) -> Self::Output {
        NodeTime(self.0 + rhs)
    }
}

impl Signature {
    pub fn sign(hash: u64, author: Author) -> Self {
        let mut hasher = DefaultHasher::new();
        hash.hash(&mut hasher);
        author.hash(&mut hasher);
        Signature(hasher.finish())
    }

    pub fn check(&self, hash: u64, author: Author) -> Result<()> {
        let mut hasher = DefaultHasher::new();
        hash.hash(&mut hasher);
        author.hash(&mut hasher);
        ensure!(hasher.finish() == self.0, "Signatures must be valid.");
        Ok(())
    }
}

impl Round {
    pub fn max_update(&mut self, round: Round) {
        *self = std::cmp::max(*self, round);
    }
}
