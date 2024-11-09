//! # Messages
//!
//! https://www.bittorrent.org/beps/bep_0003.html#peer-messages
//!
//! https://wiki.theory.org/BitTorrentSpecification#Messages

/// Message types
///
/// All non-keepalive messages contain a single byte which holds their type.
#[derive(Debug)]
pub enum MessageId {
    /// choke: `<len=0001><id=0>`
    ///
    /// The `choke` message is fixed-length (1 byte) and has no payload.
    Choke = 0,

    /// unchoke: `<len=0001><id=1>`
    ///
    /// The `unchoke` message is fixed-length (1 byte) and has no payload.
    Unchoke = 1,

    /// interested: `<len=0001><id=2>`
    ///
    /// The `interested` message is fixed-length (1 byte) and has no payload.
    Interested = 2,

    /// not interested: `<len=0001><id=3>`
    ///
    /// The `not interested` message is fixed-length (1 byte) and has no payload.
    NotInterested = 3,

    /// have: `<len=0005><id=4><piece index>`
    ///
    /// The `have` message is fixed-length (5 bytes). The payload is the zero-based index of a piece
    /// that has just been successfully downloaded and verified via the hash.
    Have = 4,

    /// bitfield: `<len=0001+X><id=5><bitfield>`
    ///
    /// The `bitfield` message may only be sent immediately after the handshaking sequence is completed,
    /// and before any other messages are sent. It is optional, and need not be sent if a client has no pieces.
    ///
    /// The bitfield message is variable length (1 + X bytes), where X is the length of the bitfield.
    ///
    /// The payload is a bitfield representing the pieces that have been successfully downloaded.
    ///
    /// The high bit in the first byte corresponds to piece index 0. Bits that are cleared indicated a missing piece,
    /// and set bits indicate a valid and available piece. Spare bits at the end are set to zero.
    Bitfield = 5,

    /// request: `<len=0013><id=6><index><begin><length>`
    ///
    /// The `request` message is fixed length (13 bytes), and is used to request a block.
    ///
    /// The payload contains the following information:
    ///   - index: integer specifying the zero-based piece index
    ///   - begin: integer specifying the zero-based byte offset within the piece
    ///   - length: integer specifying the requested length.
    Request = 6,

    /// piece: `<len=0009+X><id=7><index><begin><block>`
    ///
    /// The `piece` message is variable length (9 + X bytes), where X is the length of the block.
    ///
    /// The payload contains the following information:
    ///   - index: integer specifying the zero-based piece index
    ///   - begin: integer specifying the zero-based byte offset within the piece
    ///   - block: block of data, which is a subset of the piece specified by index.
    Piece = 7,

    /// cancel: `<len=0013><id=8><index><begin><length>`
    ///
    /// The `cancel` message is fixed length (13 bytes), and is used to cancel block requests.
    ///
    /// The payload is identical to that of the "request" message. It is typically used during "End Game".
    Cancel = 8,

    /// port: `<len=0003><id=9><listen-port>`
    ///
    /// The `port` message is fixed length (3 bytes).
    ///
    /// The port message is sent by newer versions of the Mainline that implements a DHT tracker.
    /// The listen port is the port this peer's DHT node is listening on.
    /// This peer should be inserted in the local routing table (if DHT tracker is supported).
    Port = 9,
}

/// All messages in the protocol take the form of `<length prefix><message ID><payload>`.
/// - The length prefix is a four byte big-endian value.
/// - The message ID is a single decimal byte.
/// - The payload is message-dependent.
///
/// The keep-alive message is a message with zero bytes, specified with the length prefix set to zero.
/// There is no message ID and no payload for it.
#[derive(Debug)]
pub struct Message<'a> {
    len: u32, // todo: is it needed?
    id: MessageId,
    payload: &'a [u8],
}

impl<'a> Message<'a> {
    /// Creates a new message consisting of message length, type and payload for sending to a peer
    pub fn new(id: MessageId, payload: &'a [u8]) -> Self {
        let len = 4 + 1 + payload.len() as u32;

        Self { len, id, payload }
    }
}

impl<'a> From<Message<'a>> for &'a [u8] {
    fn from(val: Message<'a>) -> Self {
        &[2u8; 1]
    }
}
