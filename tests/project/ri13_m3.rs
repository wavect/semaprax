//! Saved RI-13 M3 Project: a real local HTTP host effect under explicit Tokio.
//! The owning gate stays inside the Project test binary, so it never starts a
//! second Cargo build while verifying the saved application's source contract.

use semaprax::project::{with_authenticated_project, ProjectRevision};
use semaprax::resumable_effects::source_local_future::{
    SourceLocalFuture, SourceLocalFutureFailure,
};
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Debug, Eq, PartialEq)]
enum HostError {
    Timeout,
    HttpStatus(u16),
    InvalidBody,
    Transport,
}

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
struct CopyMetrics {
    foreign_response_body_copied_bytes: u64,
    host_callback_captured_bytes: u64,
}

/// Test-only controls for proving that the integration oracle notices a host
/// callback that returns the wrong value or omits its status guard. Production
/// calls always use `AUTHENTIC_HOST_CONTROLS` below.
#[derive(Clone, Copy)]
struct HostMutationControls {
    check_http_status: bool,
    return_adjustment: i64,
}

const AUTHENTIC_HOST_CONTROLS: HostMutationControls = HostMutationControls {
    check_http_status: true,
    return_adjustment: 0,
};

/// Exact fixture-owned copies only. reqwest and HTTP internals stay outside
/// this observation because the host does not expose their copy operations.
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

fn transport(error: reqwest::Error) -> HostError {
    if error.is_timeout() {
        HostError::Timeout
    } else {
        HostError::Transport
    }
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

/// Release a just-reserved loopback address before the client starts so this
/// route observes connection refusal rather than a delayed HTTP response.
fn refused_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    endpoint
}

fn selected_call_with_controls(
    revision: Arc<ProjectRevision>,
    endpoint: String,
    timeout: Option<Duration>,
    copies: CopyLedger,
    controls: HostMutationControls,
) -> Result<
    impl Future<Output = (Result<i64, SourceLocalFutureFailure>, Option<HostError>)>,
    Vec<semaprax::diagnostic::Diagnostic>,
> {
    let mut builder = reqwest::Client::builder().retry(reqwest::retry::never());
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    let client = builder.build().unwrap();
    let error = Rc::new(RefCell::new(None));
    let slot = Rc::clone(&error);
    let selected = SourceLocalFuture::prepare_revision(revision, 41, 10_000, move |request| {
        let slot = Rc::clone(&slot);
        let copies = copies.clone();
        async move {
            let request_result = async {
                let url = reqwest::Url::parse(&format!("{endpoint}/value/{request}"))
                    .map_err(|_| HostError::Transport)?;
                let response = client.get(url).send().await.map_err(transport)?;
                if controls.check_http_status && !response.status().is_success() {
                    return Err(HostError::HttpStatus(response.status().as_u16()));
                }
                let body = response.bytes().await.map_err(transport)?;
                let captured = body.to_vec();
                copies.capture_foreign_response_body(captured.len());
                std::str::from_utf8(&captured)
                    .map_err(|_| HostError::InvalidBody)?
                    .parse::<i64>()
                    .map_err(|_| HostError::InvalidBody)?
                    .checked_add(controls.return_adjustment)
                    .ok_or(HostError::InvalidBody)
            }
            .await;
            match request_result {
                Ok(value) => Ok(value),
                Err(failure) => {
                    *slot.borrow_mut() = Some(failure);
                    Err(())
                }
            }
        }
    })?;
    Ok(async move {
        let result = selected.await;
        let host_error = error.borrow_mut().take();
        (result, host_error)
    })
}

fn selected_call(
    revision: Arc<ProjectRevision>,
    endpoint: String,
    timeout: Option<Duration>,
    copies: CopyLedger,
) -> Result<
    impl Future<Output = (Result<i64, SourceLocalFutureFailure>, Option<HostError>)>,
    Vec<semaprax::diagnostic::Diagnostic>,
> {
    selected_call_with_controls(revision, endpoint, timeout, copies, AUTHENTIC_HOST_CONTROLS)
}

