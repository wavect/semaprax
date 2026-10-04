//! RI-09: held-input Project admission to an interpreter-backed Rust Future.

use semaprax::project::{with_authenticated_project, ProjectManifest, ProjectProfile};
use semaprax::resumable_effects::source_local_future::SourceLocalFuture;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"local-future\"\nversion = \"1.0.0\"\nprofile = \"source-local-future.v1\"\n\n[modules]\nentry = \"local_future.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"local_future.tests\"]\n\n[exports]\nweb = []\nrust_async = [\"local_future.ask\"]\n";

const APP: &str = "module local_future.app;\n@id(\"local_future.ask\")\nfn ask(seed: i64) -> i64 yields i64 -> i64 {\n    let answer = yield seed + 1;\n    answer + seed\n}\n@id(\"local_future.main\")\nfn main() -> i64 { 0 }\n";
const TESTS: &str =
    "module local_future.tests;\n@id(\"local_future.tests.main\")\nfn main() -> i64 { 0 }\n";
const RI13_MANIFEST: &str =
    include_str!("../../examples/ri13-combined-app/unified-project/semaprax.toml");
const RI13_APP: &str = include_str!("../../examples/ri13-combined-app/unified-project/src/app.spx");
const RI13_TESTS: &str =
    include_str!("../../examples/ri13-combined-app/unified-project/src/tests.spx");

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
    let mut caller = Box::pin(selected);
    let mut context = Context::from_waker(Waker::noop());
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

