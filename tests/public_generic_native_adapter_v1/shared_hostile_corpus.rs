//! Issue #160: the shared malformed-input/wrong-binding corpus, executed
//! identically by the Rust (#156), C11 (#158), and C++17 (#159) generated
//! calling consumers against the SAME compiled native provider, and
//! cross-checked against ONE manifest
//! (`tests/support/public_generic_hostile_corpus.rs`) rather than three
//! independent hand-written approximations.
//!
//! Distinct from `rust_calling_consumer.rs`/`c_calling_consumer.rs`/
//! `cxx_calling_consumer.rs`: those harnesses run each generator's OWN
//! independently-authored hostile tests (already exercising a mutated
//! descriptor, a mutated binding, a wrong-descriptor pairing, and the
//! per-leaf byte bound -- see the coverage audit in this issue's report).
//! This module does not duplicate those; it adds one MORE test per
//! consumer, generated against a single shared baseline descriptor
//! (`public_generic_hostile_corpus::BASELINE_DESCRIPTOR_BYTES`), that prints
//! its observed outcome for each shared case rather than merely asserting it
//! locally -- so this file's own test can parse all three consumers' actual
//! outcomes and assert they AGREE with each other, not only that each one
//! independently thinks it is correct.
//!
//! How the shared test reaches each generated consumer: this harness cannot
//! edit `consumer.files()` (that is the generator's own deterministic,
//! contracted output -- `docs/PUBLIC-GENERIC-CONSUMERS-V1.md`'s "byte for
//! byte" claim would break if this harness altered it). Instead it appends
//! (Rust) or splices in front of the fixed `main`/settlement line (C, C++) a
//! hand-written test function defined ONLY in terms of that generated file's
//! own already-generated helpers (`sample_input`/`input_with_first_field`/
//! `assert_reversed`, the embedded `TRUSTED_DESCRIPTOR_BYTES`/
//! `TRUSTED_BINDING_BYTES`, and the provider's own test-only diagnostics) --
//! it never re-derives a generated field name itself (those are
//! `pub(crate)`-only inside `semaprax::public_generic_consumer`, unreachable
//! from this external integration-test crate by design).
//!
//! Zero-leak evidence, exactly like the sibling harnesses: every terminal
//! case checks the native provider's OWN test-only counters
//! (`spx_pg_consumer_test_live_allocations`/`_handles`, reached through each
//! consumer's own accessor), never a consumer's own bookkeeping.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use semaprax::public_generic_abi::native::binding::NativeProviderBindingV1;
use semaprax::public_generic_abi::native::template::render_reference_provider;
use semaprax::public_generic_consumer::c_calling::generate_c_calling_consumer;
use semaprax::public_generic_consumer::cxx_calling::generate_cxx_calling_consumer;
use semaprax::public_generic_consumer::rust_calling::{
    generate_rust_calling_consumer, OwnedByteField, RecordShape,
};

#[path = "../support/public_generic_hostile_corpus.rs"]
mod public_generic_hostile_corpus;
use public_generic_hostile_corpus::{
    assert_matches_expected, parse_shared_corpus_lines, BASELINE_DESCRIPTOR_BYTES,
    MAX_BYTES_PER_LEAF,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture_binding() -> NativeProviderBindingV1 {
    NativeProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:6060606060606060606060606060606060606060606060606060606060606060",
            TargetProfile::NativeC11,
            "runtime:native-c11-fixture-issue-160-shared-corpus",
        ),
        "sha256:6161616161616161616161616161616161616161616161616161616161616161",
        "spx_pg_endpoint_reverse_bytes_v1",
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

/// Issue #173's `binding_wrong_target_profile` case: a fully well-formed
/// [`NativeProviderBindingV1`] whose wrapped [`CarrierBindingV1`] names
/// `TargetProfile::CoreWasm` instead of the real `NativeC11` this route
/// actually is -- everything else matches [`fixture_binding`] exactly, so
/// only the target-profile confusion is under test. Real cross-runtime
/// interop (compiling a second Wasm module and literally sharing a binding
/// value across the two adapter crates in one process) is not attempted --
/// the native and Wasm calling-consumer routes are deliberately separate
/// test binaries with disjoint toolchain preconditions, exactly like the
/// per-ordinal failure matrices this corpus already declines to compare
/// literally across engines (see this file's own doc comment) -- so this
/// constructs a value a Wasm-side generator's own inputs *could* have
/// produced and proves the real native provider still rejects it, rather
/// than silently accepting a binding meant for a different runtime.
fn cross_target_binding() -> NativeProviderBindingV1 {
    NativeProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:6060606060606060606060606060606060606060606060606060606060606060",
            TargetProfile::CoreWasm,
            "runtime:native-c11-fixture-issue-160-shared-corpus",
        ),
        "sha256:6161616161616161616161616161616161616161616161616161616161616161",
        "spx_pg_endpoint_reverse_bytes_v1",
        "semaprax-0.4.1",
    )
}

/// Issue #173's `binding_valid_for_different_artifact` case: a fully
/// well-formed [`NativeProviderBindingV1`] -- same descriptor identity
/// digest, same `TargetProfile::NativeC11`, same runtime identity -- but a
/// DIFFERENT `provider_artifact_digest` and `exported_endpoint_symbol`, as
/// if minted for a genuinely different deployed provider rather than
/// corrupted. Proves the compiled provider's open-time check requires exact
/// agreement with ITS OWN trusted binding rather than accepting any
/// well-formed binding that merely names the right target profile.
fn cross_artifact_binding() -> NativeProviderBindingV1 {
    NativeProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:6060606060606060606060606060606060606060606060606060606060606060",
            TargetProfile::NativeC11,
            "runtime:native-c11-fixture-issue-160-shared-corpus",
        ),
        "sha256:9999999999999999999999999999999999999999999999999999999999999999",
        "spx_pg_endpoint_reverse_bytes_v1_different_artifact",
        "semaprax-0.4.1",
    )
}

