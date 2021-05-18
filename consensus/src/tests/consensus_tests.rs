use super::*;
use crate::common::{committee, keys, MockMempool};
use crate::config::Parameters;
use crypto::SecretKey;
use futures::future::try_join_all;
use std::fs;
use tokio::sync::mpsc::channel;
use tokio::task::JoinHandle;

fn spawn_nodes(
    keys: Vec<(PublicKey, SecretKey)>,
    committee: Committee,
    store_path: &str,
) -> Vec<JoinHandle<Block>> {
    keys.into_iter()
        .enumerate()
        .map(|(i, (name, secret))| {
            let committee = committee.clone();
            let parameters = Parameters {
                timeout_delay: 100,
                ..Parameters::default()
            };
            let store_path = format!("{}_{}", store_path, i);
            let _ = fs::remove_dir_all(&store_path);
            let store = Store::new(&store_path).unwrap();
            let signature_service = SignatureService::new(secret);
            let (tx_consensus, rx_consensus) = channel(10);
            let (tx_consensus_mempool, rx_consensus_mempool) = channel(1);
            MockMempool::run(rx_consensus_mempool);
            let (tx_commit, mut rx_commit) = channel(1);
            tokio::spawn(async move {
                Consensus::run(
                    name,
                    committee,
                    parameters,
                    store,
                    signature_service,
                    tx_consensus,
                    rx_consensus,
                    tx_consensus_mempool,
                    tx_commit,
                )
                .await
                .unwrap();

                rx_commit.recv().await.unwrap()
            })
        })
        .collect()
}

#[tokio::test]
async fn end_to_end() {
    let mut committee = committee();
    committee.increment_base_port(6000);

    // Run all nodes.
    let store_path = ".db_test_end_to_end";
    let handles = spawn_nodes(keys(), committee, store_path);

    // Ensure all threads terminated correctly.
    let blocks = try_join_all(handles).await.unwrap();
    assert!(blocks.windows(2).all(|w| w[0] == w[1]));
}
