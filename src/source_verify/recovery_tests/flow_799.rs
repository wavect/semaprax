//! Conditional and match refusal provenance; full old receivers remain executable.
use super::{codes, receive_route};

const PREFIX: &str = "module t; @id(\"choice\") variant Choice { @id(\"none\") None, @id(\"some\") Some { @id(\"value\") value: i64, }, } ";
fn with_choice(body: &str) -> String {
    format!("{PREFIX} fn main(choice: Choice) -> i64 {{ {body} }}")
}

#[test]
fn refused_conditions_and_incompatible_branches_never_publish_checked_results() {
    for (body, expected) in [
        ("if missing { 1 } else { 0 }", vec!["SPX-T202"]),
        ("if 1 { 1 } else { 0 }", vec!["SPX-T210"]),
        ("if true { 1 } else { false }", vec!["SPX-T211"]),
        (
            "if missing { let x = 1; x } else { let y = 0; y }",
            vec!["SPX-T202"],
        ),
    ] {
        let source = super::source(body);
        let old = receive_route(&source, 3);
        let current = receive_route(&source, 0);
        assert!(old.0.is_some(), "{body}");
        assert!(current.0.is_none(), "{body}");
        assert_eq!(codes(&current.1), expected);
        assert_eq!(current.1, old.1);
    }
}

#[test]
fn nested_conditional_refusal_preserves_both_branch_errors_and_outer_sibling() {
    let source = super::source("(if 1 { missing_then } else { missing_else }) + missing_sibling");
    let old = receive_route(&source, 3);
    let current = receive_route(&source, 0);
    assert_eq!(
        codes(&current.1),
        ["SPX-T210", "SPX-T202", "SPX-T202", "SPX-T202"]
    );
    assert_eq!(current.1, old.1);
    assert!(current.0.is_none());
}

#[test]
fn unknown_match_scrutinee_supplies_no_pattern_type_and_keeps_independent_body_error() {
    let source = with_choice(
        "match missing { Choice::Some { value: item } => item, Choice::None {} => independent, }",
    );
    let old = receive_route(&source, 3);
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-T202"]);
    assert!(current.0.is_none());
    assert_eq!(current.1[0], old.1[0]);
    assert_eq!(current.1[1], *old.1.last().unwrap());
    assert!(old.1.iter().any(|diagnostic| diagnostic.code == "SPX-M103"));
}

#[test]
fn unknown_match_keeps_independent_guard_errors_and_refused_names_stay_arm_local() {
    let source = with_choice(
        "match missing { Choice::Some { value: item } if item > 0 => item, _ => item, }",
    );
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-T202"]);
    assert!(current.0.is_none());
    let source = with_choice(
        "match missing { Choice::Some { value: item } if independent_guard => item, _ => 0, }",
    );
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-T202"]);
}

#[test]
fn unknown_match_keeps_real_duplicate_fields_bindings_and_outer_shadowing() {
    let source = with_choice("let item = 1; match missing { Choice::Some { value: item, value: duplicate } => 0, _ => 0, }");
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-M104", "SPX-M104"]);
    assert!(current.0.is_none());
    let source = with_choice("match missing { Choice::Some { value: item } => item, Choice::Some { value: other } => other, _ => 0, }");
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-M102"]);
}

#[test]
fn known_match_missing_case_and_admitted_conditional_receivers_remain_exact() {
    let source = with_choice("match choice { Choice::None {} => 0, }");
    let old = receive_route(&source, 3);
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-M101"]);
    assert_eq!(current, old);
    for body in [
        "if true { 1 } else { 0 }",
        "if false { let x = 1; x } else { let y = 0; y }",
        "if true { 1 } else { 0 } + 2",
    ] {
        let source = super::source(body);
        let old = receive_route(&source, 3);
        assert!(old.1.is_empty());
        assert_eq!(receive_route(&source, 0), old);
    }
}

#[test]
fn complete_matching_and_arm_receivers_have_exact_full_source_inverses() {
    let mut matching =
        ci_str!("../iterative/matching/matching_before_799.rs").replace("before_match_799_", "");
    matching = matching.replace(
        "            variant_name: variant_name.into(),\n",
        "            variant_name,\n",
    );
    assert_eq!(
        matching,
        ci_str!("../../../docs/evidence/conditional-refusal-799/iterative__matching.rs.before")
    );
    let arms = ci_str!("../iterative/match_arms/arms_before_799.rs")
        .replace("before_match_799_", "")
        .replace(
            "(state.variant_name.as_ref(), state.declared_cases)",
            "(&state.variant_name, state.declared_cases)",
        );
    assert_eq!(
        arms,
        ci_str!("../../../docs/evidence/conditional-refusal-799/iterative__match_arms.rs.before")
    );
}

