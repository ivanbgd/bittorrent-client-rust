//! # Errors
//!
//! Error types and helper functions used in the application

use std::array::TryFromSliceError;
use std::io;
use std::net::SocketAddrV4;
use std::num::TryFromIntError;

use thiserror::Error;

use crate::message::MessageId;

/// Errors related to working with [`crate::meta_info`]
#[derive(Debug, Error)]
pub enum MetaInfoError {
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("Deserialize error: {0}")]
    DeserializeError(#[from] serde_bencode::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<MetaInfoError> for String {
    fn from(value: MetaInfoError) -> Self {
        value.to_string()
    }
}

/// Errors related to working with [`crate::tracker`]
#[derive(Debug, Error)]
pub enum TrackerError {
    #[error("Metainfo error: {0}")]
    MetaInfoError(#[from] MetaInfoError),

    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Deserialize error: {0}")]
    DeserializeError(#[from] serde_bencode::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<TrackerError> for String {
    fn from(value: TrackerError) -> Self {
        value.to_string()
    }
}

/// Errors related to working with [`crate::message::MessageCodec`]
#[derive(Debug, Error)]
pub enum MessageCodecError {
    #[error("Frame of length {0} is too large.")]
    LengthError(String),
}

impl From<io::Error> for MessageCodecError {
    fn from(value: io::Error) -> Self {
        MessageCodecError::LengthError(value.to_string())
    }
}

/// Errors related to working with [`crate::peer::Peer`]
#[derive(Debug, Error)]
pub enum PeerError {
    #[error("Handshake error: {0}")]
    HandshakeError(String),

    #[error("Tracker error: {0}")]
    TrackerError(#[from] TrackerError),

    #[error("Wrong message ID: {0}; expected {1}")]
    WrongMessageId(MessageId, MessageId),

    /// The peer doesn't have the piece.
    #[error("The peer {0} doesn't have the piece index {1}")]
    MissingPiece(SocketAddrV4, usize),

    /// No peer has the required piece.
    #[error("No peer has the piece index {0}")]
    NoPeerHasPiece(usize),

    #[error("Wrong piece index: {0}; expected index < {1}")]
    WrongPieceIndex(usize, usize),

    #[error(transparent)]
    TryFromIntError(#[from] TryFromIntError),

    #[error("Wrong length: expected {0}, got {1} bytes")]
    WrongLen(usize, usize),

    #[error(transparent)]
    FrameLengthError(MessageCodecError),

    #[error("Hash mismatch: expected {0}, calculated {1}")]
    HashMismatch(String, String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<io::Error> for PeerError {
    fn from(value: io::Error) -> Self {
        // PeerError::HandshakeError(value.to_string())
        PeerError::Other(anyhow::Error::from(value))
    }
}

impl From<TryFromSliceError> for PeerError {
    fn from(value: TryFromSliceError) -> Self {
        PeerError::HandshakeError(value.to_string())
    }
}

impl From<(MessageId, MessageId)> for PeerError {
    fn from(value: (MessageId, MessageId)) -> Self {
        PeerError::WrongMessageId(value.0, value.1)
    }
}

impl From<MessageCodecError> for PeerError {
    fn from(value: MessageCodecError) -> Self {
        PeerError::FrameLengthError(value)
    }
}

impl From<PeerError> for String {
    fn from(value: PeerError) -> Self {
        value.to_string()
    }
}

/// Converts an error, [`anyhow::Error`], to [`String`].
///
/// This is intended as a helper function to be used as an argument to
/// [`Result::map_err`] for a shorter syntax.
///
/// Both error types are foreign, so we can't `impl From<anyhow::Error> for String`.
pub fn ae2s(err: anyhow::Error) -> String {
    err.to_string()
}

/// Converts an error, [`std::io::Error`], to [`String`].
///
/// This is intended as a helper function to be used as an argument to
/// [`Result::map_err`] for a shorter syntax.
///
/// Both error types are foreign, so we can't `impl From<std::io::Error> for String`.
pub fn ioe2s(err: std::io::Error) -> String {
    err.to_string()
}
