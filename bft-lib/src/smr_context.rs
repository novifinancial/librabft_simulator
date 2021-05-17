// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::upper_case_acronyms)]

use crate::{base_types::*, EpochConfiguration};

// -- BEGIN FILE smr_apis --
pub trait CommandFetcher {
    /// How to fetch valid commands to submit to the consensus protocol.
    fn fetch(&mut self) -> Option<Command>;
}

pub trait StateComputer {
    /// How to execute a command and obtain the next state.
    /// If execution fails, the value `None` is returned, meaning that the
    /// command should be rejected.
    fn compute(
        &mut self,
        // The state before executing the command.
        base_state: &State,
        // Command to execute.
        command: Command,
        // Time associated to this execution step, in agreement with
        // other consensus nodes.
        time: NodeTime,
        // Suggest to reward the author of the previous block, if any.
        previous_author: Option<Author>,
        // Suggest to reward the voters of the previous block, if any.
        previous_voters: Vec<Author>,
    ) -> Option<State>;
}

/// How to communicate that a state was committed or discarded.
pub trait StateFinalizer<CommitCertificate> {
    /// Report that a state was committed, together with a commit certificate.
    fn commit(&mut self, state: &State, commit_certificate: Option<&CommitCertificate>);

    /// Report that a state was discarded.
    fn discard(&mut self, state: &State);
}

/// How to read epoch ids and configuration from a state.
pub trait EpochReader {
    /// Read the id of the epoch in a state.
    fn read_epoch_id(&self, state: &State) -> EpochId;

    /// Return the configuration (i.e. voting rights) for the epoch starting at a given state.
    fn configuration(&self, state: &State) -> EpochConfiguration;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub author: Author,
    pub target_commit_interval: Duration,
    pub delta: Duration,
    pub gamma: f64,
    pub lambda: f64,
}

pub trait Storage {
    fn config(&self) -> &Config;

    fn state(&self) -> State;
}

pub trait SMRContext<CommitCertificate>:
    CommandFetcher + StateComputer + StateFinalizer<CommitCertificate> + EpochReader + Storage
{
}
// -- END FILE --

impl Config {
    pub fn new(author: Author) -> Config {
        Config {
            author,
            target_commit_interval: Duration::default(),
            delta: Duration::default(),
            gamma: 0.0,
            lambda: 0.0,
        }
    }
}
