use crate::config::{Committee, Parameters};
use crate::error::{ConsensusError, ConsensusResult};
use crate::leader::LeaderElector;
use crate::mempool::MempoolDriver;
use crate::messages::{Block, Timeout, Vote, QC, TC};
//use crate::synchronizer::Synchronizer;
use crate::timer::Timer;
use crypto::{Digest, PublicKey, SignatureService};
use log::{debug, error, warn};
use network::NetMessage;
use serde::{Deserialize, Serialize};
use store::Store;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::context::Context;
use bft_lib::base_types::NodeTime;
use bft_lib::interfaces::ConsensusNode;
use bft_lib::smr_context::SmrContext;
use futures::executor::block_on;
use librabft_v2::data_sync::{DataSyncNotification, DataSyncRequest, DataSyncResponse};

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

#[allow(dead_code)] // TODO: Temporarily silence clippy.
pub struct Core<Node> {
    name: PublicKey,
    parameters: Parameters,
    store: Store,
    leader_elector: LeaderElector,
    //synchronizer: Synchronizer,
    core_channel: Receiver<ConsensusMessage>,
    network_channel: Sender<NetMessage>,
    commit_channel: Sender<Block>,
    round: RoundNumber,
    last_voted_round: RoundNumber,
    last_committed_round: RoundNumber,
    high_qc: QC,
    timer: Timer,
    node: Node
}

impl<Node> Core<Node> 
where
    Node: ConsensusNode<Context>,
    Context: SmrContext,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: PublicKey,
        committee: Committee,
        parameters: Parameters,
        signature_service: SignatureService,
        store: Store,
        leader_elector: LeaderElector,
        mempool_driver: MempoolDriver,
        //synchronizer: Synchronizer,
        core_channel: Receiver<ConsensusMessage>,
        network_channel: Sender<NetMessage>,
        commit_channel: Sender<Block>,
    ) -> Self {

        let node_time = NodeTime(0);
        let mut context = Context::new(
            name,
            committee,
            signature_service,
            mempool_driver,
            parameters.max_payload_size
        );
        let node = block_on(Node::load_node(&mut context, node_time));

        let timer = Timer::new(parameters.timeout_delay);
        Self {
            name,
            parameters,
            store,
            leader_elector,
            //synchronizer,
            network_channel,
            commit_channel,
            core_channel,
            round: 1,
            last_voted_round: 0,
            last_committed_round: 0,
            high_qc: QC::genesis(),
            timer,
            node
        }
    }

    /// Main reactor loop.
    pub async fn run(&mut self) {
        // Schedule a timer in case we don't hear from the leader.
        self.timer.reset();

        // Process incoming messages and events.
        loop {
            let _result = tokio::select! {
                Some(message) = self.core_channel.recv() => {
                    match message {
                        ConsensusMessage::DataSyncNotify{receiver, sender, notification} => {
                            // TODO
                        },
                        ConsensusMessage::DataSyncRequest{receiver, sender, request} => {
                            // TODO
                        },
                        ConsensusMessage::DataSyncResponse{receiver, sender, response} => {
                            // TODO
                        },
                    }
                },
                () = &mut self.timer => {
                    // TODO
                }
            };
        }
    }
}
