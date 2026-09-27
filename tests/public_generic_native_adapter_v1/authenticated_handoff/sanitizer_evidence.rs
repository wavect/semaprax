//! LOCAL Clang AddressSanitizer + UndefinedBehaviorSanitizer evidence for the
//! native authenticated hostile corpus and positive controls of all four
//! private profiles (identity-v1, moves-v1, allocating-v1 and
//! moves-nested.v1; issue #292 / #288 follow-on). This is **LOCAL evidence on
//! macOS arm64 only**: it runs on whatever `clang`/`clang++` this developer
//! machine provides (Apple Clang, confirmed to support
//! `-fsanitize=address,undefined` for ordinary, unsigned C/C++ binaries with
//! no extra provisioning), never in hosted/Linux CI, and it is not the
//! `rust-host-address-sanitizer`/`callable-host-sanitizers` hosted lanes
//! [`docs/RUST-HOST-SANITIZERS.md`](../../../docs/RUST-HOST-SANITIZERS.md)
//! documents. It reuses this harness's own existing corpus builders
//! ([`super::caller_hostility::run_identity_corpus`] and
//! [`super::profile_hostility::run_profile`]) rather than a second, drifting
//! copy of their shape/descriptor/provider assembly, now compiled and run
//! through [`super::caller_hostility::run_sanitized`]'s sanitized path.
//!
//! **LeakSanitizer is unsupported on this platform.** An empirical probe
//! (`ASAN_OPTIONS=detect_leaks=1` against an otherwise-clean binary) aborts
//! immediately with `AddressSanitizer: detect_leaks is not supported on this
//! platform`, confirmed on this exact machine (Apple Clang, arm64-apple-
//! darwin). Leak detection is therefore not requested here; the existing
//! `auth_live`/`fixture_live` live-allocation counters every one of these
//! fixtures already asserts back to the exact baseline (and to zero at close)
//! remain the leak/settlement oracle, exactly as they are for the
//! unsanitized runs this module reuses.
//!
//! **Fail-closed proof.** [`negative_control_one_byte_heap_overflow_is_caught_and_reverted`]
//! shrinks a test-only provider copy's own allocator (`allocations.c`'s
//! `fixture_malloc`, `#include`d ahead of every rendered provider in this
//! harness) by exactly one byte relative to what it reports allocating, an
//! in-memory mutation of the compiled *text* only -- `allocations.c` itself,
//! and every other file this module reads, is never written to. The
//! generated provider's own real codec still writes the full, correct byte
//! count into that now-one-byte-short heap block, a genuine one-byte
//! heap-buffer-overflow. AddressSanitizer must report it and the process
//! must not exit with its ordinary settled status; the same clean fixture
//! with the mutation reverted (the default, unmutated `allocations.c` every
//! other test in this module compiles) produces zero sanitizer reports.
use super::*;

// Anchored on `REQUIRE(size != 0)`, unique to `allocations.c`'s own
// `fixture_malloc` (the test-only fixture allocator): the rendered
// provider's OWN production bounded allocator
// (`src/public_generic_abi/native/provider_body.c`) contains the bare
// `void *pointer = malloc(size);` line too, so an unanchored match is
// ambiguous and must never silently mutate the production allocator.
const MALLOC_CALL: &str = "REQUIRE(size != 0);\n    void *pointer = malloc(size);";
const MALLOC_SHORT: &str = "REQUIRE(size != 0);\n    void *pointer = malloc(size - 1);";

/// Identity-v1's own hostile corpus AND its canonical positive control
/// (already embedded in [`super::caller_hostility::run_identity_corpus`] as
/// the `"canonical"`/raw==0 case, which runs at mode 1, the real checked
/// call), recompiled and rerun with Clang ASan+UBSan through
/// [`super::caller_hostility::run_sanitized`].
#[test]
fn identity_hostile_corpus_and_positive_control_under_asan_ubsan() {
    super::caller_hostility::run_identity_corpus(true);
}

