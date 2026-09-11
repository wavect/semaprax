//! Executable evidence for gate PG-3: the compatibility rules are closed and
//! total, they are driven by identity rather than presentation, they are
//! stricter than source compatibility wherever a foreign consumer would see
//! more than a SEMAPRAX caller, and they infer neither a version nor a support
//! decision.
//!
//! Each variant below is a whole module written out rather than a patched
//! string, because every one has to be a program the compiler accepts: a
//! rejected variant would prove nothing about compatibility.

use std::path::Path;

use serde_json::Value;

use super::*;
use crate::hir;
use crate::parse;

/// One owned nested instance parameter, one instance result, and one borrowed
/// byte view, so the non-data position vocabulary is exercised too.
const BASE: &str = r#"
module test.public_generic_surface;

@id("surface.leaf")
record Leaf {
    @id("surface.leaf.head")
    head: Bytes,
}

@id("surface.pair")
record Pair<T, U> {
    @id("surface.pair.left")
    left: T,
    @id("surface.pair.right")
    right: U,
}

@id("surface.take")
fn take(value: own Pair<Leaf, bool>) -> i64 {
    match own value {
        Pair { left: Leaf { head: payload }, right: present } =>
            if present && byte_len(bytes_as_slice(payload)) > 0usize { 1 } else { 0 },
    }
}

