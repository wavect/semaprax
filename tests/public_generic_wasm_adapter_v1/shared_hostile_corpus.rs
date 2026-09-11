//! Issue #160: the shared malformed-input/wrong-binding corpus, executed by
//! the generated TypeScript/Wasm calling consumer (#157) against the SAME
//! canonical descriptor baseline the native shared-corpus harness
//! (`tests/public_generic_native_adapter_v1/shared_hostile_corpus.rs`) feeds
//! to the Rust/C11/C++17 consumers, and cross-checked against the identical
//! manifest (`tests/support/public_generic_hostile_corpus.rs`).
//!
//! The native and Wasm harnesses are separate test binaries with disjoint
//! toolchain preconditions (clang/cargo vs. node/tsc) and cannot compare
//! outcomes inside one process. Real agreement is still enforced
//! transitively: this test's outcomes and the native trio's outcomes are
//! each asserted against the SAME `EXPECTED` table in the SAME on-disk
//! manifest file, so a route that diverges from the other three fails a
//! hard assertion in its own harness, not merely a hand-written local
//! approximation nobody else is compared against.
//!
//! Known gap, stated once here rather than papered over (see #229 and this
//! directory's own `typescript_calling_consumer.rs`): no compiled `.wasm`
//! artifact implements the Core Wasm provider ABI yet. This test runs the
//! SAME hand-assembled, clearly test-only `reference_wasm_module` the sibling
//! harness uses -- one real endpoint export over real `WebAssembly.Memory`,
//! never a second provider implementation -- so the TypeScript route here
//! cannot honestly be said to exercise a real provider ABI, only the
//! generated consumer's own codec/lifecycle logic against it. Its "provider"
//! zero-allocation counters are therefore the generated `wasm-provider.ts`'s
//! OWN host-side bookkeeping (`Provider.diagnostics.liveAllocations`),
//! standing in for a missing real provider's counters, not an independent
//! compiled provider's counters the way the native trio's
//! `spx_pg_consumer_test_live_allocations` genuinely is.
//!
//! For the same reason the descriptor-mutation cases are the one family
//! that is byte-for-byte identical with the native trio (the descriptor is
//! provider-family-agnostic); the binding-mutation case necessarily mutates
//! THIS route's own `WasmProviderBindingV1`-encoded bytes (a native binding
//! and a Wasm binding are different types with different content), applying
//! the identical mutation RECIPE (flip the last byte) to each route's own
//! valid binding rather than literally shared bytes.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use semaprax::public_generic_abi::wasm::binding::WasmProviderBindingV1;
use semaprax::public_generic_abi::wasm::provider::FIXTURE_ENDPOINT_EXPORT_NAME;
use semaprax::public_generic_consumer::rust_calling::{OwnedByteField, RecordShape};
use semaprax::public_generic_consumer::typescript_calling::generate_typescript_calling_consumer;

use super::reference_wasm_module;

