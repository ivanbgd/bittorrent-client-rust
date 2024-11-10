//! # Errors
//!
//! Error types and helper functions used in the application

use std::array::TryFromSliceError;
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

/// Errors related to working with [`crate::peer::Peer`]
#[derive(Debug, Error)]
pub enum PeerError {
    #[error("Handshake error: {0}")]
    HandshakeError(String),

    #[error("Tracker error: {0}")]
    TrackerError(#[from] TrackerError),

    #[error("Wrong message ID: {0}; expected {1}")]
    WrongMessageId(MessageId, MessageId),

    #[error(transparent)]
    Other(anyhow::Error),
}

impl From<std::io::Error> for PeerError {
    fn from(value: std::io::Error) -> Self {
        PeerError::HandshakeError(value.to_string())
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
