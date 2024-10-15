use anyhow::Result;
use sha1::{Digest, Sha1};

pub fn calc_sha1(info: &serde_json::Value) -> Result<String> {
    let bencoded = bencode_encode(info)?;

    let result = hex::encode(Sha1::digest(bencoded));

    Ok(result)
}

fn bencode_encode(info: &serde_json::Value) -> Result<Vec<u8>> {
    let serialized = serde_bencode::to_bytes(&info)?;

    Ok(serialized)
}
