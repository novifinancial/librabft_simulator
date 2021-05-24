// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use bft_lib::{
    base_types::*,
    smr_context::{Authored, BcsSignable, CryptographicModule, SignedValue, SmrContext},
};
use serde::{Deserialize, Serialize};

#[cfg(all(test, feature = "simulator"))]
#[path = "unit_tests/record_tests.rs"]
mod record_tests;

// -- BEGIN FILE records --
/// A record read from the network.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub(crate) enum Record<Context: SmrContext> {
    /// Proposed block, containing a command, e.g. a set of Libra transactions.
    #[serde(bound(deserialize = "Block<Context>: Deserialize<'de>"))]
    Block(Block<Context>),
    /// A single vote on a proposed block and its execution state.
    #[serde(bound(deserialize = "Vote<Context>: Deserialize<'de>"))]
    Vote(Vote<Context>),
    /// A quorum of votes related to a given block and execution state.
    #[serde(bound(deserialize = "QuorumCertificate<Context>: Deserialize<'de>"))]
    QuorumCertificate(QuorumCertificate<Context>),
    /// A signal that a particular round of an epoch has reached a timeout.
    #[serde(bound(deserialize = "Timeout<Context>: Deserialize<'de>"))]
    Timeout(Timeout<Context>),
}

pub(crate) type Block<C> = SignedValue<Block_<C>, <C as CryptographicModule>::Signature>;

pub(crate) type Vote<C> = SignedValue<Vote_<C>, <C as CryptographicModule>::Signature>;

pub(crate) type QuorumCertificate<C> =
    SignedValue<QuorumCertificate_<C>, <C as CryptographicModule>::Signature>;

pub(crate) type Timeout<C> = SignedValue<Timeout_<C>, <C as CryptographicModule>::Signature>;

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Debug, Serialize, Deserialize)]
pub(crate) struct BlockHash<V>(pub V);

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Debug, Serialize, Deserialize)]
pub(crate) struct QuorumCertificateHash<V>(pub V);

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Block_<Context: SmrContext> {
    /// User-defined command to execute in the state machine.
    pub(crate) command: Context::Command,
    /// Time proposed for command execution.
    pub(crate) time: NodeTime,
    /// Hash of the quorum certificate of the previous block.
    pub(crate) previous_quorum_certificate_hash: QuorumCertificateHash<Context::HashValue>,
    /// Number used to identify repeated attempts to propose a block.
    pub(crate) round: Round,
    /// Creator of the block.
    pub(crate) author: Context::Author,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Vote_<Context: SmrContext> {
    /// The current epoch.
    pub(crate) epoch_id: EpochId,
    /// The round of the voted block.
    pub(crate) round: Round,
    /// Hash of the certified block.
    pub(crate) certified_block_hash: BlockHash<Context::HashValue>,
    /// Execution state.
    pub(crate) state: Context::State,
    /// Execution state of the ancestor block (if any) that will match
    /// the commit rule when a QC is formed at this round.
    pub(crate) committed_state: Option<Context::State>,
    /// Creator of the vote.
    pub(crate) author: Context::Author,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub struct QuorumCertificate_<Context: SmrContext> {
    /// The current epoch.
    pub(crate) epoch_id: EpochId,
    /// The round of the certified block.
    pub(crate) round: Round,
    /// Hash of the certified block.
    pub(crate) certified_block_hash: BlockHash<Context::HashValue>,
    /// Execution state
    pub(crate) state: Context::State,
    /// Execution state of the ancestor block (if any) that matches
    /// the commit rule thanks to this QC.
    pub(crate) committed_state: Option<Context::State>,
    /// A collections of votes sharing the fields above.
    pub(crate) votes: Vec<(Context::Author, Context::Signature)>,
    /// The leader who proposed the certified block should also sign the QC.
    pub(crate) author: Context::Author,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Timeout_<Context: SmrContext> {
    /// The current epoch.
    pub(crate) epoch_id: EpochId,
    /// The round that has timed out.
    pub(crate) round: Round,
    /// Round of the highest block with a quorum certificate.
    pub(crate) highest_certified_block_round: Round,
    /// Creator of the timeout object.
    pub(crate) author: Context::Author,
}
// -- END FILE --

impl<Context: SmrContext> bft_lib::smr_context::CommitCertificate<Context::State>
    for QuorumCertificate_<Context>
{
    fn committed_state(&self) -> Option<&Context::State> {
        self.committed_state.as_ref()
    }
}

// Requirements for SignedValue. To avoid computing hashes in the
// wrong way, `Record` should not implement `BcsSignable`.
impl<Context: SmrContext> BcsSignable for Block_<Context> {}
impl<Context: SmrContext> BcsSignable for Vote_<Context> {}
impl<Context: SmrContext> BcsSignable for QuorumCertificate_<Context> {}
impl<Context: SmrContext> BcsSignable for Timeout_<Context> {}

impl<Context: SmrContext> Authored<Context::Author> for Block_<Context> {
    fn author(&self) -> Context::Author {
        self.author
    }
}

impl<Context: SmrContext> Authored<Context::Author> for Vote_<Context> {
    fn author(&self) -> Context::Author {
        self.author
    }
}

impl<Context: SmrContext> Authored<Context::Author> for QuorumCertificate_<Context> {
    fn author(&self) -> Context::Author {
        self.author
    }
}

impl<Context: SmrContext> Authored<Context::Author> for Timeout_<Context> {
    fn author(&self) -> Context::Author {
        self.author
    }
}
