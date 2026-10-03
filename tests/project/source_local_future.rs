//! RI-09: held-input Project admission to an interpreter-backed Rust Future.

use semaprax::project::{with_authenticated_project, ProjectManifest, ProjectProfile};
use semaprax::resumable_effects::source_local_future::SourceLocalFuture;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"local-future\"\nversion = \"1.0.0\"\nprofile = \"source-local-future.v1\"\n\n[modules]\nentry = \"local_future.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"local_future.tests\"]\n\n[exports]\nweb = []\nrust_async = [\"local_future.ask\"]\n";

const APP: &str = "module local_future.app;\n@id(\"local_future.ask\")\nfn ask(seed: i64) -> i64 yields i64 -> i64 {\n    let answer = yield seed + 1;\n    answer + seed\n}\n@id(\"local_future.main\")\nfn main() -> i64 { 0 }\n";
const TESTS: &str =
    "module local_future.tests;\n@id(\"local_future.tests.main\")\nfn main() -> i64 { 0 }\n";

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-ri09-project-future-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
        for (name, source) in [("app.spx", APP), ("tests.spx", TESTS)] {
            let path = Path::new(name);
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(
                root.join("src").join(name),
                semaprax::format::canonical(&parsed),
            )
            .unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
}

struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

#[test]
fn authenticated_project_selected_async_export_awaits_host_future_and_refuses_drift() {
    let fixture = Fixture::new();
    let parsed = ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(
        parsed.project_profile(),
        ProjectProfile::SourceLocalFutureV1
    );
    assert_eq!(parsed.to_canonical_toml(), MANIFEST);
    assert_eq!(parsed.web_exports(), &[] as &[String]);
    assert_eq!(parsed.rust_async_exports(), ["local_future.ask"]);

    let revision = with_authenticated_project(&fixture.manifest(), |snapshot| {
        snapshot.check()?;
        let signature = snapshot.source_local_future_signature()?;
        assert_eq!(signature.function_id(), "local_future.ask");
        assert_eq!(signature.yield_count(), 1);
        let web = match snapshot.build_web_inline(1024) {
            Err(error) => error,
            Ok(_) => panic!("source Future profile must refuse Web emission"),
        };
        assert_eq!(web[0].code, "SPX-W120");
        let npm = match snapshot.build_npm_inline(1024) {
            Err(error) => error,
            Ok(_) => panic!("source Future profile must refuse npm emission"),
        };
        assert_eq!(npm[0].code, "SPX-W120");
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let selected =
        SourceLocalFuture::prepare_revision(revision, 41, 10_000, |request| async move {
            Ok::<i64, ()>(request + 1)
        })
        .unwrap();
    let mut caller = Box::pin(async move { selected.await });
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    assert_eq!(
        Pin::as_mut(&mut caller).poll(&mut context),
        Poll::Ready(Ok(84))
    );

    let result = with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        std::fs::write(fixture.0.join("src/app.spx"), "module local_future.app;\n").unwrap();
        Ok(revision)
    });
    assert!(
        result.is_err(),
        "held source drift must refuse retained revision release"
    );

    let wrong = Fixture::new();
    let wrong_manifest = MANIFEST.replace(
        "rust_async = [\"local_future.ask\"]",
        "rust_async = [\"local_future.main\"]",
    );
    std::fs::write(wrong.manifest(), wrong_manifest).unwrap();
    let refusal = with_authenticated_project(&wrong.manifest(), |_snapshot| Ok(())).unwrap_err();
    assert_eq!(refusal[0].code, "SPX-H006");
}

fn receive_request(stream: &mut TcpStream, path: &str) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut bytes = [0u8; 2048];
    let mut used = 0;
    while !bytes[..used].windows(4).any(|window| window == b"\r\n\r\n") {
        let count = stream.read(&mut bytes[used..]).unwrap();
        assert!(count > 0 && used + count < bytes.len());
        used += count;
    }
    assert!(bytes[..used].starts_with(format!("GET {path} HTTP/1.1\r\n").as_bytes()));
}

#[test]
#[ignore = "requires explicit checkout-private Cargo target and local HTTP sockets"]
fn selected_project_future_awaits_locked_reqwest_and_cancel_does_not_undo_request() {
    let target = PathBuf::from(
        std::env::var_os("SEMAPRAX_RI09_TARGET_DIR")
            .expect("set SEMAPRAX_RI09_TARGET_DIR to a checkout-private target"),
    )
    .canonicalize()
    .unwrap();
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(target.starts_with(checkout.join("target")));

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install explicit rustls provider");
    let fixture = Fixture::new();
    let revision = with_authenticated_project(&fixture.manifest(), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (received_tx, received_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut received_tx = Some(received_tx);
        for (path, response) in [
            ("/checked", b"43".as_slice()),
            ("/cancelled", b"99".as_slice()),
        ] {
            let (mut socket, _) = listener.accept().unwrap();
            receive_request(&mut socket, path);
            if path == "/cancelled" {
                received_tx.take().unwrap().send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            }
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.len()
            );
            let _ = socket.write_all(header.as_bytes());
            let _ = socket.write_all(response);
        }
        listener.set_nonblocking(true).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    });

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        let first_url = format!("{endpoint}/checked");
        let first = SourceLocalFuture::prepare_revision(
            Arc::clone(&revision),
            41,
            10_000,
            move |request| async move {
                assert_eq!(request, 42);
                let response = reqwest::get(first_url).await?;
                let body = response.text().await?;
                Ok::<i64, reqwest::Error>(body.parse().unwrap())
            },
        )
        .unwrap();
        // The physical response is 43. Authored source adds the seed (41),
        // so a pass requires resuming the selected checked source body.
        assert_eq!(first.await, Ok(84));

        let second_url = format!("{endpoint}/cancelled");
        let second =
            SourceLocalFuture::prepare_revision(revision, 41, 10_000, move |request| async move {
                assert_eq!(request, 42);
                let response = reqwest::get(second_url).await?;
                let body = response.text().await?;
                Ok::<i64, reqwest::Error>(body.parse().unwrap())
            })
            .unwrap();
        let task = tokio::task::spawn_local(second);
        received_rx.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        release_tx.send(()).unwrap();
    });
    server.join().unwrap();
}
