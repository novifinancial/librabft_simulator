// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

// We only use comparison for testing.
#![allow(clippy::derive_hash_xor_eq)]
#![allow(clippy::too_many_arguments)]

use crate::base_types::*;
use bft_lib::{base_types::*, smr_context::SmrContext};
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

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Block<Context: SmrContext> {
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
    /// Signs the hash of the block, that is, all the fields above.
    #[serde(skip)]
    // FIX ME: Here and below we use serde traits for hashing/signing. Because we skip this field,
    // data are no longer serializable for networking purpose.
    pub(crate) signature: Context::Signature,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Vote<Context: SmrContext> {
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
    /// Signs the hash of the vote, that is, all the fields above.
    #[serde(skip)]
    pub(crate) signature: Context::Signature,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub struct QuorumCertificate<Context: SmrContext> {
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
    /// Signs the hash of the QC, that is, all the fields above.
    #[serde(skip)]
    pub(crate) signature: Context::Signature,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Timeout<Context: SmrContext> {
    /// The current epoch.
    pub(crate) epoch_id: EpochId,
    /// The round that has timed out.
    pub(crate) round: Round,
    /// Round of the highest block with a quorum certificate.
    pub(crate) highest_certified_block_round: Round,
    /// Creator of the timeout object.
    pub(crate) author: Context::Author,
    /// Signs the hash of the timeout, that is, all the fields above.
    #[serde(skip)]
    pub(crate) signature: Context::Signature,
}
// -- END FILE --

impl<Context: SmrContext> bft_lib::smr_context::CommitCertificate<Context::State>
    for QuorumCertificate<Context>
{
    fn committed_state(&self) -> Option<&Context::State> {
        self.committed_state.as_ref()
    }
}

impl<Context: SmrContext> Record<Context> {
    pub(crate) fn make_block(
        context: &mut Context,
        command: Context::Command,
        time: NodeTime,
        previous_quorum_certificate_hash: QuorumCertificateHash<Context::HashValue>,
        round: Round,
        author: Context::Author,
    ) -> Record<Context> {
        let mut value = Record::Block(Block {
            command,
            time,
            previous_quorum_certificate_hash,
            round,
            author,
            signature: Context::Signature::default(),
        });
        assert_eq!(&author, context.author());
        let h = context.hash(&value);
        let s = context.sign(h).expect("Signing should not fail");
        match &mut value {
            Record::Block(block) => block.signature = s,
            _ => unreachable!(),
        }
        value
    }

    pub(crate) fn make_vote(
        context: &mut Context,
        epoch_id: EpochId,
        round: Round,
        certified_block_hash: BlockHash<Context::HashValue>,
        state: Context::State,
        author: Context::Author,
        committed_state: Option<Context::State>,
    ) -> Record<Context> {
        let mut value = Record::Vote(Vote {
            epoch_id,
            round,
            certified_block_hash,
            state,
            author,
            signature: Context::Signature::default(),
            committed_state,
        });
        assert_eq!(&author, context.author());
        let h = context.hash(&value);
        let s = context.sign(h).expect("Signing should not fail");
        match &mut value {
            Record::Vote(vote) => vote.signature = s,
            _ => unreachable!(),
        }
        value
    }

    pub(crate) fn make_timeout(
        context: &mut Context,
        epoch_id: EpochId,
        round: Round,
        highest_certified_block_round: Round,
        author: Context::Author,
    ) -> Record<Context> {
        let mut value = Record::Timeout(Timeout {
            epoch_id,
            round,
            highest_certified_block_round,
            author,
            signature: Context::Signature::default(),
        });
        assert_eq!(&author, context.author());
        let h = context.hash(&value);
        let s = context.sign(h).expect("Signing should not fail");
        match &mut value {
            Record::Timeout(timeout) => timeout.signature = s,
            _ => unreachable!(),
        }
        value
    }

    pub(crate) fn make_quorum_certificate(
        context: &mut Context,
        epoch_id: EpochId,
        round: Round,
        certified_block_hash: BlockHash<Context::HashValue>,
        state: Context::State,
        votes: Vec<(Context::Author, Context::Signature)>,
        committed_state: Option<Context::State>,
        author: Context::Author,
    ) -> Record<Context> {
        let mut value = Record::QuorumCertificate(QuorumCertificate {
            epoch_id,
            round,
            certified_block_hash,
            state,
            votes,
            committed_state,
            author,
            signature: Context::Signature::default(),
        });
        assert_eq!(&author, context.author());
        let h = context.hash(&value);
        let s = context.sign(h).expect("Signing should not fail");
        match &mut value {
            Record::QuorumCertificate(qc) => qc.signature = s,
            _ => unreachable!(),
        }
        value
    }

    #[cfg(all(test, feature = "simulator"))]
    pub(crate) fn author(&self) -> Context::Author {
        match self {
            Record::Block(x) => x.author,
            Record::Vote(x) => x.author,
            Record::QuorumCertificate(x) => x.author,
            Record::Timeout(x) => x.author,
        }
    }

    #[cfg(all(test, feature = "simulator"))]
    pub(crate) fn signature(&self) -> Context::Signature {
        match self {
            Record::Block(x) => x.signature,
            Record::Vote(x) => x.signature,
            Record::QuorumCertificate(x) => x.signature,
            Record::Timeout(x) => x.signature,
        }
    }
}
