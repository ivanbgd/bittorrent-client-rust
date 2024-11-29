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

use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::bencode::decode_bencoded_value;
use crate::constants::{MAX_FRAME_SIZE, UT_METADATA};
use crate::errors::{MessageCodecError, MessageError, MessageIdError, PiecePayloadError};
use anyhow::{Context, Result};
use bytes::{Buf, BufMut, BytesMut};
use serde_derive::Serialize;
use tokio_util::codec::{Decoder, Encoder};

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
    ///
    /// Note that the `piece` messages are correlated with `request` messages implicitly.
    /// It's possible for an unexpected `piece` to arrive if `choke` and `unchoke` messages
    /// are sent in quick succession and/or transfer is going very slowly.
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

    /// Extended messages follow the standard BitTorrent message format:
    /// - message length prefix (4 bytes),
    /// - message id (1 byte) - this will be `20` for all messages implemented by extensions,
    /// - payload (variable size).
    ///
    /// The Extended message payload is structured as follows:
    /// - Extended message id (1 byte):
    ///     - This will be 0 for the extended handshake.
    /// - Bencoded dictionary (variable size):
    ///     - This will contain a key "m" with another dictionary as its value.
    ///     - The inner dictionary maps supported extension names to their corresponding message IDs.
    Extended = 20,

    /// Unsupported message ID
    Unsupported,
}

/// In case we'd like to print [`MessageId`] as raw byte, i.e., as [`u8`].
///
/// Use [`Debug`] for human-readable output, which we derived for this enum.
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

impl TryFrom<u8> for MessageId {
    type Error = MessageIdError;

    fn try_from(value: u8) -> Result<MessageId, MessageIdError> {
        match value {
            0 => Ok(MessageId::Choke),
            1 => Ok(MessageId::Unchoke),
            2 => Ok(MessageId::Interested),
            3 => Ok(MessageId::NotInterested),
            4 => Ok(MessageId::Have),
            5 => Ok(MessageId::Bitfield),
            6 => Ok(MessageId::Request),
            7 => Ok(MessageId::Piece),
            8 => Ok(MessageId::Cancel),
            9 => Ok(MessageId::Port),
            20 => Ok(MessageId::Extended),
            v => Err(MessageIdError::UnsupportedId(v)),
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
        write!(f, "Message {{ len: {}, id: {:?}", self.len, self.id)?;
        if self.id == MessageId::Extended {
            let payload = self
                .payload
                .as_ref()
                .expect("Payload should exist for extended messages.")
                .to_owned();
            let payload: ExtendedMessagePayload = payload
                .try_into()
                .expect("Failed to convert raw payload, Vec<u8>, to ExtendedMessagePayload.");
            write!(f, ", payload: {} }}", payload)
        } else {
            write!(f, ", payload: {:02X?} }}", &self.payload)
        }
    }
}

/// Converts a [`Message`] into a byte stream.
impl From<Message> for Vec<u8> {
    /// Serializes a [`Message`] for a send transfer over the wire.
    fn from(value: Message) -> Vec<u8> {
        let len = u32::to_be_bytes(value.len);
        let id = value.id.into();
        let payload = value.payload.unwrap_or_default();
        let payload_len = payload.len();

        let mut buf = Vec::with_capacity(4 + 1 + payload_len);

        buf.extend(len);
        buf.push(id);
        buf.extend(payload);

        buf
    }
}

/// Converts a byte stream into a [`Message`].
impl TryFrom<Vec<u8>> for Message {
    type Error = MessageError;

    /// Deserializes a byte stream received from a wire transfer into [`Message`].
    fn try_from(value: Vec<u8>) -> Result<Message, MessageError> {
        let len = u32::from_be_bytes(<[u8; 4]>::try_from(&value[0..4]).with_context(|| {
            format!(
                "Failed to deserialize message length; received: {:?}",
                value
            )
        })?);
        let id = value[4].try_into()?; // Same as: let id = MessageId::try_from(value[4]);
        let payload = if len == 1 {
            None
        } else {
            Some(value[5..4 + len as usize].to_vec())
        };

        Ok(Self { len, id, payload })
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

/// Unused
impl Display for RequestPayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "-> piece_i = {:3}, begin = {:6}, length = {:5}",
            self.index, self.begin, self.length
        )
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

/// Payload for the [`MessageId::Piece`] message
///
/// The payload contains the following information:
///   - index: integer specifying the zero-based piece index
///   - begin: integer specifying the zero-based byte offset within the piece
///   - block: block of data, which is a subset of the piece specified by index
#[derive(Debug)]
pub struct PiecePayload<'a> {
    pub index: u32,
    pub begin: u32,
    pub block: &'a [u8],
}

/// Converts a reference to a [`MessageId::Piece`] into a [`PiecePayload`].
impl<'a> TryFrom<&'a Message> for PiecePayload<'a> {
    type Error = PiecePayloadError;