/// moves-v1's, allocating-v1's and moves-nested.v1's shared hostile corpus
/// (7 recipes, the legacy flat refusal, and the generation/cleanup omission
/// controls -- [`super::profile_hostility::run_profile`]'s own corpus,
/// unchanged), recompiled and rerun with Clang ASan+UBSan. This module does
/// not carry a second copy of each profile's checked-body source or render
/// closure: these are the exact closures
/// `moves_and_allocating_profiles_reject_the_hostile_corpus_before_physical_work`
/// already proves correct unsanitized.
#[test]
fn moves_allocating_and_nested_moves_hostile_corpus_under_asan_ubsan() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-r292-sanitizer-hostility-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let moves = SOURCE.replace("{ value }", super::checked_moves::BODY);
    let mut processes = super::profile_hostility::run_profile(
        &root,
        "moves",
        &moves,
        "auth.identity",
        true,
        |program, revision, descriptor| {
            let artifact = semaprax::public_generic_abi::native::authenticated::render_authenticated_moves_provider(program, revision, descriptor).unwrap();
            let c = semaprax::public_generic_consumer::c_calling::generate_authenticated_moves_calling_consumer_v1(descriptor, &artifact)
                .unwrap();
            let cxx = semaprax::public_generic_consumer::cxx_calling::generate_authenticated_moves_calling_consumer_v1(
                descriptor, &artifact,
            )
            .unwrap();
            (
                artifact.source().to_owned(),
                c.files().to_vec(),
                cxx.files().to_vec(),
                artifact.binding().clone(),
            )
        },
    );
    let allocating = format!(
        "{}\n{}",
        SOURCE.replace("{ value }", super::checked_allocating::BODY),
        super::checked_allocating::HELPERS
    );
    processes += super::profile_hostility::run_profile(
        &root,
        "allocating",
        &allocating,
        "auth.identity",
        true,
        |program, revision, descriptor| {
            let artifact = semaprax::public_generic_abi::native::authenticated::render_authenticated_allocating_provider(program, revision, descriptor).unwrap();
            let c = semaprax::public_generic_consumer::c_calling::generate_authenticated_allocating_calling_consumer_v1(
                descriptor, &artifact,
            )
            .unwrap();
            let cxx = semaprax::public_generic_consumer::cxx_calling::generate_authenticated_allocating_calling_consumer_v1(
                descriptor, &artifact,
            )
            .unwrap();
            (
                artifact.source().to_owned(),
                c.files().to_vec(),
                cxx.files().to_vec(),
                artifact.binding().clone(),
            )
        },
    );
    processes += super::profile_hostility::run_profile(
        &root,
        "moves-nested",
        super::super::checked_nested_moves::SOURCE,
        super::super::checked_nested_moves::EXPORT_ID,
        true,
        |program, revision, descriptor| {
            let artifact = semaprax::public_generic_abi::native::authenticated::render_authenticated_nested_moves_provider(program, revision, descriptor).unwrap();
            let c = semaprax::public_generic_consumer::c_calling::generate_authenticated_nested_moves_calling_consumer_v1(
                descriptor, &artifact,
            )
            .unwrap();
            let cxx = semaprax::public_generic_consumer::cxx_calling::generate_authenticated_nested_moves_calling_consumer_v1(
                descriptor, &artifact,
            )
            .unwrap();
            (
                artifact.source().to_owned(),
                c.files().to_vec(),
                cxx.files().to_vec(),
                artifact.binding().clone(),
            )
        },
    );
    // Three profiles x two languages x (7 recipes + legacy flat + 2 omission
    // controls) x O0/O2, same corpus as the unsanitized selector, now clean
    // under Clang ASan+UBSan.
    assert_eq!(processes, 120);
    fs::remove_dir_all(root).unwrap();
}

/// Fail-closed proof: an actual one-byte heap-buffer-overflow, injected only
/// into an in-memory copy of the test-only provider fixture (never written
/// back to `allocations.c` on disk), must be caught by AddressSanitizer, not
/// silently pass. Uses identity-v1's own canonical fixture (the cheapest of
/// the four): [`super::super::fixture`] is the exact (provider, driver) pair
/// `authenticated_handoff`'s own direct-native-entry tests already compile
/// and run cleanly; only this one temp-directory copy's `provider.c` is
/// mutated.
#[test]
fn negative_control_one_byte_heap_overflow_is_caught_and_reverted() {
    let (provider, driver) = super::super::fixture(true);
    assert_eq!(
        provider.matches(MALLOC_CALL).count(),
        1,
        "exact allocator boundary this control mutates"
    );
    let mutated = provider.replacen(MALLOC_CALL, MALLOC_SHORT, 1);
    // The mutation is a pure in-memory `String`; `provider` itself, and the
    // `allocations.c` source file it was assembled from, are untouched --
    // this is the "revert" the negative control requires: there is nothing
    // on disk to revert.
    assert_ne!(mutated, provider);

    let root = std::env::temp_dir().join(format!(
        "semaprax-r292-sanitizer-negative-control-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    for opt in ["-O0", "-O2"] {
        let directory = root.join(opt);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("provider.c"), &mutated).unwrap();
        fs::write(directory.join("driver.c"), &driver).unwrap();
        let executable = directory.join(format!("probe{}", std::env::consts::EXE_SUFFIX));
        let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
        let compiled = Command::new(&clang)
            .args([
                "-std=c11",
                opt,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-fsanitize=address,undefined",
                "-fno-omit-frame-pointer",
                "-fno-sanitize-recover=all",
            ])
            .arg(directory.join("provider.c"))
            .arg(directory.join("driver.c"))
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new(&executable)
            .env("ASAN_OPTIONS", "halt_on_error=1")
            .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert!(
            !run.status.success(),
            "{opt}: the one-byte overflow must not settle cleanly (stderr={stderr})"
        );
        assert!(
            stderr.contains("AddressSanitizer") && stderr.contains("heap-buffer-overflow"),
            "{opt}: expected an AddressSanitizer heap-buffer-overflow report, got: {stderr}"
        );
        eprintln!(
            "R292 sanitizer negative control {opt}: one-byte heap overflow correctly reported and reverted (in-memory only)"
        );
    }
    fs::remove_dir_all(root).unwrap();
}
