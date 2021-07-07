use crate::config::{Committee, Parameters};
use crate::context::Context;
use crate::core::{ConsensusMessage, CoreDriver};
use async_trait::async_trait;
use bft_lib::interfaces::{ConsensusNode, DataSyncNode};
use bft_lib::smr_context::SmrContext;
use bytes::Bytes;
use crypto::{PublicKey, SignatureService};
use log::info;
use network::{MessageHandler, Receiver as NetworkReceiver, Writer};
use serde::{de::DeserializeOwned, Serialize};
use std::error::Error;
use std::fmt::Debug;
use store::Store;
use tokio::sync::mpsc::{Receiver, Sender};

pub struct Consensus;

impl Consensus {
    #[allow(clippy::too_many_arguments)]
    pub async fn run<Node, Payload, Notification, Request, Response>(
        name: PublicKey,
        committee: Committee,
        parameters: Parameters,
        store: Store,
        signature_service: SignatureService,
        tx_consensus: Sender<ConsensusMessage<Notification, Request, Response>>,
        rx_consensus: Receiver<ConsensusMessage<Notification, Request, Response>>,
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
        // NOTE: The following log entries are used to compute performance.
        info!(
            "Consensus timeout delay set to {} ms",
            parameters.timeout_delay
        );
        info!(
            "Consensus synchronizer retry delay set to {} ms",
            parameters.sync_retry_delay
        );
        info!(
            "Consensus max payload size set to {} B",
            parameters.max_payload_size
        );
        info!(
            "Consensus min block delay set to {} ms",
            parameters.min_block_delay
        );

        // Make the network sender and receiver.
        let address = committee
            .address(&name)
            .map(|mut x| {
                x.set_ip("0.0.0.0".parse().unwrap());
                x
            })
            .expect("Our public key is not in the committee");
        NetworkReceiver::spawn(address, /* handler */ ReceiverHandler { tx_consensus });

        // Make the mempool driver which will mediate our requests to the mempool.
        //let mempool_driver = MempoolDriver::new(tx_consensus_mempool);

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
