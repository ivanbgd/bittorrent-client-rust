//! # Peer
//!
//! Peer data structure for easier work with multiple peers at once

use std::fmt::{Display, Formatter};
use std::net::SocketAddrV4;

use crate::constants::*;
use crate::errors::PeerError;
use crate::message::{Message, MessageCodec};

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

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
    pub stream: Option<Framed<TcpStream, MessageCodec>>,

    /// A 20 bytes long SHA1 representation of the peer ID received during the handshake
    peer_id: Option<HashType>,

    /// Represents the pieces that the peer has; received in a Bitfield message
    pub bitfield: Option<Vec<u8>>,
}

impl Peer {
    /// Creates a new [`Peer`] object by taking the peer's IP address and port.
    pub(crate) fn new(addr: &SocketAddrV4) -> Self {
        Self {
            addr: *addr,
            stream: None,
            peer_id: None,
            bitfield: None,
        }
    }

    /// Sends a handshake to a peer, and receives a handshake from the peer, in the same format.
    ///
    /// `info_hash` can be obtained and calculated from a torrent file or from a magnet link.
    ///
    /// The handshake is a required message and must be the first message transmitted by the client.
    ///
    /// During handshake, we announce extension support.
    ///
    /// Tries to connect to the peer; sets the `stream` field if it succeeds.
    ///
    /// Also sets the 20 bytes long SHA1 representation of the `peer_id` received during a successful handshake.
    pub(crate) async fn handshake(&mut self, info_hash: &HashType) -> Result<(), PeerError> {
        let mut reserved = HANDSHAKE_RESERVED;
        reserved[5] |= EXTENSION_SUPPORT_BIT;

        let mut buf = [0u8; HANDSHAKE_MSG_LEN];
        buf[0] = BT_PROTO_LEN as u8;
        buf[BT_PROTOCOL_RANGE].copy_from_slice(BT_PROTOCOL.as_bytes());
        buf[HANDSHAKE_RESERVED_RANGE].copy_from_slice(&reserved);
        buf[INFO_HASH_RANGE].copy_from_slice(info_hash);
        buf[PEER_ID_RANGE].copy_from_slice(PEER_ID.as_bytes());

        let mut stream = TcpStream::connect(self.addr)
            .await
            .context(format!("Failed to connect to peer {}", self.addr))?;

        stream
            .write_all(&buf)
            .await
            .context(format!("Failed to send handshake to peer {}", self.addr))?;

        stream.read_exact(&mut buf).await.context(format!(
            "Failed to receive handshake from peer {}",
            self.addr
        ))?;

        if (&buf[BT_PROTOCOL_RANGE] == BT_PROTOCOL.as_bytes())
            && (&buf[INFO_HASH_RANGE] == info_hash)
        {
            let peer_id = &buf[PEER_ID_RANGE];
            self.peer_id = Some(<HashType>::try_from(peer_id)?);
        } else {
            return Err(PeerError::HandshakeError(format!(
                "Received handshake parameters from peer {} don't match the sent parameters.",
                self.addr
            )));
        }

        let stream = Framed::new(stream, MessageCodec);

        self.stream = Some(stream);

        Ok(())
    }

    /// Flush the sink, processing all pending messages.
    ///
    /// This adapter is intended to be used when we want to stop sending
    /// to the peer until all current requests are processed.
    ///
    /// Meant for working with a batch of message requests, rather than
    /// with single messages, for increased speed.
    pub(crate) async fn flush(&mut self) -> Result<(), PeerError> {
        let stream = self.stream.as_mut().unwrap_or_else(|| {
            panic!(
                "Expected the peer {} to have its stream field populated.",
                self.addr
            )
        });
        stream.flush().await.context("flush messages to the peer")?;
        Ok(())
    }

    /// Feeds a message to a peer.
    ///
    /// Unlike [`Peer::_send`], does not flush messages to the peer.
    /// It is the caller’s responsibility to ensure all pending messages are processed,
    /// which can be done via [`Peer::flush`].
    ///
    /// Meant for working with a batch of message requests, rather than
    /// with single messages, for increased speed.
    pub(crate) async fn feed(&mut self, msg: Message) -> Result<(), PeerError> {
        let stream = self.stream.as_mut().unwrap_or_else(|| {
            panic!(
                "Expected the peer {} to have its stream field populated.",
                self.addr
            )
        });
        stream.feed(msg).await.context("feed a message")?;
        Ok(())
    }

    /// Sends a message to a peer.
    ///
    /// This includes flushing into the sink, so it is usually better to batch together messages to send
    /// via [`Peer::feed`] or [`Peer::send_all`] rather than flushing between each message.
    pub(crate) async fn _send(&mut self, msg: Message) -> Result<(), PeerError> {
        let stream = self.stream.as_mut().unwrap_or_else(|| {
            panic!(
                "Expected the peer {} to have its stream field populated.",
                self.addr
            )
        });
        stream.send(msg).await.context("send a message")?;
        Ok(())
    }

    /// Receives a message from a peer.
    pub(crate) async fn recv_msg(&mut self) -> Result<Message, PeerError> {
        let stream = self.stream.as_mut().unwrap_or_else(|| {
            panic!(
                "Expected the peer {} to have its stream field populated.",
                self.addr
            )
        });
        let msg = stream.next().await.context("receive a message")??;
        Ok(msg)
    }
}

/// Displays the 40 characters long hexadecimal representation of the peer ID received during the handshake.
impl Display for Peer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.peer_id.unwrap()))
    }
}
