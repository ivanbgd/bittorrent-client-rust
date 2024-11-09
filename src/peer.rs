//! # Peer
//!
//! Peer data structure for easier work with multiple peers at once

use std::fmt::{Display, Formatter};
use std::io::{Read, Write};
use std::net::{SocketAddrV4, TcpStream};

use anyhow::{anyhow, Result};

use crate::constants::*;
use crate::errors::PeerError;

/// Peer object
///
/// Holds an open stream to the peer and the peer's ID, as well as its socket address.
///
/// Created through [`Peer::new`] by passing it the peer's socket address.
///
/// Initialized through a call to [`Peer::handshake`].
///
/// [`Peer`] implements [`Display`] so it can be printed as the 40 characters long hexadecimal
/// representation of the peer ID received during the handshake.
#[derive(Debug)]
pub struct Peer {
    /// The peer's IP address and port
    pub addr: SocketAddrV4,

    /// An open stream to the peer
    pub stream: Option<TcpStream>,

    /// A 20 bytes long SHA1 representation of the peer ID received during the handshake
    peer_id: Option<[u8; SHA1_LEN]>,
}

impl Peer {
    /// Creates a new [`Peer`] object by taking the peer's IP address and port
    pub(crate) fn new(addr: &SocketAddrV4) -> Self {
        Self {
            addr: *addr,
            stream: None,
            peer_id: None,
        }
    }

    /// Sends a handshake to a peer, and receives a handshake from the peer, in the same format.
    ///
    /// `info_hash` can be obtained and calculated from a torrent file.
    ///
    /// The handshake is a required message and must be the first message transmitted by the client.
    ///
    /// Tries to connect to the peer; sets the `stream` field if it succeeds.
    ///
    /// Also sets the 20 bytes long SHA1 representation of the `peer_id` received during a successful handshake.
    pub(crate) fn handshake(&mut self, info_hash: &[u8; SHA1_LEN]) -> Result<(), PeerError> {
        let mut buf = [0u8; HANDSHAKE_MSG_LEN];
        buf[0] = BT_PROTO_LEN as u8;
        buf[BT_PROTOCOL_RANGE].copy_from_slice(BT_PROTOCOL.as_bytes());
        buf[HANDSHAKE_RESERVED_RANGE].copy_from_slice(&HANDSHAKE_RESERVED);
        buf[INFO_HASH_RANGE].copy_from_slice(info_hash);
        buf[PEER_ID_RANGE].copy_from_slice(PEER_ID.as_bytes());

        let mut stream = TcpStream::connect(self.addr)?;

        let written = stream.write(&buf)?;
        assert_eq!(HANDSHAKE_MSG_LEN, written);

        stream.read_exact(&mut buf)?;
        if (&buf[BT_PROTOCOL_RANGE] == BT_PROTOCOL.as_bytes())
            && (&buf[INFO_HASH_RANGE] == info_hash)
        {
            let peer_id = &buf[(HANDSHAKE_MSG_LEN - SHA1_LEN)..HANDSHAKE_MSG_LEN];
            self.peer_id = Some(<[u8; SHA1_LEN]>::try_from(peer_id)?);
            eprintln!("{peer_id:?}"); //todo
        } else {
            return Err(PeerError::HandshakeError(anyhow!(
                "received handshake parameters from peer {} don't match the sent parameters",
                self.addr
            )));
        }

        self.stream = Some(stream);

        Ok(())
    }

    // TODO
    /// Receive a message from a peer
    pub(crate) fn recv_msg(&mut self) -> Result<()> {
        // let _ = self.stream.as_mut().unwrap().write(&[0u8; 1])?;
        Ok(())
    }
}

/// Displays the 40 characters long hexadecimal representation of the peer ID received during the handshake.
impl Display for Peer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.peer_id.unwrap()))
    }
}
