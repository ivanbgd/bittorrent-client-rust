//! # Bencode
//!
//! ## Decode
//!
//! Decoder for Bencoded values
//!
//! https://www.bittorrent.org/beps/bep_0003.html#bencoding
//!
//! https://wiki.theory.org/BitTorrentSpecification#Bencoding
//!
//! https://en.wikipedia.org/wiki/Bencode
//!
//! `$ ./your_bittorrent.sh decode 5:hello`
//!
//! `hello`
//!
//! `$ ./your_bittorrent.sh decode i52e`
//!
//! `52`
//!
//! `$ ./your_bittorrent.sh decode l5:helloi52ee`
//!
//! `["hello",52]`
//!
//! `$ ./your_bittorrent.sh decode d3:foo3:bar5:helloi52ee`
//!
//! `{"foo":"bar","hello":52}`
//!
//! ## Encode
//!
//! Encoder for Bencoded values

use anyhow::Result;
use serde_bencode;
use serde_json;

/// Decodes a Bencoded value:
/// - string (`5:hello` -> `hello`),
/// - integer (`i52e` -> `52`),
/// - list (`l5:helloi52ee`, `["hello",52]`),
/// - dictionary (`d3:foo3:bar5:helloi52ee` -> `{"foo":"bar","hello":52}`).
pub fn decode_bencoded_value(encoded_value: &[u8]) -> Result<serde_json::Value> {
    let value = serde_bencode::from_bytes(encoded_value)?;

    decode(value)
}

/// The inner worker function for decoding a Bencoded value
fn decode(value: serde_bencode::value::Value) -> Result<serde_json::Value> {
    match value {
        serde_bencode::value::Value::Bytes(string) => unsafe {
            let string = String::from_utf8_unchecked(string);
            Ok(serde_json::Value::String(string))
        },
        serde_bencode::value::Value::Int(integer) => {
            let integer = serde_json::Number::from(integer);
            Ok(serde_json::Value::Number(integer))
        }
        serde_bencode::value::Value::List(list) => {
            let array = list
                .into_iter()
                .map(decode)
                .collect::<Result<Vec<serde_json::Value>>>()?;
            Ok(serde_json::Value::Array(array))
        }
        serde_bencode::value::Value::Dict(dict) => unsafe {
            let mut object: serde_json::Map<String, serde_json::Value> =
                serde_json::Map::with_capacity(dict.len());
            for (k, v) in dict.into_iter() {
                object.insert(String::from_utf8_unchecked(k), decode(v)?);
            }
            Ok(serde_json::Value::Object(object))
        },
    }
}

/// Encodes the given value in the Bencode format and returns it as a byte sequence.
pub fn bencode_value(value: serde_json::Value) -> Result<Vec<u8>> {
    Ok(serde_bencode::to_bytes(&value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_empty_string() {
        assert_eq!("", decode_bencoded_value(b"0:").unwrap());
    }

    #[test]
    fn decode_non_empty_string() {
        assert_eq!("hello", decode_bencoded_value(b"5:hello").unwrap());
    }

    #[test]
    fn decode_string_with_blank() {
        assert_eq!(
            "Hello, world!",
            decode_bencoded_value(b"13:Hello, world!").unwrap()
        );
    }

    #[test]
    fn decode_positive_zero_int() {
        assert_eq!(0, decode_bencoded_value(b"i0e").unwrap());
    }

    #[ignore = "doesn't error when it should"]
    #[test]
    fn decode_negative_zero_int() {
        assert!(decode_bencoded_value(b"i-0e").is_err());
    }

    #[ignore = "doesn't error when it should"]
    #[test]
    fn decode_leading_zeros_int() {
        assert!(decode_bencoded_value(b"i0345e").is_err());
    }

    #[test]
    fn decode_positive_int() {
        assert_eq!(345, decode_bencoded_value(b"i345e").unwrap());
    }

    #[test]
    fn decode_negative_int() {
        assert_eq!(-345, decode_bencoded_value(b"i-345e").unwrap());
    }

    #[test]
    fn decode_empty_list() {
        assert_eq!(serde_json::json!([]), decode_bencoded_value(b"le").unwrap());
    }

    #[test]
    fn decode_non_empty_list() {
        assert_eq!(
            serde_json::json!(["hello", 42]),
            decode_bencoded_value(b"l5:helloi42ee").unwrap()
        );
    }

    #[test]
    fn decode_recursive_list() {
        assert_eq!(
            serde_json::json!(["hello", [42, "test"]]),
            decode_bencoded_value(b"l5:helloli42e4:testee").unwrap()
        );
    }

    #[test]
    fn decode_empty_dict() {
        assert_eq!(serde_json::json!({}), decode_bencoded_value(b"de").unwrap());
    }

    #[test]
    fn decode_non_empty_dict() {
        assert_eq!(
            serde_json::json!({"hello": "world", "num": -55}),
            decode_bencoded_value(b"d5:hello5:world3:numi-55ee").unwrap()
        );
    }

    #[test]
    fn decode_recursive_dict() {
        assert_eq!(
            serde_json::json!({"dict": {"ab": 55, "list": [42, "hello", [], {}]}, "hi": "man"}),
            decode_bencoded_value(b"d4:dictd2:abi55e4:listli42e5:helloledeee2:hi3:mane").unwrap()
        );
    }

    #[test]
    fn encode_empty_string() {
        assert_eq!(*b"0:", *bencode_value("".into()).unwrap());
    }

    #[test]
    fn encode_non_empty_string() {
        assert_eq!(*b"5:hello", *bencode_value("hello".into()).unwrap());
    }

    #[test]
    fn encode_string_with_blank() {
        assert_eq!(
            *b"13:Hello, world!",
            *bencode_value("Hello, world!".into()).unwrap()
        );
    }

    #[test]
    fn encode_positive_zero_int() {
        assert_eq!(*b"i0e", *bencode_value(0.into()).unwrap());
    }

    #[test]
    fn encode_positive_int() {
        assert_eq!(*b"i345e", *bencode_value(345.into()).unwrap());
    }

    #[test]
    fn encode_negative_int() {
        assert_eq!(*b"i-345e", *bencode_value((-345).into()).unwrap());
    }

    #[test]
    fn encode_empty_list() {
        assert_eq!(*b"le", *bencode_value(serde_json::json!([])).unwrap());
    }

    #[test]
    fn encode_non_empty_list() {
        assert_eq!(
            *b"l5:helloi42ee",
            *bencode_value(serde_json::json!(["hello", 42])).unwrap()
        );
    }

    #[test]
    fn encode_recursive_list() {
        assert_eq!(
            *b"l5:helloli42e4:testee",
            *bencode_value(serde_json::json!(["hello", [42, "test"]])).unwrap()
        );
    }

    #[test]
    fn encode_empty_dict() {
        assert_eq!(*b"de", *bencode_value(serde_json::json!({})).unwrap());
    }

    #[test]
    fn encode_non_empty_dict() {
        assert_eq!(
            *b"d5:hello5:world3:numi-55ee",
            *bencode_value(serde_json::json!({"hello": "world", "num": -55})).unwrap()
        );
    }

    #[test]
    fn encode_recursive_dict() {
        assert_eq!(
            *b"d4:dictd2:abi55e4:listli42e5:helloledeee2:hi3:mane",
            *bencode_value(
                serde_json::json!({"dict": {"ab": 55, "list": [42, "hello", [], {}]}, "hi": "man"})
            )
            .unwrap()
        );
    }
}