/// Render `bytes` as a Rust `&[u8]` slice literal, e.g. `&[0x01u8,0x02]`,
/// for splicing a fixed byte value into generated Rust test source that
/// cannot depend on the `semaprax` crate to construct it itself.
fn rust_byte_slice_literal(bytes: &[u8]) -> String {
    let mut out = String::from("&[");
    for (index, byte) in bytes.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        write!(out, "0x{byte:02x}u8").unwrap();
    }
    out.push(']');
    out
}

/// Render `bytes` as a braced C/C++ initializer list, e.g. `{0x01,0x02}`,
/// for splicing a fixed byte value into generated C/C++ test source.
fn c_byte_array_literal(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "{0}".to_owned();
    }
    let mut out = String::from("{");
    for (index, byte) in bytes.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        write!(out, "0x{byte:02x}").unwrap();
    }
    out.push('}');
    out
}

struct Workspace(PathBuf);

impl Workspace {
    fn new(label: &str) -> Self {
        let root = env::temp_dir().join(format!(
            "spx-pg-shared-hostile-corpus-{}-{}-{label}",
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

fn tool(variable: &str, fallback: &str) -> PathBuf {
    env::var_os(variable).map_or_else(|| PathBuf::from(fallback), PathBuf::from)
}

fn run(command: &mut Command, label: &str) -> Output {
    command
        .output()
        .unwrap_or_else(|error| panic!("run {label}: {error}"))
}

fn compile_provider_object(
    root: &Path,
    descriptor_bytes: &[u8],
    binding: &NativeProviderBindingV1,
    clang: &Path,
) -> PathBuf {
    let provider_source = render_reference_provider(descriptor_bytes, binding);
    let source_path = root.join("provider.c");
    fs::write(&source_path, &provider_source).unwrap();
    let object_path = root.join("provider.o");
    let compiled = run(
        Command::new(clang)
            .current_dir(root)
            .args(["-std=c11", "-O1", "-Wall", "-Wextra", "-Werror", "-c"])
            .arg(&source_path)
            .arg("-o")
            .arg(&object_path),
        "compile provider.c",
    );
    assert!(
        compiled.status.success(),
        "compiling the shared native provider failed: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert!(object_path.is_file());
    object_path
}

/// Appended verbatim to the end of the generated `tests/round_trip.rs`: a
/// standalone `#[test]` using only that file's own already-in-scope helpers
/// and imports. The outer harness runs this test by a literal substring
/// filter on its exact name (`cargo test ... shared_hostile_corpus_prints_
/// its_observed_outcomes`), so issue #173's two cross-runtime/cross-artifact
/// cases are added as extra blocks INSIDE this same function rather than as
/// a second `#[test]` fn, whose name a substring filter would not match.
/// `__CROSS_TARGET_BINDING_BYTES__`/`__CROSS_ARTIFACT_BINDING_BYTES__` are
/// substituted with a literal `&[u8]` slice at test-build time (see
/// `rust_byte_slice_literal`) since the generated crate cannot depend on
/// `semaprax` to construct a [`NativeProviderBindingV1`] itself.
const RUST_APPENDIX: &str = r#"
#[test]
fn shared_hostile_corpus_prints_its_observed_outcomes() {
    // success_baseline
    {
        let status = match Provider::open(TRUSTED_DESCRIPTOR_BYTES, TRUSTED_BINDING_BYTES) {
            Ok(mut provider) => {
                let original = sample_input();
                let expected = sample_input();
                let result = match provider.transform(original) {
                    Ok(output) => {
                        assert_reversed(&output, &expected);
                        "ACCEPTED"
                    }
                    Err(_) => "TRANSFORM_REJECTED",
                };
                drop(provider);
                assert_eq!(
                    diagnostics::live_allocations(),
                    0,
                    "success_baseline leaked a native allocation after settlement"
                );
                result
            }
            Err(Error::DescriptorRejected(_)) => "DESCRIPTOR_REJECTED",
            Err(Error::ProviderMismatch(_)) => "PROVIDER_MISMATCH",
            Err(_) => "OTHER",
        };
        println!("SHARED_CORPUS success_baseline {status}");
    }

    // descriptor_first_byte_flipped
    {
        let allocations_before = diagnostics::live_allocations();
        let mut mutated = TRUSTED_DESCRIPTOR_BYTES.to_vec();
        mutated[0] ^= 0xff;
        let status = match Provider::open(&mutated, TRUSTED_BINDING_BYTES) {
            Ok(provider) => {
                drop(provider);
                "ACCEPTED"
            }
            Err(Error::DescriptorRejected(_)) => "DESCRIPTOR_REJECTED",
            Err(Error::ProviderMismatch(_)) => "PROVIDER_MISMATCH",
            Err(_) => "OTHER",
        };
        assert_eq!(
            diagnostics::live_allocations(),
            allocations_before,
            "descriptor_first_byte_flipped: a native allocation happened before rejection"
        );
        println!("SHARED_CORPUS descriptor_first_byte_flipped {status}");
    }

    // binding_last_byte_flipped
    {
        let allocations_before = diagnostics::live_allocations();
        let mut mutated = TRUSTED_BINDING_BYTES.to_vec();
        let last = mutated.len() - 1;
        mutated[last] ^= 0xff;
        let status = match Provider::open(TRUSTED_DESCRIPTOR_BYTES, &mutated) {
            Ok(provider) => {
                drop(provider);
                "ACCEPTED"
            }
            Err(Error::DescriptorRejected(_)) => "DESCRIPTOR_REJECTED",
            Err(Error::ProviderMismatch(_)) => "PROVIDER_MISMATCH",
            Err(_) => "OTHER",
        };
        assert_eq!(
            diagnostics::live_allocations(),
            allocations_before,
            "binding_last_byte_flipped: a native allocation happened before rejection"
        );
        println!("SHARED_CORPUS binding_last_byte_flipped {status}");
    }

    // descriptor_names_different_document
    {
        let mut different = TRUSTED_DESCRIPTOR_BYTES.to_vec();
        different.extend_from_slice(b"-a-different-but-well-formed-descriptor");
        let status = match Provider::open(&different, TRUSTED_BINDING_BYTES) {
            Ok(provider) => {
                drop(provider);
                "ACCEPTED"
            }
            Err(Error::DescriptorRejected(_)) => "DESCRIPTOR_REJECTED",
            Err(Error::ProviderMismatch(_)) => "PROVIDER_MISMATCH",
            Err(_) => "OTHER",
        };
        println!("SHARED_CORPUS descriptor_names_different_document {status}");
    }

    // exactly_per_leaf_bound_accepted
    {
        let mut provider =
            Provider::open(TRUSTED_DESCRIPTOR_BYTES, TRUSTED_BINDING_BYTES).expect("open");
        let at_bound = input_with_first_field(vec![0x5a; MAX_BYTES_PER_LEAF]);
        let expected = input_with_first_field(vec![0x5a; MAX_BYTES_PER_LEAF]);
        let status = match provider.transform(at_bound) {
            Ok(output) => {
                assert_reversed(&output, &expected);
                "ACCEPTED"
            }
            Err(Error::CapacityExceeded(_)) => "CAPACITY_EXCEEDED",
            Err(_) => "OTHER",
        };
        drop(provider);
        assert_eq!(
            diagnostics::live_allocations(),
            0,
            "exactly_per_leaf_bound_accepted leaked a native allocation"
        );
        println!("SHARED_CORPUS exactly_per_leaf_bound_accepted {status}");
    }

    // one_byte_over_per_leaf_bound_rejected
    {
        let mut provider =
            Provider::open(TRUSTED_DESCRIPTOR_BYTES, TRUSTED_BINDING_BYTES).expect("open");
        let over_bound = input_with_first_field(vec![0x5a; MAX_BYTES_PER_LEAF + 1]);
        let status = match provider.transform(over_bound) {
            Ok(_) => "ACCEPTED",
            Err(Error::CapacityExceeded(_)) => "CAPACITY_EXCEEDED",
            Err(_) => "OTHER",
        };
        drop(provider);
        assert_eq!(
            diagnostics::live_allocations(),
            0,
            "one_byte_over_per_leaf_bound_rejected leaked a native allocation"
        );
        println!("SHARED_CORPUS one_byte_over_per_leaf_bound_rejected {status}");
    }

    // binding_wrong_target_profile: a fully well-formed alternate binding
    // naming the OTHER route's target profile, not a corrupted byte string.
    {
        const CROSS_TARGET_BINDING: &[u8] = __CROSS_TARGET_BINDING_BYTES__;
        let allocations_before = diagnostics::live_allocations();
        let status = match Provider::open(TRUSTED_DESCRIPTOR_BYTES, CROSS_TARGET_BINDING) {
            Ok(provider) => {
                drop(provider);
                "ACCEPTED"
            }
            Err(Error::DescriptorRejected(_)) => "DESCRIPTOR_REJECTED",
            Err(Error::ProviderMismatch(_)) => "PROVIDER_MISMATCH",
            Err(_) => "OTHER",
        };
        assert_eq!(
            diagnostics::live_allocations(),
            allocations_before,
            "binding_wrong_target_profile: a native allocation happened before rejection"
        );
        println!("SHARED_CORPUS binding_wrong_target_profile {status}");
    }

    // binding_valid_for_different_artifact: a fully well-formed alternate
    // binding naming a different provider artifact digest and endpoint
    // symbol, not a corrupted byte string.
    {
        const CROSS_ARTIFACT_BINDING: &[u8] = __CROSS_ARTIFACT_BINDING_BYTES__;
        let allocations_before = diagnostics::live_allocations();
        let status = match Provider::open(TRUSTED_DESCRIPTOR_BYTES, CROSS_ARTIFACT_BINDING) {
            Ok(provider) => {
                drop(provider);
                "ACCEPTED"
            }
            Err(Error::DescriptorRejected(_)) => "DESCRIPTOR_REJECTED",
            Err(Error::ProviderMismatch(_)) => "PROVIDER_MISMATCH",
            Err(_) => "OTHER",
        };
        assert_eq!(
            diagnostics::live_allocations(),
            allocations_before,
            "binding_valid_for_different_artifact: a native allocation happened before rejection"
        );
        println!("SHARED_CORPUS binding_valid_for_different_artifact {status}");
    }
}
"#;

/// Spliced in before the generated `int main(void) { ... }` (C11's fixed
/// settlement runner) and called immediately before the final `puts`.
const C_APPENDIX_FN: &str = r#"static void test_shared_hostile_corpus(void) {
    /* success_baseline */
    {
        spx_pg_calling_consumer *consumer = NULL;
        spx_pg_consumer_status open_status = spx_pg_consumer_open(
            spx_pg_trusted_descriptor_bytes, spx_pg_trusted_descriptor_len,
            spx_pg_trusted_binding_bytes, spx_pg_trusted_binding_len, &consumer);
        const char *status = "OTHER";
        if (open_status == SPX_PG_CONSUMER_OK) {
            spx_pg_input input = sample_input();
            spx_pg_input expected = sample_input();
            spx_pg_output output;
            int native_status = -1;
            if (spx_pg_consumer_transform(consumer, &input, &output, &native_status) ==
                SPX_PG_CONSUMER_OK) {
                assert_reversed(&output, &expected);
                spx_pg_output_free(&output);
                status = "ACCEPTED";
            } else {
                status = "TRANSFORM_REJECTED";
            }
            free_input(&expected);
            spx_pg_consumer_close(&consumer);
            REQUIRE(spx_pg_consumer_test_live_allocations() == 0);
        } else if (open_status == SPX_PG_CONSUMER_DESCRIPTOR_REJECTED) {
            status = "DESCRIPTOR_REJECTED";
        } else if (open_status == SPX_PG_CONSUMER_PROVIDER_MISMATCH) {
            status = "PROVIDER_MISMATCH";
        }
        printf("SHARED_CORPUS success_baseline %s\n", status);
    }

    /* descriptor_first_byte_flipped */
    {
        size_t len = spx_pg_trusted_descriptor_len;
        uint8_t *mutated = (uint8_t *)malloc(len == 0 ? 1 : len);
        REQUIRE(mutated != NULL);
        if (len != 0) {
            memcpy(mutated, spx_pg_trusted_descriptor_bytes, len);
            mutated[0] ^= 0xffu;
        }
        size_t allocations_before = spx_pg_consumer_test_live_allocations();
        spx_pg_calling_consumer *consumer = NULL;
        spx_pg_consumer_status open_status = spx_pg_consumer_open(
            mutated, len, spx_pg_trusted_binding_bytes, spx_pg_trusted_binding_len, &consumer);
        const char *status = "OTHER";
        if (open_status == SPX_PG_CONSUMER_OK) {
            spx_pg_consumer_close(&consumer);
            status = "ACCEPTED";
        } else if (open_status == SPX_PG_CONSUMER_DESCRIPTOR_REJECTED) {
            status = "DESCRIPTOR_REJECTED";
        } else if (open_status == SPX_PG_CONSUMER_PROVIDER_MISMATCH) {
            status = "PROVIDER_MISMATCH";
        }
        REQUIRE(spx_pg_consumer_test_live_allocations() == allocations_before);
        free(mutated);
        printf("SHARED_CORPUS descriptor_first_byte_flipped %s\n", status);
    }

    /* binding_last_byte_flipped */
    {
        size_t len = spx_pg_trusted_binding_len;
        uint8_t *mutated = (uint8_t *)malloc(len == 0 ? 1 : len);
        REQUIRE(mutated != NULL);
        if (len != 0) {
            memcpy(mutated, spx_pg_trusted_binding_bytes, len);
            mutated[len - 1] ^= 0xffu;
        }
        size_t allocations_before = spx_pg_consumer_test_live_allocations();
        spx_pg_calling_consumer *consumer = NULL;
        spx_pg_consumer_status open_status = spx_pg_consumer_open(
            spx_pg_trusted_descriptor_bytes, spx_pg_trusted_descriptor_len, mutated, len,
            &consumer);
        const char *status = "OTHER";
        if (open_status == SPX_PG_CONSUMER_OK) {
            spx_pg_consumer_close(&consumer);
            status = "ACCEPTED";
        } else if (open_status == SPX_PG_CONSUMER_DESCRIPTOR_REJECTED) {
            status = "DESCRIPTOR_REJECTED";
        } else if (open_status == SPX_PG_CONSUMER_PROVIDER_MISMATCH) {
            status = "PROVIDER_MISMATCH";
        }
        REQUIRE(spx_pg_consumer_test_live_allocations() == allocations_before);
        free(mutated);
        printf("SHARED_CORPUS binding_last_byte_flipped %s\n", status);
    }

    /* descriptor_names_different_document */
    {
        static const char suffix[] = "-a-different-but-well-formed-descriptor";
        size_t len = spx_pg_trusted_descriptor_len + (sizeof(suffix) - 1);
        uint8_t *different = (uint8_t *)malloc(len);
        REQUIRE(different != NULL);
        if (spx_pg_trusted_descriptor_len != 0) {
            memcpy(different, spx_pg_trusted_descriptor_bytes, spx_pg_trusted_descriptor_len);
        }
        memcpy(different + spx_pg_trusted_descriptor_len, suffix, sizeof(suffix) - 1);
        spx_pg_calling_consumer *consumer = NULL;
        spx_pg_consumer_status open_status = spx_pg_consumer_open(
            different, len, spx_pg_trusted_binding_bytes, spx_pg_trusted_binding_len, &consumer);
        const char *status = "OTHER";
        if (open_status == SPX_PG_CONSUMER_OK) {
            spx_pg_consumer_close(&consumer);
            status = "ACCEPTED";
        } else if (open_status == SPX_PG_CONSUMER_DESCRIPTOR_REJECTED) {
            status = "DESCRIPTOR_REJECTED";
        } else if (open_status == SPX_PG_CONSUMER_PROVIDER_MISMATCH) {
            status = "PROVIDER_MISMATCH";
        }
        free(different);
        printf("SHARED_CORPUS descriptor_names_different_document %s\n", status);
    }

    /* binding_wrong_target_profile: a fully well-formed alternate binding
     * naming the OTHER route's target profile, not a corrupted byte string.
     */
    {
        static const uint8_t cross_target_binding[] = __CROSS_TARGET_BINDING_BYTES__;
        size_t len = sizeof(cross_target_binding);
        size_t allocations_before = spx_pg_consumer_test_live_allocations();
        spx_pg_calling_consumer *consumer = NULL;
        spx_pg_consumer_status open_status = spx_pg_consumer_open(
            spx_pg_trusted_descriptor_bytes, spx_pg_trusted_descriptor_len, cross_target_binding,
            len, &consumer);
        const char *status = "OTHER";
        if (open_status == SPX_PG_CONSUMER_OK) {
            spx_pg_consumer_close(&consumer);
            status = "ACCEPTED";
        } else if (open_status == SPX_PG_CONSUMER_DESCRIPTOR_REJECTED) {
            status = "DESCRIPTOR_REJECTED";
        } else if (open_status == SPX_PG_CONSUMER_PROVIDER_MISMATCH) {
            status = "PROVIDER_MISMATCH";
        }
        REQUIRE(spx_pg_consumer_test_live_allocations() == allocations_before);
        printf("SHARED_CORPUS binding_wrong_target_profile %s\n", status);
    }

    /* binding_valid_for_different_artifact: a fully well-formed alternate
     * binding naming a different provider artifact digest and endpoint
     * symbol, not a corrupted byte string. */
    {
        static const uint8_t cross_artifact_binding[] = __CROSS_ARTIFACT_BINDING_BYTES__;
        size_t len = sizeof(cross_artifact_binding);
        size_t allocations_before = spx_pg_consumer_test_live_allocations();
        spx_pg_calling_consumer *consumer = NULL;
        spx_pg_consumer_status open_status = spx_pg_consumer_open(
            spx_pg_trusted_descriptor_bytes, spx_pg_trusted_descriptor_len, cross_artifact_binding,
            len, &consumer);
        const char *status = "OTHER";
        if (open_status == SPX_PG_CONSUMER_OK) {
            spx_pg_consumer_close(&consumer);
            status = "ACCEPTED";
        } else if (open_status == SPX_PG_CONSUMER_DESCRIPTOR_REJECTED) {
            status = "DESCRIPTOR_REJECTED";
        } else if (open_status == SPX_PG_CONSUMER_PROVIDER_MISMATCH) {
            status = "PROVIDER_MISMATCH";
        }
        REQUIRE(spx_pg_consumer_test_live_allocations() == allocations_before);
        printf("SHARED_CORPUS binding_valid_for_different_artifact %s\n", status);
    }

    /* exactly_per_leaf_bound_accepted */
    {
        spx_pg_calling_consumer *consumer = NULL;
        REQUIRE(spx_pg_consumer_open(spx_pg_trusted_descriptor_bytes,
                                      spx_pg_trusted_descriptor_len,
                                      spx_pg_trusted_binding_bytes, spx_pg_trusted_binding_len,
                                      &consumer) == SPX_PG_CONSUMER_OK);
        uint8_t *data = (uint8_t *)malloc(MAX_BYTES_PER_LEAF);
        REQUIRE(data != NULL);
        memset(data, 0x5a, MAX_BYTES_PER_LEAF);
        spx_pg_input input = input_with_first_field(data, MAX_BYTES_PER_LEAF);
        spx_pg_output output;
        spx_pg_consumer_status transform_status =
            spx_pg_consumer_transform(consumer, &input, &output, NULL);
        const char *status = "OTHER";
        if (transform_status == SPX_PG_CONSUMER_OK) {
            spx_pg_output_free(&output);
            status = "ACCEPTED";
        } else if (transform_status == SPX_PG_CONSUMER_CAPACITY_EXCEEDED) {
            status = "CAPACITY_EXCEEDED";
        }
        spx_pg_consumer_close(&consumer);
        REQUIRE(spx_pg_consumer_test_live_allocations() == 0);
        printf("SHARED_CORPUS exactly_per_leaf_bound_accepted %s\n", status);
    }

    /* one_byte_over_per_leaf_bound_rejected */
    {
        spx_pg_calling_consumer *consumer = NULL;
        REQUIRE(spx_pg_consumer_open(spx_pg_trusted_descriptor_bytes,
                                      spx_pg_trusted_descriptor_len,
                                      spx_pg_trusted_binding_bytes, spx_pg_trusted_binding_len,
                                      &consumer) == SPX_PG_CONSUMER_OK);
        uint8_t *data = (uint8_t *)malloc(MAX_BYTES_PER_LEAF + 1);
        REQUIRE(data != NULL);
        memset(data, 0x5a, MAX_BYTES_PER_LEAF + 1);
        spx_pg_input input = input_with_first_field(data, MAX_BYTES_PER_LEAF + 1);
        spx_pg_output output;
        spx_pg_consumer_status transform_status =
            spx_pg_consumer_transform(consumer, &input, &output, NULL);
        const char *status = "OTHER";
        if (transform_status == SPX_PG_CONSUMER_OK) {
            spx_pg_output_free(&output);
            status = "ACCEPTED";
        } else if (transform_status == SPX_PG_CONSUMER_CAPACITY_EXCEEDED) {
            status = "CAPACITY_EXCEEDED";
        }
        spx_pg_consumer_close(&consumer);
        REQUIRE(spx_pg_consumer_test_live_allocations() == 0);
        printf("SHARED_CORPUS one_byte_over_per_leaf_bound_rejected %s\n", status);
    }
}

"#;

/// Spliced in before the generated `int main() { ... }` (C++17's fixed
/// settlement runner) and called immediately before the final `puts`. Reuses
/// aggregate initialization (`Input{ vector }`) for the one-field `Input`
/// this harness's shape always produces, exactly like a caller who does not
/// know (and must not need to know) the generated field's name.
const CXX_APPENDIX_FN: &str = r#"static void test_shared_hostile_corpus() {
    /* success_baseline */
    {
        auto opened = Provider::open();
        const char *status = "OTHER";
        if (opened.has_value()) {
            Provider provider = std::move(opened).value();
            Input original = sample_input();
            Input to_send = sample_input();
            auto result = provider.transform(std::move(to_send));
            if (result.has_value()) {
                assert_reversed(result.value(), original);
                status = "ACCEPTED";
            } else {
                status = "TRANSFORM_REJECTED";
            }
            provider.close();
            REQUIRE(::spx_pg_consumer_test_live_allocations() == 0);
        } else if (opened.error().kind() == ErrorKind::DescriptorRejected) {
            status = "DESCRIPTOR_REJECTED";
        } else if (opened.error().kind() == ErrorKind::ProviderMismatch) {
            status = "PROVIDER_MISMATCH";
        }
        std::printf("SHARED_CORPUS success_baseline %s\n", status);
    }

    /* descriptor_first_byte_flipped */
    {
        std::vector<std::uint8_t> mutated(::spx_pg_trusted_descriptor_bytes,
                                           ::spx_pg_trusted_descriptor_bytes +
                                               ::spx_pg_trusted_descriptor_len);
        if (!mutated.empty()) {
            mutated[0] = static_cast<std::uint8_t>(mutated[0] ^ 0xffu);
        }
        std::size_t allocations_before = ::spx_pg_consumer_test_live_allocations();
        auto opened = Provider::open(mutated.data(), mutated.size(),
                                      ::spx_pg_trusted_binding_bytes,
                                      ::spx_pg_trusted_binding_len);
        const char *status = "OTHER";
        if (opened.has_value()) {
            Provider provider = std::move(opened).value();
            provider.close();
            status = "ACCEPTED";
        } else if (opened.error().kind() == ErrorKind::DescriptorRejected) {
            status = "DESCRIPTOR_REJECTED";
        } else if (opened.error().kind() == ErrorKind::ProviderMismatch) {
            status = "PROVIDER_MISMATCH";
        }
        REQUIRE(::spx_pg_consumer_test_live_allocations() == allocations_before);
        std::printf("SHARED_CORPUS descriptor_first_byte_flipped %s\n", status);
    }

    /* binding_last_byte_flipped */
    {
        std::vector<std::uint8_t> mutated(::spx_pg_trusted_binding_bytes,
                                           ::spx_pg_trusted_binding_bytes +
                                               ::spx_pg_trusted_binding_len);
        if (!mutated.empty()) {
            mutated.back() = static_cast<std::uint8_t>(mutated.back() ^ 0xffu);
        }
        std::size_t allocations_before = ::spx_pg_consumer_test_live_allocations();
        auto opened = Provider::open(::spx_pg_trusted_descriptor_bytes,
                                      ::spx_pg_trusted_descriptor_len, mutated.data(),
                                      mutated.size());
        const char *status = "OTHER";
        if (opened.has_value()) {
            Provider provider = std::move(opened).value();
            provider.close();
            status = "ACCEPTED";
        } else if (opened.error().kind() == ErrorKind::DescriptorRejected) {
            status = "DESCRIPTOR_REJECTED";
        } else if (opened.error().kind() == ErrorKind::ProviderMismatch) {
            status = "PROVIDER_MISMATCH";
        }
        REQUIRE(::spx_pg_consumer_test_live_allocations() == allocations_before);
        std::printf("SHARED_CORPUS binding_last_byte_flipped %s\n", status);
    }

    /* descriptor_names_different_document */
    {
        static const char suffix[] = "-a-different-but-well-formed-descriptor";
        std::vector<std::uint8_t> different(::spx_pg_trusted_descriptor_bytes,
                                             ::spx_pg_trusted_descriptor_bytes +
                                                 ::spx_pg_trusted_descriptor_len);
        different.insert(different.end(), suffix, suffix + (sizeof(suffix) - 1));
        auto opened = Provider::open(different.data(), different.size(),
                                      ::spx_pg_trusted_binding_bytes,
                                      ::spx_pg_trusted_binding_len);
        const char *status = "OTHER";
        if (opened.has_value()) {
            Provider provider = std::move(opened).value();
            provider.close();
            status = "ACCEPTED";
        } else if (opened.error().kind() == ErrorKind::DescriptorRejected) {
            status = "DESCRIPTOR_REJECTED";
        } else if (opened.error().kind() == ErrorKind::ProviderMismatch) {
            status = "PROVIDER_MISMATCH";
        }
        std::printf("SHARED_CORPUS descriptor_names_different_document %s\n", status);
    }

    /* binding_wrong_target_profile: a fully well-formed alternate binding
     * naming the OTHER route's target profile, not a corrupted byte string.
     */
    {
        std::vector<std::uint8_t> cross_target_binding __CROSS_TARGET_BINDING_BYTES__;
        std::size_t allocations_before = ::spx_pg_consumer_test_live_allocations();
        auto opened = Provider::open(::spx_pg_trusted_descriptor_bytes,
                                      ::spx_pg_trusted_descriptor_len, cross_target_binding.data(),
                                      cross_target_binding.size());
        const char *status = "OTHER";
        if (opened.has_value()) {
            Provider provider = std::move(opened).value();
            provider.close();
            status = "ACCEPTED";
        } else if (opened.error().kind() == ErrorKind::DescriptorRejected) {
            status = "DESCRIPTOR_REJECTED";
        } else if (opened.error().kind() == ErrorKind::ProviderMismatch) {
            status = "PROVIDER_MISMATCH";
        }
        REQUIRE(::spx_pg_consumer_test_live_allocations() == allocations_before);
        std::printf("SHARED_CORPUS binding_wrong_target_profile %s\n", status);
    }

    /* binding_valid_for_different_artifact: a fully well-formed alternate
     * binding naming a different provider artifact digest and endpoint
     * symbol, not a corrupted byte string. */
    {
        std::vector<std::uint8_t> cross_artifact_binding __CROSS_ARTIFACT_BINDING_BYTES__;
        std::size_t allocations_before = ::spx_pg_consumer_test_live_allocations();
        auto opened =
            Provider::open(::spx_pg_trusted_descriptor_bytes, ::spx_pg_trusted_descriptor_len,
                            cross_artifact_binding.data(), cross_artifact_binding.size());
        const char *status = "OTHER";
        if (opened.has_value()) {
            Provider provider = std::move(opened).value();
            provider.close();
            status = "ACCEPTED";
        } else if (opened.error().kind() == ErrorKind::DescriptorRejected) {
            status = "DESCRIPTOR_REJECTED";
        } else if (opened.error().kind() == ErrorKind::ProviderMismatch) {
            status = "PROVIDER_MISMATCH";
        }
        REQUIRE(::spx_pg_consumer_test_live_allocations() == allocations_before);
        std::printf("SHARED_CORPUS binding_valid_for_different_artifact %s\n", status);
    }

    /* exactly_per_leaf_bound_accepted */
    {
        auto opened = Provider::open();
        REQUIRE(opened.has_value());
        Provider provider = std::move(opened).value();
        Input input{std::vector<std::uint8_t>(MAX_BYTES_PER_LEAF, 0x5au)};
        auto result = provider.transform(std::move(input));
        const char *status = "OTHER";
        if (result.has_value()) {
            status = "ACCEPTED";
        } else if (result.error().kind() == ErrorKind::CapacityExceeded) {
            status = "CAPACITY_EXCEEDED";
        }
        provider.close();
        REQUIRE(::spx_pg_consumer_test_live_allocations() == 0);
        std::printf("SHARED_CORPUS exactly_per_leaf_bound_accepted %s\n", status);
    }

    /* one_byte_over_per_leaf_bound_rejected */
    {
        auto opened = Provider::open();
        REQUIRE(opened.has_value());
        Provider provider = std::move(opened).value();
        Input input{std::vector<std::uint8_t>(MAX_BYTES_PER_LEAF + 1, 0x5au)};
        auto result = provider.transform(std::move(input));
        const char *status = "OTHER";
        if (result.has_value()) {
            status = "ACCEPTED";
        } else if (result.error().kind() == ErrorKind::CapacityExceeded) {
            status = "CAPACITY_EXCEEDED";
        }
        provider.close();
        REQUIRE(::spx_pg_consumer_test_live_allocations() == 0);
        std::printf("SHARED_CORPUS one_byte_over_per_leaf_bound_rejected %s\n", status);
    }
}

"#;

fn write_generated_files(root: &Path, files: &[(String, String)], splice: Option<(&str, &str)>) {
    for (relative, contents) in files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut contents = contents.clone();
        if let Some((anchor, appendix)) = splice {
            if let Some(position) = contents.find(anchor) {
                contents.insert_str(position, appendix);
            }
        }
        fs::write(&path, &contents).unwrap();
    }
}

/// Locate the exact anchor text `write_generated_files` splices in front of,
/// and the exact call site the appended function needs registered at, for a
/// C/C++-style fixed-`main` round trip. Panics loudly (never silently
/// no-ops) if either anchor is not found in the generated content -- a
/// missing anchor means this splice has drifted from the generator's actual
/// output and must be re-synced, never silently skipped.
fn splice_main_call(contents: &mut String, call_before: &str, call: &str) {
    let position = contents
        .find(call_before)
        .unwrap_or_else(|| panic!("splice anchor {call_before:?} not found in generated main"));
    contents.insert_str(position, call);
}

#[test]
fn shared_hostile_corpus_agrees_across_rust_c11_and_cxx17_consumers() {
    // The corpus manifest restates `MAX_BYTES_PER_LEAF` (a generated
    // artifact cannot depend on the `semaprax` crate), exactly like every
    // generated consumer already restates it; this harness CAN depend on
    // `semaprax`, so it asserts the restated literal has not drifted from
    // the real constant it stands in for.
    assert_eq!(
        MAX_BYTES_PER_LEAF,
        semaprax::public_generic_abi::boundary_profile::MAX_BYTES_PER_LEAF,
        "the shared corpus's restated per-leaf bound has drifted from the real constant"
    );

    let clang = tool("CLANG", "clang");
    let (input, output) = shapes();
    let binding = fixture_binding();

    // Issue #173: the byte literals for `binding_wrong_target_profile` and
    // `binding_valid_for_different_artifact` are computed once here (this
    // harness CAN depend on `semaprax`) and spliced into each generated
    // language's own test source as a fixed literal, exactly like the
    // trusted descriptor/binding constants the generators themselves embed.
    let cross_target_binding_bytes = cross_target_binding().encode();
    let cross_artifact_binding_bytes = cross_artifact_binding().encode();
    let rust_appendix = RUST_APPENDIX
        .replace(
            "__CROSS_TARGET_BINDING_BYTES__",
            &rust_byte_slice_literal(&cross_target_binding_bytes),
        )
        .replace(
            "__CROSS_ARTIFACT_BINDING_BYTES__",
            &rust_byte_slice_literal(&cross_artifact_binding_bytes),
        );
    let c_appendix_fn = C_APPENDIX_FN
        .replace(
            "__CROSS_TARGET_BINDING_BYTES__",
            &c_byte_array_literal(&cross_target_binding_bytes),
        )
        .replace(
            "__CROSS_ARTIFACT_BINDING_BYTES__",
            &c_byte_array_literal(&cross_artifact_binding_bytes),
        );
    let cxx_appendix_fn = CXX_APPENDIX_FN
        .replace(
            "__CROSS_TARGET_BINDING_BYTES__",
            &c_byte_array_literal(&cross_target_binding_bytes),
        )
        .replace(
            "__CROSS_ARTIFACT_BINDING_BYTES__",
            &c_byte_array_literal(&cross_artifact_binding_bytes),
        );

    let workspace = Workspace::new("shared-corpus");
    eprintln!("shared hostile corpus workspace: {}", workspace.0.display());
    let provider_object =
        compile_provider_object(&workspace.0, BASELINE_DESCRIPTOR_BYTES, &binding, &clang);

    // ---- Rust ----
    let rust_consumer =
        generate_rust_calling_consumer(BASELINE_DESCRIPTOR_BYTES, &binding, &input, &output)
            .expect("a well-formed shape must generate");
    let rust_root = workspace.path("rust-consumer");
    for (relative, contents) in rust_consumer.files() {
        let path = rust_root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut contents = contents.clone();
        if relative == "tests/round_trip.rs" {
            contents.push_str(&rust_appendix);
        }
        fs::write(&path, &contents).unwrap();
    }
    let lib_dir = {
        let lib_dir = workspace.path("provider-lib");
        fs::create_dir_all(&lib_dir).unwrap();
        let archive_path = lib_dir.join("libspx_pg_reference_provider.a");
        let archiver = tool("AR", "ar");
        let archived = run(
            Command::new(&archiver)
                .arg("rcs")
                .arg(&archive_path)
                .arg(&provider_object),
            "archive the shared provider object",
        );
        assert!(
            archived.status.success(),
            "archiving the shared native provider failed: {}",
            String::from_utf8_lossy(&archived.stderr)
        );
        lib_dir
    };
    let rust_target_dir = workspace.path("rust-cargo-target");
    let lockfile = run(
        Command::new(tool("CARGO", "cargo"))
            .current_dir(&rust_root)
            .env("CARGO_TARGET_DIR", &rust_target_dir)
            .arg("generate-lockfile"),
        "cargo generate-lockfile",
    );
    assert!(
        lockfile.status.success(),
        "generate-lockfile: {}",
        String::from_utf8_lossy(&lockfile.stderr)
    );
    let rust_test = run(
        Command::new(tool("CARGO", "cargo"))
            .current_dir(&rust_root)
            .env("CARGO_TARGET_DIR", &rust_target_dir)
            .env("SPX_PG_PROVIDER_LIB_DIR", &lib_dir)
            .env("SPX_PG_PROVIDER_LIB_NAME", "spx_pg_reference_provider")
            .env_remove("RUSTC_WRAPPER")
            .args([
                "test",
                "--locked",
                "--test",
                "round_trip",
                "shared_hostile_corpus_prints_its_observed_outcomes",
                "--",
                "--test-threads=1",
                "--nocapture",
            ]),
        "cargo test (shared corpus)",
    );
    let rust_stdout = String::from_utf8_lossy(&rust_test.stdout).into_owned();
    assert!(
        rust_test.status.success(),
        "the generated Rust consumer's shared-corpus test failed:\nstdout:\n{rust_stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&rust_test.stderr)
    );
    let rust_outcomes = parse_shared_corpus_lines(&rust_stdout);

    // ---- C11 ----
    let c_consumer =
        generate_c_calling_consumer(BASELINE_DESCRIPTOR_BYTES, &binding, &input, &output)
            .expect("a well-formed shape must generate");
    let c_root = workspace.path("c-consumer");
    write_generated_files(&c_root, c_consumer.files(), None);
    // Splice the new test function in front of `int main(void) {`, then
    // register a call to it immediately before the final settlement `puts`.
    let c_round_trip_path = c_root.join("round_trip.c");
    let mut c_contents = fs::read_to_string(&c_round_trip_path).unwrap();
    splice_main_call(&mut c_contents, "int main(void) {", &c_appendix_fn);
    splice_main_call(
        &mut c_contents,
        "(void)puts(\"c-calling-consumer-settled\");",
        "test_shared_hostile_corpus();\n    ",
    );
    fs::write(&c_round_trip_path, &c_contents).unwrap();

    let c_executable = c_root.join("shared_corpus_probe");
    let c_built = run(
        Command::new(&clang)
            .current_dir(&c_root)
            .args(["-std=c11", "-O0", "-Wall", "-Wextra", "-Werror"])
            .arg("spx_pg_calling_consumer.c")
            .arg("round_trip.c")
            .arg(&provider_object)
            .arg("-o")
            .arg(&c_executable),
        "compile the C shared-corpus probe",
    );
    assert!(
        c_built.status.success(),
        "{}",
        String::from_utf8_lossy(&c_built.stderr)
    );
    assert!(
        c_built.stderr.is_empty(),
        "warning-free build required: {}",
        String::from_utf8_lossy(&c_built.stderr)
    );
    let c_run = run(
        Command::new(&c_executable).current_dir(&c_root),
        "run the C shared-corpus probe",
    );
    let c_stdout = String::from_utf8_lossy(&c_run.stdout).into_owned();
    assert!(
        c_run.status.success(),
        "the generated C11 consumer's shared-corpus probe failed:\nstdout:\n{c_stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&c_run.stderr)
    );
    let c_outcomes = parse_shared_corpus_lines(&c_stdout);

    // ---- C++17 ----
    let cxx_consumer =
        generate_cxx_calling_consumer(BASELINE_DESCRIPTOR_BYTES, &binding, &input, &output)
            .expect("a well-formed shape must generate");
    let cxx_root = workspace.path("cxx-consumer");
    write_generated_files(&cxx_root, cxx_consumer.files(), None);
    let cxx_round_trip_path = cxx_root.join("test/round_trip.cpp");
    let mut cxx_contents = fs::read_to_string(&cxx_round_trip_path).unwrap();
    splice_main_call(&mut cxx_contents, "int main() {", &cxx_appendix_fn);
    splice_main_call(
        &mut cxx_contents,
        "std::puts(\"cxx-calling-consumer-settled\");",
        "test_shared_hostile_corpus();\n    ",
    );
    fs::write(&cxx_round_trip_path, &cxx_contents).unwrap();

    let cxx_consumer_object = cxx_root.join("spx_pg_calling_consumer.o");
    let cxx_c_built = run(
        Command::new(&clang)
            .current_dir(&cxx_root)
            .args(["-std=c11", "-O0", "-Wall", "-Wextra", "-Werror", "-c"])
            .arg("spx_pg_calling_consumer.c")
            .arg("-o")
            .arg(&cxx_consumer_object),
        "compile the C11 consumer object for the C++17 probe",
    );
    assert!(
        cxx_c_built.status.success(),
        "{}",
        String::from_utf8_lossy(&cxx_c_built.stderr)
    );
    let clangxx = tool("CLANGXX", "clang++");
    let cxx_executable = cxx_root.join("shared_corpus_probe");
    let cxx_built = run(
        Command::new(&clangxx)
            .current_dir(&cxx_root)
            .args([
                "-std=c++17",
                "-O0",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Iinclude",
                "-I.",
            ])
            .arg("test/round_trip.cpp")
            .arg(&cxx_consumer_object)
            .arg(&provider_object)
            .arg("-o")
            .arg(&cxx_executable),
        "compile the C++17 shared-corpus probe",
    );
    assert!(
        cxx_built.status.success(),
        "{}",
        String::from_utf8_lossy(&cxx_built.stderr)
    );
    assert!(
        cxx_built.stderr.is_empty(),
        "warning-free build required: {}",
        String::from_utf8_lossy(&cxx_built.stderr)
    );
    let cxx_run = run(
        Command::new(&cxx_executable).current_dir(&cxx_root),
        "run the C++17 shared-corpus probe",
    );
    let cxx_stdout = String::from_utf8_lossy(&cxx_run.stdout).into_owned();
    assert!(
        cxx_run.status.success(),
        "the generated C++17 consumer's shared-corpus probe failed:\nstdout:\n{cxx_stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&cxx_run.stderr)
    );
    let cxx_outcomes = parse_shared_corpus_lines(&cxx_stdout);

    // ---- Cross-consumer agreement: each route's own actual outcomes, all
    // checked against the SAME shared manifest, so a route that diverges
    // (e.g. one language accepting what the others reject) fails here. ----
    assert_matches_expected("rust_calling_consumer", &rust_outcomes);
    assert_matches_expected("c_calling_consumer", &c_outcomes);
    assert_matches_expected("cxx_calling_consumer", &cxx_outcomes);
}
