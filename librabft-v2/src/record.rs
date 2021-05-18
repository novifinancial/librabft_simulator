// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

// We only use comparison for testing.
#![allow(clippy::derive_hash_xor_eq)]

use crate::base_types::*;
use bft_lib::base_types::*;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

#[cfg(test)]
#[path = "unit_tests/record_tests.rs"]
mod record_tests;

// The following comments are used for code-block generation in the consensus report:
//    "// -- BEGIN FILE name --"
//    "// -- END FILE --"
// DO NOT MODIFY definitions without changing the report as well :)

// -- BEGIN FILE records --
/// A record read from the network.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug, Hash)]
pub(crate) enum Record {
    /// Proposed block, containing a command, e.g. a set of Libra transactions.
    Block(Block),
    /// A single vote on a proposed block and its execution state.
    Vote(Vote),
    /// A quorum of votes related to a given block and execution state.
    QuorumCertificate(QuorumCertificate),
    /// A signal that a particular round of an epoch has reached a timeout.
    Timeout(Timeout),
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug)]
pub(crate) struct Block {
    /// User-defined command to execute in the state machine.
    pub(crate) command: Command,
    /// Time proposed for command execution.
    pub(crate) time: NodeTime,
    /// Hash of the quorum certificate of the previous block.
    pub(crate) previous_quorum_certificate_hash: QuorumCertificateHash,
    /// Number used to identify repeated attempts to propose a block.
    pub(crate) round: Round,
    /// Creator of the block.
    pub(crate) author: Author,
    /// Signs the hash of the block, that is, all the fields above.
    pub(crate) signature: Signature,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug)]
pub(crate) struct Vote {
    /// The current epoch.
    pub(crate) epoch_id: EpochId,
    /// The round of the voted block.
    pub(crate) round: Round,
    /// Hash of the certified block.
    pub(crate) certified_block_hash: BlockHash,
    /// Execution state.
    pub(crate) state: State,
    /// Execution state of the ancestor block (if any) that will match
    /// the commit rule when a QC is formed at this round.
    pub(crate) committed_state: Option<State>,
    /// Creator of the vote.
    pub(crate) author: Author,
    /// Signs the hash of the vote, that is, all the fields above.
    pub(crate) signature: Signature,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug)]
pub struct QuorumCertificate {
    /// The current epoch.
    pub(crate) epoch_id: EpochId,
    /// The round of the certified block.
    pub(crate) round: Round,
    /// Hash of the certified block.
    pub(crate) certified_block_hash: BlockHash,
    /// Execution state
    pub(crate) state: State,
    /// Execution state of the ancestor block (if any) that matches
    /// the commit rule thanks to this QC.
    pub(crate) committed_state: Option<State>,
    /// A collections of votes sharing the fields above.
    pub(crate) votes: Vec<(Author, Signature)>,
    /// The leader who proposed the certified block should also sign the QC.
    pub(crate) author: Author,
    /// Signs the hash of the QC, that is, all the fields above.
    pub(crate) signature: Signature,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Debug)]
pub(crate) struct Timeout {
    /// The current epoch.
    pub(crate) epoch_id: EpochId,
    /// The round that has timed out.
    pub(crate) round: Round,
    /// Round of the highest block with a quorum certificate.
    pub(crate) highest_certified_block_round: Round,
    /// Creator of the timeout object.
    pub(crate) author: Author,
    /// Signs the hash of the timeout, that is, all the fields above.
    pub(crate) signature: Signature,
}
// -- END FILE --

#[cfg(feature = "simulator")]
impl bft_lib::simulated_context::CommitCertificate for QuorumCertificate {
    fn committed_state(&self) -> Option<&State> {
        self.committed_state.as_ref()
    }
}

// TODO: Use serde + BCS instead of Hash.
impl Hash for Block {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.command.hash(state);
        self.time.hash(state);
        self.previous_quorum_certificate_hash.hash(state);
        self.round.hash(state);
        self.author.hash(state);
    }
}

impl Hash for Vote {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.epoch_id.hash(state);
        self.round.hash(state);
        self.certified_block_hash.hash(state);
        self.state.hash(state);
        self.committed_state.hash(state);
        self.author.hash(state);
    }
}

impl Hash for QuorumCertificate {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.epoch_id.hash(state);
        self.round.hash(state);
        self.certified_block_hash.hash(state);
        self.state.hash(state);
        self.committed_state.hash(state);
        self.votes.hash(state);
        self.author.hash(state);
    }
}

impl Hash for Timeout {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.epoch_id.hash(state);
        self.round.hash(state);
        self.author.hash(state);
    }
}

impl Record {
    pub(crate) fn digest(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    pub(crate) fn make_block(
        command: Command,
        time: NodeTime,
        previous_quorum_certificate_hash: QuorumCertificateHash,
        round: Round,
        author: Author,
    ) -> Record {
        let mut value = Record::Block(Block {
            command,
            time,
            previous_quorum_certificate_hash,
            round,
            author,
            signature: Signature(0),
        });
        let hash = value.digest();
        match &mut value {
            Record::Block(block) => block.signature = Signature::sign(hash, block.author),
            _ => unreachable!(),
        }
        value
    }

    pub(crate) fn make_vote(
        epoch_id: EpochId,
        round: Round,
        certified_block_hash: BlockHash,
        state: State,
        author: Author,
        committed_state: Option<State>,
    ) -> Record {
        let mut value = Record::Vote(Vote {
            epoch_id,
            round,
            certified_block_hash,
            state,
            author,
            signature: Signature(0),
            committed_state,
        });
        let hash = value.digest();
        match &mut value {
            Record::Vote(vote) => vote.signature = Signature::sign(hash, vote.author),
            _ => unreachable!(),
        }
        value
    }

    pub(crate) fn make_timeout(
        epoch_id: EpochId,
        round: Round,
        highest_certified_block_round: Round,
        author: Author,
    ) -> Record {
        let mut value = Record::Timeout(Timeout {
            epoch_id,
            round,
            highest_certified_block_round,
            author,
            signature: Signature(0),
        });
        let hash = value.digest();
        match &mut value {
            Record::Timeout(timeout) => timeout.signature = Signature::sign(hash, timeout.author),
            _ => unreachable!(),
        }
        value
    }

    pub(crate) fn make_quorum_certificate(
        epoch_id: EpochId,
        round: Round,
        certified_block_hash: BlockHash,
        state: State,
        votes: Vec<(Author, Signature)>,
        committed_state: Option<State>,
        author: Author,
    ) -> Record {
        let mut value = Record::QuorumCertificate(QuorumCertificate {
            epoch_id,
            round,
            certified_block_hash,
            state,
            votes,
            committed_state,
            author,
            signature: Signature(0),
        });
        let hash = value.digest();
        match &mut value {
            Record::QuorumCertificate(qc) => qc.signature = Signature::sign(hash, qc.author),
            _ => unreachable!(),
        }
        value
    }

    #[cfg(test)]
    pub(crate) fn author(&self) -> Author {
        match self {
            Record::Block(x) => x.author,
            Record::Vote(x) => x.author,
            Record::QuorumCertificate(x) => x.author,
            Record::Timeout(x) => x.author,
        }
    }

    #[cfg(test)]
    pub(crate) fn signature(&self) -> Signature {
        match self {
            Record::Block(x) => x.signature,
            Record::Vote(x) => x.signature,
            Record::QuorumCertificate(x) => x.signature,
            Record::Timeout(x) => x.signature,
        }
    }
}
