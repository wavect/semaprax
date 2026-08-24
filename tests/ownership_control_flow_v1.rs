//! Ownership control-flow regression battery v1.
//!
//! Pins the exact move-safety behavior across every admitted control-flow
//! shape: lazy boolean operands, match scrutinees, refutable-match guards,
//! branch-block joins, while-loop admission, explicit mutation boundaries,
//! and `?` propagation with live resources. Each diagnostic case names its
//! stable code; each success case must verify with zero errors.
//!
//! These regressions back the completion-matrix row "Unique ownership and
//! move safety" for the roadmap item "Complete ownership/lifetime/region
//! analysis across control flow": every route a moved resource could take
//! through an expression tree is either rejected with a stable compile-time
//! diagnostic or proven safe on both paths.

use std::path::Path;

use semaprax::{parse, verify};

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("ownership_control_flow_v1.spx")).unwrap();
    verify::verify(&program)
}

fn codes(source: &str) -> Vec<String> {
    diagnostics(source)
        .iter()
        .map(|item| item.code.to_owned())
        .collect()
}

const HEADER: &str = r#"
module test.ownership_flow;
@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
@id("buffer.inspect")
fn inspect(buffer: borrow Buffer) -> i64 { 1 }
@id("buffer.consume")
fn consume(buffer: own Buffer) -> i64 { 1 }
"#;

const MAIN: &str = r#"
@id("app.main")
fn main() -> i64 { 42 }
"#;

#[test]
fn conditional_move_on_untaken_lazy_operand_without_use_is_admitted() {
    let source = format!(
        "{HEADER}\n@id(\"flow.lazy\")\nfn lazy(flag: bool, buffer: own Buffer) -> bool {{ flag || consume(buffer) == 1 }}\n{MAIN}"
    );
    let found = diagnostics(&source);
    assert!(
        found.iter().all(|item| !item.severity.is_error()),
        "a conditional move with no later use is admitted, got {found:?}"
    );
}

#[test]
fn move_in_lazy_boolean_lhs_is_definite_and_rhs_use_is_rejected() {
    let source = format!(
        "{HEADER}\n@id(\"flow.lazy_lhs\")\nfn lazy_lhs(buffer: own Buffer) -> bool {{ consume(buffer) == 1 && inspect(buffer) == 1 }}\n{MAIN}"
    );
    let found = codes(&source);
    assert!(found.iter().any(|code| code == "SPX-O101"));
    assert!(!found.iter().any(|code| code == "SPX-O107"));
}

#[test]
fn sequential_lazy_moves_are_a_definite_use_after_move() {
    let source = format!(
        "{HEADER}\n@id(\"flow.lazy_seq\")\nfn lazy_seq(buffer: own Buffer) -> bool {{ consume(buffer) == 1 || consume(buffer) == 2 }}\n{MAIN}"
    );
    assert!(codes(&source).iter().any(|code| code == "SPX-O101"));
}

#[test]
fn conditionally_moved_operand_use_after_or_join_is_conditional() {
    let source = format!(
        "{HEADER}\n@id(\"flow.lazy_both\")\nfn lazy_both(flag: bool, buffer: own Buffer) -> bool {{ (flag && consume(buffer) == 1) || inspect(buffer) == 2 }}\n{MAIN}"
    );
    let found = codes(&source);
    assert!(found.iter().any(|code| code == "SPX-O107"));
    assert!(!found.iter().any(|code| code == "SPX-O101"));
}

#[test]
fn move_as_match_scrutinee_invalidates_the_place() {
    let source = format!(
        "{HEADER}\n@id(\"flow.scrutinee\")\nfn scrutinee(buffer: own Buffer) -> i64 {{ let value = match consume(buffer) {{ 0 => 5, n => n, }}; inspect(buffer) + value }}\n{MAIN}"
    );
    assert!(codes(&source).iter().any(|code| code == "SPX-O101"));
}

#[test]
fn move_inside_refutable_match_guard_is_conditional() {
    let source = format!(
        "{HEADER}\n@id(\"flow.guard\")\nfn guarded(code: i64, buffer: own Buffer) -> i64 {{ let value = match code {{ n if n == consume(buffer) => 1, _ => 2, }}; inspect(buffer) + value }}\n{MAIN}"
    );
    let found = codes(&source);
    assert!(found.iter().any(|code| code == "SPX-O107"));
    assert!(!found.iter().any(|code| code == "SPX-O101"));
}

