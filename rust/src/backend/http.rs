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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    /// Starts a one-shot HTTP server on loopback: accepts exactly one connection, reads (and
    /// discards) the request, writes `raw_response` verbatim, then closes. Returns the URL to
    /// hit it at. No real network access involved - this never leaves 127.0.0.1.
    fn one_shot_server(raw_response: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf); // drain the request so the client isn't blocked
                let _ = stream.write_all(raw_response);
                let _ = stream.flush();
            }
        });
        format!("http://127.0.0.1:{port}/")
    }

    #[test]
    fn get_bytes_returns_the_response_body() {
        let url = one_shot_server(
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
        );
        let bytes = get_bytes(&url, Duration::from_secs(5)).unwrap();
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn get_bytes_errors_on_a_4xx_status_instead_of_returning_the_body() {
        let url = one_shot_server(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found",
        );
        assert!(get_bytes(&url, Duration::from_secs(5)).is_err());
    }

    #[test]
    fn get_bytes_errors_cleanly_when_nothing_is_listening() {
        // Bind to grab a free port, then drop the listener immediately so the port is
        // guaranteed to have nothing behind it - the connection attempt should fail fast
        // with a clean Err, not hang or panic.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let url = format!("http://127.0.0.1:{port}/");
        assert!(get_bytes(&url, Duration::from_secs(5)).is_err());
    }

    #[test]
    fn get_json_parses_a_well_formed_json_body() {
        let body = br#"{"tag_name":"v1.2.3"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let mut raw = response.into_bytes();
        raw.extend_from_slice(body);
        // Leak it to get a 'static slice the spawned thread can hold onto - fine for a test.
        let url = one_shot_server(Box::leak(raw.into_boxed_slice()));
        let value = get_json(&url, Duration::from_secs(5)).unwrap();
        assert_eq!(value.get("tag_name").and_then(|v| v.as_str()), Some("v1.2.3"));
    }

    #[test]
    fn get_json_errors_on_a_body_that_is_not_valid_json() {
        let url = one_shot_server(
            b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nnot json!!!",
        );
        assert!(get_json(&url, Duration::from_secs(5)).is_err());
    }
}
