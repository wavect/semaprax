use crate::host::http::*;
use std::io::{Read, Write};
use std::net::TcpListener;

/// One-shot server: reads the request head, answers `reply`, returns the head.
fn serve(reply: Vec<u8>) -> (String, std::thread::JoinHandle<String>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let t = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut buf = vec![0u8; 8192];
        let n = s.read(&mut buf).unwrap();
        let _ = s.write_all(&reply);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    });
    (url, t)
}

fn client(url: &str, cred: Option<Credential>, max: usize) -> HttpClient {
    HttpClient::new(
        ApprovedEndpoint::from_host_config(url).unwrap(),
        cred,
        HttpLimits {
            max_response_bytes: max,
            connect_timeout_ms: 1000,
            total_timeout_ms: 3000,
        },
    )
}

#[test]
fn loopback_request_injects_the_credential_and_returns_the_body() {
    let (url, t) = serve(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\n{\"ok\":true}".to_vec());
    let cred = Credential::new("Authorization", "Bearer s3cr3t-token").unwrap();
    assert!(!format!("{cred:?}").contains("s3cr3t"));
    let r = client(&url, Some(cred), 1024)
        .request("POST", "/v1/x", Some(b"{}"))
        .unwrap();
    assert_eq!((r.status, r.body.as_slice()), (200, &b"{\"ok\":true}"[..]));
    let head = t.join().unwrap();
    assert!(
        head.starts_with("POST /v1/x HTTP/1.1\r\n")
            && head.contains("Authorization: Bearer s3cr3t-token\r\n")
    );
}

#[test]
fn chunked_bodies_decode_and_error_statuses_are_returned() {
    let (url, _t) = serve(
        b"HTTP/1.1 503 Busy\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n"
            .to_vec(),
    );
    let r = client(&url, None, 1024).request("GET", "/h", None).unwrap();
    assert_eq!((r.status, r.body.as_slice()), (503, &b"abcde"[..]));
}

#[test]
fn redirects_are_refused_not_followed() {
    let (url, _t) = serve(
        b"HTTP/1.1 302 Found\r\nLocation: http://evil.example/\r\nContent-Length: 0\r\n\r\n"
            .to_vec(),
    );
    assert_eq!(
        client(&url, None, 1024)
            .request("GET", "/", None)
            .unwrap_err()
            .code,
        "SPX-HPC032"
    );
}

#[test]
fn oversized_responses_are_refused() {
    let mut reply = b"HTTP/1.1 200 OK\r\nContent-Length: 5000\r\n\r\n".to_vec();
    reply.extend(vec![b'x'; 5000]);
    let (url, _t) = serve(reply);
    assert_eq!(
        client(&url, None, 100)
            .request("GET", "/", None)
            .unwrap_err()
            .code,
        "SPX-HPC033"
    );
    let mut reply = b"HTTP/1.1 200 OK\r\n\r\n".to_vec();
    reply.extend(vec![b'x'; 5000]);
    let (url, _t) = serve(reply);
    assert_eq!(
        client(&url, None, 100)
            .request("GET", "/", None)
            .unwrap_err()
            .code,
        "SPX-HPC033"
    );
}

#[test]
fn silent_server_times_out() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let c = HttpClient::new(
        ApprovedEndpoint::from_host_config(&url).unwrap(),
        None,
        HttpLimits {
            max_response_bytes: 10,
            connect_timeout_ms: 500,
            total_timeout_ms: 300,
        },
    );
    assert_eq!(c.request("GET", "/", None).unwrap_err().code, "SPX-HPC034");
}

#[test]
fn non_loopback_and_tls_endpoints_are_honestly_refused() {
    for url in [
        "http://example.com:80",
        "http://10.0.0.5:8080",
        "https://127.0.0.1:443",
        "https://example.com",
        "http://[2001:db8::1]:80",
        "http://0.0.0.0:80",
    ] {
        let e = ApprovedEndpoint::from_host_config(url).unwrap_err();
        assert_eq!(e.code, "SPX-HPC030", "{url}");
        assert!(e
            .message
            .contains("remote transport requires TLS, unsupported in v1"));
    }
    for url in [
        "ftp://127.0.0.1",
        "http://127.0.0.1:80/path",
        "http://u:p@127.0.0.1",
        "http://",
        "127.0.0.1:80",
    ] {
        assert_eq!(
            ApprovedEndpoint::from_host_config(url).unwrap_err().code,
            "SPX-HPC037",
            "{url}"
        );
    }
    assert!(ApprovedEndpoint::from_host_config("http://localhost:8080").is_ok());
    assert!(ApprovedEndpoint::from_host_config("http://[::1]:8080/").is_ok());
    assert!(Credential::new("Host", "x").is_err() && Credential::new("X-Key", "a\r\nb").is_err());
    let c = client("http://127.0.0.1:9", None, 10);
    assert_eq!(
        c.request("GET", "/a b", None).unwrap_err().code,
        "SPX-HPC037"
    );
    assert_eq!(
        c.request("GET", "http://evil/", None).unwrap_err().code,
        "SPX-HPC037"
    );
}

fn chunked(body: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{body}").into_bytes()
}

fn chunked_outcome(body: &str, max: usize) -> Result<Vec<u8>, &'static str> {
    let (url, _t) = serve(chunked(body));
    client(&url, None, max)
        .request("GET", "/c", None)
        .map(|r| r.body)
        .map_err(|e| e.code)
}

#[test]
fn chunk_sizes_that_overflow_are_refused_not_panics() {
    let huge = format!("{:x}", usize::MAX);
    let near = format!("{:x}", usize::MAX - 1);
    for size in [huge.as_str(), near.as_str(), "ffffffffffffffffffff"] {
        let first = chunked_outcome(&format!("1\r\nx\r\n{size}\r\nzz\r\n0\r\n\r\n"), 64);
        assert!(
            matches!(first, Err("SPX-HPC033") | Err("SPX-HPC036")),
            "{size}: {first:?}"
        );
        let alone = chunked_outcome(&format!("{size}\r\nzz\r\n"), 64);
        assert!(alone.is_err(), "{size}: {alone:?}");
    }
}

#[test]
fn malformed_chunk_framing_is_refused() {
    for body in [
        "1\r\nxZZ0\r\n",
        "1\r\nx\r\n0\r\n",
        "1\r\nx\r\n0",
        "1\r\nx",
        "5\r\nab",
        "1\r\nx\r\n0\r\nbad trailer\r\n\r\n",
        "1\r\nx\r\n0\r\nX-A: 1\r\n",
    ] {
        let r = chunked_outcome(body, 64);
        assert_eq!(r, Err("SPX-HPC036"), "{body:?}");
    }
    let oversized_trailer = format!("1\r\nx\r\n0\r\nX-A: {}\r\n\r\n", "a".repeat(20_000));
    assert!(chunked_outcome(&oversized_trailer, 64).is_err());
}

#[test]
fn valid_chunked_bodies_extensions_trailers_and_exact_cap_still_decode() {
    assert_eq!(
        chunked_outcome("3;ext=1\r\nabc\r\n2\r\nde\r\n0\r\nX-Sum: 1\r\n\r\n", 5),
        Ok(b"abcde".to_vec())
    );
    assert_eq!(chunked_outcome("0\r\n\r\n", 5), Ok(Vec::new()));
    assert_eq!(
        chunked_outcome("6\r\nabcdef\r\n0\r\n\r\n", 5),
        Err("SPX-HPC033")
    );
}
