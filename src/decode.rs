use anyhow::Result;
use serde_bencode;
use serde_json;

pub fn decode_bencoded_value(encoded_value: &str) -> Result<serde_json::Value> {
    let first = encoded_value.chars().next().unwrap();
    if first.is_ascii_digit() {
        // if encoded_value starts with a digit, it's a string
        Ok(serde_json::Value::String(serde_bencode::from_str(
            encoded_value,
        )?))
    } else if first == 'i' {
        // if encoded_value starts with the letter 'i', it's a number
        Ok(serde_json::Value::Number(serde_bencode::from_str(
            encoded_value,
        )?))
    } else {
        panic!("Unhandled encoded value: {}", encoded_value);
    }
}
