//! # Peer Communication Facilities
//!
//! These are CLI commands per project requirements.
//!
//! ## Peer Wire Protocol & Handshake
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
//!
//! ## Peer Messages
//!
//! https://www.bittorrent.org/beps/bep_0003.html#peer-messages
//!
//! https://wiki.theory.org/BitTorrentSpecification#Messages
//!
//! `$ ./your_bittorrent.sh download_piece -o /tmp/test-piece sample.torrent <piece_index>`

use std::cmp::min;
// use std::fs::OpenOptions;
use std::net::SocketAddrV4;
use std::path::PathBuf;
// use std::sync::OnceLock;

use anyhow::Result;
use sha1::{Digest, Sha1};
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

use crate::constants::{BLOCK_SIZE, DEF_MSG_LEN, MAX_PIPELINED_REQUESTS, SHA1_LEN};
use crate::errors::PeerError;
use crate::message::{Message, MessageId, RequestPayload};
use crate::meta_info::Mode;
use crate::peer::Peer;
use crate::tracker::get_peers;

// static INFO_HASH: OnceLock<Result<[u8; SHA1_LEN]>> = OnceLock::new();

/// Sends a handshake to a single peer, and receives a handshake from the peer, in the same format.
///
/// Arguments:
/// - peer: &[`SocketAddrV4`], peer's socket address, <peer_ip>:<peer_port> (example: 127.0.0.1:8080)
/// - info_hash: &[u8; SHA1_LEN], can be obtained and calculated from a torrent file
///
/// Returns [`Peer`] which holds a 20 bytes long SHA1 representation of the peer ID received during the handshake.
///
/// [`Peer`] implements [`Display`] so it can be printed as the 40 characters long hexadecimal
/// representation of the peer ID received during the handshake.
///
/// Works with a single peer whose socket address it takes as an argument.
///
/// The handshake is a required message and must be the first message transmitted by the client.
///
/// `$ ./your_bittorrent.sh handshake sample.torrent <peer_ip>:<peer_port>`
///
/// `Peer ID: 0102030405060708090a0b0c0d0e0f1011121314`
///
/// Exact value will be different as it is randomly generated.
pub async fn handshake(peer: &SocketAddrV4, info_hash: &[u8; SHA1_LEN]) -> Result<Peer, PeerError> {
    // let info_hash = INFO_HASH
    //     .get_or_init(|| Ok(meta_info(torrent)?.info.info_hash))
    //     .as_ref()
    //     .expect("info hash should be available at this point");

    let mut peer = Peer::new(peer);
    peer.handshake(info_hash).await?;

    Ok(peer)
}

