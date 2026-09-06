//! Exact substitution and independent ownership for multiple generic owners.
const TYPES: &str = r#"
module test.multi_owner;
@id("o.pair") record Pair<A,B> { @id("o.left") left:A, @id("o.right") right:B, }
@id("o.wrap") record Wrap<A> { @id("o.value") value:A, }
"#;

fn checked(source: &str, label: &str) -> semaprax::hir::ResolvedProgram {
    checked_owners(source, label, 2)
}

fn checked_owners(source: &str, label: &str, owners: usize) -> semaprax::hir::ResolvedProgram {
    let source = source
        .lines()
        .flat_map(|line| {
            if line.contains("@id(\"o.invoke\")") {
                ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"]
                    .into_iter()
                    .map(|ty| {
                        line.replace("o.invoke", &format!("o.invoke.{ty}"))
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
    let parsed = semaprax::check(&source, label).unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    let reparsed = semaprax::check(&canonical, label).unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let resolved = semaprax::hir::resolve(&reparsed).unwrap();
    semaprax::hir::validate(&resolved).unwrap();
    assert_eq!(resolved.function_instances.len(), 8);
    let graph = semaprax::graph::to_json(&reparsed).unwrap();
    semaprax::graph::verify_json(&reparsed, &graph).unwrap();
    let graph: serde_json::Value = serde_json::from_str(&graph).unwrap();
    for instance in graph["generic_instance_ownership"].as_array().unwrap() {
        assert_eq!(
            instance["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p["ownership_mode"] == "own")
                .count(),
            owners
        );
        assert_eq!(instance["result"]["ownership_mode"], "own");
    }
    let mut hostile = resolved.clone();
    hostile.function_instances[0].function.params[1].ownership =
        semaprax::hir::OwnershipMode::Borrow;
    assert_eq!(
        semaprax::hir::validate(&hostile).unwrap_err().code,
        "SPX-H006"
    );
    resolved
}

#[test]
fn generic_two_owners_combine_in_explicit_nested_result() {
    let source = format!(
        r#"{TYPES}
@id("o.combine") fn combine<T>(left:own Pair<Bytes,T>,right:own Wrap<Bytes>)->Pair<Pair<Bytes,T>,Wrap<Bytes>> {{
    Pair<Pair<Bytes,T>,Wrap<Bytes>> {{left:left,right:right}}
}}
@id("o.invoke") fn invoke(left:own Pair<Bytes,bool>,right:own Wrap<Bytes>)->Pair<Pair<Bytes,bool>,Wrap<Bytes>> {{combine<bool>(left,right)}}
@id("app.main") fn main()->i64 {{0}}
"#
    );
    let resolved = checked(&source, "two-owners.spx");
    for instance in &resolved.function_instances {
        assert_eq!(
            instance
                .function
                .params
                .iter()
                .filter(|p| p.ownership == semaprax::hir::OwnershipMode::Own)
                .count(),
            2
        );
    }
}

#[test]
fn generic_two_owners_replace_nested_owned_field_with_v9() {
    let source = format!(
        r#"{TYPES}
@id("o.replace") fn replace<T>(base:own Pair<Wrap<Bytes>,T>,next:own Wrap<Bytes>)->Pair<Wrap<Bytes>,T> {{
    let alias=base;
    alias with {{left:next}}
}}
@id("o.invoke") fn invoke(base:own Pair<Wrap<Bytes>,bool>,next:own Wrap<Bytes>)->Pair<Wrap<Bytes>,bool> {{replace<bool>(base,next)}}
@id("app.main") fn main()->i64 {{0}}
"#
    );
    let resolved = checked(&source, "two-owner-update.spx");
    assert_eq!(
        resolved.function_instances[0].function.cleanup_plan.schema,
        "semaprax.cleanup-plan.v9"
    );
}

#[test]
fn generic_two_parameters_cannot_receive_one_owned_value_twice() {
    let source = format!(
        r#"{TYPES}
@id("o.combine") fn combine<T>(left:own Pair<Bytes,T>,right:own Pair<Bytes,T>)->Pair<Pair<Bytes,T>,Pair<Bytes,T>> {{
    Pair<Pair<Bytes,T>,Pair<Bytes,T>> {{left:left,right:right}}
}}
@id("o.bad") fn bad<T>(value:own Pair<Bytes,T>)->Pair<Pair<Bytes,T>,Pair<Bytes,T>> {{combine<T>(value,value)}}
@id("app.main") fn main()->i64 {{0}}
"#
    );
    assert!(semaprax::check(&source, "two-owner-duplicate.spx")
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-O101"));
}

#[test]
fn generic_owner_count_uses_existing_parameter_bounds() {
    let source = format!(
        r#"{TYPES}
@id("o.combine") fn combine<T>(left:own Pair<Bytes,T>,middle:own Wrap<Bytes>,right:own Wrap<Bytes>)->Pair<Pair<Bytes,T>,Pair<Wrap<Bytes>,Wrap<Bytes>>> {{
    Pair<Pair<Bytes,T>,Pair<Wrap<Bytes>,Wrap<Bytes>>> {{left:left,right:Pair<Wrap<Bytes>,Wrap<Bytes>> {{left:middle,right:right}}}}
}}
@id("o.invoke") fn invoke(left:own Pair<Bytes,bool>,middle:own Wrap<Bytes>,right:own Wrap<Bytes>)->Pair<Pair<Bytes,bool>,Pair<Wrap<Bytes>,Wrap<Bytes>>> {{combine<bool>(left,middle,right)}}
@id("app.main") fn main()->i64 {{0}}
"#
    );
    checked_owners(&source, "three-owners.spx", 3);
}

#[test]
fn generic_multiple_owners_do_not_admit_unrelated_carriers() {
    let source = format!(
        r#"{TYPES}
@id("o.bad") fn bad<T>(value:own Pair<Bytes,T>,other:string)->Wrap<Pair<Bytes,T>> {{Wrap<Pair<Bytes,T>> {{value:value}}}}
@id("app.main") fn main()->i64 {{0}}
"#
    );
    assert!(semaprax::check(&source, "unrelated-owner.spx")
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-T224"));
}