    /// Converts a reference to a [`MessageId::Piece`] into a [`PiecePayload`].
    ///
    /// Validates the payload length against the message length.
    ///
    /// Uses `PiecePayload::try_from(value: &'a [u8])`.
    ///
    /// # Errors
    /// If message length is different from payload length increased by one,
    /// returns [`PiecePayloadError::WrongLen`].
    fn try_from(value: &'a Message) -> Result<PiecePayload, PiecePayloadError> {
        let payload: &[u8] = value
            .payload
            .as_ref()
            .context("Expected to have received some payload")?;

        let msg_len = value.len as usize;
        if msg_len != 1 + payload.len() {
            return Err(PiecePayloadError::WrongLen(msg_len, 1 + payload.len()));
        }

        payload.try_into()
    }
}

/// Converts a byte stream into a [`PiecePayload`].
impl<'a> TryFrom<&'a [u8]> for PiecePayload<'a> {
    type Error = PiecePayloadError;

    /// Deserializes a [`PiecePayload`] received from a wire transfer.
    ///
    /// This function is not aware of the requested length of the block of data,
    /// hence it can't check whether it has received the entire requested block.
    fn try_from(value: &'a [u8]) -> Result<PiecePayload, PiecePayloadError> {
        let index = u32::from_be_bytes(value[0..4].try_into().context("failed to convert index")?);
        let begin = u32::from_be_bytes(value[4..8].try_into().context("failed to convert begin")?);
        let block = &value[8..];

        Ok(Self {
            index,
            begin,
            block,
        })
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

        let id = src[4].try_into()?;

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

//////////////////////////////////////////////////////////////////////////////

/*                     Extended Message Support                             */

//////////////////////////////////////////////////////////////////////////////

/// See: https://www.bittorrent.org/beps/bep_0010.html
#[derive(Debug, PartialEq, Serialize)]
pub enum ExtendedMessageId {
    /// Extension handshake message
    Handshake = 0,

    /// Unsupported message ID
    Unsupported,
}

/// In case we'd like to print [`ExtendedMessageId`] as raw byte, i.e., as [`u8`].
///
/// Use [`Debug`] for human-readable output, which we derived for this enum.
impl Display for ExtendedMessageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self as *const Self as u8)
    }
}

impl From<ExtendedMessageId> for u8 {
    fn from(value: ExtendedMessageId) -> u8 {
        value as u8
    }
}

impl TryFrom<u8> for ExtendedMessageId {
    type Error = MessageIdError;

    fn try_from(value: u8) -> Result<ExtendedMessageId, MessageIdError> {
        match value {
            0 => Ok(ExtendedMessageId::Handshake),
            v => Err(MessageIdError::UnsupportedId(v)),
        }
    }
}

/// The Extended message payload is structured as follows:
/// - Extended message id (1 byte):
///     - This will be 0 for the extended handshake.
/// - Bencoded dictionary (variable size):
///     - This will contain a key "m" with another dictionary as its value.
///     - The inner dictionary maps supported extension names to their corresponding message IDs.
///
/// See: https://www.bittorrent.org/beps/bep_0010.html
#[derive(Debug, Serialize)]
pub struct ExtendedMessagePayload {
    pub id: ExtendedMessageId,
    pub dict: Vec<u8>,
}

impl Display for ExtendedMessagePayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ id: {:?}, dict: {:?} }}",
            self.id,
            String::from_utf8_lossy(&self.dict)
        )
    }
}

impl ExtendedMessagePayload {
    pub(crate) fn new(
        id: ExtendedMessageId,
        // dict: serde_json::Value,todo rem
        dict: ExtendedMessageHandshakeDict,
    ) -> Result<Self, anyhow::Error> {
        // let dict = bencode_value(dict)?;todo rem
        let dict = serde_bencode::to_bytes(&dict)?;

        Ok(Self { id, dict })
    }
}

/// Converts a [`ExtendedMessagePayload`] into a byte stream.
impl From<ExtendedMessagePayload> for Vec<u8> {
    /// Serializes a [`ExtendedMessagePayload`] for a send transfer over the wire.
    fn from(value: ExtendedMessagePayload) -> Self {
        let id = value.id.into();
        let dict = value.dict;

        let mut buf = Vec::with_capacity(1 + dict.len());

        buf.push(id);
        buf.extend(dict);

        buf
    }
}

/// Converts a byte stream into a [`ExtendedMessagePayload`].
impl TryFrom<Vec<u8>> for ExtendedMessagePayload {
    type Error = MessageError;

