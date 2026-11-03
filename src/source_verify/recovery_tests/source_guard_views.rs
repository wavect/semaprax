//! Exact full-source inverses of retained receivers across the FunctionRef migration.
//! This test-only adapter supplies no compiler or resource authority.
use sha2::{Digest, Sha256};
fn inputs(stage: &str, source: &str) -> (&'static str, &'static str) {
    match (stage, source) {
        ("recovery", "src/source_verify/iterative/enter/enter_before_extended.rs") => (
            include_str!("../iterative/enter/enter_before_extended.rs"),
            include_str!(
                "../../../e/c02a2e949346f78c89cf94f707cfbab369367c38a3b4c7490e608417876395ba"
            ),
        ),
        ("recovery", "src/source_verify/iterative/calls/calls_before_extended.rs") => (
            include_str!("../iterative/calls/calls_before_extended.rs"),
            include_str!(
                "../../../e/940b6120358c99e90101196d06e96aec5a884ff4f6600f77fa8e36149d76643b"
            ),
        ),
        ("recovery", "src/source_verify/iterative/records/records_before_extended.rs") => (
            include_str!("../iterative/records/records_before_extended.rs"),
            include_str!(
                "../../../e/01e067513231693880a91fac84413ce2d2d1cfd2c8693c69f571c3ae639c50b0"
            ),
        ),
        ("recovery", "src/source_verify/scope.rs") => (
            include_str!("../scope.rs"),
            include_str!(
                "../../../e/ea01202a694f030acc597163cb157513cdda4cf39ca29abc6e7fab82f28878ef"
            ),
        ),
        ("recovery", "src/source_verify/recovery_legacy.rs") => (
            include_str!("../recovery_legacy.rs"),
            include_str!(
                "../../../e/68e95e9723de78f8829a5a2c2db4960fde8ca285c924ecaa88adae66a7e01b51"
            ),
        ),
        ("recovery", "src/source_verify/iterative/matching/matching_before_799.rs") => (
            include_str!("../iterative/matching/matching_before_799.rs"),
            include_str!(
                "../../../e/71b92d75a4126e289167418b110d03ad74729525aedac9c999f404a6f5054669"
            ),
        ),
        ("recovery", "src/source_verify/iterative/match_arms/arms_before_799.rs") => (
            include_str!("../iterative/match_arms/arms_before_799.rs"),
            include_str!(
                "../../../e/785954adec3bf8b9086e8f8ba4fa5481a3b914208ee7356adf0aa45261aa503d"
            ),
        ),
        ("recovery", "src/source_verify/recovery_legacy/match_arms.rs") => (
            include_str!("../recovery_legacy/match_arms.rs"),
            include_str!(
                "../../../e/111693274eb072e3dc9d33d44b25db0de90780e1993a9a1d256b45e68e3a0203"
            ),
        ),
        ("recovery", "src/source_verify/recovery_legacy/enter.rs") => (
            include_str!("../recovery_legacy/enter.rs"),
            include_str!(
                "../../../e/6ba3691f769083e3da929796cf7d1fd439f59ec1dc061bdc238c96e27cdb9baf"
            ),
        ),
        ("recovery", "src/source_verify/recovery_tests/extended_components.rs") => (
            include_str!("../recovery_tests/extended_components.rs"),
            include_str!(
                "../../../e/83ec2cd7f9d701a42e3cfd01c756344798d2d274edb24ec43eba6d5b8261646c"
            ),
        ),
        ("recovery", "src/source_verify/recovery.rs") => (
            include_str!("../recovery.rs"),
            include_str!(
                "../../../e/262bb0030111b4f31e939117f6c7144feabaa922c28b20ef94bbfa4d97f5d5e8"
            ),
        ),
        ("recovery", "src/source_verify/iterative/enter.rs") => (
            include_str!("../iterative/enter.rs"),
            include_str!(
                "../../../e/7566025fca09d0cec7839f7754a0d54ebbb3313b902617cc3a1d9d22b7516227"
            ),
        ),
        ("recovery", "src/source_verify/iterative/calls.rs") => (
            include_str!("../iterative/calls.rs"),
            include_str!(
                "../../../e/e6fb511a3f1d294a6ae8dfe4fd024c9233cdb7ad73197c1d0331b5c5435f9430"
            ),
        ),
        (
            "recovery",
            "src/source_verify/recovery/callee_components/callee_components_before_actual.rs",
        ) => (
            include_str!("../recovery/callee_components/callee_components_before_actual.rs"),
            include_str!(
                "../../../e/e5261a226e560e12c664a44f43f4d295416f4d403172d363fc179a663821ceea"
            ),
        ),
        ("public-template", "src/source_verify/declaration.rs") => (
            include_str!("../declaration.rs"),
            include_str!(
                "../../../e/11b26f1bc56c129657268cf90436753cfaf037fa7c37ec49596284fd5d1bdcd8"
            ),
        ),
        ("public-template", "src/source_verify/declaration/public_signature_before.rs") => (
            include_str!("../declaration/public_signature_before.rs"),
            include_str!(
                "../../../e/7c385c983031164a5f1e292256c7ec0b4a387439ff7ab414050aab89f9a1fab3"
            ),
        ),
        ("public-template", "src/source_verify/declaration/functions.rs") => (
            include_str!("../declaration/functions.rs"),
            include_str!(
                "../../../e/66c5704c3d423fb73c790c34bf273b30ed21e835790e6034b42e299f0a71a474"
            ),
        ),
        (
            "public-template",
            "src/source_verify/declaration/functions/public_signature_before.rs",
        ) => (
            include_str!("../declaration/functions/public_signature_before.rs"),
            include_str!(
                "../../../e/21cf514809f4f9d271d5497744d948203ecca1afbd924c4a22061884117bf129"
            ),
        ),
        ("public-template", "src/source_verify/iterative.rs") => (
            include_str!("../iterative.rs"),
            include_str!(
                "../../../e/ab396db1af79d103ff988289513111d8ee8c902378320bcde9c04bce98b7e3c8"
            ),
        ),
        ("public-template", "src/source_verify/diagnostics.rs") => (
            include_str!("../diagnostics.rs"),
            include_str!(
                "../../../e/95bd8721029021074f39f5c76e4e0183771831112f591143e4a42bd90ce670d9"
            ),
        ),
        ("public-template", "src/source_verify/recovery.rs") => (
            include_str!("../recovery.rs"),
            include_str!(
                "../../../e/262bb0030111b4f31e939117f6c7144feabaa922c28b20ef94bbfa4d97f5d5e8"
            ),
        ),
        ("public-template", "src/source_verify/iterative/enter.rs") => (
            include_str!("../iterative/enter.rs"),
            include_str!(
                "../../../e/3a03b4c4773e2d72e87077031cf1b68fd19f678a24ae7cab33934ad7403a0405"
            ),
        ),
        ("public-template", "src/source_verify/recovery_tests/extended_components.rs") => (
            include_str!("../recovery_tests/extended_components.rs"),
            include_str!(
                "../../../e/83ec2cd7f9d701a42e3cfd01c756344798d2d274edb24ec43eba6d5b8261646c"
            ),
        ),
        _ => panic!("unregistered complete receiver: {source}"),
    }
}
pub(crate) fn before(source: &str, current: &str) -> String {
    at_stage("recovery", source, current)
}
pub(crate) fn at_stage(stage: &str, source: &str, current: &str) -> String {
    let (compiled, saved) = inputs(stage, source);
    assert_eq!(current, compiled, "complete current receiver: {source}");
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("source_guard_views.json")).unwrap();
    let matching: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["source"] == source && row["stage"] == stage)
        .collect();
    assert_eq!(matching.len(), 1, "unique full receiver: {source}");
    let row = matching[0];
    for (key, text) in [("current", current), ("before", saved)] {
        assert_eq!(row[key][0].as_u64().unwrap() as usize, text.len());
        assert_eq!(
            row[key][1].as_str().unwrap(),
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            )
        );
    }
    let mut result = current.to_owned();
    let mut end = current.len();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let offset = edit[0].as_u64().unwrap() as usize;
        let after = edit[1].as_str().unwrap();
        let old = edit[2].as_str().unwrap();
        let stop = offset.checked_add(after.len()).unwrap();
        assert!(stop <= end, "ordered, disjoint full-source inverse");
        assert_eq!(result.get(offset..stop), Some(after));
        result.replace_range(offset..stop, old);
        end = offset;
    }
    assert_eq!(result, saved, "entire prior receiver: {source}");
    result
}
#[test]
fn every_registered_full_receiver_has_an_exact_inverse() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("source_guard_views.json")).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 23);
    for row in rows.as_array().unwrap() {
        let path = row["source"].as_str().unwrap();
        let stage = row["stage"].as_str().unwrap();
        let (current, saved) = inputs(stage, path);
        assert_eq!(at_stage(stage, path, current), saved);
    }
}
#[test]
fn incomplete_foreign_and_mutated_current_receivers_are_refused() {
    let path = "src/source_verify/scope.rs";
    let (current, _) = inputs("recovery", path);
    for invalid in [
        &current[..current.len() - 1],
        &format!("{current} "),
        "foreign receiver",
    ] {
        assert!(std::panic::catch_unwind(|| before(path, invalid)).is_err());
    }
    assert!(std::panic::catch_unwind(|| before("foreign.rs", current)).is_err());
}
