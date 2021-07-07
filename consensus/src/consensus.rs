use crate::config::{Committee, Parameters};
use crate::context::Context;
use crate::core::{ConsensusMessage, CoreDriver};
use async_trait::async_trait;
use bft_lib::interfaces::{ConsensusNode, DataSyncNode};
use bft_lib::smr_context::SmrContext;
use bytes::Bytes;
use crypto::{PublicKey, SecretKey, SignatureService};
use log::info;
use network::{MessageHandler, Receiver as NetworkReceiver, Writer};
use serde::{de::DeserializeOwned, Serialize};
use std::error::Error;
use std::fmt::Debug;
use store::Store;
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// The default channel capacity for each channel of the consensus.
pub const CHANNEL_CAPACITY: usize = 1_000;

pub struct Consensus;

impl Consensus {
    pub fn spawn<Node, Payload, Notification, Request, Response>(
        name: PublicKey,
        secret: SecretKey,
        committee: Committee,
        parameters: Parameters,
        store: Store,
        rx_mempool: Receiver<Payload>,
        //tx_commit: Sender<dyn CommitCertificate<State>>, //  doesn't have a size known at compile-time
    ) where
        Node: ConsensusNode<Context>
            + Send
            + Sync
            + 'static
            + DataSyncNode<
                Context,
                Notification = Notification,
                Request = Request,
                Response = Response,
            >,
        Context: SmrContext,
        Payload: Send + 'static + Default + Serialize + DeserializeOwned + Debug,
        Notification: Send + 'static + Debug + Serialize + DeserializeOwned + Debug + Sync + Clone,
        Request: Send + 'static + Debug + Serialize + DeserializeOwned + Debug + Sync + Clone,
        Response: Send + 'static + Debug + Serialize + DeserializeOwned + Debug + Sync + Clone,
    {
        let (tx_consensus, rx_consensus) = channel(CHANNEL_CAPACITY);

        // Write the parameters to the logs.
        parameters.log();

        // Make the network sender and receiver.
        let address = committee
            .address(&name)
            .map(|mut x| {
                x.set_ip("0.0.0.0".parse().unwrap());
                x
            })
            .expect("Our public key is not in the committee");
        NetworkReceiver::spawn(address, /* handler */ ReceiverHandler { tx_consensus });

        // The `SignatureService` is used to require signatures on specific digests.
        let signature_service = SignatureService::new(secret);

        // Spawn the core driver.
        CoreDriver::<Node, Payload, Notification, Request, Response>::spawn(
            name,
            committee,
            parameters,
            signature_service,
            store,
            rx_consensus,
            rx_mempool,
            //tx_commit
        );

        info!("Consensus engine successfully booted");
    }
}

/// Defines how the network receiver handles incoming primary messages.
#[derive(Clone)]
struct ReceiverHandler<Notification, Request, Response> {
    tx_consensus: Sender<ConsensusMessage<Notification, Request, Response>>,
}

#[async_trait]
impl<Notification, Request, Response> MessageHandler
    for ReceiverHandler<Notification, Request, Response>
where
    Notification: Clone + Send + Sync + 'static + DeserializeOwned + Debug,
    Request: Clone + Send + Sync + 'static + DeserializeOwned + Debug,
    Response: Clone + Send + Sync + 'static + DeserializeOwned + Debug,
{
    async fn dispatch(
        &self,
        _writer: &mut Writer,
        serialized: Bytes,
    ) -> Result<(), Box<dyn Error>> {
        let message = bincode::deserialize(&serialized)?;
        self.tx_consensus
            .send(message)
            .await
            .expect("Failed to send transaction");
        Ok(())
    }
}
