//! Full-HIR regression: the arena resolver against the recursive reference,
//! whose text paths keep the formatting encoder, on identical source.
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::hir::{
    Binding, DeclarationIndex, FunctionExecutionId, FunctionInstanceId, FunctionResolutionWork,
    Resolver,
};

/// Statement, argument, field and arm indices cross 9/10, 99/100 and
/// 999/1000; nested branches build long prefixes.
fn source() -> String {
    let mut source = String::from("module test.path_arena_oracle;\n");
    source.push_str("@id(\"wide.record\") record Wide {");
    for field in 0..11 {
        write!(source, " @id(\"wide.record.f{field}\") f{field}: i64,").unwrap();
    }
    source.push_str(" }\n");
    source.push_str(
        "@id(\"choice\") variant Choice {\n  @id(\"choice.a\") A { @id(\"choice.a.v\") v: i64, \
         @id(\"choice.a.w\") w: i64, },\n  @id(\"choice.b\") B,\n}\n",
    );
    let parameters = (0..11)
        .map(|index| format!("a{index}: i64"))
        .collect::<Vec<_>>();
    writeln!(
        source,
        "@id(\"many\") fn many({}) -> i64 {{ a0 + a10 }}",
        parameters.join(", ")
    )
    .unwrap();
    source.push_str("@id(\"scalar\") fn scalar(value: i64) -> i64 {\n  match value {");
    for arm in 0..10 {
        write!(source, " {arm} => {arm},").unwrap();
    }
    source.push_str(" 10 if value > 3 => 10, 11 | 12 => 11, n => n, }\n}\n");
    source.push_str("@id(\"deep\") fn deep(x: i64) -> i64 {\n");
    for level in 0..12 {
        write!(source, "if x > {level} {{ ").unwrap();
    }
    source.push_str("-x");
    for level in 0..12 {
        write!(source, " }} else {{ {level} }}").unwrap();
    }
    source.push_str("\n}\n");
    source.push_str("@id(\"wide\") fn wide(flag: bool, row: Wide, choice: Choice) -> i64 {\n");
    source.push_str("  let s0 = 0;\n");
    for index in 1..=1000 {
        writeln!(source, "  let s{index} = s{} + 1;", index - 1).unwrap();
    }
    let arguments = (0..11).map(|index| format!("s{index}")).collect::<Vec<_>>();
    writeln!(source, "  let mut total = many({});", arguments.join(", ")).unwrap();
    source.push_str(
        "  while total < 0 { total = total + 1; if total == 2 { continue; } else { 0 } \
         if total == 3 { break; } else { 0 } 0 }\n",
    );
    source.push_str("  let updated = row with { f10: total };\n  let rebuilt = Wide {");
    for field in 0..10 {
        write!(source, " f{field}: s{},", 990 + field).unwrap();
    }
    source.push_str(" f10: updated.f10 };\n");
    source.push_str(
        "  let picked = match choice { Choice::A { v, w } => v + w, \
         Choice::B {} => -rebuilt.f9, };\n",
    );
    source.push_str("  if flag && !false { return picked; } else { 0 }\n");
    source.push_str("  let outcome = -(picked * 2) + scalar(s999) + deep(s1000);\n  outcome\n}\n");
    // An actual new whole-row argument reaches the same path-text receiver
    // through paid arena spelling and the saved recursive borrowed-text route.
    source.push_str(
        "@id(\"loan.row\") record LoanRow { @id(\"loan.row.n\") n:i64, \
         @id(\"loan.row.text\") text:string, }\n\
         @id(\"loan.read\") fn loan_read(row:borrow LoanRow)->i64 {row.n+1}\n\
         @id(\"loan.loop\") fn loan_loop()->i64 {\n\
         let rows=vec_push<LoanRow>(vec_with_capacity<LoanRow>(1usize),LoanRow{n:7,text:\"x\"});\n\
         let mut total=0; for borrow row in rows {total=total+loan_read(row);0} total }\n",
    );
    source.push_str("@id(\"main\") fn main() -> i64 { 0 }\n");
    source
}

