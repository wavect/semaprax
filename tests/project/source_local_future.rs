//! RI-09: held-input Project admission to an interpreter-backed Rust Future.

use semaprax::project::{with_authenticated_project, ProjectManifest, ProjectProfile};
use semaprax::resumable_effects::source_local_future::SourceLocalFuture;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

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
