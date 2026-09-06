//! Proposed GEN-06 composition cases; register only with the implementation.
use semaprax::{format, hir};

const TYPES: &str = r#"
module test.generic_composition;
@id("c.pair") record Pair<A,B> { @id("c.payload") payload:A, @id("c.marker") marker:B, }
@id("c.wrap") record Wrap<A> { @id("c.value") value:A, }
"#;

fn checked(body: &str) -> hir::ResolvedProgram {
    let body = body
        .lines()
        .flat_map(|line| {
            if line.contains("@id(\"c.invoke\")") {
                ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"]
                    .into_iter()
                    .map(|ty| {
                        line.replace("c.invoke", &format!("c.invoke.{ty}"))
                            .replace("fn invoke(", &format!("fn invoke_{ty}("))
                            .replace("bool", ty)
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![line.to_owned()]
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("{TYPES}\n{body}\n@id(\"app.main\") fn main()->i64 {{0}}\n");
    let program = semaprax::check(&source, "composition.spx").unwrap();
    let canonical = format::canonical(&program);
    let reparsed = semaprax::check(&canonical, "composition.spx").unwrap();
    assert_eq!(canonical, format::canonical(&reparsed));
    let resolved = hir::resolve(&reparsed).unwrap();
    hir::validate(&resolved).unwrap();
    let graph = semaprax::graph::to_json(&reparsed).unwrap();
    semaprax::graph::verify_json(&reparsed, &graph).unwrap();
    assert!(graph.contains("\"generic_instance_ownership\""));
    resolved
}

#[test]
fn generic_nested_owned_reconstruction_is_structural() {
    let resolved = checked(
        r#"
@id("c.rebuild") fn rebuild<T>(value:own Wrap<Pair<Bytes,T>>)->Wrap<Pair<Bytes,T>> {
    match own value {
        Wrap { value: Pair { payload, marker } } => Wrap<Pair<Bytes,T>> { value: Pair<Bytes,T> { payload:payload, marker:marker } },
    }
}
@id("c.invoke") fn invoke(value:own Wrap<Pair<Bytes,bool>>)->Wrap<Pair<Bytes,bool>> {rebuild<bool>(value)}
"#,
    );
    let mut hostile = resolved.clone();
    let hir::ResolvedExprKind::Block { tail, .. } =
        &mut hostile.function_instances[0].function.body.kind
    else {
        panic!("expected block")
    };
    let hir::ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        panic!("expected match")
    };
    let hir::ResolvedMatchPattern::Record { fields, .. } = &mut arms[0].pattern else {
        panic!("expected record")
    };
    fields[0].field = hir::DeclarationId::new("foreign.field");
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

#[test]
fn generic_owning_reconstruction_composes_in_branches_and_call_arguments() {
    checked(
        r#"
@id("c.relay") fn relay<T>(value:own Pair<Bytes,T>)->Pair<Bytes,T> {value}
@id("c.compose") fn compose<T>(value:own Pair<Bytes,T>, choose:bool)->Pair<Bytes,T> {
    relay<T>(if choose {
        match own value { Pair { payload, marker } => Pair<Bytes,T> { payload:payload, marker:marker }, }
    } else { value })
}
@id("c.invoke") fn invoke(value:own Pair<Bytes,bool>)->Pair<Bytes,bool> {compose<bool>(value,true)}
"#,
    );
}

#[test]
fn generic_owned_reconstruction_checks_new_nominal_result_explicitly() {
    checked(
        r#"
@id("c.wrap_owned") fn wrap_owned<T>(value:own Pair<Bytes,T>)->Wrap<Pair<Bytes,T>> {
    match own value { Pair { payload, marker } => Wrap<Pair<Bytes,T>> { value:Pair<Bytes,T> { payload:payload, marker:marker } }, }
}
@id("c.invoke") fn invoke(value:own Pair<Bytes,bool>)->Wrap<Pair<Bytes,bool>> {wrap_owned<bool>(value)}
"#,
    );
}

#[test]
fn generic_composition_does_not_duplicate_owned_leaves() {
    let body = r#"
@id("c.duplicate") fn duplicate<T>(value:own Pair<Bytes,T>)->Pair<Bytes,T> {
    match own value { Pair { payload, marker } => {
        let first=Pair<Bytes,T> {payload:payload,marker:marker};
        Pair<Bytes,T> {payload:payload,marker:marker}
    }, }
}
"#;
    let source = format!("{TYPES}\n{body}\n@id(\"app.main\") fn main()->i64 {{0}}\n");
    let errors = semaprax::check(&source, "composition-duplicate.spx").unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-O101"));
}

#[test]
fn generic_nested_borrowed_copy_result_composes_with_reconstruction() {
    checked(
        r#"
@id("c.observe") fn observe<T>(value:own Wrap<Pair<Bytes,T>>)->Wrap<Pair<Bytes,T>> {
    let observed = match borrow value { Wrap { value:Pair { payload, marker } } => marker, };
    match own value { Wrap { value:Pair { payload, marker:_ } } => Wrap<Pair<Bytes,T>> { value:Pair<Bytes,T> {payload:payload,marker:observed} }, }
}
@id("c.invoke") fn invoke(value:own Wrap<Pair<Bytes,bool>>)->Wrap<Pair<Bytes,bool>> {observe<bool>(value)}
"#,
    );
}

#[test]
fn generic_nested_update_accepts_checked_owned_alias() {
    let resolved = checked(
        r#"
@id("c.update") fn update<T>(value:own Pair<Wrap<Bytes>,T>,next:T)->Pair<Wrap<Bytes>,T> {
    let alias=value;
    alias with {marker:next}
}
@id("c.invoke") fn invoke(value:own Pair<Wrap<Bytes>,bool>,next:bool)->Pair<Wrap<Bytes>,bool> {update<bool>(value,next)}
"#,
    );
    for instance in &resolved.function_instances {
        assert_eq!(
            instance.function.cleanup_plan.schema,
            "semaprax.cleanup-plan.v9"
        );
    }
    let mut hostile = resolved.clone();
    hostile.function_instances[0].function.cleanup_plan.schema = "semaprax.cleanup-plan.v7";
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

#[test]
fn generic_owned_match_result_composes_inside_constructor_field() {
    checked(
        r#"
@id("c.wrap_field") fn wrap_field<T>(value:own Pair<Bytes,T>)->Wrap<Pair<Bytes,T>> {
    Wrap<Pair<Bytes,T>> {value:match own value {Pair{payload,marker}=>Pair<Bytes,T>{payload:payload,marker:marker},}}
}
@id("c.invoke") fn invoke(value:own Pair<Bytes,bool>)->Wrap<Pair<Bytes,bool>> {wrap_field<bool>(value)}
"#,
    );
}
