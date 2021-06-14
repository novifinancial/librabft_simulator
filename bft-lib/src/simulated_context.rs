// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use crate::{base_types::*, configuration::EpochConfiguration, smr_context::*};
use anyhow::ensure;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, HashMap},
    fmt::Debug,
    hash::{Hash, Hasher},
};

#[cfg(test)]
#[path = "unit_tests/simulated_context_tests.rs"]
mod simulated_context_tests;

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Debug, Serialize, Deserialize)]
pub struct Author(pub usize);

#[derive(Eq, PartialEq, Copy, Clone, Hash, Debug, Serialize, Deserialize, Default)]
pub struct Signature(pub usize, pub u64);

#[derive(Eq, PartialEq, Copy, Clone, Hash, Debug, Serialize, Deserialize)]
pub struct HashValue(pub u64);

#[derive(Eq, PartialEq, Clone, Hash, Debug, Serialize, Deserialize)]
pub struct State(pub u64);

#[derive(Eq, PartialEq, Clone, Hash, Debug, Serialize, Deserialize)]
pub struct Command {
    pub proposer: Author,
    pub index: usize,
}

#[derive(Eq, PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub struct SimulatedLedgerState {
    /// All the executed commands and theirs consensus times of execution.
    /// TODO: use linked lists with sharing
    execution_history: Vec<(Command, NodeTime)>,
}

impl SimulatedLedgerState {
    fn new() -> SimulatedLedgerState {
        SimulatedLedgerState {
            execution_history: Vec::new(),
        }
    }

    fn key(&self) -> State {
        let mut hasher = DefaultHasher::new();
        self.execution_history.hash(&mut hasher);
        State(hasher.finish())
    }

    fn execute(&mut self, command: Command, time: NodeTime) {
        self.execution_history.push((command, time));
    }

