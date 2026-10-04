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
