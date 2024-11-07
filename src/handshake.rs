//! Peer Wire Protocol & Handshake
//!
//! https://www.bittorrent.org/beps/bep_0003.html#peer-protocol
//!
//! https://wiki.theory.org/BitTorrentSpecification#Peer_wire_protocol_.28TCP.29
//!
//! `$ ./your_bittorrent.sh handshake sample.torrent <peer_ip>:<peer_port>`
//!
//! `Peer ID: 0102030405060708090a0b0c0d0e0f1011121314`
//!
//! Exact value will be different as it is randomly generated.
//!
//! *Note:* To get a peer IP & port to test this locally, run `./your_bittorrent.sh peers sample.torrent`
//! and pick any peer from the list.

use std::io::{Read, Write};
use std::net::{SocketAddrV4, TcpStream};
use std::path::PathBuf;

use crate::constants::{BT_PROTOCOL, BT_PROTO_LEN, HANDSHAKE_LEN, PEER_ID};
use crate::handshake::reserved::Reserved;
use crate::meta_info::meta_info;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Sends a handshake to a peer, and receives a handshake from the peer, in the same format.
///
/// Returns the 40 characters long hexadecimal representation of the peer ID received during the handshake.
pub fn handshake(torrent: &PathBuf, peer: &SocketAddrV4) -> Result<String> {
    let meta = meta_info(torrent)?;
    let info_hash = meta.info.info_hash;

    let mut buf = Vec::with_capacity(HANDSHAKE_LEN);
    buf.push(BT_PROTO_LEN);
    buf.extend(BT_PROTOCOL.as_bytes());
    buf.extend([0; 8]);
    buf.extend(hex::decode(&info_hash)?);
    buf.extend(PEER_ID.bytes());

    let mut stream = TcpStream::connect(peer)?;

    let written = stream.write(&buf)?;
    assert_eq!(HANDSHAKE_LEN, written);

    stream.read_exact(&mut buf)?;
    let peer_id = &buf[48..];
    let peer_id = hex::encode(peer_id);

    Ok(peer_id)
}

/// Unused
///
/// The handshake is a required message and must be the first message transmitted by the client.
/// It is (49+len(pstr)) bytes long.
///
/// handshake: <pstrlen><pstr><reserved><info_hash><peer_id>
#[derive(Debug, Deserialize, Serialize)]
struct Handshake {
    /// String length of <pstr>, as a single raw byte
    pstrlen: u8,

    /// String identifier of the protocol
    pstr: String,

    /// Eight (8) reserved bytes. All current implementations use all zeroes.
    reserved: Reserved,

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

    /// The Reserved field
    ///
    /// Eight (8) reserved bytes. All current implementations use all zeroes.
    #[derive(Debug)]
    pub struct Reserved(pub [u8; 8]);

    impl Reserved {
        pub(crate) fn new() -> Self {
            Self([0; 8])
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
