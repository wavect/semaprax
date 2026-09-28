// Actual loader/authority/registry tests; no endpoint or interpreter claim.
use super::*;
use crate::desktop_api::NativeHost;
use semaprax::codegen::emit_native_adapter_admission;
use semaprax::hir::{self, DeclarationId};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const SOURCE: &str = r#"module test.fresh_token;
@id("token.type")
resource Token { @id("token.drop") drop trivial; }
@id("token.discard")
fn discard(value: own Token) -> i64 { 7 }
@id("test.main")
fn main() -> i64 { 0 }
"#;
static NEXT: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    directory: PathBuf,
    library: PathBuf,
    descriptor: Vec<u8>,
    getter: String,
}
impl Fixture {
    fn build() -> Self {
        let parsed = semaprax::parse(SOURCE, Path::new("fresh-token.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        let artifact = emit_native_adapter_admission(
            &resolved,
            &DeclarationId::new("token.discard"),
            "adapter.h",
        )
        .unwrap();
        let directory = std::env::temp_dir().join(format!(
            "semaprax-fresh-token-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let directory = fs::canonicalize(directory).unwrap();
        let source = directory.join("provider.c");
        let library = directory.join(if cfg!(target_os = "macos") {
            "provider.dylib"
        } else {
            "provider.so"
        });
        fs::write(directory.join("adapter.h"), artifact.header()).unwrap();
        fs::write(&source, artifact.provider_source()).unwrap();
        let mut command = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()));
        if cfg!(target_os = "macos") {
            command.args(["-dynamiclib", "-fPIC"]);
        } else {
            command.args(["-shared", "-fPIC"]);
        }
        let result = command
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-fvisibility=hidden",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&library)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        Self {
            directory,
            library: fs::canonicalize(library).unwrap(),
            descriptor: artifact.descriptor().to_vec(),
            getter: artifact.getter_symbol().to_owned(),
        }
    }
    fn open(&self) -> NativeHost {
        // SAFETY: Exact compiler-generated provider, canonical private path,
        // immutable selected getter bytes and synchronous non-unwinding ABI.
        unsafe {
            NativeHost::open_admitted_exact(&self.library, self.getter.as_bytes(), &self.descriptor)
        }
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

#[test]
fn native_fresh_token_real_acquisition_credential_and_exact_once_retirement() {
    let fixture = Fixture::build();
    let mut host = fixture.open();
    let mut first = host.acquire_fresh_local_token().unwrap();
    let second = host.acquire_fresh_local_token().unwrap();
    assert_eq!(host.live_owner_count(), 2);
    host.validate_fresh_local_token(&first).unwrap();
    host.validate_fresh_local_token(&second).unwrap();
    first.retire().unwrap();
    assert!(first.retire().is_err());
    assert_eq!(host.live_owner_count(), 1);
    drop(first);
    assert_eq!(host.live_owner_count(), 1);
    drop(second);
    assert_eq!(host.live_owner_count(), 0);
}

#[test]
fn native_fresh_token_credential_failure_rolls_back_real_registry_slot() {
    let fixture = Fixture::build();
    let mut host = fixture.open();
    assert!(matches!(
        host.test_fresh_credential_failure(),
        Err(FreshTokenError::Credential(_))
    ));
    assert_eq!(host.live_owner_count(), 0);
    let owner = host.acquire_fresh_local_token().unwrap();
    assert_eq!(owner.acquisition.as_ref().unwrap().slot(), 1);
    host.validate_fresh_local_token(&owner).unwrap();
    drop(owner);
    assert_eq!(host.live_owner_count(), 0);
}

#[test]
fn native_fresh_token_foreign_open_refuses_without_retiring_original_backing() {
    let fixture = Fixture::build();
    let mut one = fixture.open();
    let two = fixture.open();
    let owner = one.acquire_fresh_local_token().unwrap();
    assert!(matches!(
        two.validate_fresh_local_token(&owner),
        Err(FreshTokenError::ForeignHost)
    ));
    one.validate_fresh_local_token(&owner).unwrap();
    assert_eq!((one.live_owner_count(), two.live_owner_count()), (1, 0));
    drop(owner);
    assert_eq!(one.live_owner_count(), 0);
}

#[test]
fn native_fresh_token_draining_refuses_new_acquisition_but_retires_incurred_owner() {
    let fixture = Fixture::build();
    let mut host = fixture.open();
    let owner = host.acquire_fresh_local_token().unwrap();
    host.begin_draining();
    assert!(matches!(
        host.acquire_fresh_local_token(),
        Err(FreshTokenError::Draining)
    ));
    assert_eq!(host.live_owner_count(), 1);
    drop(owner);
    assert_eq!(host.live_owner_count(), 0);
}

#[test]
fn native_fresh_token_source_audit_joins_private_lexical_child_without_adoption_fallback() {
    let root = include_str!("../lib.rs");
    let child = include_str!("../session_endpoint.rs");
    assert!(root.contains("include!(\"session_endpoint.rs\");"));
    let joined = [root, child].join("\n");
    assert!(joined.contains("fn fresh_token_provenance("));
    assert!(joined.contains(".registry\n"));
    assert!(child.contains("authority.mint_owner("));
    assert!(!child.contains(".adopt_trusted_owner("));
    assert!(!child.contains("pub fn acquire_fresh_local_token"));
    assert!(!child.contains("pub fn open_endpoint"));
}
