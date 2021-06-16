use crate::config::{Committee, Parameters};
use crate::context::Context;
use crate::core::{ConsensusMessage, CoreDriver};
use crate::error::ConsensusResult;
use crate::mempool::{ConsensusMempoolMessage, MempoolDriver};
use crate::messages::Block;
use bft_lib::interfaces::{ConsensusNode, DataSyncNode};
use bft_lib::smr_context::SmrContext;
use crypto::{PublicKey, SignatureService};
use librabft_v2::data_sync::{DataSyncNotification, DataSyncRequest, DataSyncResponse};
use log::info;
use network::{NetReceiver, NetSender};
use store::Store;
use tokio::sync::mpsc::{channel, Receiver, Sender};

// TODO: Temporarily disable tests.
// #[cfg(test)]
// #[path = "tests/consensus_tests.rs"]
// pub mod consensus_tests;

pub struct Consensus;

impl Consensus {
    #[allow(clippy::too_many_arguments)]
    pub async fn run<Node>(
        name: PublicKey,
        committee: Committee,
        parameters: Parameters,
        store: Store,
        signature_service: SignatureService,
        tx_core: Sender<ConsensusMessage>,
        rx_core: Receiver<ConsensusMessage>,
        tx_consensus_mempool: Sender<ConsensusMempoolMessage>,
        tx_commit: Sender<Block>,
    ) -> ConsensusResult<()>
    where
        Node: ConsensusNode<Context>
            + Send
            + 'static
            + DataSyncNode<
                Context,
                Notification = DataSyncNotification<Context>,
                Request = DataSyncRequest,
                Response = DataSyncResponse<Context>,
            >,
        Context: SmrContext,
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

        let (tx_network, rx_network) = channel(1000);

        // Make the network sender and receiver.
        let address = committee.address(&name).map(|mut x| {
            x.set_ip("0.0.0.0".parse().unwrap());
            x
        })?;
        let network_receiver = NetReceiver::new(address, tx_core.clone());
        tokio::spawn(async move {
            network_receiver.run().await;
        });

        let mut network_sender = NetSender::new(rx_network);
        tokio::spawn(async move {
            network_sender.run().await;
        });

        // Make the mempool driver which will mediate our requests to the mempool.
        let mempool_driver = MempoolDriver::new(tx_consensus_mempool);

        // Spawn the core driver.
        let mut core_driver = CoreDriver::<Node>::new(
            name,
            committee,
            parameters,
            signature_service,
            store,
            mempool_driver,
            /* core_channel */ rx_core,
            /* network_channel */ tx_network,
            /* commit_channel */ tx_commit,
        );
        //tokio::spawn(async move {
        //    core_driver.run().await;
        //});

        Ok(())
    }
}
