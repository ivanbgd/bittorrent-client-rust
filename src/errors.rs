//! # Errors
//!
//! Error types and helper functions used in the application

use std::array::TryFromSliceError;

use thiserror::Error;

/// Errors related to working with [`crate::peer::Peer`]
#[derive(Debug, Error)]
pub enum PeerError {
    #[error("Handshake error: {0}")]
    HandshakeError(#[from] anyhow::Error),
}

impl From<std::io::Error> for PeerError {
    fn from(value: std::io::Error) -> Self {
        PeerError::HandshakeError(anyhow::Error::from(value))
    }
}

impl From<TryFromSliceError> for PeerError {
    fn from(value: TryFromSliceError) -> Self {
        PeerError::HandshakeError(anyhow::Error::from(value))
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
pub fn ae2s(err: anyhow::Error) -> String {
    err.to_string()
}

/// Converts an error, [`std::io::Error`], to [`String`].
///
/// This is intended as a helper function to be used as an argument to
/// [`Result::map_err`] for a shorter syntax.
pub fn ioe2s(err: std::io::Error) -> String {
    err.to_string()
}
