#![allow(clippy::upper_case_acronyms)]

#[macro_use]
mod error;
pub mod aggregator; // TODO: This module can be private.
mod config;
mod consensus;
pub mod core; // TODO: This module can be private.
mod leader;
mod mempool;
mod messages;
mod synchronizer;
mod timer;
mod context;

#[cfg(test)]
#[path = "tests/common.rs"]
mod common;

pub use crate::config::{Committee, Parameters};
pub use crate::consensus::Consensus;
pub use crate::core::{ConsensusMessage, RoundNumber};
pub use crate::error::ConsensusError;
pub use crate::mempool::{ConsensusMempoolMessage, PayloadStatus};
pub use crate::messages::{Block, QC, TC};
