//! Issue #160: the shared malformed-input/wrong-binding corpus definition.
//!
//! This is the ONE source of truth every generated calling consumer's
//! shared-corpus driver (Rust/C11/C++17/TypeScript) is checked against: the
//! canonical descriptor bytes every one of the four consumers is generated
//! from, the closed set of case IDs, and the one expected outcome per case.
//! It holds pure data only -- no process spawning, no file I/O -- so it can
//! be `#[path]`-included, unmodified, from both
//! `tests/public_generic_native_adapter_v1/shared_hostile_corpus.rs` (Rust,
//! C11, C++17) and `tests/public_generic_wasm_adapter_v1/shared_hostile_corpus.rs`
//! (TypeScript/Wasm), which otherwise share no compiled crate: each
//! `#[path]`-include compiles its own copy of this same on-disk file into its
//! own separate test binary, so the two harnesses cannot literally compare
//! outcomes inside one process, but both are asserted against this single,
//! textually shared definition -- so a route that diverges from every other
//! route's outcome for the same case fails a hard assertion in its own
//! harness rather than silently passing an independent, drifted copy.
//!
//! What this corpus does NOT cover, stated once here rather than implied:
//! it exercises only the outcomes every one of the four consumers can
//! express identically today (open-time descriptor/binding rejection and
//! the per-leaf byte bound). `CarrierRejected`, `ExecutionFailed`,
//! `ResultRejected`, `AllocationFailure`, `NullArgument` (native-only), and
//! C++'s `ReleaseFailed` are exercised by each consumer's OWN existing
//! generator-produced hostile tests already (see the coverage audit in this
//! issue's report) -- duplicating them here under a false "shared" label
//! would not make them any more cross-checked, since those specific reasons
//! are not uniformly reachable across all four targets from the same
//! artifact bytes today. The full ordinal 0..=13 (native) / 0..=7 (Wasm)
//! failure-injection matrices are also deliberately excluded from this file:
//! the native and Core Wasm carrier protocols have different phase counts
//! (14 vs. 8 injectable ordinals), so "ordinal N" is not the same logical
//! phase across native and Wasm and a literal per-ordinal cross-language
//! comparison would be comparing different things under the same name. Each
//! route's own full local matrix remains covered by its existing generated
//! test, unmodified.

/// The one descriptor baseline every one of the four generated consumers is
/// generated from in the shared-corpus harnesses. Provider-family-agnostic
/// (unlike a provider binding, which is inherently native- or Wasm-shaped),
/// so this exact byte sequence is fed to `generate_rust_calling_consumer`,
/// `generate_c_calling_consumer`, `generate_cxx_calling_consumer`, and
/// `generate_typescript_calling_consumer` alike -- the descriptor-mutation
/// cases below are the one case family that is byte-for-byte identical
/// across all four routes, not merely recipe-identical.
pub const BASELINE_DESCRIPTOR_BYTES: &[u8] =
    b"fixture-public-generic-descriptor-bytes-issue-160-shared-corpus";

/// Restates `src/public_generic_abi/boundary_profile.rs::MAX_BYTES_PER_LEAF`
/// (64 KiB), exactly like every generated consumer already restates it
/// rather than depending on the `semaprax` crate (a generated artifact must
/// build standalone). The native shared-corpus harness additionally asserts
/// this literal equals the real constant so a future bound change cannot
/// drift silently -- see `shared_hostile_corpus.rs`'s own assertion.
pub const MAX_BYTES_PER_LEAF: usize = 64 * 1024;

/// One entry per case: a stable id (also the exact prefix each generated
/// driver prints as `SHARED_CORPUS <case_id> <STATUS>`) and the single
/// expected normalized status every route must agree on. `STATUS` is one of
/// `ACCEPTED`, `DESCRIPTOR_REJECTED`, `PROVIDER_MISMATCH`,
/// `CAPACITY_EXCEEDED` -- the closed subset of the shared
/// `DescriptorRejected`/`ProviderMismatch`/`CapacityExceeded`/accepted
/// vocabulary every one of Rust's `Error`, C's `spx_pg_consumer_status`,
/// C++'s `ErrorKind`, and TypeScript's `SemapraxPublicGenericError.kind`
/// already restate identically (see the coverage audit).
pub const EXPECTED: &[(&str, &str)] = &[
    ("success_baseline", "ACCEPTED"),
    ("descriptor_first_byte_flipped", "DESCRIPTOR_REJECTED"),
    ("binding_last_byte_flipped", "PROVIDER_MISMATCH"),
    ("descriptor_names_different_document", "DESCRIPTOR_REJECTED"),
    ("exactly_per_leaf_bound_accepted", "ACCEPTED"),
    ("one_byte_over_per_leaf_bound_rejected", "CAPACITY_EXCEEDED"),
];

/// Parse every `SHARED_CORPUS <case_id> <STATUS>` line a spliced driver
/// printed to its own stdout into an ordered list of `(case_id, status)`
/// pairs, in the order printed. Never panics on unrelated output lines (a
/// compiler warning, a libtest summary line, a `console.log`) -- it only
/// looks for its own fixed marker. The marker is located anywhere within a
/// line, not only at its start: `cargo test -- --nocapture` prints a test's
/// own stdout directly after its `test <name> ... ` prefix on the SAME
/// line for the first line a test prints, so a strict line-prefix match
/// would silently drop exactly that first case.
pub fn parse_shared_corpus_lines(stdout: &str) -> Vec<(String, String)> {
    const MARKER: &str = "SHARED_CORPUS ";
    stdout
        .lines()
        .filter_map(|line| line.find(MARKER).map(|at| &line[at + MARKER.len()..]))
        .filter_map(|rest| {
            let mut parts = rest.splitn(2, ' ');
            let case_id = parts.next()?.trim();
            let status = parts.next()?.trim();
            if case_id.is_empty() || status.is_empty() {
                None
            } else {
                Some((case_id.to_owned(), status.to_owned()))
            }
        })
        .collect()
}

