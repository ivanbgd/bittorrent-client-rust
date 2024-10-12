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
            _ => {
                panic!("Unhandled encoded value: {:?}", value);
            }
        }
    }

    let value = serde_bencode::from_str(encoded_value)?;

    decode(value)
}
