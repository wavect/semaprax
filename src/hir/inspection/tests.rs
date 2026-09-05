//! Read-only inspection queries over resolved HIR.
//!
//! These pin the call-graph projections agent tooling reads (`semaprax
//! context`, workspace impact, project candidate movement) rather than the
//! resolver that produces them: which expressions are searched for calls, the
//! order the answers arrive in, and that repeated runs agree.

use std::path::Path;

use super::*;

/// Source order is deliberately not alphabetical order: `zed` is declared
/// first and `alpha` third, so an answer sorted by name is distinguishable
/// from an answer in authored order.
const CALLS: &str = r#"
module test.hir_inspection_calls;

@id("app.zed")
fn zed(value: i64) -> i64
{
    value + 1
}

@id("app.beta")
fn beta(value: i64) -> i64
{
    zed(value)
}

@id("app.alpha")
fn alpha(value: i64) -> i64
    requires zed(value) > 0
    ensures result > 0
{
    beta(value) + zed(value)
}

@id("app.main")
fn main() -> i64
{
    alpha(1)
}
"#;

fn resolved(source: &str, path: &str) -> ResolvedProgram {
    let ast = crate::parse(source, Path::new(path)).expect("fixture parses");
    resolve(&ast).expect("fixture resolves")
}

fn site_pairs(program: &ResolvedProgram) -> Vec<(String, String)> {
    workspace_call_sites(program)
        .into_iter()
        .map(|(owner, _, callee)| (owner.as_str().to_owned(), callee.as_str().to_owned()))
        .collect()
}

fn edge_pairs(program: &ResolvedProgram) -> Vec<(String, String)> {
    workspace_call_edges(program)
        .into_iter()
        .map(|(caller, callee)| (caller.as_str().to_owned(), callee.as_str().to_owned()))
        .collect()
}

#[test]
fn call_edges_cover_requires_body_and_ensures_and_deduplicate() {
    let program = resolved(CALLS, "hir-inspection-calls.spx");
    // `alpha` reaches `zed` from both its `requires` clause and its body, and
    // the edge set collapses that to one relationship. Dropping either the
    // `requires` or the `ensures` chain from the walk would silently shrink
    // this set for contract-only callees.
    assert_eq!(
        edge_pairs(&program),
        vec![
            ("app.alpha".to_owned(), "app.beta".to_owned()),
            ("app.alpha".to_owned(), "app.zed".to_owned()),
            ("app.beta".to_owned(), "app.zed".to_owned()),
            ("app.main".to_owned(), "app.alpha".to_owned()),
        ]
    );
}

#[test]
fn call_sites_keep_authored_order_and_repeat_one_callee_per_site() {
    let program = resolved(CALLS, "hir-inspection-calls.spx");
    // Owners appear in declaration order (`beta`, `alpha`, `main`), which is
    // not alphabetical order (`alpha`, `beta`, `main`). Within `alpha` the
    // `requires` clause precedes the body, and the body's binary operands are
    // visited left to right, so `beta` precedes the second `zed`.
    assert_eq!(
        site_pairs(&program),
        vec![
            ("app.beta".to_owned(), "app.zed".to_owned()),
            ("app.alpha".to_owned(), "app.zed".to_owned()),
            ("app.alpha".to_owned(), "app.beta".to_owned()),
            ("app.alpha".to_owned(), "app.zed".to_owned()),
            ("app.main".to_owned(), "app.alpha".to_owned()),
        ]
    );
}

#[test]
fn every_call_site_carries_a_distinct_expression_identity() {
    let program = resolved(CALLS, "hir-inspection-calls.spx");
    let sites = workspace_call_sites(&program);
    let identities = sites
        .iter()
        .map(|(_, expression, _)| expression.clone())
        .collect::<BTreeSet<_>>();
    // `alpha` calls `zed` twice. Both sites must be addressable separately or
    // a workspace projection keyed by call-site identity loses one of them.
    assert_eq!(identities.len(), sites.len());
    assert!(identities.iter().all(|identity| !identity.is_empty()));
}

#[test]
fn call_projections_are_identical_across_repeated_resolutions() {
    let first = resolved(CALLS, "hir-inspection-calls.spx");
    let second = resolved(CALLS, "hir-inspection-calls.spx");
    assert_eq!(workspace_call_sites(&first), workspace_call_sites(&second));
    assert_eq!(workspace_call_edges(&first), workspace_call_edges(&second));
    // The same program queried twice must also answer identically.
    assert_eq!(workspace_call_sites(&first), workspace_call_sites(&first));
}

