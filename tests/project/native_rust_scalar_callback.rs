//! Project v1 (`ScalarV1`) sources that declare a Native Rust import callback.
//!
//! The bidirectional Rust SDK needs a Rust caller to reach a SEMAPRAX export
//! that calls back into a Rust import, so the scalar workspace linker must
//! retain the interface and the effectful function that carries its authority
//! instead of dropping both.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::with_authenticated_project;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"callback\"\nentry = \"callback.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"callback.apply\"]\ntests = [\"callback.tests\"]\n";

const APP: &str = r#"
module callback.app;

permit { host.adjust }

@id("callback.host")
interface CallbackHost permits { host.adjust } {
    @id("callback.host.adjust")
    import rust fn adjust(value: i64) -> i64
        effects { host.adjust }
        failure status "callback.host.v1";
}

@id("callback.apply")
fn apply(left: i64, right: i64) -> i64 uses { host.adjust } { adjust(left + right) }

@id("callback.main")
fn main() -> i64 uses { host.adjust } { apply(19, 23) }
"#;

const TESTS: &str =
    "module callback.tests;\n\n@id(\"callback.tests.main\")\nfn main() -> i64 { 0 }\n";

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-native-rust-scalar-callback-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
    for (relative, source) in [("src/app.spx", APP), ("src/tests.spx", TESTS)] {
        let program = semaprax::parse(source, Path::new(relative)).unwrap();
        std::fs::write(root.join(relative), semaprax::format::canonical(&program)).unwrap();
    }
    Fixture(root.canonicalize().unwrap())
}

// The scalar workspace linker now retains the callback interface, but Project
// v1 admission still derives its target through the Public Scalar Export
// Profile v1 WebAssembly emitter, which admits neither module permits nor
// interfaces (`SPX-W115`) and rejects Native Rust imports outright
// (`SPX-W114`). Those are WebAssembly target rules and stay closed; a
// bidirectional Rust SDK needs Project v1 admission to select a non-WebAssembly
// route for a native-callback program. This test pins that exact boundary, so
// the day the route exists the failure it records changes here first.
#[test]
fn scalar_project_native_rust_callback_reaches_the_wasm_target_admission_gate() {
    let fixture = fixture();
    let error = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        snapshot.with_authenticated_native_rust_sdk_subject(|subject| {
            Ok(subject.program().interfaces.len())
        })
    })
    .expect_err("Project v1 admission still emits a WebAssembly scalar-export module");
    assert_eq!(error[0].code, "SPX-W115");
    assert_eq!(
        error[0].message,
        "Public Scalar Export Profile v1 does not admit module permits"
    );
}
