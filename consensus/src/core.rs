use crate::config::{Committee, Parameters};
use crate::context::Context;
use crate::error::ConsensusResult;
use crate::timer::Timer;
use bft_lib::base_types::NodeTime;
use bft_lib::interfaces::{ConsensusNode, DataSyncNode, NodeUpdateActions};
use bft_lib::smr_context::SmrContext;
use bytes::Bytes;
use crypto::{PublicKey, SignatureService};
use futures::executor::block_on;
use librabft_v2::data_sync::{DataSyncNotification, DataSyncRequest, DataSyncResponse};
use log::{debug, warn};
use network::NetMessage;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
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
    DataSyncNotification {
        sender: PublicKey,
        notification: DataSyncNotification<Context>,
    },
    DataSyncRequest {
        sender: PublicKey,
        request: DataSyncRequest,
    },
    DataSyncResponse {
        response: DataSyncResponse<Context>,
    },
}

pub struct CoreDriver<Node, Payload> {
    name: PublicKey,
    committee: Committee,
    rx_consensus: Receiver<ConsensusMessage>,
    rx_mempool: Receiver<Payload>,
    tx_network: Sender<NetMessage>,
    node: Node,
    context: Context,
    timer: Timer,
}

impl<Node, Payload> CoreDriver<Node, Payload>
where
    Node: ConsensusNode<Context>
        + DataSyncNode<
            Context,
            Notification = DataSyncNotification<Context>,
            Request = DataSyncRequest,
            Response = DataSyncResponse<Context>,
        > + Send
        + Sync
        + 'static,
    Context: SmrContext,
    Payload: Send + 'static + Default + Serialize + DeserializeOwned + Debug,
{
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        name: PublicKey,
        committee: Committee,
        parameters: Parameters,
        signature_service: SignatureService,
        store: Store,
        rx_consensus: Receiver<ConsensusMessage>,
        rx_mempool: Receiver<Payload>,
        tx_network: Sender<NetMessage>,
    ) {
        let mut context = Context::new(
            name,
            committee.clone(),
            store,
            signature_service,
            parameters.max_payload_size,
        );
        let node = block_on(Node::load_node(&mut context, Self::local_time()));
        let timer = Timer::new(parameters.timeout_delay);

        tokio::spawn(async move {
            Self {
                name,
                committee,
                rx_consensus,
                rx_mempool,
                tx_network,
                context,
                node,
                timer,
            }
            .run()
            .await;
        });
    }

    fn local_time() -> NodeTime {
        NodeTime(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Failed to measure time")
                .as_millis() as i64,
        )
    }

    async fn transmit(
        &self,
        message: &ConsensusMessage,
        to: Option<&PublicKey>,
    ) -> ConsensusResult<()> {
        let addresses = if let Some(to) = to {
            debug!("Sending {:?} to {}", message, to);
            vec![self.committee.address(to)?]
        } else {
            debug!("Broadcasting {:?}", message);
            self.committee.broadcast_addresses(&self.name)
        };
        let bytes = bincode::serialize(message).expect("Failed to serialize core message");
        let message = NetMessage(Bytes::from(bytes), addresses);
        if let Err(e) = self.tx_network.send(message).await {
            panic!("Failed to send message through network channel: {}", e);
        }
        Ok(())
    }

    async fn process_node_actions(
        &mut self,
        actions: NodeUpdateActions<Context>,
    ) -> ConsensusResult<()> {
        self.node.save_node(&mut self.context).await;

        let notification = self.node.create_notification();
        let message = ConsensusMessage::DataSyncNotification {
            sender: self.name,
            notification,
        };

        if actions.should_broadcast {
            self.transmit(&message, None).await?;
        } else {
            for receiver in actions.should_send {
                self.transmit(&message, Some(&receiver)).await?;
            }
        }

        // Schedule sending requests.
        let request = self.node.create_request();
        let message = ConsensusMessage::DataSyncRequest {
            sender: self.name,
            request,
        };
        if actions.should_query_all {
            self.transmit(&message, None).await?;
        }

        self.timer.reset(actions.next_scheduled_update.0 as u64);
        Ok(())
    }

    /// Main reactor loop.
    pub async fn run(&mut self) {
        // Bootstrap.
        self.timer.reset(100);

        // Process incoming messages and events.
        loop {
            let result = tokio::select! {
                Some(message) = self.rx_consensus.recv() => {
                    match message {
                        ConsensusMessage::DataSyncNotification{sender, notification} => {
                            let request = self.node.handle_notification(&mut self.context, notification).await;
                            let actions = self.node.update_node(&mut self.context, Self::local_time());
                            if let Some(request) = request {
                                let message = ConsensusMessage::DataSyncRequest{sender: self.name, request};
                                if let Err(e) = self.transmit(&message, Some(&sender)).await{
                                    warn!("{}", e);
                                }
                            }
                            self.process_node_actions(actions).await
                        },
                        ConsensusMessage::DataSyncRequest{sender, request} => {
                            let response = self.node.handle_request(&mut self.context, request).await;
                            let message = ConsensusMessage::DataSyncResponse{response};
                            self.transmit(&message, Some(&sender)).await
                        },
                        ConsensusMessage::DataSyncResponse{response} => {
                            let clock = Self::local_time();
                            self.node.handle_response(&mut self.context, response, clock).await;
                            let actions = self.node.update_node(&mut self.context, clock);
                            self.process_node_actions(actions).await
                        },
                    }
                },
                Some(payload) = self.rx_mempool.recv() => {
                    let bytes = bincode::serialize(&payload).expect("Failed to serialize payload");
                    self.context.mempool.push_back(bytes);
                    Ok(())
                },
                () = &mut self.timer => {
                    let clock = Self::local_time();
                    let actions = self.node.update_node(&mut self.context, clock);
                    self.process_node_actions(actions).await
                }
            };
            if let Err(e) = result {
                warn!("{}", e);
            }
        }
    }
}
