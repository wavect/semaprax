use super::MAX_SOURCE_NESTING;
use crate::parse;

/// A left-associative chain deep enough to exceed `MAX_SOURCE_NESTING`.
fn deep_expression() -> String {
    format!("0{}", " + 1".repeat(MAX_SOURCE_NESTING + 1))
}

fn code(source: &str) -> &'static str {
    parse(source, "deep.spx")
        .expect_err("the source must be rejected")
        .code
}

#[test]
fn contract_clauses_are_walked_as_nesting_roots() {
    // `requires` and `ensures` expressions are compiled like any other, so a
    // walker that only visited function bodies would let unbounded nesting
    // through the front end.
    let deep = deep_expression();
    for clause in [
        format!("requires {deep} > 0"),
        format!("ensures {deep} > 0"),
    ] {
        let source = format!(
            "module test.deep_contract;\n@id(\"app.main\")\nfn main() -> i64\n{clause}\n{{\n0\n}}\n"
        );
        assert_eq!(code(&source), "SPX-P207", "{clause}");
    }
}

/// Issue #247: `parser::entry::reject_token_nesting` is a *token-level*
/// pre-check that runs before any AST exists, guarding the recursive-descent
/// parser's own stack rather than the expression tree `validate_program`
/// (this module's other tests) walks. It used to count every `<` token as an
/// opening delimiter matched only by a literal `>`, exactly like `(`, `{`, and
/// `[`, and never reset between sibling declarations. Since `<` doubles as the
/// plain less-than comparison operator -- which need never be followed by any
/// `>` at all -- a file of many small, independently shallow functions, each
/// contributing one unmatched scalar `<` comparison and none of them a
/// generic bracket, exhausted the same 128 budget as real expression nesting,
/// even though no single function nested anywhere near that deep.
///
/// The fix gives `<`/`>` their own tentative counter that only accumulates
/// while the run since the last unmatched `<` still looks like a
/// generic-argument list (an identifier immediately followed by `<`, then
/// only identifiers/`.`/`,` and further nested `<`/`>` until it closes), and
/// resets the instant a token appears that a real generic-argument list could
/// never contain. See `reject_token_nesting`'s doc comment for the full
/// reasoning and its one remaining known gap.
fn many_shallow_functions_each_with_one_unmatched_less_than(count: usize) -> String {
    let mut source = String::from("module test.many_comparisons;\n\n");
    for i in 0..count {
        source.push_str(&format!(
            "@id(\"test.f{i}\")\nfn f{i}(value: i64) -> i64 {{ if value < {i} {{ value }} else {{ value }} }}\n\n"
        ));
    }
    source.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
    source
}

#[test]
fn many_unmatched_less_than_tokens_no_longer_exhaust_the_token_level_nesting_precheck() {
    // Before the fix for issue #247, this many unmatched `<` tokens spread
    // across independently shallow functions tripped `SPX-P207` even though
    // no single function nests more than a handful of levels deep. This
    // assertion used to pin that defect (`code(&source) == "SPX-P207"`); it
    // is inverted here to `is_ok()` because the token-level counter no longer
    // treats a bare, never-closed `<` comparison as an opening bracket.
    let source = many_shallow_functions_each_with_one_unmatched_less_than(MAX_SOURCE_NESTING + 4);
    assert!(
        parse(&source, "many-comparisons.spx").is_ok(),
        "an unmatched comparison `<` in each of many independent, shallow \
         functions must not exhaust the token-level nesting precheck"
    );
}

#[test]
fn the_same_less_than_count_parses_once_each_is_closed_by_a_literal_greater_than() {
    // Same shallow-function shape and more than twice as many `<` tokens, but
    // each is immediately balanced by a literal `>` in the same condition, so
    // the token-level counter returns to its baseline after every function.
    // This isolates "unmatched `<` count" as the actual trigger, rather than
    // any other property of a file with this many declarations.
    let count = (MAX_SOURCE_NESTING + 4) * 2;
    let mut source = String::from("module test.many_balanced_comparisons;\n\n");
    for i in 0..count {
        source.push_str(&format!(
            "@id(\"test.f{i}\")\nfn f{i}(value: i64) -> i64 {{ if value < {i} && value > -1000000 {{ value }} else {{ value }} }}\n\n"
        ));
    }
    source.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
    assert!(
        parse(&source, "many-balanced.spx").is_ok(),
        "a literal `>` for every `<` must keep the token-level counter at baseline"
    );
}

/// The control that proves the fix narrowed the check's *measurement* rather
/// than deleting its protection: a return type built from genuinely nested
/// generic-argument brackets (`T<T<T<...>>>`), deep enough to exceed
/// `MAX_SOURCE_NESTING`, is still refused with the same `SPX-P207` before the
/// recursive-descent type parser (`Parser::ty`/`Parser::type_arguments` in
/// `parser/types.rs`, which recurses once per nesting level) ever runs.
fn deep_generic_type() -> String {
    let depth = MAX_SOURCE_NESTING + 4;
    format!("{}X{}", "T<".repeat(depth), ">".repeat(depth))
}

#[test]
fn token_level_precheck_still_refuses_genuinely_nested_generic_brackets() {
    let source = format!(
        "module test.deep_generic_type;\n@id(\"app.main\")\nfn main() -> {} {{ 0 }}\n",
        deep_generic_type()
    );
    assert_eq!(code(&source), "SPX-P207");
}

#[test]
fn class_method_bodies_are_walked_as_nesting_roots() {
    // Methods hang off a type declaration rather than `program.functions`, so
    // they are the other root list the bound has to reach. Only the method is
    // deep; the free function is there because a module must declare one.
    let source = format!(
        "module test.deep_method;\n\
         @id(\"test.holder\")\n\
         class Holder {{\n\
         @id(\"test.holder.count\")\n\
         count: i64,\n\
         @id(\"test.holder.deep\")\n\
         fn deep(self: Holder) -> i64\n{{\n{}\n}}\n\
         }}\n\
         @id(\"app.main\")\n\
         fn main() -> i64 {{ 0 }}\n",
        deep_expression()
    );
    assert_eq!(code(&source), "SPX-P207");
}