    /// Deserializes a byte stream received from a wire transfer into [`ExtendedMessagePayload`].
    fn try_from(value: Vec<u8>) -> Result<ExtendedMessagePayload, MessageError> {
        // let val = String::from_utf8_lossy(&value).into_owned(); //.unwrap(); // todo rem
        // eprintln!("val = {val:?}");

        let id = value[0].try_into()?;
        let dict = value[1..].to_vec();

        Ok(ExtendedMessagePayload { id, dict })
    }
}

/// See: https://www.bittorrent.org/beps/bep_0010.html
///
/// For the bencoding part see: https://www.bittorrent.org/beps/bep_0003.html
///
/// Keys must be strings and appear in sorted order (sorted as raw strings, not alphanumerics).
#[derive(Debug, Serialize)]
pub struct ExtendedMessageHandshakeDict {
    pub(crate) m: Option<HashMap<String, u8>>,
    p: Option<u16>,
    v: Option<String>,
}

impl Display for ExtendedMessageHandshakeDict {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ExtendedMessageHandshakeDict {{ m: {:?}, p: {:?}, v: {:?} }}",
            self.m, self.p, self.v
        )
    }
}

impl ExtendedMessageHandshakeDict {
    /// Takes fields `m`, `p` and `v` and stores them in the dictionary.
    ///
    /// See: https://www.bittorrent.org/beps/bep_0010.html
    pub(crate) fn new(m: Option<HashMap<String, u8>>, p: Option<u16>, v: Option<String>) -> Self {
        Self { m, p, v }
    }
}

// // TODO: I don't think this is correct. We should bencode a ExtendedMessageHandshakeDict instead.
// /// Converts a [`ExtendedMessageHandshakeDict`] into a byte stream.
// impl TryFrom<ExtendedMessageHandshakeDict<'_>> for Vec<u8> {
//     type Error = MessageError;
//
//     /// Serializes a [`ExtendedMessageHandshakeDict`] for a send transfer over the wire.
//     fn try_from(value: ExtendedMessageHandshakeDict) -> Result<Self, Self::Error> {
//         // let mut res: Vec<u8> = vec![];
//         //
//         // if let Some(m) = value.m {
//         //     res.push(b'm');
//         //     res.extend_from_slice(&serde_json::to_vec(&m)?);
//         // }
//         //
//         // if let Some(p) = value.p {
//         //     res.push(b'p');
//         //     res.extend_from_slice(&serde_json::to_vec(&p)?);
//         // }
//         //
//         // if let Some(v) = value.v {
//         //     res.push(b'v');
//         //     res.extend_from_slice(&serde_json::to_vec(&v)?);
//         // }
//         //
//         // Ok(serde_bencode::to_bytes(&res)?)
//
//         Ok(serde_bencode::to_bytes(&value)?)
//     }
// }

/// Converts a byte stream into a [`ExtendedMessageHandshakeDict`].
impl From<Vec<u8>> for ExtendedMessageHandshakeDict {
    /// Deserializes a byte stream received from a wire transfer into [`ExtendedMessageHandshakeDict`].
    fn from(value: Vec<u8>) -> Self {
        // todo!() // todo rem
        eprintln!("value = {value:?}"); // todo rem
        let val = String::from_utf8_lossy(&value).into_owned(); //.unwrap(); // todo rem
        eprintln!("val = {val:?}"); // todo rem

        let val = decode_bencoded_value(&value).unwrap();
        eprintln!("val = {val:?}"); // todo rem

        let m: Option<HashMap<String, u8>>; // todo rem
        let m = Some(HashMap::from([("aaa".to_string(), 33)])); // TODO!

        let map = val.get("m").map(|m| m.as_object().unwrap());
        let mut m: HashMap<String, u8> = HashMap::new();
        // TODO: Insert ALL fields!
        m.insert(
            UT_METADATA.to_string(),
            map.unwrap_or_else(|| panic!("Expected field \"{UT_METADATA}\"."))
                .get(UT_METADATA)
                .unwrap()
                .as_u64()
                .unwrap() as u8,
        );
        let m = Some(m);
        let p = val.get("p").map(|p| p.as_u64().unwrap() as u16);
        let v = val.get("v").map(|v| v.as_str().unwrap().to_string());
        eprintln!("* m = {m:?}"); // todo rem

        Self { m, p, v }
    }
}

// todo: needed?
/// See: https://www.bittorrent.org/beps/bep_0009.html
#[derive(Debug, PartialEq)]
pub enum ExtensionMessageId {
    Request = 0,
    Data = 1,
    Reject = 2,

    /// Unsupported message ID
    Unsupported,
}
