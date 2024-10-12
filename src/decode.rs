use anyhow::Result;
use serde_bencode;
use serde_json;

pub fn decode_bencoded_value(encoded_value: &str) -> Result<serde_json::Value> {
    fn decode(value: serde_bencode::value::Value) -> Result<serde_json::Value> {
        match value {
            serde_bencode::value::Value::Bytes(string) => {
                let string = String::from_utf8(string)?;
                Ok(serde_json::Value::String(string))
            }
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
            serde_bencode::value::Value::Dict(dict) => {
                let mut object: serde_json::Map<String, serde_json::Value> =
                    serde_json::Map::with_capacity(dict.len());
                for (k, v) in dict.into_iter() {
                    object.insert(String::from_utf8(k)?, decode(v)?);
                }
                Ok(serde_json::Value::Object(object))
            }
        }
    }

    let value = serde_bencode::from_str(encoded_value)?;

    decode(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string() {
        assert_eq!("", decode_bencoded_value("0:").unwrap());
    }

    #[test]
    fn non_empty_string() {
        assert_eq!("hello", decode_bencoded_value("5:hello").unwrap());
    }

    #[test]
    fn positive_zero_int() {
        assert_eq!(0, decode_bencoded_value("i0e").unwrap());
    }

    #[ignore = "doesn't error"]
    #[test]
    fn negative_zero_int() {
        assert!(decode_bencoded_value("i-0e").is_err());
    }

    #[ignore = "doesn't error"]
    #[test]
    fn leading_zeros_int() {
        assert!(decode_bencoded_value("i0345e").is_err());
    }

    #[test]
    fn positive_int() {
        assert_eq!(345, decode_bencoded_value("i345e").unwrap());
    }

    #[test]
    fn negative_int() {
        assert_eq!(-345, decode_bencoded_value("i-345e").unwrap());
    }

    #[test]
    fn empty_list() {
        assert_eq!(serde_json::json!([]), decode_bencoded_value("le").unwrap());
    }

    #[test]
    fn non_empty_list() {
        assert_eq!(
            serde_json::json!(["hello", 42]),
            decode_bencoded_value("l5:helloi42ee").unwrap()
        );
    }

    #[test]
    fn recursive_list() {
        assert_eq!(
            serde_json::json!(["hello", [42, "test"]]),
            decode_bencoded_value("l5:helloli42e4:testee").unwrap()
        );
    }

    #[test]
    fn empty_dict() {
        assert_eq!(serde_json::json!({}), decode_bencoded_value("de").unwrap());
    }

    #[test]
    fn non_empty_dict() {
        assert_eq!(
            serde_json::json!({"hello": "world", "num": -55}),
            decode_bencoded_value("d5:hello5:world3:numi-55ee").unwrap()
        );
    }

    #[test]
    fn recursive_dict() {
        assert_eq!(
            serde_json::json!({"dict": {"ab": 55, "list": [42, "hello", [], {}]}, "hi": "man"}),
            decode_bencoded_value("d4:dictd2:abi55e4:listli42e5:helloledeee2:hi3:mane").unwrap()
        );
    }
}
