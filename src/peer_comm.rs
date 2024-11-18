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
use std::net::SocketAddrV4;
use std::path::PathBuf;

use crate::constants::{BLOCK_SIZE, MAX_PIPELINED_REQUESTS, SHA1_LEN};
use crate::errors::PeerError;
use crate::message::{Message, MessageCodec, MessageId, RequestPayload};
use crate::peer::Peer;
use crate::tracker::get_peers;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use log::{debug, info};
use sha1::{Digest, Sha1};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

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
/// and avoids delays between blocks being sent to us from the peers.
/// Source (PDF): [BitTorrent Economics Paper](http://bittorrent.org/bittorrentecon.pdf)
/// Also see: https://wiki.theory.org/BitTorrentSpecification#Queuing
///
/// `$ ./your_bittorrent.sh download_piece -o /tmp/test-piece sample.torrent <piece_index>`
pub async fn download_piece(
    output: &PathBuf,
    torrent: &PathBuf,
    piece_index: usize,
) -> Result<(), PeerError> {
    let output = File::create(output).await?;
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
    let file_len = info.length();
    let piece_len = info.plen;
    let mut last_piece_len = file_len % piece_len;
    let num_pcs = file_len / piece_len + last_piece_len.clamp(0, 1);
    if last_piece_len == 0 {
        last_piece_len = piece_len;
    }
    let is_last_piece = piece_index == num_pcs - 1;

    debug!("piece_index = {}", piece_index);
    debug!("file_len = {}", file_len);
    debug!("piece_len = {}", piece_len);
    debug!("last_piece_len = {}", last_piece_len);
    debug!("num_pcs = {}", num_pcs);
    debug!("is_last_piece = {}", is_last_piece);

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

    debug!("block_len = {}", block_len);
    debug!("num_blocks_per_piece = {}", num_blocks_per_piece);
    debug!("num_blocks_in_last_piece = {}", num_blocks_in_last_piece);
    debug!("last_block_len = {}", last_block_len);
    debug!("total_num_blocks = {}", total_num_blocks);

    // Support working with multiple peers at the same time
    let mut num_peers = min(MAX_PIPELINED_REQUESTS, num_blocks_per_piece);
    if is_last_piece {
        num_peers = num_blocks_in_last_piece;
    }
    num_peers = min(peers.len(), num_peers);
    debug!("num_peers = {num_peers}; peers.len() = {}", peers.len());

    let mut streams: Vec<Framed<TcpStream, MessageCodec>> = Vec::with_capacity(num_peers);

    // Get all peers to work with - handshake with them and store their streams.
    // The selection of peers could be randomized, but it isn't necessary; rather, this is just an idea.
    for (peer_idx, peer) in peers.iter_mut().enumerate().take(num_peers) {
        // Establish a TCP connection with a peer, and perform a handshake
        let peer = handshake(peer, &info.info_hash).await?;

        debug!("00 Handshake with peer_idx {peer_idx}: {}", peer.addr);

        let mut stream = peer
            .stream
            .unwrap_or_else(|| panic!("Expected to get a stream from the peer {}", peer.addr));

        // Exchange messages with the peer: receive Bitfield, send Interested, receive Unchoke

        // Receive a Bitfield message
        let msg = stream
            .next()
            .await
            .context("Receive a Bitfield message")??;
        debug!("01 peer_idx {peer_idx}: {msg}");
        if msg.id != MessageId::Bitfield {
            return Err(PeerError::from((msg.id, MessageId::Bitfield)));
        }

        // Send the Interested message
        let msg = Message::new(MessageId::Interested, None);
        stream
            .send(msg)
            .await
            .context("Send the Interested message")?;

        // Receive an Unchoke message
        let msg = stream
            .next()
            .await
            .context("Receive an Unchoke message")??;
        debug!("02 peer_idx {peer_idx}: {msg}");
        if msg.id != MessageId::Unchoke {
            return Err(PeerError::from((msg.id, MessageId::Unchoke)));
        }

        streams.push(stream);
    }

    // Number of the outer loop iterations, which is per blocks
    let block_iters = num_blocks_per_piece / streams.len() + num_blocks_per_piece % streams.len();

    // The combined loop counter - from both loops; represents the block ordinal number
    let mut i = 0usize;

    debug!("streams.len() = {}", streams.len());
    debug!("block_iters = {block_iters}",);

    // Fetch blocks from peers

    // Outer loop is by blocks, while the inner loop is by peers.
    // I am not sure that this brings any speed improvements; it might.
    'outer: for block_idx in 0..block_iters {
        for (peer_idx, stream) in streams.iter_mut().enumerate().take(num_peers) {
            // Exchange messages with the peer

            // Send a Request message for each block - we don't request pieces but blocks.
            let index = piece_index as u32;
            let begin = u32::try_from(i * block_len)?;
            let mut length = block_len as u32;
            if is_last_piece && i == num_blocks_per_piece - 1 {
                length = last_block_len as u32;
            }
            let msg = Message::new(
                MessageId::Request,
                Some(RequestPayload::new(index, begin, length).into()),
            );
            debug!("i = {i}: block_idx = {block_idx}, peer_idx = {peer_idx}; piece index = {index}, begin = {begin}, length = {length}");
            stream.send(msg).await.context("Send a Request message")?;

            // Wait for a Piece message for each block we've requested.
            let msg = stream.next().await.context("Receive a Piece message")??;
            if msg.id != MessageId::Piece {
                return Err(PeerError::from((msg.id, MessageId::Piece)));
            }

            let payload = &msg.payload.expect("Expected to have some payload received")[8..];
            hasher.update(payload);
            file_writer.write_all(payload).await?;

            if i == num_blocks_per_piece - 1 {
                break 'outer;
            }

            i += 1;
        }
    }

    let piece = hex::encode(info.pieces.0[piece_index]);
    let hash = hex::encode(hasher.finalize());
    if piece != hash {
        return Err(PeerError::HashMismatch(piece, hash));
    }

    file_writer.flush().await?;

    Ok(())
}

