//! # Errors
//!
//! Error types and helper functions used in the application

use std::array::TryFromSliceError;
use std::net::SocketAddrV4;
use std::num::TryFromIntError;
use std::string::FromUtf8Error;
use thiserror::Error;

use crate::message::MessageId;

/// Errors related to working with [`crate::meta_info`]
#[derive(Debug, Error)]
pub enum MetaInfoError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Deserialize error: {0}")]
    DeserializeError(#[from] serde_bencode::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl PartialEq for MetaInfoError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
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

impl PartialEq for TrackerError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl From<TrackerError> for String {
    fn from(value: TrackerError) -> Self {
        value.to_string()
    }
}

/// Errors related to working with [`MessageId`]
#[derive(Debug, Error, PartialEq)]
pub enum MessageIdError {
    #[error("Unsupported message ID: {0}")]
    UnsupportedId(u8),
}

/// Errors related to working with [`crate::message::Message`]
#[derive(Debug, Error)]
pub enum MessageError {
    #[error("{0}")]
    TryFromIntError(#[from] TryFromIntError),

    #[error("{0}")]
    TryFromSliceError(#[from] TryFromSliceError),

    #[error("{0}")]
    UnsupportedId(#[from] MessageIdError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors related to working with [`crate::message::PiecePayload`]
#[derive(Debug, Error)]
pub enum PiecePayloadError {
    #[error("Wrong Piece message length: expected {0}, received {1} bytes")]
    WrongLen(usize, usize),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors related to working with [`crate::message::MessageCodec`]
#[derive(Debug, Error)]
pub enum MessageCodecError {
    #[error("Frame of length {0} is too large.")]
    LengthError(String),

    #[error(transparent)]
    UnsupportedId(#[from] MessageIdError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl PartialEq for MessageCodecError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl From<std::io::Error> for MessageCodecError {
    fn from(value: std::io::Error) -> Self {
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

    /// Used at the beginning, if we can't find any per to work with at all.
    #[error("No peers to work with could be found.")]
    NoPeers,

    /// The peer doesn't have the piece.
    #[error("The peer {0} doesn't have the piece index {1}")]
    MissingPiece(SocketAddrV4, usize),

    /// No peer has the required piece.
    #[error("No peer has the piece index {0}")]
    NoPeerHasPiece(usize),

    /// No currently available peer or piece.
    #[error("No currently available peer or piece index {0}")]
    NoCurrentlyAvailablePeerOrPiece(usize),

    #[error("Wrong piece index: {0}; expected index < {1}")]
    WrongPieceIndex(usize, usize),

    #[error(transparent)]
    TryFromIntError(#[from] TryFromIntError),

    #[error("Wrong length: expected {0}, got {1} bytes")]
    WrongLen(usize, usize),

    #[error(transparent)]
    FrameLengthError(MessageCodecError),

    #[error(transparent)]
    PiecePayloadError(#[from] PiecePayloadError),

    #[error("Hash mismatch: expected {0}, calculated {1}")]
    HashMismatch(String, String),

    #[error("Wrong number of bytes written to file: expected {0}, got {1} bytes")]
    WrongWritten(usize, usize),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<std::io::Error> for PeerError {
    fn from(value: std::io::Error) -> Self {
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

/// Errors related to working with [`crate::magnet::*`]
#[derive(Debug, Error)]
pub enum MagnetError {
    #[error(transparent)]
    PeerError(#[from] PeerError),

    #[error("Parsing magnet link {0}.")]
    MagnetLinkParseError(#[from] MagnetLinkError),

    #[error(transparent)]
    HexError(#[from] hex::FromHexError),

    #[error("Tracker missing from magnet link.")]
    TrackerMissing,

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    DeserializeError(#[from] serde_bencode::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl PartialEq for MagnetError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl From<MagnetError> for String {
    fn from(value: MagnetError) -> Self {
        value.to_string()
    }
}

/// Errors related to working with [`crate::magnet::MagnetLink`]
#[derive(Debug, Error)]
pub enum MagnetLinkError {
    #[error("'{0}' failed as it doesn't start with 'magnet:?'")]
    NoMagnet(String),

    #[error("'{0}' failed as it doesn't contain the 'xt' field")]
    NoXt(String),

    #[error(transparent)]
    UrlDecode(#[from] FromUtf8Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl PartialEq for MagnetLinkError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
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
