//! # Messages
//!
//! https://www.bittorrent.org/beps/bep_0003.html#peer-messages
//!
//! https://wiki.theory.org/BitTorrentSpecification#Messages
//!
//! # Implementation note
//!
//! We are using the [`From`] trait and its associated [`From::from`] function.
//! The function returns raw result of conversion, meaning, it is not wrapped in [`Result`].
//! We cannot change that, because it is not our trait.
//!
//! This further means that we can only panic in case something goes wrong.
//! The rest of our code, i.e., most of it, returns raw results wrapped in [`anyhow::Error`],
//! so this part is not in line with the rest of the library code.
//!
//! This isn't crucial; we are just mentioning it in case we decide to change that in the future.
//! We could write our own functions or methods that perform the conversions and that return
//! results wrapped in [`anyhow::Error`].

use std::fmt::{Display, Formatter};

use anyhow::Result;
use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::constants::MAX_FRAME_SIZE;
use crate::errors::MessageCodecError;

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
    ///   - block: block of data, which is a subset of the piece specified by index
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
pub struct Message {
    len: u32,
    pub id: MessageId,
    pub payload: Option<Vec<u8>>,
}

impl Message {
    /// Creates a new message consisting of message length, type (`id`) and payload for sending to a peer.
    ///
    /// The message length is calculated automatically and stored in the message.
    pub fn new(id: MessageId, payload: Option<Vec<u8>>) -> Self {
        let payload_len = payload.as_ref().unwrap_or(&vec![]).len();
        let len = 1 + payload_len as u32;

        Self { len, id, payload }
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Message {{ len: {}, id: {:?}, payload: {:02X?} }}",
            self.len, self.id, &self.payload
        )
    }
}

/// Converts a [`Message`] into a byte stream.
impl From<Message> for Vec<u8> {
    /// Serializes a [`Message`] for a send transfer over the wire.
    fn from(val: Message) -> Vec<u8> {
        let len = u32::to_be_bytes(val.len);
        let id = val.id.into();
        let payload = val.payload.unwrap_or_default();
        let payload_len = payload.len();

        let mut buf = Vec::with_capacity(4 + 1 + payload_len);

        buf.extend(len);
        buf.push(id);
        buf.extend(payload);

        buf
    }
}

/// Converts a byte stream into a [`Message`].
impl From<Vec<u8>> for Message {
    /// Deserializes a [`Message`] received from a wire transfer.
    fn from(value: Vec<u8>) -> Message {
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
            Some(value[5..4 + len as usize].to_vec())
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

/// Unused.
///
/// Converts a [`RequestPayload`] into a byte stream.
impl<'a> From<RequestPayload> for &'a [u8] {
    /// Serializes a [`RequestPayload`] for a send transfer over the wire.
    fn from(value: RequestPayload) -> &'a [u8] {
        let mut buf = Vec::with_capacity(12);
        buf.extend(u32::to_be_bytes(value.index));
        buf.extend(u32::to_be_bytes(value.begin));
        buf.extend(u32::to_be_bytes(value.length));
        buf.leak()
    }
}

/// Unused.
///
/// Payload for the [`MessageId::Piece`] message
///
/// The payload contains the following information:
///   - index: integer specifying the zero-based piece index
///   - begin: integer specifying the zero-based byte offset within the piece
///   - block: block of data, which is a subset of the piece specified by index
#[derive(Debug)]
pub struct PiecePayload<'a> {
    _index: u32,
    _begin: u32,
    _block: &'a [u8],
}

impl<'a> PiecePayload<'a> {
    /// Creates a new piece payload consisting of piece index, byte offset within the piece
    /// and block of data from a message received from a peer.
    pub fn new(index: u32, begin: u32, block: &'a [u8]) -> Self {
        Self {
            _index: index,
            _begin: begin,
            _block: block,
        }
    }
}

/// Converts a byte stream into a [`PiecePayload`].
impl<'a> From<&'a [u8]> for PiecePayload<'a> {
    /// Deserializes a [`PiecePayload`] received from a wire transfer.
    ///
    /// This function is not aware of the requested length of the block of data,
    /// hence it can't check whether it has received the entire requested block.
    fn from(value: &'a [u8]) -> PiecePayload {
        let index = u32::from_be_bytes(value[0..4].try_into().expect("failed to convert index"));
        let begin = u32::from_be_bytes(value[4..8].try_into().expect("failed to convert begin"));
        let block = &value[8..];

        Self {
            _index: index,
            _begin: begin,
            _block: block,
        }
    }
}

#[derive(Debug)]
pub struct MessageCodec;

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = MessageCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            // Not enough data to read length prefix.
            return Ok(None);
        }

        // Read length prefix.
        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&src[..4]);
        let length = u32::from_be_bytes(length_bytes) as usize;

        // Check for heartbeat messages.
        if length == 0 {
            // Discard it.
            src.advance(4);

            // But also try again in case the src buffer has more messages.
            return self.decode(src);
        }

        if src.len() < 5 {
            // Not enough data to read length prefix and the message id.
            return Ok(None);
        }

        // Check that the length is not too large.
        if length > MAX_FRAME_SIZE - 4 {
            return Err(MessageCodecError::LengthError(length.to_string()));
        }

        if src.len() < 4 + length {
            // The full message has not yet arrived.
            //
            // We reserve more space in the buffer. This is not strictly
            // necessary, but is a good idea performance-wise.
            src.reserve(4 + length - src.len());

            // We inform the Framed that we need more bytes to form the next frame.
            return Ok(None);
        }

        let id = src[4].into();

        let payload = if length > 1 {
            Some(src[5..4 + length].to_vec())
        } else {
            None
        };

        // Use advance to modify src such that it no longer contains this frame.
        src.advance(4 + length);

        let msg = Message::new(id, payload);

        Ok(Some(msg))
    }
}

impl Encoder<Message> for MessageCodec {
    type Error = MessageCodecError;

    fn encode(&mut self, msg: Message, dst: &mut BytesMut) -> std::result::Result<(), Self::Error> {
        let length = msg.len as usize;

        // Don't send a message if it is longer than the other end will accept.
        if length > MAX_FRAME_SIZE - 4 {
            return Err(MessageCodecError::LengthError(length.to_string()));
        }

        // Convert the length into a byte array.
        // The cast to u32 cannot overflow due to the length check above.
        let len_slice = u32::to_be_bytes(length as u32);

        // Reserve space in the buffer.
        dst.reserve(4 + length);

        dst.extend_from_slice(&len_slice);
        dst.put_u8(msg.id as u8);
        if let Some(payload) = msg.payload {
            dst.extend_from_slice(&payload);
        }

        Ok(())
    }
}
