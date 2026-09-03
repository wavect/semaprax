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

// Project v1 admission proves a callback closure without deriving any target.
// WebAssembly rejects Native Rust imports and the ordinary native backend
// cannot lower a callback call site, so such a Project has no Web target and
// no scalar WIT descriptor; its only consumer is the generated C and safe Rust
// bridge the SDK builder renders from linked HIR.
#[test]
fn scalar_project_native_rust_callback_is_admitted_and_retains_its_interface() {
    let fixture = fixture();
    let (interfaces, imports, exports) =
        with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            snapshot.with_authenticated_native_rust_sdk_subject(|subject| {
                let program = subject.program();
                let imports = program
                    .interfaces
                    .iter()
                    .flat_map(|interface| &interface.imports)
                    .filter(|import| import.native_rust)
                    .map(|import| import.id.as_str().to_owned())
                    .collect::<Vec<_>>();
                let exports = subject
                    .exports()
                    .iter()
                    .map(|export| export.stable_id().to_owned())
                    .collect::<Vec<_>>();
                Ok((program.interfaces.len(), imports, exports))
            })
        })
        .expect("a declared Native Rust callback is admitted without a Web target");

    assert_eq!(interfaces, 1);
    assert_eq!(imports, ["callback.host.adjust"]);
    assert_eq!(exports, ["callback.apply"]);
}

// The Web target stays closed for exactly the same program, so admitting the
// callback never silently promises a WebAssembly artifact.
#[test]
fn the_same_callback_program_is_still_refused_by_the_wasm_target() {
    let fixture = fixture();
    let source = std::fs::read_to_string(fixture.0.join("src/app.spx")).unwrap();
    let program = semaprax::parse(&source, Path::new("src/app.spx")).unwrap();
    let error = semaprax::wasm::emit_module(&program).unwrap_err();
    assert_eq!(error.code, "SPX-W114");
    assert_eq!(
        error.message,
        "Native Rust imports are unavailable for WebAssembly targets"
    );
}
