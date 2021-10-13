// Copyright (c) Facebook, Inc. and its affiliates.
// SPDX-License-Identifier: Apache-2.0

/// Commond definitions.
pub mod base_types;

pub mod configuration;

pub mod interfaces;

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
