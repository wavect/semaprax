//! Full immediate source custody in both directions; no excluded interiors.
pub(super) fn apply(source: &str, patch: &str, reverse: bool) -> String {
    let mut input = source.split_inclusive('\n');
    let mut output = String::new();
    let mut consumed = 0usize;
    let mut in_hunk = false;
    for line in patch.split_inclusive('\n') {
        if line.starts_with("@@ ") {
            let span = line
                .split_whitespace()
                .nth(if reverse { 2 } else { 1 })
                .unwrap();
            let start = span[1..]
                .split(',')
                .next()
                .unwrap()
                .parse::<usize>()
                .unwrap();
            while consumed < start.saturating_sub(1) {
                output.push_str(input.next().expect("complete source prefix"));
                consumed += 1;
            }
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        let prefix = line.as_bytes()[0];
        let contents = &line[1..];
        let remove = prefix == if reverse { b'+' } else { b'-' };
        let add = prefix == if reverse { b'-' } else { b'+' };
        if remove || prefix == b' ' {
            assert_eq!(
                input.next(),
                Some(contents),
                "complete authenticated source hunk"
            );
            consumed += 1;
        }
        if add || prefix == b' ' {
            output.push_str(contents);
        }
        assert!(remove || add || prefix == b' ');
    }
    for remainder in input {
        output.push_str(remainder);
    }
    output
}

#[test]
fn ordered_complete_immediate_sources_round_trip_in_both_directions() {
    macro_rules! bind {
        ($current:literal, $old:literal, $patch:literal) => {{
            let current = super::amortized_source::checkpoint($current, include_str!($current));
            let old = include_str!($old);
            let patch = include_str!($patch);
            assert_eq!(apply(old, patch, false), current);
            assert_eq!(apply(&current, patch, true), old);
        }};
    }
    // Bind full current files and their complete immediate predecessor bytes.
    bind!(
        "../../canonical_shared.rs",
        "../../../../../../docs/evidence/weighted-storage-838/src__cleanup_plan__replay__factored__canonical_shared.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/src__cleanup_plan__replay__factored__canonical_shared.rs.patch.txt"
    );
    bind!(
        "../epsilon_alias/tests.rs",
        "../../../../../../docs/evidence/weighted-storage-838/src__cleanup_plan__replay__factored__canonical_shared__epsilon_alias__tests.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/src__cleanup_plan__replay__factored__canonical_shared__epsilon_alias__tests.rs.patch.txt"
    );
    bind!(
        "../scratch_tests.rs",
        "../../../../../../docs/evidence/weighted-storage-838/src__cleanup_plan__replay__factored__canonical_shared__scratch_tests.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/src__cleanup_plan__replay__factored__canonical_shared__scratch_tests.rs.patch.txt"
    );
    bind!(
        "../../../../../../tests/hir_module_boundaries.rs",
        "../../../../../../docs/evidence/weighted-storage-838/tests__hir_module_boundaries.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/tests__hir_module_boundaries.rs.patch.txt"
    );
    bind!(
        "../../../../../../tests/workspace/semantic_graph/source_inventory.rs",
        "../../../../../../docs/evidence/weighted-storage-838/tests__workspace__semantic_graph__source_inventory.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/tests__workspace__semantic_graph__source_inventory.rs.patch.txt"
    );
    bind!(
        "../../../../../../docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md",
        "../../../../../../docs/evidence/weighted-storage-838/docs__RFC-0003-CLEANUP-AND-RESOURCE-ABI.md.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/docs__RFC-0003-CLEANUP-AND-RESOURCE-ABI.md.patch.txt"
    );
    bind!(
        "../../../../../../docs/ARCHITECTURE.md",
        "../../../../../../docs/evidence/weighted-storage-838/docs__ARCHITECTURE.md.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/docs__ARCHITECTURE.md.patch.txt"
    );
}

#[test]
fn ordered_full_old_receiver_and_alias_adaptation_are_exact() {
    let original = include_str!(
        "../../../../../../docs/evidence/weighted-storage-838/src__cleanup_plan__replay__factored__canonical_shared.rs.before.txt"
    );
    let end = original.find("\n#[cfg(test)]\nmod before_alias;").unwrap();
    let mut expected = original[..end].replacen("use super::{", "use super::super::{", 1);
    expected
        .push_str("\nuse super::{app_census, epsilon_alias, memo, successor_targets, tests};\n\n");
    let body = &original[original.find("type SharedWeighted").unwrap()..];
    let body = body.split("\n#[cfg(test)]\nmod tests;").next().unwrap();
    expected.push_str(
        &body
            .replacen("    fn closure(", "    pub(super) fn closure(", 1)
            .replacen("    fn successors(", "    pub(super) fn successors(", 1),
    );
    expected.push_str("\n\nimpl Canonicalizer<'_> {\n    pub(super) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> { &self.ids }\n    pub(super) fn states(&self) -> &BTreeMap<SharedWeighted, usize> { &self.states }\n}\n");
    let immediate = crate::cleanup_plan::replay::physical_budget::authentication::previous(
        "src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs",
        include_str!("../before_ordered.rs"),
    );
    assert_eq!(expected, immediate);
    let alias = include_str!("../epsilon_alias.rs")
        .replace("use super::{BTreeMap, Diagnostic, Rc, ReplayBudget, ResolvedFunction};",
            "use super::ordered_weights::OrderedWeights;\nuse super::{Diagnostic, Rc, ReplayBudget, ResolvedFunction};")
        .replace("Rc<BTreeMap<usize, u64>>", "Rc<OrderedWeights>")
        .replace("\n#[cfg(test)]\nmod tests;\n", "")
        .trim_end().to_owned() + "\n";
    assert_eq!(alias, include_str!("../ordered_alias.rs"));
}
