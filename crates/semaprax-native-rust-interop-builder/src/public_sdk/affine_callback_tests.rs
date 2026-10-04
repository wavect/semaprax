use super::*;
use std::process::{Command, Output};
static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module affine.fixture;
@id("affine.consume") fn consume(payload: own Bytes) -> i64 { 42 }
@id("affine.make") fn make() -> FnOnce() -> i64 {
    let payload = bytes_zeroed(4usize);
    once fn() -> i64 { consume(payload) }
}
@id("app.main") fn main() -> i64 { 0 }
"#;

fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn affine_capture_rust_consumer_retains_source_owner_and_invokes_actual_body() {
    let root = Temp(std::env::temp_dir().join(format!(
        "spx-affine-rust-{}-{}",
        std::process::id(),
        NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir(&root.0).unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    for (label, source, main, expected_success) in [
        ("correct", SOURCE.to_owned(), MAIN, true),
        ("flipped", SOURCE.replace("{ 42 }", "{ 43 }"), MAIN, false),
        (
            "call-failure",
            SOURCE.replace("-> i64 { 42 }", "-> i64 ensures false { 42 }"),
            CALL_FAILURE,
            true,
        ),
        (
            "factory-failure",
            SOURCE.replace(
                "fn make() -> FnOnce() -> i64 {",
                "fn make() -> FnOnce() -> i64 ensures false {",
            ),
            CREATE_FAILURE,
            true,
        ),
    ] {
        let generated =
            prepare_native_rust_affine_callback(&source, Path::new("affine.spx"), "affine.make")
                .unwrap();
        let canonical =
            semaprax::format::canonical(&semaprax::check(&source, "affine.spx").unwrap());
        assert_eq!(
            generated,
            prepare_native_rust_affine_callback(&canonical, Path::new("affine.spx"), "affine.make")
                .unwrap()
        );
        let c = format!("{ALLOCATOR}\n{}\n{COUNTS}", generated.c_source);
        std::fs::write(root.0.join("module.c"), c).unwrap();
        std::fs::write(root.0.join("adapter.rs"), &generated.safe_rust).unwrap();
        success(
            Command::new(&clang)
                .current_dir(&root.0)
                .args(["-std=c11", "-O2", "-c", "module.c", "-o", "module.o"])
                .output()
                .unwrap(),
        );
        success(
            Command::new(&rustc)
                .current_dir(&root.0)
                .args([
                    "--edition=2021",
                    "--crate-name=affine_adapter",
                    "--crate-type=rlib",
                    "adapter.rs",
                    "-o",
                    "libaffine_adapter.rlib",
                ])
                .output()
                .unwrap(),
        );
        std::fs::write(root.0.join("main.rs"), main).unwrap();
        success(
            Command::new(&rustc)
                .current_dir(&root.0)
                .args([
                    "--edition=2021",
                    "main.rs",
                    "--extern",
                    "affine_adapter=libaffine_adapter.rlib",
                    "-C",
                    "link-arg=module.o",
                    "-o",
                    "consumer",
                ])
                .output()
                .unwrap(),
        );
        let output = Command::new(root.0.join("consumer")).output().unwrap();
        assert_eq!(
            output.status.success(),
            expected_success,
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for (source, code) in [
        (
            "let f=AffineCallback::new().unwrap().into_fn_once(); let _=f(); let _=f();",
            "E0382",
        ),
        (
            "let f=AffineCallback::new().unwrap(); std::thread::spawn(move||f.call());",
            "E0277",
        ),
        (
            "let f=AffineCallback::new().unwrap().retain(); std::thread::spawn(move||drop(f));",
            "E0277",
        ),
        (
            "let f=AffineCallback::new().unwrap(); let _=f.clone();",
            "E0599",
        ),
    ] {
        std::fs::write(
            root.0.join("negative.rs"),
            format!("use affine_adapter::*; fn main(){{{source}}}"),
        )
        .unwrap();
        let output = Command::new(&rustc)
            .current_dir(&root.0)
            .args([
                "--edition=2021",
                "--emit=metadata",
                "negative.rs",
                "--extern",
                "affine_adapter=libaffine_adapter.rlib",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(code));
    }
}

const ALLOCATOR: &str = r#"
#include <stdlib.h>
#include <stdint.h>
static uint64_t allocations, frees;
static void *fixture_malloc(size_t n) { allocations++; return malloc(n); }
static void *fixture_calloc(size_t n, size_t s) { allocations++; return calloc(n,s); }
static void fixture_free(void *p) { if(p) frees++; free(p); }
#define malloc fixture_malloc
#define calloc fixture_calloc
#define free fixture_free
"#;
const COUNTS: &str = r#"
#undef malloc
#undef calloc
#undef free
uint64_t affine_allocations(void) { return allocations; }
uint64_t affine_frees(void) { return frees; }
"#;
const MAIN: &str = r#"
use affine_adapter::*;
extern "C" { fn affine_allocations()->u64; fn affine_frees()->u64; }
fn counts()->(u64,u64) { unsafe { (affine_allocations(),affine_frees()) } }
fn retain()->impl FnOnce()->Result<i64,AffineCallbackError> {
    AffineCallback::new().unwrap().into_fn_once()
}
trait Consume { fn finish(self)->Result<i64,AffineCallbackError>; }
struct Adapter(AffineCallback);
impl Consume for Adapter { fn finish(self)->Result<i64,AffineCallbackError> { self.0.call() } }
trait ForeignRetained {
    fn register(&mut self, callback: RetainedAffineCallback) -> Result<(), AffineCallbackError>;
    fn dispatch(&mut self) -> Result<i64, AffineCallbackError>;
    fn unregister(&mut self);
}
struct ForeignRegistry {
    retained: Option<RetainedAffineCallback>,
    offset: i64,
    state_accesses: u64,
    torn_down: bool,
}
impl ForeignRegistry {
    fn new(offset: i64) -> Self {
        Self { retained: None, offset, state_accesses: 0, torn_down: false }
    }
}
impl ForeignRetained for ForeignRegistry {
    fn register(&mut self, callback: RetainedAffineCallback) -> Result<(), AffineCallbackError> {
        if self.torn_down || self.retained.is_some() || !callback.is_active() {
            return Err(AffineCallbackError::RegistrationClosed);
        }
        self.retained = Some(callback);
        Ok(())
    }
    fn dispatch(&mut self) -> Result<i64, AffineCallbackError> {
        let mut callback = self.retained.take().ok_or(AffineCallbackError::RegistrationClosed)?;
        self.state_accesses += 1;
        callback.invoke().map(|value| value + self.offset)
    }
    fn unregister(&mut self) {
        self.torn_down = true;
        if let Some(mut callback) = self.retained.take() { callback.unregister(); }
    }
}
fn main() {
    let f=retain();
    assert_eq!(counts(),(2,0));
    assert_eq!(std::iter::once_with(f).next().unwrap().unwrap(),42);
    assert_eq!(counts(),(2,2));
    let unused=retain(); drop(unused); assert_eq!(counts(),(4,4));
    assert_eq!(Adapter(AffineCallback::new().unwrap()).finish().unwrap(),42);
    assert_eq!(counts(),(6,6));

    let mut registry=ForeignRegistry::new(7);
    registry.register(AffineCallback::new().unwrap().retain()).unwrap();
    assert_eq!(counts(),(8,6));
    assert_eq!(registry.dispatch().unwrap(),49);
    assert_eq!(counts(),(8,8));
    assert_eq!(registry.dispatch(),Err(AffineCallbackError::RegistrationClosed));
    assert_eq!(registry.state_accesses,1);

    let mut torn_down=ForeignRegistry::new(9);
    torn_down.register(AffineCallback::new().unwrap().retain()).unwrap();
    assert_eq!(counts(),(10,8));
    torn_down.unregister();
    assert_eq!(counts(),(10,10));
    assert_eq!(torn_down.dispatch(),Err(AffineCallbackError::RegistrationClosed));
    assert_eq!(torn_down.state_accesses,0);
    assert_eq!(counts(),(10,10));
}
"#;

const CALL_FAILURE: &str = r#"
use affine_adapter::*;
extern "C" { fn affine_allocations()->u64; fn affine_frees()->u64; }
fn main() {
    let callback=AffineCallback::new().unwrap();
    assert_eq!(callback.call(), Err(AffineCallbackError::Call));
    unsafe { assert_eq!((affine_allocations(),affine_frees()),(2,2)); }
}
"#;
const CREATE_FAILURE: &str = r#"
use affine_adapter::*;
extern "C" { fn affine_allocations()->u64; fn affine_frees()->u64; }
fn main() {
    assert!(matches!(AffineCallback::new(),Err(AffineCallbackError::Creation)));
    unsafe { assert_eq!((affine_allocations(),affine_frees()),(2,2)); }
}
"#;
