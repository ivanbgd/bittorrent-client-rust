use anyhow::Result;
use sha1::{Digest, Sha1};

fn bencode_encode(info: &serde_json::Value) -> Result<Vec<u8>> {
    let serialized = serde_bencode::to_bytes(&info)?;

    Ok(serialized)
}

pub fn calc_sha1(info: &serde_json::Value) -> Result<String> {
    let bencoded = bencode_encode(info)?;

    // test
    use crate::decode::decode_bencoded_value;
    let decoded = decode_bencoded_value(&bencoded)?;
    // println!("{}\n{}", info, decoded);
    assert_eq!(info, &decoded); // this passes, so dict is sorted
                                // test

    // let mut hasher = Sha1::new();
    // hasher.update(bencoded);
    // let hash = hasher.finalize();
    // let result = hex::encode(hash);

    let result = hex::encode(Sha1::digest(bencoded));

    Ok(result)
}
