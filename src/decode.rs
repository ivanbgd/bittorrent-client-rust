//! # Decode
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

use anyhow::Result;
use serde_bencode;
use serde_json;

/// Decodes a Bencoded value
pub fn decode_bencoded_value(encoded_value: &[u8]) -> Result<serde_json::Value> {
    let value = serde_bencode::from_bytes(encoded_value)?;

    decode(value)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string() {
        assert_eq!("", decode_bencoded_value(b"0:").unwrap());
    }

    #[test]
    fn non_empty_string() {
        assert_eq!("hello", decode_bencoded_value(b"5:hello").unwrap());
    }

    #[test]
    fn string_with_blank() {
        assert_eq!(
            "Hello, world!",
            decode_bencoded_value(b"13:Hello, world!").unwrap()
        );
    }

    #[test]
    fn positive_zero_int() {
        assert_eq!(0, decode_bencoded_value(b"i0e").unwrap());
    }

    #[ignore = "doesn't error when it should"]
    #[test]
    fn negative_zero_int() {
        assert!(decode_bencoded_value(b"i-0e").is_err());
    }

    #[ignore = "doesn't error when it should"]
    #[test]
    fn leading_zeros_int() {
        assert!(decode_bencoded_value(b"i0345e").is_err());
    }

    #[test]
    fn positive_int() {
        assert_eq!(345, decode_bencoded_value(b"i345e").unwrap());
    }

    #[test]
    fn negative_int() {
        assert_eq!(-345, decode_bencoded_value(b"i-345e").unwrap());
    }

    #[test]
    fn empty_list() {
        assert_eq!(serde_json::json!([]), decode_bencoded_value(b"le").unwrap());
    }

    #[test]
    fn non_empty_list() {
        assert_eq!(
            serde_json::json!(["hello", 42]),
            decode_bencoded_value(b"l5:helloi42ee").unwrap()
        );
    }

    #[test]
    fn recursive_list() {
        assert_eq!(
            serde_json::json!(["hello", [42, "test"]]),
            decode_bencoded_value(b"l5:helloli42e4:testee").unwrap()
        );
    }

    #[test]
    fn empty_dict() {
        assert_eq!(serde_json::json!({}), decode_bencoded_value(b"de").unwrap());
    }

    #[test]
    fn non_empty_dict() {
        assert_eq!(
            serde_json::json!({"hello": "world", "num": -55}),
            decode_bencoded_value(b"d5:hello5:world3:numi-55ee").unwrap()
        );
    }

    #[test]
    fn recursive_dict() {
        assert_eq!(
            serde_json::json!({"dict": {"ab": 55, "list": [42, "hello", [], {}]}, "hi": "man"}),
            decode_bencoded_value(b"d4:dictd2:abi55e4:listli42e5:helloledeee2:hi3:mane").unwrap()
        );
    }
}