#[test]
fn call_sites_include_generic_templates_while_call_edges_do_not() {
    let source = r#"
module test.hir_inspection_generics;

@id("app.helper")
fn helper(value: i64) -> i64
{
    value + 1
}

@id("app.identity")
fn identity<T>(value: T) -> T
{
    let _ = helper(1);
    value
}

@id("app.main")
fn main() -> i64
{
    identity<i64>(4)
}
"#;
    let program = resolved(source, "hir-inspection-generics.spx");
    assert!(!program.function_templates.is_empty(), "template resolved");

    // `workspace_call_sites` walks templates, so the template body's call to
    // `helper` is reported against the template identity.
    assert!(site_pairs(&program).contains(&("app.identity".to_owned(), "app.helper".to_owned())));
    // `workspace_call_edges` walks concrete functions only. A caller that
    // relies on it to reach template-internal callees would be wrong, so this
    // asymmetry is pinned rather than assumed.
    assert_eq!(
        edge_pairs(&program),
        vec![("app.main".to_owned(), "app.identity".to_owned())]
    );
}

#[test]
fn visiting_calls_reports_the_instance_and_type_arguments_of_a_generic_call() {
    let source = r#"
module test.hir_inspection_instances;

@id("app.identity")
fn identity<T>(value: T) -> T
{
    value
}

@id("app.main")
fn main() -> i64
{
    if identity<bool>(true) { identity<i64>(4) } else { 0 }
}
"#;
    let program = resolved(source, "hir-inspection-instances.spx");
    let main = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .expect("entry point resolved");
    let mut observed = Vec::new();
    visit_resolved_calls(&main.body, &mut |callee, instance, type_arguments| {
        observed.push((
            callee.as_str().to_owned(),
            instance.map(|instance| instance.as_str().to_owned()),
            type_arguments.to_vec(),
        ));
    });
    // The condition is visited before either branch, and each call carries the
    // instance identity the backends dispatch on. Losing `instance` here would
    // make two distinct specializations look like one callee.
    assert_eq!(observed.len(), 2);
    assert_eq!(observed[0].0, "app.identity");
    assert_eq!(observed[0].2, vec![ResolvedType::Bool]);
    assert_eq!(observed[1].2, vec![ResolvedType::I64]);
    assert!(observed[0].1.is_some());
    assert_ne!(observed[0].1, observed[1].1);
}

const LIFECYCLE: &str = r#"
module test.hir_inspection_lifecycle;

@id("io.file")
resource File {
    @id("io.file.drop")
    drop import "io.file.finalize";
}

@id("io.file.host")
interface FileHost
    permits { filesystem.handle.release, filesystem.audit.record }
{
    @id("io.file.finalize")
    import fn finalize(file: own File) -> unit
        effects { filesystem.handle.release, filesystem.audit.record }
        failure infallible
        consumes file always;
}

@id("app.main")
fn main() -> i64
{
    42
}
"#;

#[test]
fn lifecycle_effects_come_back_in_canonical_sorted_order() {
    let program = resolved(LIFECYCLE, "hir-inspection-lifecycle.spx");
    let file = ResolvedType::Nominal {
        declaration: DeclarationId::new("io.file"),
        arguments: Vec::new(),
    };
    let effects = resolved_lifecycle_effects(&program, &file).expect("resource has a lifecycle");
    // Declared in `handle.release, audit.record` order; reported sorted, so a
    // caller comparing two effect sets never depends on authored order.
    assert_eq!(
        effects.into_iter().collect::<Vec<_>>(),
        vec![
            "filesystem.audit.record".to_owned(),
            "filesystem.handle.release".to_owned(),
        ]
    );
}

#[test]
fn lifecycle_effects_are_empty_for_scalars_and_fail_closed_for_unknown_types() {
    let program = resolved(LIFECYCLE, "hir-inspection-lifecycle.spx");
    assert!(resolved_lifecycle_effects(&program, &ResolvedType::I64)
        .expect("scalars have no lifecycle")
        .is_empty());

    let missing = ResolvedType::Nominal {
        declaration: DeclarationId::new("io.absent"),
        arguments: Vec::new(),
    };
    // A nominal type with no declaration must be an error, never an empty
    // effect set: an empty set would read as "this value needs no authority".
    let diagnostic =
        resolved_lifecycle_effects(&program, &missing).expect_err("unknown type fails closed");
    assert_eq!(diagnostic.code, "SPX-H006");
}

#[test]
fn path_prefix_matching_is_directional() {
    assert!(path_is_prefix::<u8>(&[], &[1, 2]));
    assert!(path_is_prefix(&[1], &[1, 2]));
    assert!(path_is_prefix(&[1, 2], &[1, 2]));
    // A longer candidate is not a prefix, and a shared length with a different
    // element is not either. Both directions matter: place overlap decisions
    // are built on this predicate.
    assert!(!path_is_prefix(&[1, 2], &[1]));
    assert!(!path_is_prefix(&[2], &[1, 2]));
}
