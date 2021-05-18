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

// TODO: add error handling + remove Unpin
// Alternatively, we may want to use a generic associated type when there are available on
// rust-stable:   https://github.com/rust-lang/rust/issues/44265
pub type AsyncResult<T> = Box<dyn std::future::Future<Output = T> + Unpin + 'static>;