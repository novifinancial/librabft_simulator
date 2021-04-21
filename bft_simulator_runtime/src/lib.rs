// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

#![allow(bare_trait_objects)]

#[macro_use]
extern crate failure;
extern crate rand;
extern crate rand_distr;
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
#[derive(Debug, Default)]
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
        NodeUpdateActions::default()
    }
}

// TODO: add error handling + remove Unpin
// Alternatively, we may want to use a generic associated type when there are available on
// rust-stable:   https://github.com/rust-lang/rust/issues/44265
pub type AsyncResult<T> = Box<dyn std::future::Future<Output = T> + Unpin + 'static>;

// -- BEGIN FILE consensus_node --
pub trait ConsensusNode<Context>: Sized {
    /// Read data from storage and crate a view of the node state in memory.
    fn load_node(context: &mut Context, clock: NodeTime) -> AsyncResult<Self>;

    /// Execute one step of the main event loop of the protocol.
    /// "Stage" changes to the node state by mutating `self`.
    fn update_node(&mut self, context: &mut Context, clock: NodeTime) -> NodeUpdateActions;

    /// Save the "staged" node state into storage, possibly after applying additional async
    /// operations.
    fn save_node(&mut self, context: &mut Context) -> AsyncResult<()>;
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
    fn handle_request(
        &self,
        context: &mut Context,
        request: Self::Request,
    ) -> AsyncResult<Self::Response>;

    /// Receiver role: accept or refuse a notification.
    fn handle_notification(
        &mut self,
        context: &mut Context,
        notification: Self::Notification,
    ) -> AsyncResult<Option<Self::Request>>;

    /// Receiver role: receive data.
    fn handle_response(
        &mut self,
        context: &mut Context,
        response: Self::Response,
        clock: NodeTime,
    ) -> AsyncResult<()>;
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
