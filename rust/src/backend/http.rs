//! Thin synchronous HTTP helpers shared by the mod catalog and the campaign installers: a
//! GET request that returns either the raw response body or its body parsed as JSON.
//! Blocking/synchronous on purpose - this is a desktop app with no other use for an async
//! runtime, so pulling one in just for a handful of downloads would be the opposite of
//! simple (see `docs/adr/0005-boring-rust.md`).

use std::io::Read;
use std::time::Duration;

pub fn get_bytes(url: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    let resp =
        ureq::get(url).set("User-Agent", "f2launcher").timeout(timeout).call().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    resp.into_reader().read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

pub fn get_json(url: &str, timeout: Duration) -> Result<serde_json::Value, String> {
    ureq::get(url)
        .set("User-Agent", "f2launcher")
        .timeout(timeout)
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())
}
