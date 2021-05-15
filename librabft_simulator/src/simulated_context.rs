// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use base_types::*;
use record::*;
use smr_context::*;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};

#[cfg(test)]
#[path = "unit_tests/simulated_context_tests.rs"]
mod simulated_context_tests;

#[derive(Eq, PartialEq, Clone, Debug, Hash)]
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

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct SimulatedContext {
    author: Author,
    num_nodes: usize,
    max_command_per_epoch: usize,
    next_fetched_command_index: usize,
    last_committed_ledger_state: SimulatedLedgerState,
    pending_ledger_states: HashMap<State, SimulatedLedgerState>,
}

impl SimulatedContext {
    pub fn new(author: Author, num_nodes: usize, max_command_per_epoch: usize) -> Self {
        SimulatedContext {
            author,
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

impl CommandFetcher for SimulatedContext {
    fn fetch(&mut self) -> Option<Command> {
        let command = Command {
            proposer: self.author,
            index: self.next_fetched_command_index,
        };
        self.next_fetched_command_index += 1;
        Some(command)
    }
}

impl StateComputer for SimulatedContext {
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

impl StateFinalizer for SimulatedContext {
    fn commit(&mut self, state: &State, certificate: Option<&QuorumCertificate>) {
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
            if let Some(state2) = &qc.committed_state {
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

impl EpochReader for SimulatedContext {
    fn read_epoch_id(&self, state: &State) -> EpochId {
        let num_commands = self
            .get_ledger_state(state)
            .expect("Read states should be known")
            .execution_history
            .len();
        EpochId(num_commands / self.max_command_per_epoch)
    }

    fn configuration(&self, _state: &State) -> EpochConfiguration {
        // We do not simulate changes in the voting rights yet.
        let mut voting_rights = BTreeMap::new();
        for index in 0..self.num_nodes {
            voting_rights.insert(Author(index), 1);
        }
        EpochConfiguration::new(voting_rights)
    }
}

impl SMRContext for SimulatedContext {}
