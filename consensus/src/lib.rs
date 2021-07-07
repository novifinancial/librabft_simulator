#[macro_use]
mod error;
mod config;
mod consensus;
mod context;
pub mod core; // TODO: This module can be private.
mod timer;

#[cfg(test)]
#[path = "tests/common.rs"]
mod common;

pub use crate::config::{Committee, Parameters, Stake};
pub use crate::consensus::Consensus;
pub use crate::core::{ConsensusMessage, RoundNumber};
pub use crate::error::ConsensusError;
