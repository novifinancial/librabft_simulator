// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use crate::{base_types::*, configuration::EpochConfiguration};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, hash::Hash};

// -- BEGIN FILE smr_apis --
pub trait ContextTypes {
    /// An execution state.
    type State: Eq
        + PartialEq
        + Ord
        + PartialOrd
        + Clone
        + Debug
        + Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// A sequence of transactions.
    type Command: Eq
        + PartialEq
        + Ord
        + PartialOrd
        + Clone
        + Debug
        + Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;
}

pub trait CommandFetcher<Command> {
    /// How to fetch valid commands to submit to the consensus protocol.
    fn fetch(&mut self) -> Option<Command>;
}

pub trait CommandExecutor<Author, State, Command> {
    /// How to execute a command and obtain the next state.
    /// If execution fails, the value `None` is returned, meaning that the
    /// command should be rejected.
    fn compute(
        &mut self,
        // The state before executing the command.
        base_state: &State,
        // Command to execute.
        command: Command,
        // Time associated to this execution step, in agreement with
        // other consensus nodes.
        time: NodeTime,
        // Suggest to reward the author of the previous block, if any.
        previous_author: Option<Author>,
        // Suggest to reward the voters of the previous block, if any.
        previous_voters: Vec<Author>,
    ) -> Option<State>;
}

/// A commit certificate.
// TODO: more APIs
pub trait CommitCertificate<State> {
    fn committed_state(&self) -> Option<&State>;
}

/// How to communicate that a state was committed or discarded.
// TODO: The exact data type for commit certificates is specific to
// each consensus implementation and depends on the cryptographic
// module provided by the SMR Context. We use a trait object for now
// to avoid circular dependencies and keep things simple. (We could also
// separate the SMRContext and the crypto module)
pub trait StateFinalizer<State> {
    /// Report that a state was committed, together with an optional commit certificate.
    fn commit(&mut self, state: &State, commit_certificate: Option<&dyn CommitCertificate<State>>);

    /// Report that a state was discarded.
    fn discard(&mut self, state: &State);
}

/// How to read epoch ids and configuration from a state.
pub trait EpochReader<Author, State> {
    /// Read the id of the epoch in a state.
    fn read_epoch_id(&self, state: &State) -> EpochId;

    /// Return the configuration (i.e. voting rights) for the epoch starting at a given state.
    fn configuration(&self, state: &State) -> EpochConfiguration<Author>;
}

/// Something that we know how to hash and sign.
pub trait Signable<Hasher> {
    fn write(&self, hasher: &mut Hasher);
}

/// Activate the default implementation of `Signable` based on serde.
/// * We use `serde_name` to extract a seed from the name of structs and enums.
/// * We use `BCS` to generate canonical bytes suitable for hashing and signing.
pub trait BcsSignable: serde::Serialize + serde::de::DeserializeOwned {}

impl<T, Hasher> Signable<Hasher> for T
where
    T: BcsSignable,
    Hasher: std::io::Write,
{
    fn write(&self, hasher: &mut Hasher) {
        let name = serde_name::trace_name::<Self>().expect("Self must be a struct or an enum");
        write!(hasher, "{}::", name).expect("Hasher should not fail");
        bcs::serialize_into(hasher, &self)
            .expect("Serialization should not fail for consensus messages");
    }
}

/// Public and private cryptographic functions.
pub trait CryptographicModule {
    type Hasher: std::io::Write;
    type Author: serde::Serialize
        + serde::de::DeserializeOwned
        + Debug
        + Clone
        + Copy
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + 'static; // A public key
    type Signature: serde::Serialize
        + serde::de::DeserializeOwned
        + Debug
        + Clone
        + Copy
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + 'static;
    type HashValue: serde::Serialize
        + serde::de::DeserializeOwned
        + Debug
        + Clone
        + Copy
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + 'static;

    /// Hash the given message, including a type-based seed.
    fn hash(&self, message: &dyn Signable<Self::Hasher>) -> Self::HashValue;

    fn verify(
        &self,
        author: Self::Author,
        hash: Self::HashValue,
        signature: Self::Signature,
    ) -> Result<()>;

    /// The public key of this node.
    fn author(&self) -> Self::Author;

    /// Sign a message using the private key of this node.
    // TODO: make async to enable HSM implementations.
    fn sign(&mut self, hash: Self::HashValue) -> Result<Self::Signature>;
}

// TODO: some of this belongs to LibraBFT.
#[derive(PartialEq, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub struct Config<Author> {
    pub author: Author,
    pub target_commit_interval: Duration,
    pub delta: Duration,
    pub gamma: f64,
    pub lambda: f64,
}

// TODO: work in progress
pub trait Storage<Author, State> {
    fn config(&self) -> &Config<Author>;

    fn state(&self) -> State;
}

pub trait SmrContext:
    ContextTypes
    + CryptographicModule
    + CommandExecutor<
        <Self as CryptographicModule>::Author,
        <Self as ContextTypes>::State,
        <Self as ContextTypes>::Command,
    > + CommandFetcher<<Self as ContextTypes>::Command>
    + StateFinalizer<<Self as ContextTypes>::State>
    + EpochReader<<Self as CryptographicModule>::Author, <Self as ContextTypes>::State>
    + Storage<<Self as CryptographicModule>::Author, <Self as ContextTypes>::State>
    // TODO: minimize trait requirements. The following bounds are
    // required to work around the infamous limitations of
    // #[derive(..)] macros on generic types (see
    // https://github.com/rust-lang/rust/issues/26925 ). The real fix
    // is to implement traits manually in librabft_v2/record.rs (and
    // probably in other places).
    + Eq + PartialEq + Ord + PartialOrd + Clone + Debug + serde::Serialize + serde::de::DeserializeOwned + 'static
{
}
// -- END FILE --

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub struct SignedValue<T, S> {
    pub value: T,
    pub signature: S,
}

impl<T, S> AsRef<T> for SignedValue<T, S> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

/// Helper trait for SignedValue.
pub trait Authored<A> {
    fn author(&self) -> A;
}

impl<T, S> SignedValue<T, S> {
    pub fn make<C>(context: &mut C, value: T) -> Self
    where
        S: Copy,
        C: SmrContext<Signature = S>,
        T: Authored<C::Author> + Signable<C::Hasher>,
    {
        assert_eq!(value.author(), context.author());
        let h = context.hash(&value);
        let signature = context.sign(h).expect("Signing should not fail");
        SignedValue { value, signature }
    }

    pub fn verify<C>(&self, context: &C) -> Result<()>
    where
        S: Copy,
        C: SmrContext<Signature = S>,
        T: Authored<C::Author> + Signable<C::Hasher>,
    {
        let h = context.hash(&self.value);
        context.verify(self.value.author(), h, self.signature)
    }
}

impl<Author> Config<Author> {
    pub fn new(author: Author) -> Self {
        Config {
            author,
            target_commit_interval: Duration::default(),
            delta: Duration::default(),
            gamma: 0.0,
            lambda: 0.0,
        }
    }
}
