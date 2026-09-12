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

/// Issue #188/#241: `MAX_SOURCE_NESTING` is also enforced by a *token-level*
/// pre-check (`parser::entry::reject_token_nesting`) that runs before any AST
/// exists. It treats every `<` token as an opening delimiter matched only by
/// a literal `>`, exactly like `(`, `{`, and `[`, and never resets between
/// sibling declarations. So a file of many small, independently shallow
/// functions -- each contributing one unmatched scalar `<` comparison and
/// none of them a generic bracket -- exhausts the same 128 budget as real
/// expression nesting, even though no single function nests anywhere near
/// that deep. This is a materially cheaper and easier ceiling to hit than
/// true nesting depth for ordinary validation/classification code, and it is
/// unrelated to the per-function AST walk this module's other tests cover.
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
fn many_unmatched_less_than_tokens_exhaust_the_token_level_nesting_precheck() {
    // No single function here nests more than a handful of levels deep; only
    // the file-wide count of unmatched `<` tokens crosses `MAX_SOURCE_NESTING`.
    let source = many_shallow_functions_each_with_one_unmatched_less_than(MAX_SOURCE_NESTING + 4);
    assert_eq!(code(&source), "SPX-P207");
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
