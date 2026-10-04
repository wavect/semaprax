mod generated {
    include!("../generated.rs");
}

use generated::AsyncCallError;
use semaprax::project::{with_authenticated_project, ProjectRevision};
use semaprax::resumable_effects::source_local_future::SourceLocalFutureFailure;
use std::cell::Cell;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Debug, Eq, PartialEq)]
enum DemoError {
    Timeout,
    HttpStatus(u16),
    InvalidBody,
    Transport,
}

/// Exact fixture-owned copies. Bytes allocated by reqwest or HTTP decoding
/// before this conversion are foreign implementation details and are not
/// represented here.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CopyMetrics {
    foreign_response_body_copied_bytes: u64,
    host_callback_captured_bytes: u64,
}

#[derive(Clone, Default)]
struct CopyLedger(Rc<Cell<CopyMetrics>>);

impl CopyLedger {
    fn capture_foreign_response_body(&self, bytes: usize) {
        let bytes = u64::try_from(bytes).expect("fixture response length fits u64");
        let mut metrics = self.0.get();
        metrics.foreign_response_body_copied_bytes = metrics
            .foreign_response_body_copied_bytes
            .checked_add(bytes)
            .expect("fixture response copy total fits u64");
        metrics.host_callback_captured_bytes = metrics
            .host_callback_captured_bytes
            .checked_add(bytes)
            .expect("fixture callback copy total fits u64");
        self.0.set(metrics);
    }

    fn metrics(&self) -> CopyMetrics {
        self.0.get()
    }
}

fn transport(error: reqwest::Error) -> DemoError {
    if error.is_timeout() {
        DemoError::Timeout
    } else {
        DemoError::Transport
    }
}

fn selected_call(
    revision: Arc<ProjectRevision>,
    endpoint: String,
    timeout: Option<Duration>,
    copies: CopyLedger,
) -> impl Future<Output = Result<i64, AsyncCallError<DemoError>>> {
    let mut builder = reqwest::Client::builder().retry(reqwest::retry::never());
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    let client = builder.build().expect("explicit local HTTP client");
    generated::register(revision, move |request| {
        let copies = copies.clone();
        async move {
            let url = reqwest::Url::parse(&format!("{endpoint}/value/{request}"))
                .map_err(|_| DemoError::Transport)?;
            let response = client.get(url).send().await.map_err(transport)?;
            if !response.status().is_success() {
                return Err(DemoError::HttpStatus(response.status().as_u16()));
            }
            // This is the exact application-owned copy from reqwest's foreign
            // response Bytes into host-callback Vec storage. It says nothing
            // about earlier reqwest or HTTP-decoding copies.
            let body = response.bytes().await.map_err(transport)?;
            let captured = body.to_vec();
            copies.capture_foreign_response_body(captured.len());
            std::str::from_utf8(&captured)
                .map_err(|_| DemoError::InvalidBody)?
                .parse::<i64>()
                .map_err(|_| DemoError::InvalidBody)
        }
    })
    .expect("exact generated Project registration")
    .call_typed(41, 10_000)
    .expect("checked source prefix")
}

fn receive_request(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = [0u8; 2048];
    let mut used = 0;
    while !bytes[..used].windows(4).any(|window| window == b"\r\n\r\n") {
        let count = stream.read(&mut bytes[used..]).unwrap();
        assert!(count > 0 && used + count < bytes.len());
        used += count;
    }
    assert!(bytes[..used].starts_with(b"GET /value/42 HTTP/1.1\r\n"));
}

fn local_server(
    status: u16,
    body: &'static str,
    delay: Duration,
) -> (String, tokio::sync::oneshot::Receiver<()>, JoinHandle<()>) {
    local_server_bytes(status, body.as_bytes().to_vec(), delay)
}

fn local_server_bytes(
    status: u16,
    body: Vec<u8>,
    delay: Duration,
) -> (String, tokio::sync::oneshot::Receiver<()>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (received_tx, received_rx) = tokio::sync::oneshot::channel();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "local request deadline");
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("local accept: {error}"),
            }
        };
        receive_request(&mut socket);
        let _ = received_tx.send(());
        std::thread::sleep(delay);
        let reason = if status == 200 {
            "OK"
        } else {
            "Service Unavailable"
        };
        let header = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = socket.write_all(header.as_bytes());
        let _ = socket.write_all(&body);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "local client must not retry"
        );
    });
    (endpoint, received_rx, server)
}

fn padded_result_body() -> Vec<u8> {
    let mut body = vec![b'0'; 4094];
    body.extend_from_slice(b"43");
    body
}

/// Reserve a loopback port, then release it before the client starts. The
/// ensuing connection-refusal case exercises the typed transport path without
/// any external service or response body.
fn refused_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    endpoint
}

