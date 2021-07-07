// Copyright(C) Facebook, Inc. and its affiliates.
use crate::mempool::SerializedBatch;
use crypto::Digest;
use ed25519_dalek::Digest as _;
use ed25519_dalek::Sha512;
use std::convert::TryInto;
use store::Store;
use tokio::sync::mpsc::{Receiver, Sender};

#[cfg(test)]
#[path = "tests/processor_tests.rs"]
pub mod processor_tests;

/// Hashes and stores batches, it then outputs the batch's digest.
pub struct Processor;

impl Processor {
    pub fn spawn(
        // The persistent storage.
        mut store: Store,
        // Input channel to receive batches.
        mut rx_batch: Receiver<SerializedBatch>,
        // Output channel to send out batches.
        tx_batch: Sender<SerializedBatch>,
    ) {
        tokio::spawn(async move {
            while let Some(batch) = rx_batch.recv().await {
                // Hash the batch.
                let digest = Digest(Sha512::digest(&batch).as_slice()[..32].try_into().unwrap());

                // Store the batch.
                store.write(digest.to_vec(), batch.clone()).await;

                // Deliver the batch.
                tx_batch.send(batch).await.expect("Failed to send digest");
            }
        });
    }
}