#[test]
fn read_only_scrutinee_origin_is_frozen_before_later_pattern_and_body_errors() {
    let mut recovery = crate::source_verify::recovery::Recovery::default();
    recovery.fork(None, 0);
    let initial = recovery.checkpoint(&[]);
    let mut diagnostics = vec![crate::diagnostic::Diagnostic::io_static(
        "SPX-T202",
        "root refusal",
    )];
    recovery.refused_let(0, "refused", initial, &diagnostics);
    let scrutinee = recovery.checkpoint(&diagnostics);
    assert!(recovery.reads_refused(0, "refused"));
    let origin = recovery.refusal_origin(scrutinee, &diagnostics).unwrap();
    assert_eq!(origin, 0);
    diagnostics.push(crate::diagnostic::Diagnostic::io_static(
        "SPX-M104",
        "later independent pattern error",
    ));
    recovery.fork(Some(0), 1);
    recovery.refused_origin(1, "item", origin);
    let item = recovery.checkpoint(&diagnostics);
    assert!(recovery.reads_refused(1, "item"));
    assert_eq!(recovery.refusal_origin(item, &diagnostics), Some(0));
}

#[test]
fn unknown_match_keeps_independent_aggregate_body_refusal_without_scalar_unreachability() {
    let source = with_choice("match missing { _ => \"body\", }");
    let old = receive_route(&source, 3);
    let current = receive_route(&source, 0);
    assert!(current.0.is_none());
    assert_eq!(current.1[0], old.1[0]);
    // String is an admitted arm result. The old scalar-domain refusal is saved,
    // but cannot manufacture an independent aggregate-result diagnostic.
    assert_eq!(current.1, vec![old.1[0].clone()]);
    let source = with_choice("match missing { _ => bytes_zeroed(1usize), }");
    let old = receive_route(&source, 3);
    let current = receive_route(&source, 0);
    assert!(current.0.is_none());
    assert_eq!(current.1[0], old.1[0]);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-T258"]);
    let program = crate::parse(&source, std::path::Path::new("scoped-recovery.spx")).unwrap();
    let crate::ast::ExprKind::Block { tail, .. } = &program.functions[0].body.kind else {
        panic!("block");
    };
    let crate::ast::ExprKind::Match { arms, .. } = &tail.kind else {
        panic!("match");
    };
    assert_eq!(
        current.1[1],
        crate::diagnostic::Diagnostic::error(
            "SPX-T258",
            "aggregate-valued match arms are outside the executable match profile",
            arms[0].value.span
        )
        .at_path("scoped-recovery.spx")
        .with_help("build the aggregate with `if`, or make the match arms extract scalar values")
    );
    assert_eq!(current.1.len(), 2);
    let source = with_choice("match missing { 1 => 0, 1 => 0, _ => 0, _ => 0, }");
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202"]);
    assert!(current.0.is_none());
}

#[test]
fn packed_name_adapter_keeps_complete_saved_legacy_arm_body() {
    let current = include_str!("../recovery_legacy/match_arms.rs").replace(
        "(state.variant_name.as_ref(), state.declared_cases)",
        "(&state.variant_name, state.declared_cases)",
    );
    assert_eq!(current,include_str!("../../../docs/evidence/packed-match-provenance-799/recovery_legacy__match_arms.rs.before"));
}

#[test]
fn complete_legacy_constructor_and_match_checkpoint_adapters_preserve_saved_bodies() {
    let legacy = include_str!("../recovery_legacy.rs");
    let flags =
        "            operators_before_799: false,\n            extended_before_799: false,\n";
    assert_eq!(legacy.matches(flags).count(), 1);
    assert_eq!(
        legacy.replace(flags, ""),
        include_str!("../../../docs/evidence/packed-test-adapters-799/recovery_legacy.rs.before")
    );
    let enter = include_str!("../recovery_legacy/enter.rs");
    let checkpoint =
        "                    checkpoint: self.recovery.checkpoint(self.diagnostics),\n";
    assert_eq!(enter.matches(checkpoint).count(), 1);
    assert_eq!(
        enter.replace(checkpoint, ""),
        include_str!(
            "../../../docs/evidence/packed-test-adapters-799/recovery_legacy__enter.rs.before"
        )
    );
}
