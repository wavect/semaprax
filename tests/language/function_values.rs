//! Genuine first-class scalar callable values, independent of backend addresses.
use semaprax::hir::{self, ResolvedExprKind, ResolvedType};

#[path = "function_values/closures.rs"]
mod closures;
#[path = "function_values/native_closures.rs"]
mod native_closures;

const PROGRAM: &str = r#"
module test.function_values;
@id("fv.inc") fn inc(value:i64)->i64 { value + 1 }
@id("fv.dec") fn dec(value:i64)->i64 { value - 1 }
@id("fv.select") fn select(flag:bool)->fn(i64)->i64 {
    if flag { inc } else { dec }
}
@id("fv.apply") fn apply(callback:fn(i64)->i64,value:i64)->i64 { callback(value) }
@id("fv.main") fn main()->i64 {
    let chosen = select(true);
    let copied:fn(i64)->i64 = chosen;
    apply(copied,41)
}
"#;

fn checked(source: &str) -> (semaprax::ast::Program, hir::ResolvedProgram) {
    let ast = semaprax::check(source, "function-values.spx").unwrap();
    let canonical = semaprax::format::canonical(&ast);
    let ast = semaprax::check(&canonical, "function-values.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&ast));
    let resolved = hir::resolve(&ast).unwrap();
    hir::validate(&resolved).unwrap();
    (ast, resolved)
}

#[test]
fn function_values_retain_dynamic_operand_and_exact_targets() {
    let (ast, resolved) = checked(PROGRAM);
    let apply = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "fv.apply")
        .unwrap();
    let mut body = &apply.body;
    while let ResolvedExprKind::Block { tail, .. } = &body.kind {
        body = tail;
    }
    let ResolvedExprKind::Invoke { callable, args } = &body.kind else {
        panic!("expected genuine indirect invocation: {body:?}");
    };
    assert!(matches!(callable.ty, ResolvedType::Function { .. }));
    assert_eq!(args.len(), 1);
    let targets = hir::function_value::target_universe(&resolved);
    assert_eq!(
        targets.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(),
        ["fv.dec", "fv.inc"]
    );
    let graph = semaprax::graph::to_json(&ast).unwrap();
    assert!(graph.contains("semaprax.graph.v36"));
    semaprax::graph::verify_json(&ast, &graph).unwrap();
    assert_eq!(graph, semaprax::graph::to_json(&ast).unwrap());
}

#[test]
fn function_values_cover_all_scalar_signatures_and_arity_edges() {
    for (ty, literal) in [
        ("i64", "7"),
        ("i32", "7i32"),
        ("u8", "7u8"),
        ("usize", "7usize"),
        ("char", "'a'"),
        ("f32", "7.0f32"),
        ("f64", "7.0f64"),
        ("bool", "true"),
    ] {
        checked(&format!(
            r#"module test.function_values;
@id("fv.identity") fn identity(value:{ty})->{ty}{{value}}
@id("fv.probe") fn probe()->{ty}{{let f=identity; f({literal})}}
@id("fv.main") fn main()->i64{{let observed=probe(); 0}}
"#
        ));
    }
    checked(
        r#"module test.function_values;
@id("fv.zero") fn zero()->i64{42}
@id("fv.eight") fn eight(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)->i64{a+b+c+d+e+f+g+h}
@id("fv.main") fn main()->i64{let z=zero; let e=eight; e(z(),1,2,3,4,5,6,7)}
"#,
    );
}

#[test]
fn function_values_binding_shadowing_selects_local_callable() {
    checked(
        &PROGRAM
            .replace("callback:fn(i64)->i64", "inc:fn(i64)->i64")
            .replace("callback(value)", "inc(value)"),
    );
}

#[test]
fn function_values_reject_signature_and_target_forgery() {
    let (_, mut resolved) = checked(PROGRAM);
    let select = resolved
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "fv.select")
        .unwrap();
    select.return_type = ResolvedType::Function {
        parameters: vec![ResolvedType::Bool],
        result: Box::new(ResolvedType::I64),
    };
    assert!(hir::validate(&resolved).is_err());
    for source in [
        PROGRAM.replace("apply(copied,41)", "apply(copied,true)"),
        PROGRAM.replace("fn inc(value:i64)", "fn inc<T>(value:i64)"),
        PROGRAM.replace("fn(i64)->i64 = chosen", "fn(bool)->i64 = chosen"),
    ] {
        assert!(semaprax::check(&source, "bad-function-value.spx").is_err());
    }
}

#[test]
fn function_values_have_no_implicit_pointer_equality() {
    let source = PROGRAM.replace("apply(copied,41)", "if chosen == copied { 1 } else { 0 }");
    let errors = semaprax::check(&source, "function-equality.spx").unwrap_err();
    assert!(errors.iter().any(|e| e.code == "SPX-T207"), "{errors:?}");
}

#[test]
fn function_values_keep_a_wide_flat_reference_universe_without_recursive_source_shape() {
    let targets = (0..=64)
        .map(|index| {
            format!("@id(\"fv.wide.{index:02}\") fn target_{index:02}(value:i64)->i64{{{index}}}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let references = (0..64)
        .map(|index| format!("let retained_{index:02}=target_{index:02};"))
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!(
        "module test.function_values_wide;\n{targets}\n@id(\"fv.choose\") fn choose()->fn(i64)->i64{{{references} target_64}}\n@id(\"fv.apply\") fn apply(callback:fn(i64)->i64,value:i64)->i64{{callback(value)}}\n@id(\"app.main\") fn main()->i64{{let callback=choose(); apply(callback,0)}}\n"
    );
    let (_, resolved) = checked(&source);
    assert_eq!(hir::function_value::target_universe(&resolved).len(), 65);
}

#[test]
fn function_values_lexical_scalar_shadow_never_becomes_a_function_reference() {
    let source = r#"
module test.function_values_scope;
@id("fv.inc") fn inc(value:i64)->i64{value+1}
@id("fv.chosen") fn chosen(value:i64)->i64{value-1}
@id("fv.apply") fn apply(callback:fn(i64)->i64,value:i64)->i64{callback(value)}
@id("fv.shadow") fn shadow(inc:i64, callback:fn(i64)->i64)->i64{
  if true { callback(inc) } else { callback(inc) }
}
@id("app.main") fn main()->i64{let callback=chosen; shadow(41,callback)}
"#;
    let (_, resolved) = checked(source);
    assert_eq!(
        hir::function_value::target_universe(&resolved)
            .iter()
            .map(|function| function.id.as_str())
            .collect::<Vec<_>>(),
        ["fv.chosen"],
    );
}

#[path = "function_values/wasm_closures.rs"]
mod wasm_closures;