fn run_case(
    revision: Arc<ProjectRevision>,
    runtime: &tokio::runtime::Runtime,
    local: &tokio::task::LocalSet,
    status: u16,
    body: &'static str,
    delay: Duration,
    timeout: Option<Duration>,
) -> (Result<i64, SourceLocalFutureFailure>, Option<HostError>) {
    let (endpoint, _received, server) = local_server(status, body, delay);
    let result = local.block_on(
        runtime,
        selected_call(revision, endpoint, timeout, CopyLedger::default()).unwrap(),
    );
    server.join().unwrap();
    result
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/ri13-m3-local-http/project");
        let root = std::env::temp_dir().join(format!("semaprax-ri13-m3-{}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        for file in ["semaprax.toml", "src/app.spx", "src/tests.spx"] {
            std::fs::copy(source.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires local HTTP sockets"]
fn saved_m3_application_runs_offline_and_refuses_timeout_and_stale_binding_mutants() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let fixture = Fixture::new();
    let manifest = fixture.0.join("semaprax.toml");
    let (revision, module) = with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        let module = snapshot.render_source_local_future_rust_module()?;
        Ok((snapshot.retain_revision(), module))
    })
    .unwrap();
    assert!(module.contains("pub enum AsyncCallError<E>"));
    assert!(module.contains("pub fn register<H>"));
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
            timeout
        ),
        (Ok(84), None)
    ));

    // The source-local effect handler receives foreign Bytes and deliberately
    // copies them into callback-owned Vec storage. This checks the observable
    // operation, rather than inferring copies from allocation counts or
    // treating the scalar generated boundary as evidence.
    let copies = CopyLedger::default();
    let (endpoint, _received, server) = local_server(200, "43", Duration::ZERO);
    assert_eq!(
        local.block_on(
            &runtime,
            selected_call(
                Arc::clone(&revision),
                endpoint,
                Some(Duration::from_millis(100)),
                copies.clone(),
            )
            .unwrap(),
        ),
        (Ok(84), None)
    );
    server.join().unwrap();
    assert_eq!(
        copies.metrics(),
        CopyMetrics {
            foreign_response_body_copied_bytes: 2,
            host_callback_captured_bytes: 2,
        }
    );

    // Keep the source result scalar while proving that the exact callback-owned
    // body capture continues to count a nontrivial local transfer precisely.
    let copies = CopyLedger::default();
    let (endpoint, _received, server) =
        local_server_bytes(200, padded_result_body(), Duration::ZERO);
    assert_eq!(
        local.block_on(
            &runtime,
            selected_call(
                Arc::clone(&revision),
                endpoint,
                Some(Duration::from_millis(100)),
                copies.clone(),
            )
            .unwrap(),
        ),
        (Ok(84), None)
    );
    server.join().unwrap();
    assert_eq!(
        copies.metrics(),
        CopyMetrics {
            foreign_response_body_copied_bytes: 4096,
            host_callback_captured_bytes: 4096,
        }
    );

    let copies = CopyLedger::default();
    assert_eq!(
        local.block_on(
            &runtime,
            selected_call(
                Arc::clone(&revision),
                refused_endpoint(),
                Some(Duration::from_millis(100)),
                copies.clone(),
            )
            .unwrap(),
        ),
        (
            Err(SourceLocalFutureFailure::HandlerFailed),
            Some(HostError::Transport)
        )
    );
    assert_eq!(copies.metrics(), CopyMetrics::default());

    // A host callback that returns 42 rather than the actual HTTP body 43
    // produces 83 through the checked source body, rather than the admitted
    // 84. The normal success oracle would therefore fail this real route.
    let (endpoint, _received, server) = local_server(200, "43", Duration::ZERO);
    let wrong_return = local.block_on(
        &runtime,
        selected_call_with_controls(
            Arc::clone(&revision),
            endpoint,
            Some(Duration::from_millis(100)),
            CopyLedger::default(),
            HostMutationControls {
                check_http_status: true,
                return_adjustment: -1,
            },
        )
        .unwrap(),
    );
    server.join().unwrap();
    assert_eq!(wrong_return, (Ok(83), None));
    assert_ne!(
        wrong_return,
        (Ok(84), None),
        "wrong return escaped the oracle"
    );

    // Removing the explicit HTTP-status capability guard lets a 503 carrying
    // a numeric body reach the source callback. The authentic route below
    // would return the typed status failure, so this mutant is observable.
    let (endpoint, _received, server) = local_server(503, "43", Duration::ZERO);
    let dropped_status_guard = local.block_on(
        &runtime,
        selected_call_with_controls(
            Arc::clone(&revision),
            endpoint,
            Some(Duration::from_millis(100)),
            CopyLedger::default(),
            HostMutationControls {
                check_http_status: false,
                return_adjustment: 0,
            },
        )
        .unwrap(),
    );
    server.join().unwrap();
    assert_eq!(dropped_status_guard, (Ok(84), None));
    assert_ne!(
        dropped_status_guard,
        (
            Err(SourceLocalFutureFailure::HandlerFailed),
            Some(HostError::HttpStatus(503))
        ),
        "dropped HTTP-status guard escaped the typed-error oracle"
    );
    assert!(matches!(
        run_case(
            Arc::clone(&revision),
            &runtime,
            &local,
            503,
            "unavailable",
            Duration::ZERO,
            timeout
        ),
        (
            Err(SourceLocalFutureFailure::HandlerFailed),
            Some(HostError::HttpStatus(503))
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
            timeout
        ),
        (
            Err(SourceLocalFutureFailure::HandlerFailed),
            Some(HostError::InvalidBody)
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
            timeout
        ),
        (Err(SourceLocalFutureFailure::LanguageFailure(_)), None)
    ));
    assert!(matches!(
        run_case(
            Arc::clone(&revision),
            &runtime,
            &local,
            200,
            "43",
            Duration::from_millis(400),
            timeout
        ),
        (
            Err(SourceLocalFutureFailure::HandlerFailed),
            Some(HostError::Timeout)
        )
    ));
    assert!(
        matches!(
            run_case(
                Arc::clone(&revision),
                &runtime,
                &local,
                200,
                "43",
                Duration::from_millis(400),
                None
            ),
            (Ok(84), None)
        ),
        "omitting the timeout must expose the guard mutation"
    );

    let (endpoint, received, server) = local_server(200, "43", Duration::from_millis(400));
    let pending = selected_call(
        Arc::clone(&revision),
        endpoint,
        Some(Duration::from_secs(2)),
        CopyLedger::default(),
    )
    .unwrap();
    local.block_on(&runtime, async move {
        let task = tokio::task::spawn_local(pending);
        received.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    });
    server.join().unwrap();

    let source = std::fs::read_to_string(fixture.0.join("src/app.spx")).unwrap();
    let changed = source.replace("response + seed", "response - seed");
    assert_ne!(source, changed);
    let refusal = with_authenticated_project(&manifest, |snapshot| {
        let held = snapshot.retain_revision();
        std::fs::write(fixture.0.join("src/app.spx"), &changed).unwrap();
        Ok(held)
    });
    assert!(
        refusal.is_err(),
        "held source drift must refuse revision release"
    );
}
