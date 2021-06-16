use crate::config::{Committee, Parameters};
use crate::context::Context;
use crate::mempool::MempoolDriver;
use crate::messages::Block;
use crate::timer::Timer;
use bft_lib::base_types::NodeTime;
use bft_lib::interfaces::{ConsensusNode, DataSyncNode, NodeUpdateActions};
use bft_lib::smr_context::SmrContext;
use crypto::{PublicKey, SignatureService};
use futures::executor::block_on;
use librabft_v2::data_sync::{DataSyncNotification, DataSyncRequest, DataSyncResponse};
use network::NetMessage;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use store::Store;
use tokio::sync::mpsc::{Receiver, Sender};

// TODO: Temporarily disable tests.
// #[cfg(test)]
// #[path = "tests/core_tests.rs"]
// pub mod core_tests;

pub type RoundNumber = u64;

#[derive(Serialize, Deserialize, Debug)]
pub enum ConsensusMessage {
    DataSyncNotify {
        receiver: PublicKey,
        sender: PublicKey,
        notification: DataSyncNotification<Context>,
    },
    DataSyncRequest {
        receiver: PublicKey,
        sender: PublicKey,
        request: DataSyncRequest,
    },
    DataSyncResponse {
        receiver: PublicKey,
        sender: PublicKey,
        response: DataSyncResponse<Context>,
    },
}

pub struct CoreDriver<Node> {
    name: PublicKey,
    store: Store,
    core_channel: Receiver<ConsensusMessage>,
    network_channel: Sender<NetMessage>,
    commit_channel: Sender<Block>,
    node: Node,
    context: Context,
    timer: Timer,
}

impl<Node> CoreDriver<Node>
where
    Node: ConsensusNode<Context>
        + DataSyncNode<
            Context,
            Notification = DataSyncNotification<Context>,
            Request = DataSyncRequest,
            Response = DataSyncResponse<Context>,
        >,
    Context: SmrContext,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: PublicKey,
        committee: Committee,
        parameters: Parameters,
        signature_service: SignatureService,
        store: Store,
        mempool_driver: MempoolDriver,
        core_channel: Receiver<ConsensusMessage>,
        network_channel: Sender<NetMessage>,
        commit_channel: Sender<Block>,
    ) -> Self {
        let mut context = Context::new(
            name,
            committee,
            signature_service,
            mempool_driver,
            parameters.max_payload_size,
        );
        let node = block_on(Node::load_node(&mut context, Self::local_time()));
        let timer = Timer::new(parameters.timeout_delay);

        Self {
            name,
            store,
            core_channel,
            network_channel,
            commit_channel,
            context,
            node,
            timer,
        }
    }

    fn local_time() -> NodeTime {
        NodeTime(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Failed to measure time")
                .as_millis() as i64,
        )
    }

    async fn process_node_actions(&mut self, actions: NodeUpdateActions<Context>) {
        self.node.save_node(&mut self.context).await;

        let _notification = self.node.create_notification();

        if actions.should_broadcast {
            // TODO:
        } else {
            for receiver in actions.should_send {
                if receiver != self.name {
                    // TODO:
                }
            }
        }

        // Schedule sending requests.
        let request: DataSyncRequest = self.node.create_request();
        if actions.should_query_all {
            // TODO: broadcast request.
        }

        self.timer.reset(actions.next_scheduled_update.0 as u64);
    }

    /// Main reactor loop.
    pub async fn run(&mut self) {
        // Bootstrap.
        self.timer.reset(100);

        // Process incoming messages and events.
        loop {
            let _result = tokio::select! {
                Some(message) = self.core_channel.recv() => {
                    match message {
                        ConsensusMessage::DataSyncNotify{receiver, sender, notification} => {
                            let result = self.node.handle_notification(&mut self.context, notification).await;
                            let actions = self.node.update_node(&mut self.context, Self::local_time());
                            if let Some(request) = result {
                                // TODO: Send request through the network.
                            }
                            self.process_node_actions(actions).await;
                        },
                        ConsensusMessage::DataSyncRequest{receiver, sender, request} => {
                            let _response = self.node.handle_request(&mut self.context, request).await;
                            // TODO: send through network.
                        },
                        ConsensusMessage::DataSyncResponse{receiver, sender, response} => {
                            let clock = Self::local_time();
                            self.node.handle_response(&mut self.context, response, clock).await;
                            let actions = self.node.update_node(&mut self.context, clock);
                            self.process_node_actions(actions).await;
                        },
                    }
                },
                () = &mut self.timer => {
                    let clock = Self::local_time();
                    let actions = self.node.update_node(&mut self.context, clock);
                    self.process_node_actions(actions).await;
                }
            };
        }
    }
}
