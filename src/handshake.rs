//! UNUSED MODULE
//!
//! Kept as a reference for serde.

/// Unused struct
///
/// The handshake is a required message and must be the first message transmitted by the client.
/// It is (49+len(pstr)) bytes long.
///
/// handshake: <pstrlen><pstr><reserved><info_hash><peer_id>
#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Handshake {
    /// String length of <pstr>, as a single raw byte
    pstrlen: u8,

    /// String identifier of the protocol
    pstr: String,

    /// Eight (8) reserved bytes. All current implementations use all zeroes.
    reserved: reserved::Reserved,

    /// 20-byte SHA1 hash of the info key in the metainfo file.
    /// This is the same info_hash that is transmitted in tracker requests.
    info_hash: String,

    /// 20-byte string used as a unique ID for the client.
    /// This is usually the same peer_id that is transmitted in tracker requests.
    peer_id: String,
}

/// Unused module
mod reserved {
    use std::fmt::Formatter;

    use serde::de::{Deserialize, Deserializer, Error, Visitor};
    use serde::ser::{Serialize, Serializer};

    use crate::constants::HANDSHAKE_RESERVED;

    /// The Reserved field
    ///
    /// Eight (8) reserved bytes. All current implementations use all zeroes.
    #[derive(Debug)]
    pub struct Reserved(pub [u8; 8]);

    impl Reserved {
        pub(crate) fn new() -> Self {
            Self(HANDSHAKE_RESERVED)
        }
    }

    struct ReservedVisitor;

    impl<'de> Visitor<'de> for ReservedVisitor {
        type Value = Reserved;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            write!(formatter, "eight zero bytes",)
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: Error,
        {
            let length = v.len();
            if length == 8 {
                Ok(Reserved::new())
            } else {
                Err(E::custom(format!("array length, {}, is not 8", length)))
            }
        }
    }

    impl<'de> Deserialize<'de> for Reserved {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_bytes(ReservedVisitor)
        }
    }

    impl Serialize for Reserved {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let seq = self.0;

            serializer.serialize_bytes(&seq)
        }
    }
}
