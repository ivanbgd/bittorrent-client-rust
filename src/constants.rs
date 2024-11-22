//! # Constants
//!
//! Constants used throughout the application

use std::ops::Range;

/// Length of the used hash sum, which is the [`sha1::Sha1`] sum, and its length is always 20
pub const SHA1_LEN: usize = 20;

/// Our own Peer ID, 20 bytes long
///
/// https://wiki.theory.org/BitTorrentSpecification#peer_id
///
/// We fixed an arbitrary value here.
pub const PEER_ID: &str = "AAA3AAAAAAA7AAAABAAA";

pub const PORT: u16 = 6881;
pub const UPLOADED: usize = 0;
pub const DOWNLOADED: usize = 0;

/// https://www.bittorrent.org/beps/bep_0023.html
pub const COMPACT: u8 = 1;

/// Peer length in compact form
pub const PEER_LEN: usize = 6;

/// IP address (four octets), three dots, ':', port; example: "255.255.255.255:65535"
pub const PEER_DISPLAY_LEN: usize = 21;

/// String identifier of the protocol, used in handshake
pub const BT_PROTOCOL: &str = "BitTorrent protocol";
/// Length of the string identifier of the protocol, 19
pub const BT_PROTO_LEN: usize = BT_PROTOCOL.len();
/// Handshake reserved, eight zero bytes
pub const HANDSHAKE_RESERVED: [u8; 8] = [0; 8];
/// Handshake message length, 68
pub const HANDSHAKE_MSG_LEN: usize = 68;

/// Range of indices `[1..20]` of length 19 for the string identifier of the protocol; used in handshake
pub const BT_PROTOCOL_RANGE: Range<usize> = 1..(1 + BT_PROTO_LEN);
/// Range of indices `[20..28]` of length 8 for reserved bytes; used in handshake
pub const HANDSHAKE_RESERVED_RANGE: Range<usize> = (1 + BT_PROTO_LEN)..(1 + BT_PROTO_LEN + 8);
/// Range of indices `[28..48]` of length 20 for info hash SHA1; used in handshake
pub const INFO_HASH_RANGE: Range<usize> = (1 + BT_PROTO_LEN + 8)..(1 + BT_PROTO_LEN + 8 + SHA1_LEN);
/// Range of indices `[48..68]` of length 20 for peer ID SHA1; used in handshake
pub const PEER_ID_RANGE: Range<usize> = (1 + BT_PROTO_LEN + 8 + SHA1_LEN)..HANDSHAKE_MSG_LEN;

/// Default message length
pub const DEF_MSG_LEN: usize = 128;

/// Timeout in seconds when expecting to receive data
pub const TIMEOUT_SECS: u64 = 60;

/// Keep-alive period (timeout) in seconds
pub const KEEP_ALIVE_PERIOD_SECS: u64 = 120;
/// Size of a block (sub-piece) in bytes, 16 kB
pub const BLOCK_SIZE: usize = 1 << 14;

/// Maximum number of peers to work with
pub const MAX_NUM_PEERS: usize = 32;

/// Maximum number of block (sub-piece) requests pipelined at once
pub const MAX_PIPELINED_REQUESTS: usize = 32;

/// Maximal message frame size in bytes, 64 kB
pub const MAX_FRAME_SIZE: usize = 1 << 16;
