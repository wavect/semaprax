//! Loopback fixture HTTP server and a request-counting forwarding proxy
//! (std only). Shared by the fixture and provisioned endpoint tests.
#![allow(dead_code)]

use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Req {
    pub method: String,
    pub path: String,
    pub auth: Option<String>,
    pub body: Value,
}

pub enum Resp {
    Json(u16, Value),
    /// SSE events written one by one with a pause; stops when the peer closes.
    Sse(Vec<String>, Duration),
}

pub struct Server {
    pub port: u16,
    pub requests: Arc<AtomicUsize>,
    /// Set when a streaming write failed because the client closed.
    pub client_closed: Arc<AtomicBool>,
    pub log: Arc<Mutex<Vec<String>>>,
}

impl Server {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

fn read_request(s: &mut TcpStream) -> Option<(String, String, Option<String>, Vec<u8>)> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let end = loop {
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        let n = s.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        data.extend_from_slice(&buf[..n]);
    };
    let head = String::from_utf8_lossy(&data[..end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let (method, path) = (first.next()?.to_string(), first.next()?.to_string());
    let (mut len, mut auth) = (0usize, None);
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "content-length" => len = v.trim().parse().unwrap_or(0),
                "authorization" => auth = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    let mut body = data[end + 4..].to_vec();
    while body.len() < len {
        let n = s.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
    }
    Some((method, path, auth, body))
}

pub fn serve(handler: impl Fn(&Req) -> Resp + Send + Sync + 'static) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = Server {
        port,
        requests: Arc::default(),
        client_closed: Arc::default(),
        log: Arc::default(),
    };
    let (requests, closed, log) = (
        server.requests.clone(),
        server.client_closed.clone(),
        server.log.clone(),
    );
    let handler = Arc::new(handler);
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut s) = conn else { continue };
            let (handler, requests, closed, log) = (
                handler.clone(),
                requests.clone(),
                closed.clone(),
                log.clone(),
            );
            std::thread::spawn(move || {
                let Some((method, path, auth, body)) = read_request(&mut s) else {
                    return;
                };
                requests.fetch_add(1, Ordering::SeqCst);
                log.lock().unwrap().push(format!("{method} {path}"));
                let req = Req {
                    method,
                    path,
                    auth,
                    body: serde_json::from_slice(&body).unwrap_or(Value::Null),
                };
                match handler(&req) {
                    Resp::Json(status, v) => {
                        let b = v.to_string();
                        let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len());
                    }
                    Resp::Sse(events, pause) => {
                        let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n");
                        for e in events {
                            if s.write_all(format!("{e}\n\n").as_bytes())
                                .and_then(|_| s.flush())
                                .is_err()
                            {
                                closed.store(true, Ordering::SeqCst);
                                return;
                            }
                            std::thread::sleep(pause);
                        }
                        // A write after the peer closed can still succeed once; probe again.
                        std::thread::sleep(Duration::from_millis(50));
                        if s.write_all(b"\n").is_err() {
                            closed.store(true, Ordering::SeqCst);
                        }
                    }
                }
            });
        }
    });
    server
}

/// Forwarding proxy that counts parsed requests (one upstream connection per
/// request, `Connection: close` upstream) and records `METHOD path`.
pub fn counting_proxy(listen_port: u16, upstream_port: u16) -> Server {
    let listener = TcpListener::bind(("127.0.0.1", listen_port)).expect("bind counting proxy");
    let port = listener.local_addr().unwrap().port();
    let server = Server {
        port,
        requests: Arc::default(),
        client_closed: Arc::default(),
        log: Arc::default(),
    };
    let (requests, closed, log) = (
        server.requests.clone(),
        server.client_closed.clone(),
        server.log.clone(),
    );
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut client) = conn else { continue };
            let (requests, closed, log) = (requests.clone(), closed.clone(), log.clone());
            std::thread::spawn(move || {
                let Some((method, path, auth, body)) = read_request(&mut client) else {
                    return;
                };
                requests.fetch_add(1, Ordering::SeqCst);
                log.lock().unwrap().push(format!("{method} {path}"));
                let Ok(mut up) = TcpStream::connect(("127.0.0.1", upstream_port)) else {
                    return;
                };
                let mut head = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{upstream_port}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n", body.len());
                if let Some(a) = auth {
                    head.push_str(&format!("Authorization: {a}\r\n"));
                }
                head.push_str("\r\n");
                if up
                    .write_all(head.as_bytes())
                    .and_then(|_| up.write_all(&body))
                    .is_err()
                {
                    return;
                }
                let mut buf = [0u8; 4096];
                loop {
                    match up.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if client.write_all(&buf[..n]).is_err() {
                                closed.store(true, Ordering::SeqCst);
                                break; // dropping `up` closes the upstream request
                            }
                        }
                    }
                }
            });
        }
    });
    server
}