fn assert_same(
    iterative: Result<crate::hir::ResolvedExpr, crate::diagnostic::Diagnostic>,
    recursive: Result<crate::hir::ResolvedExpr, crate::diagnostic::Diagnostic>,
) -> bool {
    match (iterative, recursive) {
        (Ok(iterative), Ok(recursive)) => {
            assert_eq!(iterative, recursive);
            true
        }
        (Err(iterative), Err(recursive)) => {
            assert_eq!(iterative.code, recursive.code);
            assert_eq!(iterative.message, recursive.message);
            assert_eq!(iterative.span, recursive.span);
            false
        }
        (iterative, recursive) => {
            panic!("outcome differs: iterative={iterative:?}, recursive={recursive:?}")
        }
    }
}

fn compare() {
    let source = source();
    // Preserve the original rejected boundary independently: a unary operator
    // after the guard continues value evaluation instead of starting an item.
    let crossed = source.replace(
        "let outcome = -(picked * 2) + scalar(s999) + deep(s1000);\n  outcome",
        "-(picked * 2) + scalar(s999) + deep(s1000)",
    );
    assert_ne!(crossed, source);
    let crossed = crate::parse(&crossed, Path::new("path-arena-crossed.spx")).unwrap();
    let errors = crate::hir::resolve(&crossed).unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-T312"),
        "{errors:?}"
    );
    let parsed = crate::parse(&source, Path::new("path-arena-oracle.spx")).unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let again = crate::hir::resolve(&parsed).unwrap();
    assert_eq!(
        crate::cache_codec::encode(&resolved).unwrap(),
        crate::cache_codec::encode(&again).unwrap(),
        "deterministic cache bytes"
    );
    let resolver = Resolver {
        program: &parsed,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(&parsed),
        declarations: DeclarationIndex::from_verified(&parsed).unwrap(),
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    };
    let generic = FunctionExecutionId::Generic(FunctionInstanceId("λ.instance.".repeat(12)));
    let mut matched = 0;
    for source_function in &parsed.functions {
        let resolved_function = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == source_function.stable_id)
            .expect("every fixture function resolves");
        let bindings: BTreeMap<String, Binding> = source_function
            .params
            .iter()
            .zip(&resolved_function.params)
            .map(|(source, resolved)| {
                (
                    source.name.clone(),
                    Binding {
                        traversal_row: None,
                        id: resolved.id.clone(),
                        ty: resolved.ty.clone(),
                        ownership: resolved.ownership,
                        mutable: false,
                    },
                )
            })
            .collect();
        let monomorphic = FunctionExecutionId::Monomorphic(resolved_function.id.clone());
        for owner in [&monomorphic, &generic] {
            for root in ["body", "contract.requires.0", "λ-root", ""] {
                let body = &source_function.body;
                let iterative = resolver.resolve_expr_iterative(owner, body, &bindings, root);
                assert_eq!(
                    iterative,
                    crate::hir::borrowed_root_oracle::Resolver(&resolver)
                        .expression(owner, body, &bindings, root),
                    "complete frozen nonrecursive receiver under every path root"
                );
                let recursive =
                    resolver.resolve_expr_recursive_reference(owner, body, &bindings, root);
                let resolved = assert_same(iterative, recursive);
                matched += usize::from(resolved && owner == &monomorphic);
            }
        }
    }
    // Every monomorphic body resolves under every root spelling; generic
    // executions compare whichever outcome both evaluators select.
    assert_eq!(matched, parsed.functions.len() * 4);
}

#[test]
fn arena_resolver_matches_text_reference_across_roots_owners_and_widths() {
    // The recursive reference needs more than the default test stack.
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(compare)
        .unwrap()
        .join()
        .unwrap();
}

#[path = "path_arena_regression_tests/whole_program.rs"]
mod whole_program;