#[path = "../support/public_generic_hostile_corpus.rs"]
mod public_generic_hostile_corpus;
use public_generic_hostile_corpus::{
    assert_matches_expected, parse_shared_corpus_lines, BASELINE_DESCRIPTOR_BYTES,
    MAX_BYTES_PER_LEAF,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn module_artifact_digest(wasm_bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    const DOMAIN: &[u8] = b"semaprax.public-generic-typescript-wasm-consumer.v1.module-artifact\0";
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    hasher.update((wasm_bytes.len() as u64).to_le_bytes());
    hasher.update(wasm_bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn fixture_binding(wasm_bytes: &[u8]) -> WasmProviderBindingV1 {
    WasmProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:6262626262626262626262626262626262626262626262626262626262626262",
            TargetProfile::CoreWasm,
            "runtime:core-wasm-fixture-issue-160-shared-corpus",
        ),
        module_artifact_digest(wasm_bytes),
        FIXTURE_ENDPOINT_EXPORT_NAME,
        "semaprax-0.4.1",
    )
}

fn shapes() -> (RecordShape, RecordShape) {
    let input = RecordShape::new(vec![OwnedByteField::new(
        "consumers.shared_hostile_corpus.leaf",
    )]);
    let output = input.clone();
    (input, output)
}

struct Workspace(PathBuf);

impl Workspace {
    fn new(label: &str) -> Self {
        let root = env::temp_dir().join(format!(
            "spx-pg-wasm-shared-hostile-corpus-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Mirrors `typescript_calling_consumer.rs::locate_tsc` exactly (kept
/// independent rather than shared: that file is a sibling harness module
/// this one must not otherwise depend on).
fn locate_tsc() -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = {
        let mut list = Vec::new();
        if let Some(explicit) = env::var_os("SPX_PG_TSC") {
            list.push(PathBuf::from(explicit));
        }
        list.push(PathBuf::from("tsc"));
        if let Some(home) = env::var_os("HOME") {
            let home = PathBuf::from(home);
            list.push(home.join("Library/pnpm/tsc"));
            list.push(home.join(".local/share/pnpm/tsc"));
        }
        list
    };
    for candidate in candidates {
        let output = Command::new(&candidate).arg("--version").output();
        if let Ok(output) = output {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                if version.contains("5.8.3") {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn write_generated_package(root: &Path, files: &[(String, String)]) {
    for (relative, contents) in files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
    }
}

fn run(command: &mut Command, label: &str) -> Output {
    command
        .output()
        .unwrap_or_else(|error| panic!("run {label}: {error}"))
}

/// Spliced into the generated `test/round-trip.mjs`'s `run()` function,
/// immediately before its fixed `if (failed > 0) {` tail, so every shared
/// case still counts toward that file's own `test()`/pass-fail bookkeeping
/// while ALSO printing one `SHARED_CORPUS <case_id> <STATUS>` line this
/// harness parses back out.
const TS_APPENDIX: &str = r#"
  await test("shared corpus: success_baseline", async () => {
    const provider = await Provider.open(wasmBytes);
    const input = sampleInput();
    const expected = sampleInput();
    let status = "OTHER";
    try {
      const output = provider.transform(input);
      assertReversed(output, expected);
      status = "ACCEPTED";
    } catch (error) {
      status = "TRANSFORM_REJECTED";
    }
    provider.close();
    console.log(`SHARED_CORPUS success_baseline ${status}`);
  });

  await test("shared corpus: descriptor_first_byte_flipped", async () => {
    const mutated = Uint8Array.from(TRUSTED_DESCRIPTOR_BYTES);
    mutated[0] ^= 0xff;
    let status = "OTHER";
    try {
      const provider = await Provider.open(wasmBytes, { descriptorBytes: mutated });
      provider.close();
      status = "ACCEPTED";
    } catch (error) {
      if (error instanceof SemapraxPublicGenericException) {
        if (error.detail.kind === "descriptor-rejected") status = "DESCRIPTOR_REJECTED";
        else if (error.detail.kind === "provider-mismatch") status = "PROVIDER_MISMATCH";
      }
    }
    console.log(`SHARED_CORPUS descriptor_first_byte_flipped ${status}`);
  });

  await test("shared corpus: binding_last_byte_flipped", async () => {
    const mutated = Uint8Array.from(TRUSTED_BINDING_BYTES);
    mutated[mutated.length - 1] ^= 0xff;
    let status = "OTHER";
    try {
      const provider = await Provider.open(wasmBytes, { bindingBytes: mutated });
      provider.close();
      status = "ACCEPTED";
    } catch (error) {
      if (error instanceof SemapraxPublicGenericException) {
        if (error.detail.kind === "descriptor-rejected") status = "DESCRIPTOR_REJECTED";
        else if (error.detail.kind === "provider-mismatch") status = "PROVIDER_MISMATCH";
      }
    }
    console.log(`SHARED_CORPUS binding_last_byte_flipped ${status}`);
  });

  await test("shared corpus: descriptor_names_different_document", async () => {
    const suffix = new TextEncoder().encode("-a-different-but-well-formed-descriptor");
    const different = new Uint8Array(TRUSTED_DESCRIPTOR_BYTES.length + suffix.length);
    different.set(TRUSTED_DESCRIPTOR_BYTES);
    different.set(suffix, TRUSTED_DESCRIPTOR_BYTES.length);
    let status = "OTHER";
    try {
      const provider = await Provider.open(wasmBytes, { descriptorBytes: different });
      provider.close();
      status = "ACCEPTED";
    } catch (error) {
      if (error instanceof SemapraxPublicGenericException) {
        if (error.detail.kind === "descriptor-rejected") status = "DESCRIPTOR_REJECTED";
        else if (error.detail.kind === "provider-mismatch") status = "PROVIDER_MISMATCH";
      }
    }
    console.log(`SHARED_CORPUS descriptor_names_different_document ${status}`);
  });

  await test("shared corpus: exactly_per_leaf_bound_accepted", async () => {
    const provider = await Provider.open(wasmBytes);
    const atBound = inputWithFirstField(new Uint8Array(65536).fill(0x5a));
    let status = "OTHER";
    try {
      provider.transform(atBound);
      status = "ACCEPTED";
    } catch (error) {
      if (error instanceof SemapraxPublicGenericException && error.detail.kind === "capacity-exceeded") {
        status = "CAPACITY_EXCEEDED";
      }
    }
    assert.equal(Provider.diagnostics.liveAllocations(provider), 0);
    provider.close();
    console.log(`SHARED_CORPUS exactly_per_leaf_bound_accepted ${status}`);
  });

  await test("shared corpus: one_byte_over_per_leaf_bound_rejected", async () => {
    const provider = await Provider.open(wasmBytes);
    const overBound = inputWithFirstField(new Uint8Array(65537).fill(0x5a));
    let status = "OTHER";
    try {
      provider.transform(overBound);
      status = "ACCEPTED";
    } catch (error) {
      if (error instanceof SemapraxPublicGenericException && error.detail.kind === "capacity-exceeded") {
        status = "CAPACITY_EXCEEDED";
      }
    }
    assert.equal(Provider.diagnostics.liveAllocations(provider), 0);
    provider.close();
    console.log(`SHARED_CORPUS one_byte_over_per_leaf_bound_rejected ${status}`);
  });

"#;

#[test]
fn shared_hostile_corpus_agrees_with_the_native_manifest() {
    // The TS-side per-leaf-bound cases below restate this literal (65536 /
    // 65537), exactly like the generated consumer's own TypeScript source
    // restates it rather than depending on the `semaprax` crate; this
    // harness CAN depend on `semaprax`, so it asserts the restated bound has
    // not drifted from the real constant, exactly like the native shared
    // corpus harness does.
    assert_eq!(
        MAX_BYTES_PER_LEAF,
        semaprax::public_generic_abi::boundary_profile::MAX_BYTES_PER_LEAF,
    );
    assert_eq!(MAX_BYTES_PER_LEAF, 65536);

    if !node_available() {
        eprintln!("skipping: node is not available on PATH");
        return;
    }
    let Some(tsc) = locate_tsc() else {
        eprintln!("skipping: no repository-pinned (5.8.3) tsc is available on this host");
        return;
    };

    let wasm_bytes = reference_wasm_module::build();
    let (input, output) = shapes();
    let binding = fixture_binding(&wasm_bytes);
    let consumer =
        generate_typescript_calling_consumer(BASELINE_DESCRIPTOR_BYTES, &binding, &input, &output)
            .expect("a well-formed shape must generate");

    let workspace = Workspace::new("execute");
    eprintln!(
        "shared hostile corpus TypeScript workspace: {}",
        workspace.0.display()
    );
    let package_root = workspace.path("generated-typescript-consumer");
    write_generated_package(&package_root, consumer.files());

    let round_trip_path = package_root.join("test/round-trip.mjs");
    let mut contents = fs::read_to_string(&round_trip_path).unwrap();
    let anchor = "  if (failed > 0) {";
    let position = contents.find(anchor).unwrap_or_else(|| {
        panic!("splice anchor {anchor:?} not found in generated round-trip.mjs")
    });
    contents.insert_str(position, TS_APPENDIX);
    fs::write(&round_trip_path, &contents).unwrap();

    let wasm_path = workspace.path("reference.wasm");
    fs::write(&wasm_path, &wasm_bytes).unwrap();

    let build_dist = run(
        Command::new(&tsc)
            .current_dir(&package_root)
            .args(["-p", "tsconfig.json"]),
        "tsc -p tsconfig.json",
    );
    assert!(
        build_dist.status.success(),
        "the generated package failed to type-check:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_dist.stdout),
        String::from_utf8_lossy(&build_dist.stderr)
    );

    let round_trip = run(
        Command::new("node")
            .current_dir(&package_root)
            .arg("test/round-trip.mjs")
            .arg(&wasm_path),
        "node test/round-trip.mjs",
    );
    let stdout = String::from_utf8_lossy(&round_trip.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&round_trip.stderr).into_owned();
    assert!(
        round_trip.status.success(),
        "the generated package's own shared-corpus run failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("FAIL -"),
        "no selected test may fail:\n{stdout}"
    );

    let outcomes = parse_shared_corpus_lines(&stdout);
    assert_matches_expected("typescript_calling_consumer", &outcomes);
}
