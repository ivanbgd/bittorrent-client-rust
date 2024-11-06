//! Peer Wire Protocol & Handshake
//!
//! https://www.bittorrent.org/beps/bep_0003.html#peer-protocol
//!
//! https://wiki.theory.org/BitTorrentSpecification#Peer_wire_protocol_.28TCP.29

use std::io::{Read, Write};
use std::net::{SocketAddrV4, TcpStream};
use std::path::PathBuf;

use crate::constants::{BT_PROTOCOL, BT_PROTO_LEN, HANDSHAKE_LEN, PEER_ID};
use crate::meta_info::meta_info;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Sends a handshake to a peer, and receives a handshake from the peer, in the same format.
///
/// Returns the 40 characters long hexadecimal representation of the peer ID received during the handshake.
pub fn handshake(path: &PathBuf, peer: &SocketAddrV4) -> Result<String> {
    let meta = meta_info(path)?;
    let info_hash = meta.info.info_hash;

    let mut buf = Vec::with_capacity(HANDSHAKE_LEN);
    buf.push(BT_PROTO_LEN);
    buf.extend(BT_PROTOCOL.as_bytes());
    buf.extend([0; 8]);
    buf.extend(hex::decode(&info_hash)?);
    buf.extend(PEER_ID.bytes());
    // eprintln!("{buf:?}"); //

    let _handshake = Handshake {
        pstrlen: BT_PROTO_LEN,
        pstr: BT_PROTOCOL.to_string(),
        reserved: [0; 8],
        info_hash,
        peer_id: PEER_ID.to_string(),
    };

    // buf.push(handshake.pstrlen);
    // buf.extend(BT_PROTOCOL.bytes());
    // eprintln!("{buf:?}"); //

    // Instead of a single peer we could pass a list of peers that we could get from the `get_peers` function,
    // but the automated tester expects a single hard-coded peer ID & port value.
    let mut stream = TcpStream::connect(peer)?;

    // Doesn't get serialized
    // let w = stream.write(&handshake)?;

    let written = stream.write(&buf)?;
    assert_eq!(HANDSHAKE_LEN, written);

    stream.read_exact(&mut buf)?;
    // eprintln!("{buf:?}, {}", buf.len());
    let peer_id = &buf[48..];
    let peer_id = hex::encode(peer_id);
    // eprintln!("{peer_id:?}, {}", peer_id.len());

    Ok(peer_id)
}

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
    reserved: [u8; 8],

    /// 20-byte SHA1 hash of the info key in the metainfo file.
    /// This is the same info_hash that is transmitted in tracker requests.
    info_hash: String,

    /// 20-byte string used as a unique ID for the client.
    /// This is usually the same peer_id that is transmitted in tracker requests.
    peer_id: String,
}