    fn happened_just_before(&self, other: &SimulatedLedgerState) -> bool {
        if self.execution_history.len() + 1 != other.execution_history.len() {
            return false;
        }
        for i in 0..self.execution_history.len() {
            if self.execution_history[i] != other.execution_history[i] {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulatedContext<Config> {
    author: Author,
    config: Config,
    num_nodes: usize,
    max_command_per_epoch: usize,
    next_fetched_command_index: usize,
    last_committed_ledger_state: SimulatedLedgerState,
    pending_ledger_states: HashMap<State, SimulatedLedgerState>,
}

impl<Config> SimulatedContext<Config> {
    pub fn new(
        author: Author,
        config: Config,
        num_nodes: usize,
        max_command_per_epoch: usize,
    ) -> Self {
        SimulatedContext {
            author,
            config,
            num_nodes,
            max_command_per_epoch,
            next_fetched_command_index: 0,
            last_committed_ledger_state: SimulatedLedgerState::new(),
            pending_ledger_states: HashMap::new(),
        }
    }

    pub fn last_committed_state(&self) -> State {
        self.last_committed_ledger_state.key()
    }

    pub fn committed_history(&self) -> &Vec<(Command, NodeTime)> {
        &self.last_committed_ledger_state.execution_history
    }

    fn get_ledger_state(&self, state: &State) -> Option<&SimulatedLedgerState> {
        if state == &self.last_committed_ledger_state.key() {
            Some(&self.last_committed_ledger_state)
        } else {
            self.pending_ledger_states.get(state)
        }
    }
}

impl<Config> SmrTypes for SimulatedContext<Config> {
    type State = State;
    type Command = Command;
}

impl<Config> CommandFetcher<Command> for SimulatedContext<Config> {
    fn fetch(&mut self) -> Option<Command> {
        let command = Command {
            proposer: self.author,
            index: self.next_fetched_command_index,
        };
        self.next_fetched_command_index += 1;
        Some(command)
    }
}

impl<Config> CommandExecutor<Author, State, Command> for SimulatedContext<Config> {
    fn compute(
        &mut self,
        base_state: &State,
        command: Command,
        time: NodeTime,
        _previous_author: Option<Author>,
        _previous_voters: Vec<Author>,
    ) -> Option<State> {
        match self.get_ledger_state(base_state) {
            Some(ledger_state) => {
                let mut new_ledger_state = ledger_state.clone();
                new_ledger_state.execute(command.clone(), time);
                let new_state = new_ledger_state.key();
                self.pending_ledger_states
                    .insert(new_state.clone(), new_ledger_state);
                info!(
                    "{:?}{:?} Executing {:?} after {:?} gave {:?}",
                    self.author, time, command, base_state, new_state
                );
                Some(new_state)
            }
            None => {
                error!(
                    "{:?}{:?} Trying to executing {:?} after {:?} but the base state is not available",
                    self.author, time, command, base_state
                );
                None
            }
        }
    }
}

impl<Config> StateFinalizer<State> for SimulatedContext<Config> {
    fn commit(&mut self, state: &State, certificate: Option<&dyn CommitCertificate<State>>) {
        info!("{:?} Delivering commit for state: {:?}", self.author, state);
        let ledger_state = self
            .pending_ledger_states
            .remove(state)
            .expect("Committed states should be known");
        info!(
            "{:?} Previous ledger state: {:?}",
            self.author, self.last_committed_ledger_state
        );
        info!("{:?} New ledger state: {:?}", self.author, ledger_state);
        assert!(self
            .last_committed_ledger_state
            .happened_just_before(&ledger_state));
        if let Some(qc) = certificate {
            if let Some(state2) = qc.committed_state() {
                assert_eq!(state, state2);
                info!(
                    "{:?} Received commit certificate for state: {:?}",
                    self.author, state
                );
            }
        }
        self.last_committed_ledger_state = ledger_state
    }

    fn discard(&mut self, state: &State) {
        debug!("{:?} Discarding state: {:?}", self.author, state);
        self.pending_ledger_states
            .remove(state)
            .expect("Discarded states should be known");
    }
}

impl<Config> EpochReader<Author, State> for SimulatedContext<Config> {
    fn read_epoch_id(&self, state: &State) -> EpochId {
        let num_commands = self
            .get_ledger_state(state)
            .expect("Read states should be known")
            .execution_history
            .len();
        EpochId(num_commands / self.max_command_per_epoch)
    }

    fn configuration(&self, _state: &State) -> EpochConfiguration<Author> {
        // We do not simulate changes in the voting rights yet.
        let mut voting_rights = BTreeMap::new();
        for index in 0..self.num_nodes {
            voting_rights.insert(Author(index), 1);
        }
        EpochConfiguration::new(voting_rights)
    }
}

#[derive(Default)]
pub struct SimulatedHasher(std::collections::hash_map::DefaultHasher);

impl std::io::Write for SimulatedHasher {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<Config> CryptographicModule for SimulatedContext<Config> {
    type Hasher = SimulatedHasher;
    type Author = Author;
    type Signature = Signature;
    type HashValue = u64;

    fn hash(&self, message: &dyn Signable<Self::Hasher>) -> Self::HashValue {
        let mut hasher = SimulatedHasher::default();
        message.write(&mut hasher);
        hasher.0.finish()
    }

    fn verify(
        &self,
        author: Self::Author,
        hash: Self::HashValue,
        signature: Self::Signature,
    ) -> Result<()> {
        ensure!(author.0 == signature.0, "Unexpected signer in signature");
        ensure!(hash == signature.1, "Unexpected hash in signature");
        Ok(())
    }

    fn author(&self) -> Self::Author {
        self.author
    }

    fn sign(&mut self, hash: Self::HashValue) -> Result<Self::Signature> {
        Ok(Signature(self.author.0, hash))
    }
}

impl<Config> Storage<State> for SimulatedContext<Config> {
    type Config = Config;

    fn config(&self) -> &Config {
        &self.config
    }

    fn state(&self) -> State {
        self.last_committed_state()
    }
}

impl<Config> SmrContext for SimulatedContext<Config> where Config: Eq + Clone + Debug + 'static {}