/// Downloads a whole file and stores it.
///
/// Arguments:
/// - output: &[`PathBuf`], path to the output file for storing the downloaded piece
/// - torrent: &[`PathBuf`], path to a torrent file
///
/// `$ ./your_bittorrent.sh download -o /tmp/test.txt sample.torrent`
pub async fn download(output: &PathBuf, torrent: &PathBuf) -> Result<(), PeerError> {
    // let output = OpenOptions::new()
    //     .append(true)
    //     .create(true)
    //     .open(output)
    //     .await?;
    let file = File::create(output).await?;
    let mut file_writer = BufWriter::new(file);

    // Perform the tracker GET request to get a list of peers
    let (peers, info) = get_peers(torrent)?;
    let peers = peers.0;

    let file_len = info.length();
    let piece_len = info.plen;
    let mut last_piece_len = file_len % piece_len;
    let num_pcs = file_len / piece_len + last_piece_len.clamp(0, 1);
    if last_piece_len == 0 {
        last_piece_len = piece_len;
    }
    debug!("file_len = {}", file_len);
    debug!("piece_len = {}", piece_len);
    debug!("last_piece_len = {}", last_piece_len);
    debug!("num_pcs = {}", num_pcs);

    let block_len = BLOCK_SIZE;
    let mut num_blocks_per_piece = piece_len / block_len;
    let mut last_block_len = last_piece_len % block_len;
    let num_blocks_in_last_piece = last_piece_len / block_len + last_block_len.clamp(0, 1);
    if last_block_len == 0 {
        last_block_len = block_len;
    }
    let total_num_blocks = (num_pcs - 1) * num_blocks_per_piece + num_blocks_in_last_piece;
    debug!("block_len = {}", block_len);
    debug!("num_blocks_per_piece = {}", num_blocks_per_piece);
    debug!("num_blocks_in_last_piece = {}", num_blocks_in_last_piece);
    debug!("last_block_len = {}", last_block_len);
    debug!("total_num_blocks = {}", total_num_blocks);

    // All piece hashes from the torrent file
    let pieces = &info.pieces.0;

    let peer = &peers[0];

    // Establish a TCP connection with a peer, and perform a handshake
    let peer = handshake(peer, &info.info_hash).await?;

    let mut stream = peer
        .stream
        .unwrap_or_else(|| panic!("Expected to get a stream from the peer {}", peer.addr));

    // Exchange messages with the peer: receive Bitfield, send Interested, receive Unchoke

    // Receive a Bitfield message
    let msg = stream
        .next()
        .await
        .context("Receive a Bitfield message")??;
    if msg.id != MessageId::Bitfield {
        return Err(PeerError::from((msg.id, MessageId::Bitfield)));
    }

    // Send the Interested message
    let msg = Message::new(MessageId::Interested, None);
    stream
        .send(msg)
        .await
        .context("Send the Interested message")?;

    // Receive an Unchoke message
    let msg = stream
        .next()
        .await
        .context("Receive an Unchoke message")??;
    if msg.id != MessageId::Unchoke {
        return Err(PeerError::from((msg.id, MessageId::Unchoke)));
    }

    // Entire contents of the file
    let mut contents = vec![];

    // Block index - for logging purposes only
    let mut block = 0usize;

    // Download all pieces
    for (piece_index, piece_hash) in pieces.iter().enumerate() {
        let is_last_piece = piece_index == num_pcs - 1;
        if is_last_piece {
            num_blocks_per_piece = num_blocks_in_last_piece;
        }
        let block_iters = num_blocks_per_piece;

        // For validating the hash of the received piece
        let mut hasher = Sha1::new();

        for i in 0..block_iters {
            block += 1;

            // Send a Request message for each block - we don't request pieces but blocks.
            let index = piece_index as u32;
            let begin = u32::try_from(i * block_len)?;
            let mut length = block_len as u32;
            if is_last_piece && i == num_blocks_per_piece - 1 {
                length = last_block_len as u32;
            }
            let msg = Message::new(
                MessageId::Request,
                Some(RequestPayload::new(index, begin, length).into()),
            );
            debug!("block {block:3}/{total_num_blocks}, i = {i:3}: piece index = {index}, begin = {begin}, length = {length}");
            stream.send(msg).await.context("Send a Request message")?;

            // Wait for a Piece message for each block we've requested.
            let msg = stream.next().await.context("Receive a Piece message")??;
            if msg.id != MessageId::Piece {
                return Err(PeerError::from((msg.id, MessageId::Piece)));
            }

            let payload = &msg.payload.expect("Expected to have some payload received")[8..];
            hasher.update(payload);
            contents.extend(payload);
        }

        let piece = hex::encode(piece_hash);
        let hash = hex::encode(hasher.finalize());
        if piece != hash {
            return Err(PeerError::HashMismatch(piece, hash));
        }

        info!("piece {:2}/{num_pcs} downloaded", piece_index + 1);
    }

    file_writer.write_all(&contents).await?;
    file_writer.flush().await?;

    let file = File::open(output).await?;
    let file_size = file.metadata().await?.len() as usize;
    info!(
        "wrote {file_size} out of expected {file_len} bytes to \"{}\"",
        output.display()
    );
    if file_len != file_size {
        return Err(PeerError::WrongLen(file_len, file_size));
    }

    Ok(())
}

// // TODO: Move to a separate (new) file.
// pub struct Piece {
//     index: usize,
//     data: Vec<u8>,
//     correct_hash: String, // todo: not necessary?
//     calc_hash: String,    // todo: not necessary?
// }