/// Downloads a single piece of a file and stores it.
///
/// Arguments:
/// - output: &[`PathBuf`], path to the output file for storing the downloaded piece
/// - torrent: &[`PathBuf`], path to a torrent file
/// - piece_index: &[`usize`], zero-based piece index
///
/// The last piece can be smaller than other pieces which are of same fixed size that
/// is defined in the torrent file.
///
/// Pieces are split into blocks of 16 kB or potentially less in case of the very last block,
/// and transferred as such. The blocks are assembled into a full piece when all of them
/// have been received.
///
/// Supports multiple peers. This improves download speeds because it pipelines requests
/// and avoids delays between blocks being sent to us from the peers. Source (PDF):
/// [BitTorrent Economics Paper](http://bittorrent.org/bittorrentecon.pdf)
///
/// `$ ./your_bittorrent.sh download_piece -o /tmp/test-piece sample.torrent <piece_index>`
pub async fn download_piece(
    output: &PathBuf,
    torrent: &PathBuf,
    piece_index: usize,
) -> Result<(), PeerError> {
    let output = File::create(output).await?;
    // let output = OpenOptions::new().append(true).create(true).open(output)?; // todo rem
    let mut file_writer = BufWriter::new(output);

    // For validating the hash of the received piece
    let mut hasher = Sha1::new();

    // Perform the tracker GET request to get a list of peers
    let (peers, info) = get_peers(torrent)?;
    let mut peers = peers.0;

    // Calculate where the requested piece begins in the file - not needed in this function
    let _piece_begin = piece_index * info.plen;

    // The file to download is split into pieces of same fixed length,
    // which is defined in torrent file and is a power of two,
    // except potentially for the last piece which can be smaller.
    // File ultimately needs to be assembled from received pieces, but this function is not meant for that.
    // The file size is also provided in the torrent file.
    let file_len = match info.mode {
        Mode::SingleFile { length } => length,
        Mode::MultipleFile { .. } => unimplemented!("Multiple file mode"),
    };
    let piece_len = info.plen;
    let mut last_piece_len = file_len % piece_len;
    let num_pcs = file_len / piece_len + last_piece_len.clamp(0, 1); // not needed in this function
    if last_piece_len == 0 {
        last_piece_len = piece_len;
    }
    let is_last_piece = piece_index == num_pcs - 1;

    eprintln!("piece_index = {}", piece_index);
    eprintln!("file_len = {}", file_len);
    eprintln!("piece_len = {}", piece_len);
    eprintln!("last_piece_len = {}", last_piece_len);
    eprintln!("num_pcs = {}", num_pcs);
    eprintln!("is_last_piece = {}", is_last_piece);

    if piece_index >= num_pcs {
        return Err(PeerError::WrongPieceIndex(piece_index, num_pcs));
    }

    // Pieces are split into blocks and transferred as such.
    // Pieces ultimately need to be assembled from received blocks.
    // Block size is 16 kB (`BLOCK_SIZE`), except potentially for the last block which can be smaller.
    let block_len = BLOCK_SIZE;
    let mut num_blocks_per_piece = piece_len / block_len;
    let mut last_block_len = last_piece_len % block_len;
    let num_blocks_in_last_piece = last_piece_len / block_len + last_block_len.clamp(0, 1);
    if last_block_len == 0 {
        last_block_len = block_len;
    }
    let total_num_blocks = (num_pcs - 1) * num_blocks_per_piece + num_blocks_in_last_piece; // not needed in this function
    if is_last_piece {
        num_blocks_per_piece = num_blocks_in_last_piece;
    }

    eprintln!("block_len = {}", block_len);
    eprintln!("num_blocks_per_piece = {}", num_blocks_per_piece);
    eprintln!("num_blocks_in_last_piece = {}", num_blocks_in_last_piece);
    eprintln!("last_block_len = {}", last_block_len);
    eprintln!("total_num_blocks = {}", total_num_blocks);

    // Support working with multiple peers at the same time
    let mut num_peers = min(MAX_PIPELINED_REQUESTS, num_blocks_per_piece);
    if is_last_piece {
        num_peers = num_blocks_in_last_piece;
    }
    num_peers = min(peers.len(), num_peers);
    eprintln!("num_peers = {num_peers}; peers.len() = {}", peers.len()); // todo remove

    let mut work_peers: Vec<Peer> = Vec::with_capacity(num_peers);

    // let mut streams: Vec<TcpStream> = Vec::with_capacity(num_peers); // todo remove

    // Get all peers to work with - handshake with them and store them
    for (peer_idx, peer) in peers.iter_mut().enumerate().take(num_peers) {
        // Establish a TCP connection with a peer, and perform a handshake
        let mut peer = handshake(peer, &info.info_hash).await?;

        eprintln!("HS with peer_idx {peer_idx}: {}", peer.addr); // todo remove

        // todo remove
        // let stream = peer
        //     .stream
        //     .as_mut()
        //     .unwrap_or_else(|| panic!("Expected to get a stream from the peer {}", peer.addr));

        // Exchange messages with the peer: receive Bitfield, send Interested, receive Unchoke

        // A read buffer, for received messages
        let mut buf = [0u8; DEF_MSG_LEN];

        // Receive a Bitfield message
        // let _read = stream.read(&mut buf).await?; // todo remove
        let _read = peer.recv_msg(&mut buf).await?;
        // eprintln!("01 {peer_idx}: {}, {:?}", read, buf); // todo remove
        let msg: Message = (&buf[..]).into();
        // let msg: Message = buf[..].to_vec().into(); // todo remove
        eprintln!("01 peer_idx {peer_idx}: {:?} {}", msg, msg.id); //todo remove
        if msg.id != MessageId::Bitfield {
            return Err(PeerError::from((msg.id, MessageId::Bitfield)));
        }

        // Send the Interested message
        let msg = Message::new(MessageId::Interested, None);
        peer.send_msg(msg).await?;
        // let msg = <Vec<u8>>::from(msg); // Or just: stream.write_all(msg.into())?; // todo remove
        // eprintln!("02 peer_idx {peer_idx}: {:?}", msg); // todo remove
        // stream.write_all(&msg).await?; // todo remove

        // Receive an Unchoke message
        // let read = stream.read(&mut buf).await?; // todo remove
        let read = peer.recv_msg(&mut buf).await?;
        let msg = Message::from(&buf[..]);
        eprintln!("03 peer_idx {peer_idx}: {:?}", msg); //todo remove
        if read != 5 {
            return Err(PeerError::WrongLen(5, read));
        }
        if msg.id != MessageId::Unchoke {
            return Err(PeerError::from((msg.id, MessageId::Unchoke)));
        }

        work_peers.push(peer);

        // streams.push(stream); // todo remove
    }

    // let block_iters = num_blocks_per_piece / streams.len() + num_blocks_per_piece % streams.len();
    let block_iters =
        num_blocks_per_piece / work_peers.len() + num_blocks_per_piece % work_peers.len();

    // eprintln!("streams.len() = {}", streams.len()); //todo remove
    eprintln!("work_peers.len() = {}", work_peers.len()); //todo remove
    eprintln!("block_iters = {block_iters}",); //todo remove
    eprintln!("******************************************"); //todo remove
    let mut i = 0usize;

    // Fetch blocks from peers

    // Outer loop is by blocks, while the inner loop is by peers.
    // I am not sure that this brings any speed improvements; it might.
    'outer: for block_idx in 0..block_iters {
        for (peer_idx, peer) in work_peers.iter_mut().enumerate().take(num_peers) {
            // let i = block_idx * peer_idx + block_idx;
            eprintln!("10 block_idx = {block_idx}, peer_idx = {peer_idx}, i = {i}"); // todo remove

            // todo remove
            // let stream: &mut TcpStream = peer
            //     .stream
            //     .as_mut()
            //     .unwrap_or_else(|| panic!("Expected to get a stream from the peer {}", peer.addr));

            // Exchange messages with the peer
            //
            // Implementation note:
            // We don't use a write buffer for sending messages.
            // We instead create messages through their constructor and convert them into a stream of bytes.
            // We do this to have a cleaner code with greater readability.
            // Perhaps it would have been slightly more performant if we had used a write buffer directly,
            // without performing those function calls and conversions, but we chose a nicer-looking code.
            // An alternative might be to have methods for sending and receiving messages.
            // This note stands as a reminder in case we sometime decide to improve performance.

            // Send a Request message for each block - we don't request pieces but blocks.
            let index = piece_index as u32;
            let begin = u32::try_from(i * block_len)?;
            let mut length = block_len as u32;
            if is_last_piece && i == num_blocks_per_piece - 1 {
                length = last_block_len as u32;
            }
            eprintln!("i = {i}: index = {index}, begin = {begin}, length = {length}");
            let tmp =
                <RequestPayload as Into<Vec<u8>>>::into(RequestPayload::new(index, begin, length));
            // let tmp: Vec<u8> = RequestPayload::new(index, begin, length).into(); // todo remove
            let msg = Message::new(
                MessageId::Request,
                // Some(RequestPayload::new(index, begin, length).into()), // todo remove
                Some(&tmp),
            );
            // let msg = <Vec<u8>>::from(msg); // Or just: stream.write_all(msg.into())?; // todo remove
            eprintln!(
                "11 block_idx = {block_idx}, peer_idx = {peer_idx}, i = {i}; send: {:?}",
                msg
            ); // todo remove
            peer.send_msg(msg).await?;
            // stream.write_all(&msg).await?; // todo remove

            // Wait for a Piece message for each block we've requested
            // let mut buf = vec![0u8; (4 + 1 + 8 + length) as usize]; // todo remove
            // stream.read_exact(&mut buf).await?; // todo remove
            let buf = peer.recv_piece_msg(4 + 1 + 8 + length).await?;
            eprintln!(
                "12 block_idx = {block_idx}, peer_idx = {peer_idx}, i = {i}; receive: {:?}, payload len = {}",
                &buf[..13],
                &buf[13..].len()
            ); // todo remove
            let msg: Message = (&buf[..]).into();
            // eprintln!("{:?} {}", msg, msg.id); //todo remove
            if msg.id != MessageId::Piece {
                return Err(PeerError::from((msg.id, MessageId::Piece)));
            }

            let payload = &msg.payload.expect("Expected to have some payload received")[8..];
            hasher.update(payload); // todo: is it thread-safe (atomic)?
            file_writer.write_all(payload).await?; // todo: is it thread-safe (atomic)?

            i += 1;
            if i == num_blocks_per_piece {
                eprintln!("******************************************"); //todo remove
                break 'outer;
            }

            eprintln!("******************************************"); //todo remove
        }
    }

    let piece = hex::encode(info.pieces.0[piece_index]);
    let hash = hex::encode(hasher.finalize());
    eprintln!("{:?} {:?}", piece, hash); //todo remove
    if piece != hash {
        return Err(PeerError::HashMismatch(piece, hash));
    }

    file_writer.flush().await?;

    Ok(())
}