@id("surface.make")
fn make(input: borrow Slice<u8>) -> Pair<Leaf, bool> {
    Pair<Leaf, bool> {
        left: Leaf { head: bytes_copy(input) },
        right: byte_len(input) > 0usize,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Every display name changed; every persistent identity kept.
const RENAMED: &str = r#"
module test.public_generic_surface;

@id("surface.leaf")
record Node {
    @id("surface.leaf.head")
    contents: Bytes,
}

@id("surface.pair")
record Couple<A, B> {
    @id("surface.pair.left")
    first: A,
    @id("surface.pair.right")
    second: B,
}

@id("surface.take")
fn accept(subject: own Couple<Node, bool>) -> i64 {
    match own subject {
        Couple { first: Node { contents: bytes }, second: flag } =>
            if flag && byte_len(bytes_as_slice(bytes)) > 0usize { 1 } else { 0 },
    }
}

@id("surface.make")
fn build(source: borrow Slice<u8>) -> Couple<Node, bool> {
    Couple<Node, bool> {
        first: Node { contents: bytes_copy(source) },
        second: byte_len(source) > 0usize,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// A Copy field added to the reachable monomorphic record.
const COPY_FIELD: &str = r#"
module test.public_generic_surface;

@id("surface.leaf")
record Leaf {
    @id("surface.leaf.head")
    head: Bytes,
    @id("surface.leaf.tag")
    tag: i64,
}

@id("surface.pair")
record Pair<T, U> {
    @id("surface.pair.left")
    left: T,
    @id("surface.pair.right")
    right: U,
}

@id("surface.take")
fn take(value: own Pair<Leaf, bool>) -> i64 {
    match own value {
        Pair { left: Leaf { head: payload, tag: tag }, right: present } =>
            if present && tag > 0 && byte_len(bytes_as_slice(payload)) > 0usize { 1 } else { 0 },
    }
}

@id("surface.make")
fn make(input: borrow Slice<u8>) -> Pair<Leaf, bool> {
    Pair<Leaf, bool> {
        left: Leaf { head: bytes_copy(input), tag: 7 },
        right: byte_len(input) > 0usize,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// An owned field added to the same record.
const OWNED_FIELD: &str = r#"
module test.public_generic_surface;

@id("surface.leaf")
record Leaf {
    @id("surface.leaf.head")
    head: Bytes,
    @id("surface.leaf.extra")
    extra: Bytes,
}

@id("surface.pair")
record Pair<T, U> {
    @id("surface.pair.left")
    left: T,
    @id("surface.pair.right")
    right: U,
}

@id("surface.take")
fn take(value: own Pair<Leaf, bool>) -> i64 {
    match own value {
        Pair { left: Leaf { head: payload, extra: more }, right: present } =>
            if present && byte_len(bytes_as_slice(payload)) > 0usize
                && byte_len(bytes_as_slice(more)) > 0usize { 1 } else { 0 },
    }
}

@id("surface.make")
fn make(input: borrow Slice<u8>) -> Pair<Leaf, bool> {
    Pair<Leaf, bool> {
        left: Leaf { head: bytes_copy(input), extra: bytes_copy(input) },
        right: byte_len(input) > 0usize,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// The same two type arguments in the other order.
const PERMUTED: &str = r#"
module test.public_generic_surface;

@id("surface.leaf")
record Leaf {
    @id("surface.leaf.head")
    head: Bytes,
}

@id("surface.pair")
record Pair<T, U> {
    @id("surface.pair.left")
    left: T,
    @id("surface.pair.right")
    right: U,
}

@id("surface.take")
fn take(value: own Pair<bool, Leaf>) -> i64 {
    match own value {
        Pair { left: present, right: Leaf { head: payload } } =>
            if present && byte_len(bytes_as_slice(payload)) > 0usize { 1 } else { 0 },
    }
}

@id("surface.make")
fn make(input: borrow Slice<u8>) -> Pair<bool, Leaf> {
    Pair<bool, Leaf> {
        left: byte_len(input) > 0usize,
        right: Leaf { head: bytes_copy(input) },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// The same parameter type, borrowed instead of owned.
const BORROWED: &str = r#"
module test.public_generic_surface;

@id("surface.leaf")
record Leaf {
    @id("surface.leaf.head")
    head: Bytes,
}

@id("surface.pair")
record Pair<T, U> {
    @id("surface.pair.left")
    left: T,
    @id("surface.pair.right")
    right: U,
}

@id("surface.take")
fn take(value: borrow Pair<Leaf, bool>) -> i64 {
    match borrow value {
        Pair { left: Leaf { head: _payload }, right: present } => if present { 1 } else { 0 },
    }
}

@id("surface.make")
fn make(input: borrow Slice<u8>) -> Pair<Leaf, bool> {
    Pair<Leaf, bool> {
        left: Leaf { head: bytes_copy(input) },
        right: byte_len(input) > 0usize,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// The same signature with an `i32` result.
const RESULT_I32: &str = r#"
module test.public_generic_surface;

@id("surface.leaf")
record Leaf {
    @id("surface.leaf.head")
    head: Bytes,
}

@id("surface.pair")
record Pair<T, U> {
    @id("surface.pair.left")
    left: T,
    @id("surface.pair.right")
    right: U,
}

@id("surface.take")
fn take(value: own Pair<Leaf, bool>) -> i32 {
    match own value {
        Pair { left: Leaf { head: payload }, right: present } =>
            if present && byte_len(bytes_as_slice(payload)) > 0usize { 1i32 } else { 0i32 },
    }
}

@id("surface.make")
fn make(input: borrow Slice<u8>) -> Pair<Leaf, bool> {
    Pair<Leaf, bool> {
        left: Leaf { head: bytes_copy(input) },
        right: byte_len(input) > 0usize,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// A generic template and an owned-string function, for the selection cases.
const EXTRAS: &str = r#"
module test.public_generic_surface;

@id("surface.leaf")
record Leaf {
    @id("surface.leaf.head")
    head: Bytes,
}

@id("surface.pair")
record Pair<T, U> {
    @id("surface.pair.left")
    left: T,
    @id("surface.pair.right")
    right: U,
}

@id("surface.relay")
fn relay<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> { value }

@id("surface.text")
fn text(value: string) -> i64 { 0 }

@id("surface.count")
fn count(input: borrow Slice<u8>) -> usize { byte_len(input) }

@id("surface.take")
fn take(value: own Pair<Leaf, bool>) -> i64 {
    match own value {
        Pair { left: Leaf { head: payload }, right: present } =>
            if present && byte_len(bytes_as_slice(payload)) > 0usize { 1 } else { 0 },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const PAIR_TERM: &str = "@12:surface.pair<@12:surface.leaf<>,bool>";
const LEAF_TERM: &str = "@12:surface.leaf<>";

fn resolved(source: &str) -> hir::ResolvedProgram {
    let parsed = parse(source, Path::new("surface.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn selection(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

fn surface(source: &str, ids: &[&str]) -> CandidateSurface {
    CandidateSurface::derive(&resolved(source), &selection(ids)).unwrap()
}

fn base() -> CandidateSurface {
    surface(BASE, &["surface.take", "surface.make"])
}

fn reasons(report: &CompatibilityReport) -> Vec<&'static str> {
    let mut found = report
        .findings()
        .iter()
        .map(|finding| finding.reason.text())
        .collect::<Vec<_>>();
    found.sort_unstable();
    found.dedup();
    found
}

fn subjects(report: &CompatibilityReport, reason: Reason) -> Vec<&str> {
    report
        .findings()
        .iter()
        .filter(|finding| finding.reason == reason)
        .map(|finding| finding.subject.as_str())
        .collect()
}

#[test]
fn a_candidate_surface_describes_entries_positions_and_reachable_instances() {
    let surface = base();
    assert_eq!(
        surface.entries().keys().collect::<Vec<_>>(),
        vec!["surface.make", "surface.take"]
    );
    let take = &surface.entries()["surface.take"];
    assert_eq!(take.parameters.len(), 1);
    assert_eq!(take.parameters[0].ownership, "own");
    assert_eq!(take.parameters[0].value.kind, "data");
    assert_eq!(take.parameters[0].value.term, PAIR_TERM);
    assert_eq!(take.result.term, "i64");
    assert!(take.result.instance_digest.is_none());

    let make = &surface.entries()["surface.make"];
    assert_eq!(make.parameters[0].value.kind, "borrowed_byte_view");
    assert_eq!(make.parameters[0].value.term, "view:slice-u8");
    assert!(make.parameters[0].value.term_digest.is_none());
    assert_eq!(make.result.term, PAIR_TERM);

    assert_eq!(
        surface.instances().keys().collect::<Vec<_>>(),
        vec![LEAF_TERM, PAIR_TERM],
        "the reachable closure carries the nested record as well"
    );
    assert_eq!(
        surface.instances()[PAIR_TERM].owned_leaves,
        vec!["@17:surface.pair.left/@17:surface.leaf.head"]
    );
    assert_eq!(
        surface.instances()[LEAF_TERM].owned_leaves,
        vec!["@17:surface.leaf.head"]
    );
    assert_eq!(surface.instances()[PAIR_TERM].template.arity, 2);
    assert_eq!(surface.instances()[LEAF_TERM].template.arity, 0);

    let json: Value = serde_json::from_str(surface.canonical_json().unwrap().trim_end()).unwrap();
    assert_eq!(json["schema"], CANDIDATE_SURFACE_SCHEMA);
    assert_eq!(
        json["admission"],
        "candidate_description_only_no_public_signature_is_admitted"
    );
    assert_eq!(json["support"], "not_assessed");
    assert_eq!(json["publication"], "not_assessed");
    assert!(surface.canonical_json().unwrap().ends_with('\n'));
}

/// Presentation is never compatibility. Renaming both records, their type
/// parameters, their fields, the exports, and their parameters leaves the
/// surface digest and the verdict alone while the rendered bytes do change.
#[test]
fn renaming_presentation_changes_no_identity_and_no_verdict() {
    let before = base();
    let after = surface(RENAMED, &["surface.take", "surface.make"]);

    assert_eq!(after.digest(), before.digest());
    assert_ne!(
        after.canonical_json().unwrap(),
        before.canonical_json().unwrap(),
        "the rendered surface still shows the new presentation names"
    );
    let report = compare(&before, &after);
    assert_eq!(report.verdict(), Verdict::Unchanged);
    assert!(report.findings().is_empty());
    assert_eq!(after.entries()["surface.take"].name, "accept");
    assert_eq!(after.instances()[LEAF_TERM].template.name, "Node");
    assert_eq!(
        after.instances()[PAIR_TERM]
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
}

#[test]
fn an_added_export_is_compatible_and_a_removed_one_is_breaking() {
    let one = surface(BASE, &["surface.take"]);
    let both = base();

    let added = compare(&one, &both);
    assert_eq!(added.verdict(), Verdict::Compatible);
    assert_eq!(reasons(&added), vec!["export_added"]);
    assert_eq!(subjects(&added, Reason::ExportAdded), vec!["surface.make"]);

    let removed = compare(&both, &one);
    assert_eq!(removed.verdict(), Verdict::Breaking);
    assert_eq!(
        reasons(&removed),
        vec!["export_removed"],
        "both instances stay reachable through the remaining export"
    );
}

/// Dropping the only export that reaches an instance also reports the
/// reachability change, and that report stays informational: its cause is
/// already classified at the export.
#[test]
fn losing_the_last_reference_reports_reachability_without_double_counting() {
    let before = surface(EXTRAS, &["surface.take"]);
    let after = surface(EXTRAS, &["surface.count"]);
    let report = compare(&before, &after);
    assert_eq!(report.verdict(), Verdict::Breaking);
    assert_eq!(
        reasons(&report),
        vec![
            "export_added",
            "export_removed",
            "reachable_instance_removed"
        ]
    );
    let mut removed = subjects(&report, Reason::ReachableInstanceRemoved);
    removed.sort_unstable();
    assert_eq!(removed, vec![LEAF_TERM, PAIR_TERM]);
    assert_eq!(
        Reason::ReachableInstanceRemoved.verdict(),
        Verdict::Compatible
    );
}

/// A permuted argument vector is a different instance: the parameter term
/// changes and the reachable closure swaps one instance for another.
#[test]
fn permuting_type_arguments_is_breaking() {
    let before = surface(BASE, &["surface.take"]);
    let after = surface(PERMUTED, &["surface.take"]);
    let report = compare(&before, &after);
    assert_eq!(report.verdict(), Verdict::Breaking);
    assert_eq!(
        reasons(&report),
        vec![
            "parameter_type_changed",
            "reachable_instance_added",
            "reachable_instance_removed",
        ]
    );
    assert_eq!(
        after.entries()["surface.take"].parameters[0].value.term,
        "@12:surface.pair<bool,@12:surface.leaf<>>"
    );
}

/// Ownership is part of the signature, not a detail of the body.
#[test]
fn changing_a_parameter_ownership_mode_is_breaking() {
    let before = surface(BASE, &["surface.take"]);
    let after = surface(BORROWED, &["surface.take"]);
    let report = compare(&before, &after);
    assert_eq!(report.verdict(), Verdict::Breaking);
    assert_eq!(reasons(&report), vec!["parameter_ownership_changed"]);
    assert_eq!(
        subjects(&report, Reason::ParameterOwnershipChanged),
        vec!["surface.take#0"]
    );
}

#[test]
fn changing_the_result_type_is_breaking() {
    let before = surface(BASE, &["surface.take"]);
    let after = surface(RESULT_I32, &["surface.take"]);
    let report = compare(&before, &after);
    assert_eq!(report.verdict(), Verdict::Breaking);
    assert_eq!(reasons(&report), vec!["result_type_changed"]);
}

/// The rule that is stricter than source compatibility: a Copy field added to
/// a reachable record changes the substituted field inventory a foreign
/// consumer reads, even though every canonical term, the parameter position,
/// and the owned-leaf shape are untouched.
#[test]
fn adding_a_copy_field_to_a_reachable_record_is_breaking() {
    let before = surface(BASE, &["surface.take"]);
    let after = surface(COPY_FIELD, &["surface.take"]);
    let report = compare(&before, &after);
    assert_eq!(report.verdict(), Verdict::Breaking);
    assert_eq!(reasons(&report), vec!["instance_fields_changed"]);
    assert_eq!(
        subjects(&report, Reason::InstanceFieldsChanged),
        vec![LEAF_TERM],
        "the finding lands on the record that changed, not on every mention"
    );
    assert_eq!(
        after.entries()["surface.take"].parameters[0].value.term,
        before.entries()["surface.take"].parameters[0].value.term,
        "the term is unchanged: the field tree is what moved"
    );
    assert_eq!(
        after.instances()[LEAF_TERM].owned_leaves,
        before.instances()[LEAF_TERM].owned_leaves,
        "a Copy field must not change the owned-leaf shape"
    );
    assert_ne!(after.digest(), before.digest());
}

/// An added owned field moves the owned-leaf shape as well, in the nested
/// record and in every instance that reaches it, so neither rule hides the
/// other.
#[test]
fn adding_an_owned_field_changes_the_owned_leaf_shape_too() {
    let before = surface(BASE, &["surface.take"]);
    let after = surface(OWNED_FIELD, &["surface.take"]);
    let report = compare(&before, &after);
    assert_eq!(report.verdict(), Verdict::Breaking);
    assert_eq!(
        reasons(&report),
        vec!["instance_fields_changed", "instance_owned_leaves_changed"]
    );
    let mut leaves = subjects(&report, Reason::InstanceOwnedLeavesChanged);
    leaves.sort_unstable();
    assert_eq!(leaves, vec![LEAF_TERM, PAIR_TERM]);
    assert_eq!(
        after.instances()[PAIR_TERM].owned_leaves,
        vec![
            "@17:surface.pair.left/@17:surface.leaf.head",
            "@17:surface.pair.left/@18:surface.leaf.extra",
        ]
    );
}

/// Selection is by persistent identity and fails closed on everything else,
/// including a generic template: a public surface names no type parameters, so
/// describing one instantiation of a template would be an invention.
#[test]
fn selection_fails_closed() {
    let program = resolved(BASE);
    let cases: Vec<(Vec<String>, &str)> = vec![
        (Vec::new(), "no export was selected"),
        (
            selection(&["surface.take", "surface.take"]),
            "an export was selected twice",
        ),
        (
            selection(&["surface.absent"]),
            "is not a checked declaration",
        ),
        (selection(&["take"]), "is not a checked declaration"),
        (selection(&["surface.pair"]), "is not a checked declaration"),
    ];
    for (ids, expected) in cases {
        let error =
            CandidateSurface::derive(&program, &ids).expect_err(&format!("{ids:?} must fail"));
        assert_eq!(error.code, INVALID_SELECTION, "{ids:?}");
        assert!(
            error.message.contains(expected),
            "{ids:?} reported {}",
            error.message
        );
    }

    let extras = resolved(EXTRAS);
    let error = CandidateSurface::derive(&extras, &selection(&["surface.relay"]))
        .expect_err("a generic template is not a candidate export");
    assert_eq!(error.code, INVALID_SELECTION);
    assert!(
        error.message.contains("generic template"),
        "got {}",
        error.message
    );

    let oversized = (0..=MAX_SELECTED_EXPORTS)
        .map(|index| format!("surface.take{index}"))
        .collect::<Vec<_>>();
    assert_eq!(
        CandidateSurface::derive(&program, &oversized)
            .unwrap_err()
            .code,
        SURFACE_CAPACITY
    );
}

/// A position the grammar does not admit is a grammar rejection, not a
/// silently described surface.
#[test]
fn a_position_outside_the_grammar_rejects_with_its_grammar_reason() {
    let extras = resolved(EXTRAS);
    let error = CandidateSurface::derive(&extras, &selection(&["surface.text"]))
        .expect_err("`string` is outside the grammar vocabulary");
    assert_eq!(error.code, crate::public_generic_type::REJECTED_TYPE);
    assert!(
        error.message.ends_with("owned_string"),
        "got {}",
        error.message
    );
}

/// Replay compares bytes, on both artifacts.
#[test]
fn replay_requires_byte_equality() {
    let before = surface(BASE, &["surface.take"]);
    let after = base();
    let surface_bytes = before.canonical_json().unwrap();
    before.verify(&surface_bytes).unwrap();
    assert_eq!(
        before.verify(surface_bytes.trim_end()).unwrap_err().code,
        SURFACE_REPLAY_MISMATCH,
        "a missing trailing newline is a mismatch"
    );
    assert_eq!(
        before
            .verify(&after.canonical_json().unwrap())
            .unwrap_err()
            .code,
        SURFACE_REPLAY_MISMATCH
    );

    let comparison = compare(&before, &after).canonical_json().unwrap();
    verify_comparison(&before, &after, &comparison).unwrap();
    assert_eq!(
        verify_comparison(&after, &before, &comparison)
            .unwrap_err()
            .code,
        COMPARISON_REPLAY_MISMATCH,
        "the comparison is directional"
    );
    assert_eq!(
        verify_comparison(&before, &after, "{}\n").unwrap_err().code,
        COMPARISON_REPLAY_MISMATCH
    );
}

/// The report is a classification and says so: no version bump, no support
/// claim, no runtime observation follows from it.
#[test]
fn the_report_infers_no_version_support_or_runtime_decision() {
    let before = surface(BASE, &["surface.take"]);
    let report = compare(&before, &base());
    let json: Value = serde_json::from_str(report.canonical_json().unwrap().trim_end()).unwrap();
    assert_eq!(json["schema"], COMPATIBILITY_SCHEMA);
    assert_eq!(json["semantic_version_decision"], "not_inferred");
    assert_eq!(json["support"], "not_assessed");
    assert_eq!(json["publication"], "not_assessed");
    assert_eq!(json["runtime"], "not_observed");
    assert_eq!(
        json["comparison_basis"],
        "identity_bearing_facts_only_presentation_names_excluded"
    );
    assert_eq!(json["before_surface_digest"], before.digest());
    for finding in json["findings"].as_array().unwrap() {
        assert!(finding["reason"].is_string());
        assert!(finding["verdict"].is_string());
        assert!(finding["subject"].is_string());
    }
}

/// The reason vocabulary is closed, distinct, and mapped to exactly the
/// verdict weights the rules claim.
#[test]
fn the_reason_vocabulary_is_closed_and_weighted() {
    const REASONS: [(Reason, &str, Verdict); 13] = [
        (Reason::ExportAdded, "export_added", Verdict::Compatible),
        (Reason::ExportRemoved, "export_removed", Verdict::Breaking),
        (Reason::EffectsChanged, "effects_changed", Verdict::Breaking),
        (
            Reason::ParameterCountChanged,
            "parameter_count_changed",
            Verdict::Breaking,
        ),
        (
            Reason::ParameterOwnershipChanged,
            "parameter_ownership_changed",
            Verdict::Breaking,
        ),
        (
            Reason::ParameterTypeChanged,
            "parameter_type_changed",
            Verdict::Breaking,
        ),
        (
            Reason::ResultTypeChanged,
            "result_type_changed",
            Verdict::Breaking,
        ),
        (
            Reason::InstanceTemplateChanged,
            "instance_template_changed",
            Verdict::Breaking,
        ),
        (
            Reason::InstanceArgumentsChanged,
            "instance_arguments_changed",
            Verdict::Breaking,
        ),
        (
            Reason::InstanceFieldsChanged,
            "instance_fields_changed",
            Verdict::Breaking,
        ),
        (
            Reason::InstanceOwnedLeavesChanged,
            "instance_owned_leaves_changed",
            Verdict::Breaking,
        ),
        (
            Reason::ReachableInstanceAdded,
            "reachable_instance_added",
            Verdict::Compatible,
        ),
        (
            Reason::ReachableInstanceRemoved,
            "reachable_instance_removed",
            Verdict::Compatible,
        ),
    ];
    let mut spellings = REASONS
        .iter()
        .map(|(reason, text, verdict)| {
            assert_eq!(reason.text(), *text);
            assert_eq!(reason.verdict(), *verdict);
            *text
        })
        .collect::<Vec<_>>();
    spellings.sort_unstable();
    let count = spellings.len();
    spellings.dedup();
    assert_eq!(spellings.len(), count, "reason spellings must be distinct");
    assert!(Verdict::Breaking > Verdict::Compatible);
    assert!(Verdict::Compatible > Verdict::Unchanged);
    assert_eq!(Verdict::Unchanged.text(), "unchanged");
}

/// Comparing a surface with itself is `Unchanged`, and both artifacts are
/// deterministic across repeated computation.
#[test]
fn comparison_is_reflexive_and_deterministic() {
    let surface = base();
    let report = compare(&surface, &surface);
    assert_eq!(report.verdict(), Verdict::Unchanged);
    assert!(report.findings().is_empty());
    assert_eq!(
        compare(&surface, &surface).canonical_json().unwrap(),
        report.canonical_json().unwrap()
    );
    assert_eq!(
        base().canonical_json().unwrap(),
        surface.canonical_json().unwrap()
    );
    assert_eq!(base().digest(), surface.digest());
}
