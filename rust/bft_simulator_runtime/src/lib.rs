// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

#![allow(bare_trait_objects)]

#[macro_use]
extern crate failure;
extern crate rand;
#[macro_use]
extern crate log;
extern crate env_logger;

use std::collections::BTreeMap;

// Comments in the following form are used for code-block generation in the consensus report:
//    "// -- BEGIN FILE name --"
//    "// -- END FILE --"
// Do not modify definitions without changing the report as well :)

pub mod base_types;
pub mod configuration;
pub mod data_writer;
pub mod simulator;

use crate::base_types::{Author, NodeTime, Round};

// -- BEGIN FILE node_update_actions --
#[derive(Debug)]
pub struct NodeUpdateActions {
    /// Time at which to call `update_node` again, at the latest.
    pub next_scheduled_update: NodeTime,
    /// Whether we need to send a notification to a subset of nodes.
    pub should_send: Vec<Author>,
    /// Whether we need to send a notification to all other nodes.
    pub should_broadcast: bool,
    /// Whether we need to request data from all other nodes.
    pub should_query_all: bool,
}
// -- END FILE --

impl NodeUpdateActions {
    pub fn new() -> Self {
        NodeUpdateActions {
            next_scheduled_update: NodeTime::never(),
            should_send: Vec::new(),
            should_broadcast: false,
            should_query_all: false,
        }
    }
}

// -- BEGIN FILE consensus_node --
pub trait ConsensusNode<Context> {
    fn update_node(&mut self, clock: NodeTime, context: &mut Context) -> NodeUpdateActions;
}
// -- END FILE --

// -- BEGIN FILE data_sync_node --
pub trait DataSyncNode<Context> {
    type Notification;
    type Request;
    type Response;

    /// Sender role: what to send to initiate a data-synchronization exchange with a receiver.
    fn create_notification(&self) -> Self::Notification;
    /// Query role: what to send to initiate a query exchange and obtain data from a sender.
    fn create_request(&self) -> Self::Request;
    /// Sender role: handle a request from a receiver.
    fn handle_request(&self, request: Self::Request) -> Self::Response;
    /// Receiver role: accept or refuse a notification.
    fn handle_notification(
        &mut self,
        notification: Self::Notification,
        context: &mut Context,
    ) -> Option<Self::Request>;
    /// Receiver role: receive data.
    fn handle_response(&mut self, response: Self::Response, context: &mut Context, clock: NodeTime);
}
// -- END FILE --

pub trait ActiveRound {
    fn active_round(&self) -> Round;
}

#[derive(Eq, PartialEq, Clone, Debug)]
/// Hold voting rights for a give epoch.
pub struct EpochConfiguration {
    voting_rights: BTreeMap<Author, usize>,
    total_votes: usize,
}
