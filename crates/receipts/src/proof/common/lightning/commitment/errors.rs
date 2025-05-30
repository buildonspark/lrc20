use alloc::fmt;
use bitcoin::{
    ScriptBuf,
    blockdata::{opcodes, script},
    secp256k1,
};

use crate::{ReceiptKeyError, proof::p2wsh::errors::P2WSHWitnessParseError};

#[derive(Debug)]
pub enum LightningCommitmentProofError {
    /// Failed to create receipt key
    ReceiptKeyError(ReceiptKeyError),

    /// Invalid witness data
    InvalidWitnessData(P2WSHWitnessParseError),

    /// Redeem script mismatch
    RedeemScriptMismatch {
        expected: ScriptBuf,
        found: ScriptBuf,
    },

    /// Mismatch script pubkey
    MismatchScriptPubkey {
        expected: ScriptBuf,
        found: ScriptBuf,
    },
}

impl fmt::Display for LightningCommitmentProofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LightningCommitmentProofError::ReceiptKeyError(e) => {
                write!(f, "Failed to create receipt key: {}", e)
            }
            LightningCommitmentProofError::InvalidWitnessData(e) => {
                write!(f, "Invalid witness data: {}", e)
            }
            LightningCommitmentProofError::RedeemScriptMismatch { expected, found } => write!(
                f,
                "Redeem script mismatch expected {}, found {}",
                expected, found
            ),
            LightningCommitmentProofError::MismatchScriptPubkey { expected, found } => write!(
                f,
                "Mismatch script pubkey expected {}, found {}",
                expected, found
            ),
        }
    }
}

#[cfg(not(feature = "no-std"))]
impl std::error::Error for LightningCommitmentProofError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LightningCommitmentProofError::ReceiptKeyError(e) => Some(e),
            LightningCommitmentProofError::InvalidWitnessData(e) => Some(e),
            LightningCommitmentProofError::RedeemScriptMismatch {
                expected: _,
                found: _,
            } => None,
            LightningCommitmentProofError::MismatchScriptPubkey {
                expected: _,
                found: _,
            } => None,
        }
    }
}

impl From<ReceiptKeyError> for LightningCommitmentProofError {
    fn from(err: ReceiptKeyError) -> Self {
        LightningCommitmentProofError::ReceiptKeyError(err)
    }
}

impl From<P2WSHWitnessParseError> for LightningCommitmentProofError {
    fn from(err: P2WSHWitnessParseError) -> Self {
        LightningCommitmentProofError::InvalidWitnessData(err)
    }
}

#[derive(Debug)]
pub enum ToLocalScriptParseError {
    Instruction {
        expected: opcodes::Opcode,
        found: Option<opcodes::Opcode>,
        index: usize,
    },
    Script(script::Error),
    PublicKey(secp256k1::Error),
    ToSelfDelay,
}

impl fmt::Display for ToLocalScriptParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToLocalScriptParseError::Instruction {
                expected,
                found,
                index,
            } => {
                write!(
                    f,
                    "Invalid instruction. Expected {:?}, found {:?} at index {}",
                    expected, found, index
                )
            }
            ToLocalScriptParseError::Script(e) => write!(f, "Invalid script: {}", e),
            ToLocalScriptParseError::PublicKey(e) => write!(f, "Invalid public key: {}", e),
            ToLocalScriptParseError::ToSelfDelay => write!(f, "Invalid `to_self_delay`"),
        }
    }
}

#[cfg(not(feature = "no-std"))]
impl std::error::Error for ToLocalScriptParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ToLocalScriptParseError::Instruction {
                expected: _,
                found: _,
                index: _,
            } => None,
            ToLocalScriptParseError::Script(e) => Some(e),
            ToLocalScriptParseError::PublicKey(e) => Some(e),
            ToLocalScriptParseError::ToSelfDelay => None,
        }
    }
}

impl From<script::Error> for ToLocalScriptParseError {
    fn from(err: script::Error) -> Self {
        ToLocalScriptParseError::Script(err)
    }
}

impl From<secp256k1::Error> for ToLocalScriptParseError {
    fn from(err: secp256k1::Error) -> Self {
        ToLocalScriptParseError::PublicKey(err)
    }
}