fn run_case(
    revision: Arc<ProjectRevision>,
    runtime: &tokio::runtime::Runtime,
    local: &tokio::task::LocalSet,
    status: u16,
    body: &'static str,
    delay: Duration,
    timeout: Option<Duration>,
) -> (Result<i64, AsyncCallError<DemoError>>, CopyMetrics) {
    let (endpoint, _received, server) = local_server(status, body, delay);
    let copies = CopyLedger::default();
    let result = local.block_on(
        runtime,
        selected_call(revision, endpoint, timeout, copies.clone()),
    );
    server.join().unwrap();
    (result, copies.metrics())
}

fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install explicit rustls provider");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = root.join("project/semaprax.toml");
    let revision = with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .expect("admitted held Project");
    let mode = std::env::args().nth(1);
    if mode.as_deref() == Some("expect-stale") {
        let result = generated::register(revision, |_request: i64| async { Ok::<i64, ()>(43) });
        assert_eq!(result.err().unwrap()[0].code, "SPX-H006");
        println!("ri13-m3-stale-refused");
        return;
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    let timeout = Some(Duration::from_millis(100));
    assert!(matches!(
        run_case(
            Arc::clone(&revision),
            &runtime,
            &local,
            200,
            "43",
            Duration::ZERO,
            timeout,
        ),
        (
            Ok(84),
            CopyMetrics {
                foreign_response_body_copied_bytes: 2,
                host_callback_captured_bytes: 2,
            }
        )
    ));
    let copies = CopyLedger::default();
    let (endpoint, _received, server) =
        local_server_bytes(200, padded_result_body(), Duration::ZERO);
    assert!(matches!(
        local.block_on(
            &runtime,
            selected_call(
                Arc::clone(&revision),
                endpoint,
                Some(Duration::from_millis(100)),
                copies.clone(),
            )
        ),
        Ok(84)
    ));
    server.join().unwrap();
    assert_eq!(
        copies.metrics(),
        CopyMetrics {
            foreign_response_body_copied_bytes: 4096,
            host_callback_captured_bytes: 4096,
        }
    );
    let copies = CopyLedger::default();
    assert!(matches!(
        local.block_on(
            &runtime,
            selected_call(
                Arc::clone(&revision),
                refused_endpoint(),
                Some(Duration::from_millis(100)),
                copies.clone(),
            )
        ),
        Err(AsyncCallError::Host(DemoError::Transport))
    ));
    assert_eq!(copies.metrics(), CopyMetrics::default());
    assert!(matches!(
        run_case(
            Arc::clone(&revision),
            &runtime,
            &local,
            503,
            "unavailable",
            Duration::ZERO,
            timeout,
        ),
        (
            Err(AsyncCallError::Host(DemoError::HttpStatus(503))),
            CopyMetrics::default()
        )
    ));
    assert!(matches!(
        run_case(
            Arc::clone(&revision),
            &runtime,
            &local,
            200,
            "invalid",
            Duration::ZERO,
            timeout,
        ),
        (
            Err(AsyncCallError::Host(DemoError::InvalidBody)),
            CopyMetrics {
                foreign_response_body_copied_bytes: 7,
                host_callback_captured_bytes: 7,
            }
        )
    ));
    assert!(matches!(
        run_case(
            Arc::clone(&revision),
            &runtime,
            &local,
            200,
            "-99",
            Duration::ZERO,
            timeout,
        ),
        (
            Err(AsyncCallError::Source(
                SourceLocalFutureFailure::LanguageFailure(_)
            )),
            CopyMetrics {
                foreign_response_body_copied_bytes: 3,
                host_callback_captured_bytes: 3,
            }
        )
    ));
    let timeout = if mode.as_deref() == Some("omit-timeout") {
        None
    } else {
        timeout
    };
    assert!(
        matches!(
            run_case(
                Arc::clone(&revision),
                &runtime,
                &local,
                200,
                "43",
                Duration::from_millis(400),
                timeout,
            ),
            (
                Err(AsyncCallError::Host(DemoError::Timeout)),
                CopyMetrics::default()
            )
        ),
        "timeout guard did not reject delayed response"
    );

    let (endpoint, received, server) = local_server(200, "43", Duration::from_millis(400));
    let copies = CopyLedger::default();
    let pending = selected_call(
        revision,
        endpoint,
        Some(Duration::from_secs(2)),
        copies.clone(),
    );
    local.block_on(&runtime, async move {
        let task = tokio::task::spawn_local(pending);
        received.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    });
    server.join().unwrap();
    assert_eq!(copies.metrics(), CopyMetrics::default());
    println!("ri13-m3-local-http-ok");
}
