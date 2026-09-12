//! Executable evidence for [`super`]: the three consequences #215 named as
//! "left untested" against the pre-#215 caller-responsibility design —
//! recovery/currentness enforcement, `historical_mode` actually gating a
//! check, and "missing retained subject fails closed" against a store that
//! genuinely exists — each proven against a real filesystem-backed store,
//! never a fixture standing in for one.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::hir;
use crate::parse;
use crate::public_generic_abi::descriptor::producer;

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn temp_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "semaprax-retained-program-store-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ))
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One generic export (`retained.take`, `Pair<Leaf, i64>` in and out),
/// matching [`super::super::tests`]'s own proven-admitted shape exactly
/// (exactly one `own` parameter, a concrete record instance in both
/// positions) so shape admission is never the thing under test here.
const V1: &str = r#"
module test.retained_store;

@id("retained.leaf")
record Leaf {
    @id("retained.leaf.head")
    head: Bytes,
}

@id("retained.pair")
record Pair<T, U> {
    @id("retained.pair.left")
    left: T,
    @id("retained.pair.right")
    right: U,
}

@id("retained.take")
fn take(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64> { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// [`V1`] plus one extra declaration (`retained.extra`), so its
/// declaration-identity set — and therefore its `program_root_digest` — is
/// different from [`V1`]'s, while `retained.take`'s own shape is untouched.
const V2: &str = r#"
module test.retained_store;

@id("retained.leaf")
record Leaf {
    @id("retained.leaf.head")
    head: Bytes,
}

@id("retained.pair")
record Pair<T, U> {
    @id("retained.pair.left")
    left: T,
    @id("retained.pair.right")
    right: U,
}

@id("retained.take")
fn take(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64> { value }

@id("retained.extra")
fn extra() -> i64 { 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> hir::ResolvedProgram {
    let parsed = parse(source, std::path::Path::new("retained-store-test.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn descriptor_bytes_for(source: &str, revision: &str) -> Vec<u8> {
    let program = resolved(source);
    producer::generate_public_generic_descriptor(&program, revision, "retained.take")
        .unwrap()
        .wire_bytes()
        .to_vec()
}

#[test]
fn missing_retained_subject_fails_closed_before_any_candidate_byte_is_touched() {
    let dir = TempDir(temp_root());
    let store = RetainedProgramStore::open(&dir.0).unwrap();
    // Deliberately not valid descriptor bytes at all (not even valid UTF-8
    // framing): if the store's lookup ran *after* candidate parsing, this
    // would fail with a parse-phase code instead of the retained-subject
    // code, proving the ordering claim rather than merely asserting it.
    let garbage_bytes: &[u8] = &[0xFF, 0xFE, 0x00, 0x01, 0x02];
    let error = verify_public_generic_descriptor_against_store(
        &store,
        "sha256:0000000000000000000000000000000000000000000000000000000000000",
        "retained.take",
        garbage_bytes,
        &VerificationOptions::current_head(),
    )
    .unwrap_err();
    assert_eq!(error.code, RETAINED_SUBJECT_UNAVAILABLE);
}

#[test]
fn a_freshly_published_current_subject_verifies_through_the_store() {
    let dir = TempDir(temp_root());
    let store = RetainedProgramStore::open(&dir.0).unwrap();
    let digest = store.publish_current(V1, "rev-1").unwrap();
    let candidate = descriptor_bytes_for(V1, "rev-1");

    let verified = verify_public_generic_descriptor_against_store(
        &store,
        &digest,
        "retained.take",
        &candidate,
        &VerificationOptions::current_head(),
    )
    .unwrap();
    assert_eq!(verified.export_id(), "retained.take");
    assert_eq!(verified.program_root_digest(), digest);
    assert!(!verified.historical_mode());
}

#[test]
fn process_restart_recovers_the_identical_trusted_subject() {
    let dir = TempDir(temp_root());
    let digest = {
        let store = RetainedProgramStore::open(&dir.0).unwrap();
        store.publish_current(V1, "rev-1").unwrap()
        // `store` dropped here: no in-process state survives.
    };
    let candidate = descriptor_bytes_for(V1, "rev-1");

    // A brand new store value over the same directory, sharing nothing in
    // process memory with the one that published the entry above -- the
    // closest thing to an actual process restart a single test binary can
    // exercise.
    let restarted_store = RetainedProgramStore::open(&dir.0).unwrap();
    let verified = verify_public_generic_descriptor_against_store(
        &restarted_store,
        &digest,
        "retained.take",
        &candidate,
        &VerificationOptions::current_head(),
    )
    .unwrap();
    assert_eq!(verified.accepted_bytes(), candidate.as_slice());
    assert_eq!(verified.program_root_digest(), digest);
}

#[test]
fn a_historical_entry_is_refused_under_current_head_options() {
    let dir = TempDir(temp_root());
    let store = RetainedProgramStore::open(&dir.0).unwrap();
    let historical_digest = store.publish_current(V1, "rev-1").unwrap();
    // Publishing V2 as the new current head demotes V1's entry: it remains
    // resolvable, but is no longer current.
    let current_digest = store.publish_current(V2, "rev-2").unwrap();
    assert_ne!(historical_digest, current_digest);

    let candidate = descriptor_bytes_for(V1, "rev-1");
    let error = verify_public_generic_descriptor_against_store(
        &store,
        &historical_digest,
        "retained.take",
        &candidate,
        &VerificationOptions::current_head(),
    )
    .unwrap_err();
    assert_eq!(error.code, HISTORICAL_REVISION_REQUIRES_HISTORICAL_MODE);
}

#[test]
fn a_historical_entry_verifies_under_explicit_historical_mode() {
    let dir = TempDir(temp_root());
    let store = RetainedProgramStore::open(&dir.0).unwrap();
    let historical_digest = store.publish_current(V1, "rev-1").unwrap();
    store.publish_current(V2, "rev-2").unwrap();

    let candidate = descriptor_bytes_for(V1, "rev-1");
    let verified = verify_public_generic_descriptor_against_store(
        &store,
        &historical_digest,
        "retained.take",
        &candidate,
        &VerificationOptions::historical(),
    )
    .unwrap();
    assert!(verified.historical_mode());
    assert_eq!(verified.program_root_digest(), historical_digest);
}

#[test]
fn the_current_head_never_needs_historical_mode_even_after_being_republished() {
    // Negative control for the two tests above: current-head verification
    // must not spuriously start requiring `historical_mode` merely because
    // this store has *some* historical entries on file.
    let dir = TempDir(temp_root());
    let store = RetainedProgramStore::open(&dir.0).unwrap();
    store.publish_current(V1, "rev-1").unwrap();
    let current_digest = store.publish_current(V2, "rev-2").unwrap();

    let candidate = descriptor_bytes_for(V2, "rev-2");
    let verified = verify_public_generic_descriptor_against_store(
        &store,
        &current_digest,
        "retained.take",
        &candidate,
        &VerificationOptions::current_head(),
    )
    .unwrap();
    assert!(!verified.historical_mode());
}

#[test]
fn retain_historical_entries_are_available_only_under_historical_mode() {
    // `retain_historical` never touches the CURRENT pointer at all, unlike
    // `publish_current`: an entry it adds is never current, even in a
    // store with no current entry yet.
    let dir = TempDir(temp_root());
    let store = RetainedProgramStore::open(&dir.0).unwrap();
    let digest = store.retain_historical(V1, "rev-0").unwrap();
    let candidate = descriptor_bytes_for(V1, "rev-0");

    let refused = verify_public_generic_descriptor_against_store(
        &store,
        &digest,
        "retained.take",
        &candidate,
        &VerificationOptions::current_head(),
    )
    .unwrap_err();
    assert_eq!(refused.code, HISTORICAL_REVISION_REQUIRES_HISTORICAL_MODE);

    let verified = verify_public_generic_descriptor_against_store(
        &store,
        &digest,
        "retained.take",
        &candidate,
        &VerificationOptions::historical(),
    )
    .unwrap();
    assert!(verified.historical_mode());
}

#[test]
fn a_tampered_retained_entry_fails_closed_rather_than_adopting_mismatched_content() {
    let dir = TempDir(temp_root());
    let store = RetainedProgramStore::open(&dir.0).unwrap();
    let digest = store.publish_current(V1, "rev-1").unwrap();
    // Overwrite the persisted entry's content with a different, otherwise
    // well-formed source, without updating the filename it is filed under:
    // the on-disk analogue of a corrupted or hand-edited entry.
    std::fs::write(
        dir.0.join("entries").join(&digest),
        serde_json::json!({"source": V2, "source_revision": "rev-1"}).to_string(),
    )
    .unwrap();
    let candidate = descriptor_bytes_for(V1, "rev-1");
    let error = verify_public_generic_descriptor_against_store(
        &store,
        &digest,
        "retained.take",
        &candidate,
        &VerificationOptions::current_head(),
    )
    .unwrap_err();
    assert_eq!(error.code, RETAINED_SUBJECT_UNAVAILABLE);
}
