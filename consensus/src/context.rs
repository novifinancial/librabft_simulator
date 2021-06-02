use crate::config::Committee;
use bft_lib::base_types::{EpochId, NodeTime, Result, Duration};
use bft_lib::configuration::EpochConfiguration;
use bft_lib::smr_context::*;
use crypto::{Digest, PublicKey, Signature, SignatureService};
use serde::{Deserialize, Serialize};
use ed25519_dalek::Digest as _;
use ed25519_dalek::Sha512;
use crate::mempool::MempoolDriver;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord)]
pub struct Context {
    name: PublicKey,
    committee: Committee,
    signature_service: SignatureService,
    mempool_driver: MempoolDriver,
    max_payload_size: usize
}

impl SmrContext for Context {}

pub type Author = PublicKey;

pub type State = u64;
pub type Command = Vec<Digest>;

impl SmrTypes for Context {
    type State = State;
    type Command = Command;
}

impl CommandFetcher<Command> for Context {
    fn fetch(&mut self) -> Option<Command> {
        /*
        let payload = self
            .mempool_driver
            .get(self.max_payload_size)
            .await;
        Some(payload)
        */
        Some(Command::default())
    }
}

impl CommandExecutor<Author, State, Command> for Context {
    fn compute(
        &mut self,
        _base_state: &State,
        _command: Command,
        _time: NodeTime,
        _previous_author: Option<Author>,
        _previous_voters: Vec<Author>,
    ) -> Option<State> {
        // This implementation does not execute, it is only a sequencing engine.
        None
    }
}

impl StateFinalizer<State> for Context {
    fn commit(&mut self, state: &State, certificate: Option<&dyn CommitCertificate<State>>) {
        // Nothing to do here as we do not execute transactions (the `State` is always `None`).
    }

    fn discard(&mut self, state: &State) {
        // Nothing to do here as we do not execute transactions (the `State` is always `None`).
    }
}

// TODO: Implement epoch transition. Right now, we alway run within a single epoch.
impl EpochReader<Author, State> for Context {
    fn read_epoch_id(&self, _state: &State) -> EpochId {
        EpochId(self.committee.epoch)
    }

    fn configuration(&self, _state: &State) -> EpochConfiguration<Author> {
        let voting_rights = self
            .committee
            .authorities
            .iter()
            .map(|name, auth| (name, auth.stake))
            .collect();
        EpochConfiguration::new(voting_rights)
    }
}

impl CryptographicModule for Context {
    type Hasher = Sha512;
    type Author = Author;
    type Signature = Signature;
    type HashValue = Digest;

    fn hash(&self, message: &dyn Signable<Self::Hasher>) -> Self::HashValue {
        let mut hasher = Sha512::new();
        message.write(&mut hasher);
        Digest(hasher.finalize().as_slice()[..32].try_into().unwrap())
    }

    fn verify(
        &self,
        author: Self::Author,
        hash: Self::HashValue,
        signature: Self::Signature,
    ) -> Result<()> {
        signature.verify(hash, author)
    }

    fn author(&self) -> Self::Author {
        self.name
    }

    // TODO [issue #8]: Make async to enable HSM implementations.
    fn sign(&mut self, hash: Self::HashValue) -> Self::Signature {
        //self.signature_service.request_signature(hash).await
        Signature::default()
    }
}

// TODO: Is this the right interface for a real (networked) implementation?
impl Storage<Author, State> for Context {
    fn config(&self) -> &Config<Author> {
       & Config {
            author: self.name,
            target_commit_interval: Duration::default(),
            delta: Duration::default(),
            gamma: 0.0,
            lambda: 0.0,
        }
    }

    fn state(&self) -> State {
        State::default()
    }
}