#[test]
fn ri13_closed_indexed_rust_profile_requires_authenticated_indexed_selections() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-ri13-unified-future-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let root = root.canonicalize().unwrap();
    std::fs::write(root.join("semaprax.toml"), RI13_MANIFEST).unwrap();
    for (name, source) in [("app.spx", RI13_APP), ("tests.spx", RI13_TESTS)] {
        let parsed = semaprax::parse(source, Path::new(name)).unwrap();
        std::fs::write(
            root.join("src").join(name),
            semaprax::format::canonical(&parsed),
        )
        .unwrap();
    }
    let manifest = root.join("semaprax.toml");
    let refusal = with_authenticated_project(&manifest, |_snapshot| Ok(())).unwrap_err();
    assert_eq!(refusal[0].code, "SPX-H006");
    assert!(refusal[0]
        .message
        .contains("authenticated indexed Rust selections"));

    let untrusted = RI13_MANIFEST.replace("url = [\"=2.5.8\"]", "url = [\"=2.5.7\"]");
    std::fs::write(&manifest, untrusted).unwrap();
    let refusal = with_authenticated_project(&manifest, |_snapshot| Ok(())).unwrap_err();
    assert_eq!(refusal[0].code, "SPX-H006");

    let unsupported = RI13_MANIFEST.replace(
        "url = [\"=2.5.8\"]",
        "url = [\"=2.5.8\"]\nserde = [\"=1.0.228\"]",
    );
    std::fs::write(&manifest, unsupported).unwrap();
    let refusal = with_authenticated_project(&manifest, |_snapshot| Ok(())).unwrap_err();
    assert_eq!(refusal[0].code, "SPX-H006");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_local_future_binding_is_construction_bound_and_refuses_stale_or_forged_facts() {
    let fixture = Fixture::new();
    let retained = with_authenticated_project(&fixture.manifest(), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let retained_signature = retained.source_local_future_signature().unwrap();
    let retained_revision = retained.project_revision().to_owned();
    let retained_function = retained_signature.function_id().to_owned();
    let retained_plan = *retained_signature.plan_identity();
    retained
        .require_source_local_future_binding(&retained_revision, &retained_function, &retained_plan)
        .unwrap();

    let changed = APP.replace("answer + seed", "answer - seed");
    let parsed = semaprax::parse(&changed, Path::new("app.spx")).unwrap();
    std::fs::write(
        fixture.0.join("src/app.spx"),
        semaprax::format::canonical(&parsed),
    )
    .unwrap();
    let rebuilt = with_authenticated_project(&fixture.manifest(), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let rebuilt_signature = rebuilt.source_local_future_signature().unwrap();
    assert_ne!(rebuilt.project_revision(), retained_revision);
    assert_ne!(rebuilt_signature.plan_identity(), &retained_plan);
    assert_eq!(
        rebuilt
            .require_source_local_future_binding(
                &retained_revision,
                &retained_function,
                &retained_plan,
            )
            .unwrap_err()[0]
            .code,
        "SPX-H006"
    );
    assert_eq!(
        rebuilt
            .require_source_local_future_binding(
                rebuilt.project_revision(),
                rebuilt_signature.function_id(),
                &[0; 32],
            )
            .unwrap_err()[0]
            .code,
        "SPX-H006"
    );
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

fn exact_cargo() -> PathBuf {
    let cargo = PathBuf::from(
        std::env::var_os("CARGO").unwrap_or_else(|| "/opt/homebrew/bin/cargo".into()),
    );
    assert!(cargo.is_absolute() && cargo.is_file());
    let output = Command::new(&cargo).arg("--version").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("cargo 1."));
    cargo
}

#[test]
#[ignore = "compiles an exact generated module in a private locked Cargo consumer"]
fn generated_project_future_module_registers_source_import_and_refuses_stale_project() {
    let target = PathBuf::from(
        std::env::var_os("SEMAPRAX_RI09_TARGET_DIR")
            .expect("set SEMAPRAX_RI09_TARGET_DIR to a checkout-private target"),
    )
    .canonicalize()
    .unwrap();
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(target.starts_with(checkout.join("target")));
    let cargo = exact_cargo();

    let fixture = Fixture::new();
    let rendered = with_authenticated_project(&fixture.manifest(), |snapshot| {
        let generated = snapshot.render_source_local_future_rust_module()?;
        assert_eq!(
            generated,
            snapshot
                .retain_revision()
                .render_source_local_future_rust_module()?
        );
        Ok(generated)
    })
    .unwrap();
    assert!(rendered.contains("pub const SOURCE_FUNCTION_ID: &str = \"local_future.ask\";"));
    let consumer = fixture.0.join("consumer");
    std::fs::create_dir_all(consumer.join("src")).unwrap();
    let checkout_toml = serde_json::to_string(&checkout.display().to_string()).unwrap();
    let manifest = format!(
        "[package]\nname = \"semaprax-ri09-generated-project-sdk\"\nversion = \"0.1.0\"\nedition = \"2021\"\npublish = false\n\n[workspace]\n\n[dependencies]\nsemaprax = {{ path = {checkout_toml}, version = \"=0.7.0\" }}\ntokio = {{ version = \"=1.53.1\", features = [\"rt\", \"time\"] }}\n"
    );
    std::fs::write(consumer.join("Cargo.toml"), manifest).unwrap();
    let consumer_lock = include_str!("ri09_generated_consumer.Cargo.lock");
    std::fs::write(consumer.join("Cargo.lock"), consumer_lock).unwrap();
    std::fs::write(
        consumer.join("src/main.rs"),
        include_str!("ri09_generated_consumer.rs.txt"),
    )
    .unwrap();
    std::fs::write(consumer.join("src/generated.rs"), rendered).unwrap();

    let run = |expect_stale: bool| {
        let mut command = Command::new(&cargo);
        command
            .args(["run", "--quiet", "--locked", "--offline", "--manifest-path"])
            .arg(consumer.join("Cargo.toml"))
            .arg("--")
            .arg(fixture.manifest());
        if expect_stale {
            command.arg("expect-stale");
        }
        let output = command
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", &target)
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            if expect_stale {
                b"ri09-generated-stale-refused\n".as_slice()
            } else {
                b"ri09-generated-source-ok\n".as_slice()
            }
        );
        assert_eq!(
            std::fs::read(consumer.join("Cargo.lock")).unwrap(),
            consumer_lock.as_bytes()
        );
    };
    run(false);

    let changed = APP.replace("answer + seed", "answer - seed");
    let parsed = semaprax::parse(&changed, Path::new("app.spx")).unwrap();
    std::fs::write(
        fixture.0.join("src/app.spx"),
        semaprax::format::canonical(&parsed),
    )
    .unwrap();
    run(true);
}
