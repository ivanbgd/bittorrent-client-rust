//! # Peer
//!
//! Peer data structure for easier work with multiple peers at once

use std::fmt::{Display, Formatter};
use std::io::{Read, Write};
use std::net::{SocketAddrV4, TcpStream};

use anyhow::Result;

use crate::constants::{
    BT_PROTOCOL, BT_PROTO_LEN, HANDSHAKE_MSG_LEN, HANDSHAKE_RESERVED, PEER_ID, SHA1_LEN,
};

/// Peer object
///
/// Holds an open stream to the peer and the peer's ID, as well as its socket address.
///
/// Created through [`Peer::new`] by passing it the peer's socket address.
///
/// [`Peer`] implements [`Display`] so it can be printed as the 40 characters long hexadecimal
/// representation of the peer ID received during the handshake.
#[derive(Debug)]
pub struct Peer {
    /// The peer's IP address and port
    addr: SocketAddrV4,

    /// An open stream to the peer
    stream: Option<TcpStream>,

    /// A 20 bytes long SHA1 representation of the peer ID received during the handshake
    pub peer_id: Option<[u8; SHA1_LEN]>,
}

impl Peer {
    /// Creates a new [`Peer`] object by taking the peer's IP address and port
    pub fn new(addr: &SocketAddrV4) -> Result<Self> {
        Ok(Self {
            addr: *addr,
            stream: None,
            peer_id: None,
        })
    }

    /// Sends a handshake to a peer, and receives a handshake from the peer, in the same format.
    ///
    /// `info_hash` can be obtained and calculated from a torrent file.
    ///
    /// The handshake is a required message and must be the first message transmitted by the client.
    ///
    /// Tries to connect to the peer; sets the `stream` field if it succeeds.
    ///
    /// Sets the 20 bytes long SHA1 representation of the peer ID received during the handshake.
    pub(crate) fn handshake(&mut self, info_hash: [u8; SHA1_LEN]) -> Result<()> {
        let mut buf = Vec::with_capacity(HANDSHAKE_MSG_LEN);
        buf.push(BT_PROTO_LEN);
        buf.extend(BT_PROTOCOL.as_bytes());
        buf.extend(HANDSHAKE_RESERVED);
        buf.extend(&info_hash);
        buf.extend(PEER_ID.bytes());

        let mut stream = TcpStream::connect(self.addr)?;

        let written = stream.write(&buf)?;
        assert_eq!(HANDSHAKE_MSG_LEN, written);

        stream.read_exact(&mut buf)?;
        let peer_id = &buf[(HANDSHAKE_MSG_LEN - SHA1_LEN)..];

        self.stream = Some(stream);
        self.peer_id = Some(<[u8; SHA1_LEN]>::try_from(peer_id)?);

        Ok(())
    }
}

/// Displays the 40 characters long hexadecimal representation of the peer ID received during the handshake.
impl Display for Peer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.peer_id.unwrap()))
    }
}
