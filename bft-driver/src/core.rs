use crate::config::Committee;
use crate::context::Context;
use crate::timer::Timer;
use bft_lib::base_types::NodeTime;
use bft_lib::interfaces::{ConsensusNode, DataSyncNode, NodeUpdateActions};
use bft_lib::smr_context::SmrContext;
use bytes::Bytes;
use crypto::{PublicKey, SignatureService};
use futures::executor::block_on;
use log::{debug, warn};
use mempool::Payload;
use network::SimpleSender;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use std::time::{SystemTime, UNIX_EPOCH};
use store::Store;
use tokio::sync::mpsc::Receiver;

#[derive(Serialize, Deserialize, Debug)]
pub enum ConsensusMessage<Notification, Request, Response> {
    DataSyncNotification {
        sender: PublicKey,
        notification: Notification,
    },
    DataSyncRequest {
        sender: PublicKey,
        request: Request,
    },
    DataSyncResponse {
        response: Response,
    },
}

pub struct CoreDriver<Node, Notification, Request, Response> {
    name: PublicKey,
    committee: Committee,
    rx_consensus: Receiver<ConsensusMessage<Notification, Request, Response>>,
    rx_mempool: Receiver<Payload>,
    //tx_commit: Sender<CommitCertificate>,
    node: Node,
    context: Context,
    timer: Timer,
    network: SimpleSender,
}

impl<Node, Notification, Request, Response> CoreDriver<Node, Notification, Request, Response>
where
    Node: ConsensusNode<Context>
        + DataSyncNode<Context, Notification = Notification, Request = Request, Response = Response>
        + Send
        + Sync
        + 'static,
    Context: SmrContext,
    Notification: Send + 'static + Debug + Serialize + DeserializeOwned + Debug + Sync,
    Request: Send + 'static + Debug + Serialize + DeserializeOwned + Debug + Sync,
    Response: Send + 'static + Debug + Serialize + DeserializeOwned + Debug + Sync,
{
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        name: PublicKey,
        committee: Committee,
        signature_service: SignatureService,
        store: Store,
        rx_consensus: Receiver<ConsensusMessage<Notification, Request, Response>>,
        rx_mempool: Receiver<Payload>,
        //tx_commit: Sender<CommitCertificate>,
    ) {
        let mut context = Context::new(name, committee.clone(), store, signature_service);
        let node = block_on(Node::load_node(&mut context, Self::local_time()))
            .expect("Failed to load node");

        let timer = Timer::new(100); // Bootstrap the timer.

        tokio::spawn(async move {
            Self {
                name,
                committee,
                rx_consensus,
                rx_mempool,
                //tx_commit,
                context,
                node,
                timer,
                network: SimpleSender::new(),
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

    /// Send a message through the network.
    async fn transmit(
        &mut self,
        message: &ConsensusMessage<Notification, Request, Response>,
        to: Option<&PublicKey>,
    ) {
        let bytes = bincode::serialize(message).expect("Failed to serialize core message");
        if let Some(to) = to {
            debug!("Sending {:?} to {}", message, to);
            match self.committee.address(to) {
                Some(address) => self.network.send(address, Bytes::from(bytes)).await,
                None => warn!("Node {} is not in the committee", to),
            }
        } else {
            debug!("Broadcasting {:?}", message);
            let addresses = self
                .committee
                .broadcast_addresses(&self.name)
                .iter()
                .map(|(_, x)| *x)
                .collect();
            self.network.broadcast(addresses, Bytes::from(bytes)).await;
        }
    }

    async fn process_node_actions(&mut self, actions: NodeUpdateActions<Context>) {
        self.node
            .save_node(&mut self.context)
            .await
            .expect("Failed to save node state");

        let notification = self.node.create_notification(&self.context);
        let message = ConsensusMessage::DataSyncNotification {
            sender: self.name,
            notification,
        };

        if actions.should_broadcast {
            self.transmit(&message, None).await;
        } else {
            for receiver in actions.should_send {
                self.transmit(&message, Some(&receiver)).await;
            }
        }

        // Schedule sending requests.
        let request = self.node.create_request(&self.context);
        let message = ConsensusMessage::DataSyncRequest {
            sender: self.name,
            request,
        };
        if actions.should_query_all {
            self.transmit(&message, None).await;
        }

        self.timer.reset(actions.next_scheduled_update.0 as u64);
    }

    /// Main reactor loop.
    pub async fn run(&mut self) {
        // Process incoming messages and events.
        loop {
            tokio::select! {
                Some(message) = self.rx_consensus.recv() => {
                    match message {
                        ConsensusMessage::DataSyncNotification{sender, notification} => {
                            let request = self.node.handle_notification(&mut self.context, notification).await;
                            let actions = self.node.update_node(&mut self.context, Self::local_time());
                            if let Some(request) = request {
                                let message = ConsensusMessage::DataSyncRequest{sender: self.name, request};
                                self.transmit(&message, Some(&sender)).await;
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
                    self.context.buffer.push_back(payload);
                },
                () = &mut self.timer => {
                    let clock = Self::local_time();
                    let actions = self.node.update_node(&mut self.context, clock);
                    self.process_node_actions(actions).await
                }
            }
        }
    }
}