#[test]
fn move_inside_branch_block_joins_conditionally() {
    let source = format!(
        "{HEADER}\n@id(\"flow.branch_block\")\nfn nested(flag: bool, buffer: own Buffer) -> i64 {{ let inner = if flag {{ let moved = consume(buffer); moved }} else {{ 0 }}; inspect(buffer) + inner }}\n{MAIN}"
    );
    assert!(codes(&source).iter().any(|code| code == "SPX-O107"));
}

#[test]
fn consuming_on_every_branch_then_using_after_join_is_definite() {
    let source = format!(
        "{HEADER}\n@id(\"flow.both_consume_then_use\")\nfn both(flag: bool, buffer: own Buffer) -> i64 {{ let value = if flag {{ consume(buffer) }} else {{ consume(buffer) }}; inspect(buffer) + value }}\n{MAIN}"
    );
    let found = codes(&source);
    assert!(found.iter().any(|code| code == "SPX-O101"));
    assert!(!found.iter().any(|code| code == "SPX-O107"));
}

#[test]
fn consuming_on_exactly_one_branch_and_ending_there_is_valid() {
    let source = format!(
        "{HEADER}\n@id(\"flow.one_branch_ok\")\nfn one(flag: bool, buffer: own Buffer) -> i64 {{ if flag {{ consume(buffer) }} else {{ inspect(buffer) }} }}\n{MAIN}"
    );
    let found = diagnostics(&source);
    assert!(
        found.iter().all(|item| !item.severity.is_error()),
        "expected no ownership errors, got {found:?}"
    );
}

#[test]
fn alias_double_move_is_a_definite_error() {
    let source = format!(
        "{HEADER}\n@id(\"flow.alias\")\nfn alias(buffer: own Buffer) -> i64 {{ let first = buffer; let second = buffer; consume(first) + consume(second) }}\n{MAIN}"
    );
    let found = codes(&source);
    assert_eq!(
        found.iter().filter(|code| *code == "SPX-O101").count(),
        1,
        "exactly the second move is rejected"
    );
}

#[test]
fn while_loop_rejects_resource_ownership_changes_fail_closed() {
    let source = format!(
        "{HEADER}\n@id(\"flow.loop_move\")\nfn loop_move(flag: bool, buffer: own Buffer) -> i64 {{ let mut guard = flag; while guard && consume(buffer) == 1 {{ guard = false; guard }} 0 }}\n{MAIN}"
    );
    let found = codes(&source);
    assert!(found.iter().any(|code| code == "SPX-T252"));
}

#[test]
fn resource_reassignment_stays_outside_explicit_mutation_v1() {
    let source = format!(
        "{HEADER}\n@id(\"flow.reassign\")\nfn reassign(other: own Buffer) -> i64 {{ let mut buffer = other; let first = consume(buffer); buffer = other; first + consume(buffer) }}\n{MAIN}"
    );
    let found = codes(&source);
    assert!(found.iter().any(|code| code == "SPX-U105"));
    assert!(found.iter().any(|code| code == "SPX-O101"));
}

#[test]
fn question_propagation_with_live_resource_binding_is_fail_closed() {
    let source = format!(
        "{HEADER}\n@id(\"flow.question\")\nfn question(buffer: own Buffer) -> Result<i64, i64> {{ let moved = buffer; let value = maybe()?; Ok(value + consume(moved)) }}\n@id(\"flow.maybe\")\nfn maybe() -> Result<i64, i64> {{ Ok(1) }}\n{MAIN}"
    );
    assert!(codes(&source).iter().any(|code| code == "SPX-T218"));
}

#[test]
fn borrowed_parameter_cannot_be_transferred_as_owned() {
    let source = format!(
        "{HEADER}\n@id(\"flow.borrow_transfer\")\nfn transfer(buffer: borrow Buffer) -> i64 {{ consume(buffer) }}\n{MAIN}"
    );
    assert!(codes(&source).iter().any(|code| code == "SPX-O102"));
}
