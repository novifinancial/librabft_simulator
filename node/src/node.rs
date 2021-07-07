use crate::config::Export as _;
use crate::config::{Committee, Parameters, Secret};
use consensus::{Consensus, Context};
use librabft_v2::{
    data_sync::{DataSyncNotification, DataSyncRequest, DataSyncResponse},
    node::NodeState,
};
use log::info;
use mempool::{Mempool, Payload};
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

        // Spawn the consensus.
        Consensus::spawn::<
            NodeState<Context>,
            Payload,
            DataSyncNotification<Context>,
            DataSyncRequest,
            DataSyncResponse<Context>,
        >(
            name,
            secret_key,
            committee.consensus.clone(),
            parameters.consensus,
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
