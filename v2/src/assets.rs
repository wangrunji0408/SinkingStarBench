// Generated sorted directory of independently compressed frames; no cwd or sidecar lookup.
include!(concat!(env!("OUT_DIR"), "/assets-index.rs"));
static PACK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/assets.pack"));
pub fn read(key: &str) -> Vec<u8> {
    let i = INDEX
        .binary_search_by_key(&key, |e| e.0)
        .unwrap_or_else(|_| panic!("unknown replay asset {key}"));
    let (_, start, len, raw) = INDEX[i];
    let bytes =
        zstd::bulk::decompress(&PACK[start..start + len], raw).expect("invalid embedded data");
    assert_eq!(bytes.len(), raw);
    bytes
}
