// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use crate::{
    base_types::{Author, NodeTime},
    AsyncResult,
};

// -- BEGIN FILE node_update_actions --
/// Actions required by `ConsensusNode::update_node`.
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

// -- BEGIN FILE consensus_node --
/// Core event handlers of a consensus node.
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
/// Network event handlers of a consensus node.
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
