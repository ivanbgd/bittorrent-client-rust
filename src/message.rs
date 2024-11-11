//! # Messages
//!
//! https://www.bittorrent.org/beps/bep_0003.html#peer-messages
//!
//! https://wiki.theory.org/BitTorrentSpecification#Messages

use std::fmt::{Display, Formatter};

/// Message types
///
/// All non-keepalive messages contain a single byte which holds their type.
#[derive(Debug, PartialEq)]
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
    ///   - length: integer specifying the requested length
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

impl Display for MessageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self as *const Self as u8)
    }
}

impl From<MessageId> for u8 {
    fn from(value: MessageId) -> u8 {
        value as u8
        // match value {
        //     MessageId::Choke => 0,
        //     MessageId::Unchoke => 1,
        //     MessageId::Interested => 2,
        //     MessageId::NotInterested => 3,
        //     MessageId::Have => 4,
        //     MessageId::Bitfield => 5,
        //     MessageId::Request => 6,
        //     MessageId::Piece => 7,
        //     MessageId::Cancel => 8,
        //     MessageId::Port => 9,
        // }
    }
}

impl From<u8> for MessageId {
    fn from(value: u8) -> MessageId {
        match value {
            0 => MessageId::Choke,
            1 => MessageId::Unchoke,
            2 => MessageId::Interested,
            3 => MessageId::NotInterested,
            4 => MessageId::Have,
            5 => MessageId::Bitfield,
            6 => MessageId::Request,
            7 => MessageId::Piece,
            8 => MessageId::Cancel,
            9 => MessageId::Port,
            _ => panic!("{}", format!("Unsupported message ID: {}", value)),
        }
    }
}

/// All messages in the protocol take the form of `<length prefix><message ID><payload>`.
/// - The length prefix is a four byte big-endian value.
///     - It doesn't count toward the total message length.
///     - It represents the length of the rest of the message in bytes.
/// - The message ID is a single decimal byte.
/// - The payload is message-dependent.
///
/// The keep-alive message is a message with zero bytes, specified with the length prefix set to zero.
/// There is no message ID and no payload for it.
#[derive(Debug)]
pub struct Message<'a> {
    // pub struct Message {
    len: u32,
    pub id: MessageId,
    pub payload: Option<&'a [u8]>,
    // payload: Option<Vec<u8>>,
}

impl<'a> Message<'a> {
    // impl Message {
    /// Creates a new message consisting of message length, type and payload for sending to a peer.
    pub fn new(id: MessageId, payload: Option<&'a [u8]>) -> Self {
        // pub fn new(id: MessageId, payload: Option<Vec<u8>>) -> Self {
        let payload_len = payload.unwrap_or_default().len();
        // let payload_len = payload.as_ref().unwrap_or(&vec![]).len();
        let len = 1 + payload_len as u32;
        eprintln!("pay len 1 = {}", payload_len); // todo remove

        Self { len, id, payload }
    }
}

/// Converts a [`Message`] into a byte stream.
impl<'a> From<Message<'a>> for Vec<u8> {
    // impl From<Message> for Vec<u8> {
    /// Serializes a [`Message`] for a send transfer over the wire.
    fn from(val: Message<'a>) -> Vec<u8> {
        // fn from(val: Message) -> Vec<u8> {
        let len = u32::to_be_bytes(val.len);
        let id = val.id.into();
        eprintln!("len = {:?}, {}", len, val.len); // todo remove
                                                   // let payload = match val.payload {
                                                   //     Some(payload) => payload,
                                                   //     None => &[0u8; 0],
                                                   // };
        let payload = val.payload.unwrap_or_default();
        let payload_len = payload.len();
        eprintln!("pay len 2 = {}", payload_len); // todo remove

        let mut buf = Vec::with_capacity(4 + 1 + payload_len);

        buf.extend(len);
        buf.push(id);
        buf.extend(payload);
        eprintln!("buf len = {}, cap = {}", buf.len(), buf.capacity()); // todo remove

        buf
    }
}

/// Converts a byte stream into a [`Message`].
impl<'a> From<&'a [u8]> for Message<'a> {
    // impl From<Vec<u8>> for Message {
    /// Deserializes a received [`Message`] from a wire transfer.
    fn from(value: &'a [u8]) -> Message {
        // fn from(value: Vec<u8>) -> Message {
        let len = u32::from_be_bytes(<[u8; 4]>::try_from(&value[0..4]).unwrap_or_else(|_| {
            panic!(
                "Failed to deserialize message length; received: {:?}",
                value
            )
        }));
        let id = value[4].into(); // Same as: let id = MessageId::from(value[4]);
        let payload = if len == 1 {
            None
        } else {
            Some(&value[5..4 + len as usize])
            // Some(value[5..4 + len as usize].to_vec())
        };

        Self { len, id, payload }
    }
}

/// Payload for the [`MessageId::Request`] message
///
/// The payload contains the following information:
///   - index: integer specifying the zero-based piece index
///   - begin: integer specifying the zero-based byte offset within the piece
///   - length: integer specifying the requested length
#[derive(Debug)]
pub struct RequestPayload {
    index: u32,
    begin: u32,
    length: u32,
}

impl RequestPayload {
    /// Creates a new request payload consisting of piece index, byte offset within the piece
    /// and length for sending to a peer.
    pub fn new(index: u32, begin: u32, length: u32) -> Self {
        Self {
            index,
            begin,
            length,
        }
    }
}

/// Converts a [`RequestPayload`] into a byte stream.
impl From<RequestPayload> for Vec<u8> {
    /// Serializes a [`RequestPayload`] for a send transfer over the wire.
    fn from(value: RequestPayload) -> Vec<u8> {
        let mut buf = Vec::with_capacity(12);
        buf.extend(u32::to_be_bytes(value.index));
        buf.extend(u32::to_be_bytes(value.begin));
        buf.extend(u32::to_be_bytes(value.length));
        buf
    }
}

// /// Converts a [`RequestPayload`] into a byte stream.
// impl<'a> From<RequestPayload> for &'a [u8] {
//     /// Serializes a [`RequestPayload`] for a send transfer over the wire.
//     fn from(value: RequestPayload) -> &'a [u8] {
//         let mut buf = Vec::with_capacity(12);
//         buf.extend(u32::to_be_bytes(value.index));
//         buf.extend(u32::to_be_bytes(value.begin));
//         buf.extend(u32::to_be_bytes(value.length));
//         buf.leak()
//     }
// }

// DOESN'T COMPILE, NATURALLY
// /// Converts a [`RequestPayload`] into a byte stream.
// impl<'a> From<RequestPayload> for &'a [u8] {
//     /// Serializes a [`RequestPayload`] for a send transfer over the wire.
//     fn from(value: RequestPayload) -> &'a [u8] {
//         let mut buf = [0u8; 12];
//         buf.copy_from_slice(&u32::to_be_bytes(value.index));
//         buf.copy_from_slice(&u32::to_be_bytes(value.begin));
//         buf.copy_from_slice(&u32::to_be_bytes(value.length));
//         buf
//     }
// }
