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
