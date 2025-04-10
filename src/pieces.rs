//! # Pieces
//!
//! Pieces are SHA1 hash values, and they are concatenated into a byte string in a torrent file.

use serde::de::{Deserialize, Deserializer, Error, Visitor};
use serde::ser::{Serialize, Serializer};
use std::fmt;
use std::fmt::{Debug, Display, Formatter};

use crate::constants::{HashType, SHA1_LEN};

/// Byte string consisting of the concatenation of all 20-byte SHA1 hash values,
/// one per piece (byte string, i.e., not urlencoded)
///
/// A string whose length is a multiple of 20. It is to be subdivided into strings of length 20,
/// each of which is the SHA1 hash of the piece at the corresponding index.
///
/// Implemented as vector of 20-byte arrays.
#[derive(Clone, Default)]
pub struct Pieces(pub Vec<HashType>);

impl Debug for Pieces {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let pieces = self
            .0
            .iter()
            .map(|h| String::from_utf8_lossy(h))
            .collect::<Vec<_>>();

        write!(f, "Pieces([{pieces:?}])")
    }
}

impl Display for Pieces {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let pieces: String = self
            .0
            .iter()
            .map(hex::encode)
            .collect::<Vec<_>>()
            .join("\n");

        writeln!(f, "{pieces}")
    }
}

struct PiecesVisitor;

impl<'de> Visitor<'de> for PiecesVisitor {
    type Value = Pieces;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "a byte string whose length is a multiple of {}",
            SHA1_LEN
        )
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let pcs_len = v.len();
        if pcs_len % SHA1_LEN == 0 {
            Ok(Pieces(
                v.chunks_exact(SHA1_LEN)
                    .map(|piece| piece.as_ref().try_into().expect("Expected length 20."))
                    .collect(),
            ))
        } else {
            Err(E::custom(format!(
                "length of 'pieces', {}, is not divisible by SHA1 sum length, which is {}",
                pcs_len, SHA1_LEN
            )))
        }
    }
}

impl<'de> Deserialize<'de> for Pieces {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(PiecesVisitor)
    }
}

impl Serialize for Pieces {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let seq = self.0.concat();

        serializer.serialize_bytes(&seq)
    }
}
