// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

/// Commond definitions.
pub mod base_types;

mod configuration;

mod interfaces;

/// Requirements for the external modules provided by `Context`.
pub mod smr_context;

#[cfg(feature = "simulator")]
mod data_writer;

/// Runtime for discrete-event simulations.
#[cfg(feature = "simulator")]
pub mod simulator;

#[cfg(feature = "simulator")]
/// Implementation of SMR Context
pub mod simulated_context;

#[cfg(feature = "simulator")]
pub use simulator::ActiveRound;

pub use configuration::EpochConfiguration;

pub use interfaces::*;
