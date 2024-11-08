//! Peer Messages
//!
//! https://www.bittorrent.org/beps/bep_0003.html#peer-messages
//!
//! https://wiki.theory.org/BitTorrentSpecification#Messages
//!
//! `$ ./your_bittorrent.sh download_piece -o /tmp/test-piece sample.torrent <piece_index>`
//!
//! *Optional*: To improve download speeds, you can consider pipelining your requests.
//! [BitTorrent Economics Paper](http://bittorrent.org/bittorrentecon.pdf)
//! recommends having 5 requests pending at once, to avoid a delay between blocks being sent.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use crate::constants::{
    BT_PROTOCOL, BT_PROTO_LEN, DEF_MSG_LEN, HANDSHAKE_MSG_LEN, HANDSHAKE_RESERVED, PEER_ID,
    TIMEOUT_SECS,
};
use crate::meta_info::meta_info;
use crate::tracker::get_peers;

use anyhow::Result;

pub fn download_piece(output: &PathBuf, torrent: &PathBuf, piece_index: usize) -> Result<()> {
    // Read the torrent file to get the meta info
    let meta = meta_info(torrent)?;
    let info_hash = meta.info.info_hash;

    // Perform the tracker GET request to get a list of peers
    let peers = get_peers(torrent)?.0;
    // todo: perhaps choose PIPELINED_REQUESTS (at most) peers and connect to all of them
    let peer = peers[0];
    let mut stream = TcpStream::connect(peer)?;

    // Establish a TCP connection with a peer, and perform a handshake

    let mut buf = Vec::with_capacity(HANDSHAKE_MSG_LEN);
    buf.push(BT_PROTO_LEN);
    buf.extend(BT_PROTOCOL.as_bytes());
    buf.extend(HANDSHAKE_RESERVED);
    buf.extend(hex::decode(&info_hash)?);
    buf.extend(PEER_ID.bytes());

    let written = stream.write(&buf)?;
    assert_eq!(HANDSHAKE_MSG_LEN, written);

    stream.read_exact(&mut buf)?;
    let peer_id = &buf[48..];
    let peer_id = hex::encode(peer_id);
    eprintln!("{peer_id}");

    drop(buf);

    // Exchange messages with the peer
    eprintln!("{}", meta.info.plen);
    let mut send_buf: Vec<u8> = Vec::with_capacity(DEF_MSG_LEN);
    let mut recv_buf: Vec<u8> = Vec::with_capacity(meta.info.plen);

    stream.set_read_timeout(Some(Duration::new(TIMEOUT_SECS, 0)))?;
    stream.set_write_timeout(Some(Duration::new(TIMEOUT_SECS, 0)))?;
    // stream.set_nonblocking(false)?;

    let read = stream.read_exact(&mut recv_buf[..1])?;
    eprintln!("{:?}, {:?}", recv_buf, read);

    Ok(())
}

#[derive(Debug)]
enum MessageId {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
    Port = 9,
}

#[derive(Debug)]
/// All messages in the protocol take the form of <length prefix><message ID><payload>.
/// The length prefix is a four byte big-endian value.
/// The message ID is a single decimal byte.
/// The payload is message-dependent.
///
/// The keep-alive message is a message with zero bytes, specified with the length prefix set to zero.
/// There is no message ID and no payload.
struct Message<'a> {
    id: MessageId,
    payload: &'a [u8],
}
