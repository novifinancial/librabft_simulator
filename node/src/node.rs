use crate::config::Export as _;
use crate::config::{Committee, Parameters, Secret};
use bft_lib::base_types::NodeTime;
use bft_lib::interfaces::ConsensusNode;
use consensus::{Consensus, Context};
use crypto::SignatureService;
use futures::executor::block_on;
use librabft_v2::{
    data_sync::{DataSyncNotification, DataSyncRequest, DataSyncResponse},
    node::{NodeConfig, NodeState},
};
use log::info;
use mempool::Mempool;
use store::{Store, StoreError};
use thiserror::Error;
use tokio::sync::mpsc::{channel, Receiver};

/// The default channel capacity for each channel of the node.
pub const CHANNEL_CAPACITY: usize = 1_000;

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("Failed to read config file '{file}': {message}")]
    ReadError { file: String, message: String },

    #[error("Failed to write config file '{file}': {message}")]
    WriteError { file: String, message: String },

    #[error("Store error: {0}")]
    StoreError(#[from] StoreError),
}

pub struct LibraBftV2Node {
    pub commit: Receiver<()>, // TODO: Should be a commit certificate.
}

impl LibraBftV2Node {
    pub async fn new(
        committee_file: &str,
        key_file: &str,
        store_path: &str,
        parameters: Option<&str>,
    ) -> Result<Self, NodeError> {
        let (tx_payload, rx_payload) = channel(CHANNEL_CAPACITY);
        let (_tx_commit, rx_commit) = channel(CHANNEL_CAPACITY);

        // Read the committee and secret key from file.
        let committee = Committee::read(committee_file)?;
        let secret = Secret::read(key_file)?;
        let name = secret.name;
        let secret_key = secret.secret;

        // Load default parameters if none are specified.
        let parameters = match parameters {
            Some(filename) => Parameters::read(filename)?,
            None => Parameters::default(),
        };

        // Make the data store.
        let store = Store::new(store_path)?;

        // Spawn the mempool.
        Mempool::spawn(
            name,
            committee.mempool,
            parameters.mempool,
            store.clone(),
            /* tx_consensus */ tx_payload,
        );

        // The `SignatureService` is used to require signatures on specific digests.
        let signature_service = SignatureService::new(secret_key);

        // Initialize the node state.
        {
            let mut context = Context::new(
                name,
                committee.consensus.clone(),
                store.clone(),
                signature_service.clone(),
            );
            let config = NodeConfig {
                target_commit_interval: parameters.consensus.target_commit_interval,
                delta: parameters.consensus.delta,
                gamma: parameters.consensus.gamma,
                lambda: parameters.consensus.lambda,
            };
            let mut node = NodeState::make_initial_state(&context, config, NodeTime(0));
            block_on(node.save_node(&mut context)).expect("Failed to save initial node state");
        }

        // Spawn the consensus.
        Consensus::spawn::<
            NodeState<Context>,
            DataSyncNotification<Context>,
            DataSyncRequest,
            DataSyncResponse<Context>,
        >(
            name,
            committee.consensus.clone(),
            signature_service,
            store,
            /* rx_mempool */ rx_payload, //tx_commit,
        );

        info!(
            "Node {} successfully booted on {}",
            name,
            committee
                .consensus
                .address(&name)
                .expect("Our public key is not in the committee")
                .ip()
        );
        Ok(Self { commit: rx_commit })
    }

    pub fn print_key_file(filename: &str) -> Result<(), NodeError> {
        Secret::new().write(filename)
    }

    pub async fn analyze_block(&mut self) {
        while let Some(_certificate) = self.commit.recv().await {
            // This is where we can further process committed block.
        }
    }
}
