use crate::aggregator::Aggregator;
use crate::config::{Committee, Parameters};
use crate::error::{ConsensusError, ConsensusResult};
use crate::leader::LeaderElector;
use crate::mempool::MempoolDriver;
use crate::messages::{Block, Timeout, Vote, QC, TC};
use crate::synchronizer::Synchronizer;
use crate::timer::Timer;
use crypto::{Digest, PublicKey, SignatureService};
use log::{debug, error, warn};
use network::NetMessage;
use serde::{Deserialize, Serialize};
use store::Store;
use tokio::sync::mpsc::{Receiver, Sender};

// TODO: Temporarily disable tests.
// #[cfg(test)]
// #[path = "tests/core_tests.rs"]
// pub mod core_tests;

pub type RoundNumber = u64;

#[derive(Serialize, Deserialize, Debug)]
pub enum ConsensusMessage {
    Propose(Block),
    Vote(Vote),
    Timeout(Timeout),
    TC(TC),
    LoopBack(Block),
    SyncRequest(Digest, PublicKey),
}

#[allow(dead_code)] // TODO: Temporarily silence clippy.
pub struct Core {
    name: PublicKey,
    committee: Committee,
    parameters: Parameters,
    store: Store,
    signature_service: SignatureService,
    leader_elector: LeaderElector,
    mempool_driver: MempoolDriver,
    synchronizer: Synchronizer,
    core_channel: Receiver<ConsensusMessage>,
    network_channel: Sender<NetMessage>,
    commit_channel: Sender<Block>,
    round: RoundNumber,
    last_voted_round: RoundNumber,
    last_committed_round: RoundNumber,
    high_qc: QC,
    timer: Timer,
    aggregator: Aggregator,
}

impl Core {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: PublicKey,
        committee: Committee,
        parameters: Parameters,
        signature_service: SignatureService,
        store: Store,
        leader_elector: LeaderElector,
        mempool_driver: MempoolDriver,
        synchronizer: Synchronizer,
        core_channel: Receiver<ConsensusMessage>,
        network_channel: Sender<NetMessage>,
        commit_channel: Sender<Block>,
    ) -> Self {
        let aggregator = Aggregator::new(committee.clone());
        let timer = Timer::new(parameters.timeout_delay);
        Self {
            name,
            committee,
            parameters,
            signature_service,
            store,
            leader_elector,
            mempool_driver,
            synchronizer,
            network_channel,
            commit_channel,
            core_channel,
            round: 1,
            last_voted_round: 0,
            last_committed_round: 0,
            high_qc: QC::genesis(),
            timer,
            aggregator,
        }
    }

    /// Generate a new proposal.
    async fn generate_proposal(&mut self, _tc: Option<TC>) -> ConsensusResult<()> {
        // TODO
        Ok(())
    }

    /// Handle an incoming proposal (from other nodes).
    async fn handle_proposal(&mut self, _block: &Block) -> ConsensusResult<()> {
        // TODO
        Ok(())
    }

    /// Handle votes (including our owns).
    async fn handle_vote(&mut self, vote: &Vote) -> ConsensusResult<()> {
        debug!("Processing {:?}", vote);

        // TODO
        Ok(())
    }

    /// Handle incoming timeouts (from other nodes).
    async fn handle_timeout(&mut self, timeout: &Timeout) -> ConsensusResult<()> {
        debug!("Processing {:?}", timeout);

        // TODO
        Ok(())
    }

    /// Handle incoming TCs.
    async fn handle_tc(&mut self, tc: TC) -> ConsensusResult<()> {
        debug!("Processing {:?}", tc);

        // TODO
        Ok(())
    }

    /// Process a valid (verified) block.
    async fn process_block(&mut self, block: &Block) -> ConsensusResult<()> {
        debug!("Processing {:?}", block);

        // TODO
        Ok(())
    }

    /// Handle an incoming sync request.
    async fn handle_sync_request(
        &mut self,
        _digest: Digest,
        _sender: PublicKey,
    ) -> ConsensusResult<()> {
        // TODO
        Ok(())
    }

    /// Triggers a local timeout.
    async fn local_timeout_round(&mut self) -> ConsensusResult<()> {
        // TODO
        Ok(())
    }

    /// Main reactor loop.
    pub async fn run(&mut self) {
        // Upon booting, generate the very first block (if we are the leader).
        // Also, schedule a timer in case we don't hear from the leader.
        self.timer.reset();
        if self.name == self.leader_elector.get_leader(self.round) {
            self.generate_proposal(None)
                .await
                .expect("Failed to send the first block");
        }

        // This is the main loop: it processes incoming blocks and votes,
        // and receive timeout notifications from our Timeout Manager.
        loop {
            let result = tokio::select! {
                Some(message) = self.core_channel.recv() => {
                    match message {
                        ConsensusMessage::Propose(block) => self.handle_proposal(&block).await,
                        ConsensusMessage::Vote(vote) => self.handle_vote(&vote).await,
                        ConsensusMessage::Timeout(timeout) => self.handle_timeout(&timeout).await,
                        ConsensusMessage::TC(tc) => self.handle_tc(tc).await,
                        ConsensusMessage::LoopBack(block) => self.process_block(&block).await,
                        ConsensusMessage::SyncRequest(digest, sender) => self.handle_sync_request(digest, sender).await
                    }
                },
                () = &mut self.timer => self.local_timeout_round().await,
                else => break,
            };
            match result {
                Ok(()) => (),
                Err(ConsensusError::StoreError(e)) => error!("{}", e),
                Err(ConsensusError::SerializationError(e)) => error!("Store corrupted. {}", e),
                Err(e) => warn!("{}", e),
            }
        }
    }
}