/// Assert one route's parsed `(case_id, status)` pairs are exactly the
/// expected set -- same case ids, same order-independent statuses, no
/// missing case, no extra case, no duplicate -- against [`EXPECTED`]. On any
/// mismatch, panics with every case's expected-vs-actual so a human sees the
/// whole picture rather than the first assertion failure.
pub fn assert_matches_expected(route: &str, actual: &[(String, String)]) {
    use std::collections::BTreeMap;

    let actual_map: BTreeMap<&str, &str> = actual
        .iter()
        .map(|(id, status)| (id.as_str(), status.as_str()))
        .collect();
    assert_eq!(
        actual_map.len(),
        actual.len(),
        "{route}: a shared-corpus case id was printed more than once: {actual:?}"
    );

    let mut mismatches = Vec::new();
    for (case_id, expected_status) in EXPECTED {
        match actual_map.get(case_id) {
            None => mismatches.push(format!(
                "{case_id}: {route} never printed a SHARED_CORPUS line for this case"
            )),
            Some(actual_status) if actual_status != expected_status => mismatches.push(format!(
                "{case_id}: {route} reported {actual_status}, expected {expected_status}"
            )),
            Some(_) => {}
        }
    }
    let unexpected: Vec<&str> = actual_map
        .keys()
        .filter(|id| !EXPECTED.iter().any(|(expected_id, _)| expected_id == *id))
        .copied()
        .collect();
    if !unexpected.is_empty() {
        mismatches.push(format!(
            "{route} printed unexpected case id(s) not in the shared manifest: {unexpected:?}"
        ));
    }
    assert!(
        mismatches.is_empty(),
        "{route} disagrees with the shared hostile-corpus manifest:\n{}",
        mismatches.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shared_corpus_lines_and_ignores_unrelated_output() {
        let stdout = "running 1 test\nSHARED_CORPUS success_baseline ACCEPTED\nok - unrelated\nSHARED_CORPUS descriptor_first_byte_flipped DESCRIPTOR_REJECTED\ntest result: ok\n";
        let parsed = parse_shared_corpus_lines(stdout);
        assert_eq!(
            parsed,
            vec![
                ("success_baseline".to_owned(), "ACCEPTED".to_owned()),
                (
                    "descriptor_first_byte_flipped".to_owned(),
                    "DESCRIPTOR_REJECTED".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn finds_the_marker_mid_line_as_cargo_test_nocapture_prints_it() {
        // `cargo test -- --nocapture` prints a test's own first stdout line
        // directly after `test <name> ... ` on the SAME line -- not merely
        // at line start.
        let stdout = "test shared_hostile_corpus ... SHARED_CORPUS success_baseline ACCEPTED\nSHARED_CORPUS descriptor_first_byte_flipped DESCRIPTOR_REJECTED\nok\n";
        let parsed = parse_shared_corpus_lines(stdout);
        assert_eq!(
            parsed,
            vec![
                ("success_baseline".to_owned(), "ACCEPTED".to_owned()),
                (
                    "descriptor_first_byte_flipped".to_owned(),
                    "DESCRIPTOR_REJECTED".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn accepts_the_exact_expected_set_in_any_order() {
        let mut actual: Vec<(String, String)> = EXPECTED
            .iter()
            .map(|(id, status)| ((*id).to_owned(), (*status).to_owned()))
            .collect();
        actual.reverse();
        assert_matches_expected("test-route", &actual);
    }

    #[test]
    #[should_panic(expected = "reported PROVIDER_MISMATCH, expected DESCRIPTOR_REJECTED")]
    fn rejects_a_wrong_status_for_a_known_case() {
        let mut actual: Vec<(String, String)> = EXPECTED
            .iter()
            .map(|(id, status)| ((*id).to_owned(), (*status).to_owned()))
            .collect();
        for entry in &mut actual {
            if entry.0 == "descriptor_first_byte_flipped" {
                entry.1 = "PROVIDER_MISMATCH".to_owned();
            }
        }
        assert_matches_expected("test-route", &actual);
    }

    #[test]
    #[should_panic(expected = "never printed a SHARED_CORPUS line for this case")]
    fn rejects_a_missing_case() {
        let actual: Vec<(String, String)> = EXPECTED
            .iter()
            .skip(1)
            .map(|(id, status)| ((*id).to_owned(), (*status).to_owned()))
            .collect();
        assert_matches_expected("test-route", &actual);
    }

    #[test]
    #[should_panic(expected = "printed unexpected case id(s)")]
    fn rejects_an_unknown_extra_case() {
        let mut actual: Vec<(String, String)> = EXPECTED
            .iter()
            .map(|(id, status)| ((*id).to_owned(), (*status).to_owned()))
            .collect();
        actual.push(("an_unknown_case".to_owned(), "ACCEPTED".to_owned()));
        assert_matches_expected("test-route", &actual);
    }

    #[test]
    #[should_panic(expected = "printed more than once")]
    fn rejects_a_duplicated_case_id() {
        let mut actual: Vec<(String, String)> = EXPECTED
            .iter()
            .map(|(id, status)| ((*id).to_owned(), (*status).to_owned()))
            .collect();
        actual.push(actual[0].clone());
        assert_matches_expected("test-route", &actual);
    }
}
