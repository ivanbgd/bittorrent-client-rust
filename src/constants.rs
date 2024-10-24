/// Length of the used hash sum, which is the [`sha1::Sha1`] sum, and its length is always 20
pub const SHA1_LEN: usize = 20;

/// Our own Peer ID
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
pub const PEER_LEN: usize = 6;
