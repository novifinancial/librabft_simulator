// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use anyhow::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(test)]
#[path = "unit_tests/base_type_tests.rs"]
mod base_type_tests;

// TODO: add error handling (using thiserror to define an error type?) + remove Unpin
// Alternatively, we may want to use a generic associated type when there are available on
// rust-stable:   https://github.com/rust-lang/rust/issues/44265
pub type AsyncResult<T> = Box<dyn std::future::Future<Output = T> + Unpin + 'static>;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Serialize, Deserialize, Debug)]
pub struct Round(pub usize);
#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct NodeTime(pub i64);
#[derive(
    Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Serialize, Deserialize, Debug, Default,
)]
pub struct Duration(pub i64);

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Serialize, Deserialize, Debug)]
pub struct EpochId(pub usize);

impl EpochId {
    pub fn previous(self) -> Option<EpochId> {
        if self.0 == 0 {
            None
        } else {
            Some(EpochId(self.0))
        }
    }
}

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
        NodeTime(self.0 + rhs.0)
    }
}

impl Round {
    pub fn max_update(&mut self, round: Round) {
        *self = std::cmp::max(*self, round);
    }
}
