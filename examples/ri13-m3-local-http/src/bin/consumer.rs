mod generated {
    include!("../generated.rs");
}

use generated::AsyncCallError;
use semaprax::project::{with_authenticated_project, ProjectRevision};
use semaprax::resumable_effects::source_local_future::SourceLocalFutureFailure;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
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
) -> impl Future<Output = Result<i64, AsyncCallError<DemoError>>> {
    let mut builder = reqwest::Client::builder().retry(reqwest::retry::never());
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    let client = builder.build().expect("explicit local HTTP client");
    generated::register(revision, move |request| async move {
        let url = reqwest::Url::parse(&format!("{endpoint}/value/{request}"))
            .map_err(|_| DemoError::Transport)?;
        let response = client.get(url).send().await.map_err(transport)?;
        if !response.status().is_success() {
            return Err(DemoError::HttpStatus(response.status().as_u16()));
        }
        response
            .text()
            .await
            .map_err(transport)?
            .parse::<i64>()
            .map_err(|_| DemoError::InvalidBody)
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
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes());
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "local client must not retry"
        );
    });
    (endpoint, received_rx, server)
}

fn run_case(
    revision: Arc<ProjectRevision>,
    runtime: &tokio::runtime::Runtime,
    local: &tokio::task::LocalSet,
    status: u16,
    body: &'static str,
    delay: Duration,
    timeout: Option<Duration>,
) -> Result<i64, AsyncCallError<DemoError>> {
    let (endpoint, _received, server) = local_server(status, body, delay);
    let result = local.block_on(runtime, selected_call(revision, endpoint, timeout));
    server.join().unwrap();
    result
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
        let result = generated::register(revision, |_request| async { Ok::<i64, ()>(43) });
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
        Ok(84)
    ));
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
        Err(AsyncCallError::Host(DemoError::HttpStatus(503)))
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
        Err(AsyncCallError::Host(DemoError::InvalidBody))
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
        Err(AsyncCallError::Source(
            SourceLocalFutureFailure::LanguageFailure(_)
        ))
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
            Err(AsyncCallError::Host(DemoError::Timeout))
        ),
        "timeout guard did not reject delayed response"
    );

    let (endpoint, received, server) = local_server(200, "43", Duration::from_millis(400));
    let pending = selected_call(revision, endpoint, Some(Duration::from_secs(2)));
    local.block_on(&runtime, async move {
        let task = tokio::task::spawn_local(pending);
        received.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    });
    server.join().unwrap();
    println!("ri13-m3-local-http-ok");
}
