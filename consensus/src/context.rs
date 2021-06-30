use crate::config::Committee;
use bft_lib::base_types::{EpochId, NodeTime, Result};
use bft_lib::configuration::EpochConfiguration;
use bft_lib::smr_context::*;
use crypto::{Digest, PublicKey, Signature, SignatureService};
use ed25519_dalek::Digest as _;
use ed25519_dalek::Sha512;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::convert::TryInto as _;
use store::Store;

pub struct Context {
    name: PublicKey,
    committee: Committee,
    _store: Store,
    _signature_service: SignatureService,
    _max_payload_size: usize,
    pub mempool: VecDeque<Command>,
}

impl Context {
    pub fn new(
        name: PublicKey,
        committee: Committee,
        store: Store,
        signature_service: SignatureService,
        max_payload_size: usize,
    ) -> Self {
        Self {
            name,
            committee,
            _store: store,
            _signature_service: signature_service,
            _max_payload_size: max_payload_size,
            mempool: VecDeque::new(),
        }
    }
}

// TODO: remove (see comment in SmrContext)
impl std::cmp::PartialOrd for Context {
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        panic!("not implemented");
    }
}
impl std::cmp::Ord for Context {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        panic!("not implemented");
    }
}
impl std::cmp::PartialEq for Context {
    fn eq(&self, _other: &Self) -> bool {
        panic!("not implemented");
    }
}
impl Eq for Context {}
impl std::fmt::Debug for Context {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        panic!("not implemented");
    }
}
impl std::clone::Clone for Context {
    fn clone(&self) -> Self {
        panic!("not implemented");
    }
}
impl Serialize for Context {
    fn serialize<S>(&self, _serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        panic!("not implemented");
    }
}

impl<'de> Deserialize<'de> for Context {
    fn deserialize<D>(_deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        panic!("not implemented");
    }
}

impl SmrContext for Context {}

pub type Author = PublicKey;

pub type State = u64;
pub type Command = Vec<u8>;

impl SmrTypes for Context {
    type State = State;
    type Command = Command;
}

impl CommandFetcher<Command> for Context {
    fn fetch(&mut self) -> Option<Command> {
        self.mempool.pop_front()
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
    fn commit(&mut self, _state: &State, _certificate: Option<&dyn CommitCertificate<State>>) {
        // Nothing to do here as we do not execute transactions (the `State` is always `None`).
    }

    fn discard(&mut self, _state: &State) {
        // Nothing to do here as we do not execute transactions (the `State` is always `None`).
    }
}

// TODO: Implement epoch transition. Right now, we alway run within a single epoch.
impl EpochReader<Author, State> for Context {
    fn read_epoch_id(&self, _state: &State) -> EpochId {
        EpochId(self.committee.epoch as usize)
    }

    fn configuration(&self, _state: &State) -> EpochConfiguration<Author> {
        let voting_rights = self
            .committee
            .authorities
            .iter()
            .map(|(name, auth)| (*name, auth.stake as usize))
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
        signature.verify(&hash, &author).map_err(anyhow::Error::new)
    }

    fn author(&self) -> Self::Author {
        self.name
    }

    // TODO [issue #8]: Make async to enable HSM implementations.
    fn sign(&mut self, _hash: Self::HashValue) -> Self::Signature {
        //self.signature_service.request_signature(hash).await
        Signature::default()
    }
}

// TODO:
impl Storage<State> for Context {
    type Config = u64;

    fn config(&self) -> &Self::Config {
        &0
    }

    fn state(&self) -> State {
        State::default()
    }
}
