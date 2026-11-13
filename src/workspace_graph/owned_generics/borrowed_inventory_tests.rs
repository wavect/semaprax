//! OPT784 controls: the borrowed exact-current type and interface inventory
//! against the saved owned-map algorithm over a real validated workspace.
use std::path::Path;

use super::super::{build_owned, WorkspaceGraphBuild, WorkspaceSource};
use super::*;
use crate::bounded_output;
use crate::test_allocation_observer::measure;

const PROVIDER: &str = r#"
module outcome.provider;
@id("outcome.config") record Config { @id("outcome.config.label") label:string, }
@id("outcome.row") record Row { @id("outcome.row.sku") sku:string, }
@id("outcome.report") record Report {
    @id("outcome.report.config") config:Config,
    @id("outcome.report.rows") rows:Vec<Row>,
}
@id("outcome.decoded") variant Decoded {
    @id("outcome.decoded.ready") Ready { @id("outcome.decoded.ready.value") value:Report, },
    @id("outcome.decoded.error") Error {
        @id("outcome.decoded.error.code") code:i64,
        @id("outcome.decoded.error.offset") offset:usize,
        @id("outcome.decoded.error.field") field:i64,
    },
}
@id("outcome.unused") record Unused { @id("outcome.unused.value") value:i64, }
@id("outcome.decode") fn decode(input:borrow Slice<u8>)->Decoded { Decoded::Error { code:0, offset:0usize, field:0 } }
"#;
const APP: &str = r#"
module outcome.app;
use type @id("outcome.decoded") from outcome.provider as Decoded;
use type @id("outcome.report") from outcome.provider as Report;
use type @id("outcome.config") from outcome.provider as Config;
use type @id("outcome.row") from outcome.provider as Row;
use function @id("outcome.decode") from outcome.provider as decode;
@id("app.spare") record Spare { @id("app.spare.value") value:i64, }
@id("outcome.main") fn main()->i64 {
    let input=[0u8]; let decoded=decode(array_as_slice(input));
    match own decoded { Decoded::Error { code, offset, field } => code, Decoded::Ready { value } => 0, }
}
"#;
const UNREACHABLE: [&str; 2] = ["outcome.unused", "app.spare"];

fn built() -> WorkspaceGraphBuild {
    let sources = [("app.spx", APP), ("provider.spx", PROVIDER)]
        .into_iter()
        .map(|(path, text)| {
            let program = crate::parse(text, Path::new(path)).expect("fixture parses");
            WorkspaceSource {
                path: path.to_owned(),
                source: crate::format::canonical(&program),
            }
        })
        .collect();
    build_owned(sources).expect("outcome workspace authenticates")
}

/// The linked owned-data closure supplies real selected functions/instances.
fn linked_inputs(
    built: &WorkspaceGraphBuild,
) -> (
    hir::ResolvedProgram,
    Vec<hir::LinkedScalarFunction>,
    Vec<hir::ResolvedFunctionInstance>,
) {
    let linked = built
        .linked_owned_data_api_program_with_roots("outcome.app", &[])
        .expect("owned closure links");
    hir::validate(&linked).expect("independent HIR replay");
    let functions = linked
        .functions
        .iter()
        .map(|function| hir::LinkedScalarFunction {
            function: function.clone(),
            origin: built.hir.declarations[function.id.as_str()].origin,
        })
        .collect();
    let instances = linked.function_instances.clone();
    (linked, functions, instances)
}

/// Saved pre-OPT784 `reachable_scalar_types`: full owned clones of every
/// declaration and interface before the closure walk.
fn old_reachable_scalar_types(
    modules: &[WorkspaceResolvedModule],
    functions: &[hir::LinkedScalarFunction],
    function_instances: &[hir::ResolvedFunctionInstance],
) -> Result<Vec<hir::ResolvedTypeDeclaration>, Vec<Diagnostic>> {
    let mut available = BTreeMap::new();
    for declaration in modules.iter().flat_map(|module| &module.types) {
        if available
            .insert(declaration.id.clone(), declaration.clone())
            .is_some()
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace scalar type identity is duplicated",
            )]);
        }
    }
    let owner_interfaces = modules
        .iter()
        .flat_map(|module| &module.interfaces)
        .cloned()
        .collect::<Vec<_>>();
    hir::owned_reachable_types_oracle(
        functions,
        function_instances,
        &[],
        &available,
        &[],
        &owner_interfaces,
    )
    .map_err(|error| vec![error])
}

/// Saved pre-OPT784 owned-data type map from `linked_owned_data_with_type_roots`.
fn old_owned_data_map(
    modules: &[WorkspaceResolvedModule],
) -> Result<BTreeMap<hir::DeclarationId, hir::ResolvedTypeDeclaration>, Vec<Diagnostic>> {
    let mut available_types = BTreeMap::new();
    for module in modules {
        for declaration in &module.types {
            if available_types
                .insert(declaration.id.clone(), declaration.clone())
                .is_some()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace owned-data type identity is duplicated",
                )]);
            }
        }
    }
    Ok(available_types)
}

type Snapshot = Vec<(
    Vec<hir::ResolvedTypeDeclaration>,
    Vec<hir::ResolvedInterface>,
)>;
fn snapshot(modules: &[WorkspaceResolvedModule]) -> Snapshot {
    modules
        .iter()
        .map(|module| (module.types.to_vec(), module.interfaces.clone()))
        .collect()
}

#[test]
fn borrowed_reachability_matches_owned_oracle_over_current_workspace() {
    let built = built();
    let before = snapshot(&built.hir.modules);
    let (linked, functions, instances) = linked_inputs(&built);

    let scalar = reachable_scalar_types(&built.hir.modules, &functions, &instances);
    assert_eq!(
        scalar,
        old_reachable_scalar_types(&built.hir.modules, &functions, &instances)
    );

    let borrowed = hir::BorrowedTypeInventory::collect(
        built
            .hir
            .modules
            .iter()
            .flat_map(|module| module.types.iter()),
    )
    .unwrap();
    let owned = old_owned_data_map(&built.hir.modules).unwrap();
    assert_eq!(borrowed.len(), owned.len());
    let expected = hir::owned_reachable_types_oracle(
        &functions,
        &instances,
        &linked.interfaces,
        &owned,
        &[],
        &[],
    )
    .unwrap();
    assert_eq!(
        hir::reachable_authored_types_with_roots(
            &functions,
            &instances,
            &linked.interfaces,
            &borrowed,
            &[],
        )
        .unwrap(),
        expected
    );
    // The complete published HIR carries exactly the oracle's authored set.
    let mut published = linked
        .types
        .iter()
        .filter(|declaration| !crate::prelude::is_compiler_owned_id(declaration.id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    published.sort_by(|left, right| left.id.cmp(&right.id));
    assert_eq!(published, expected);
    for id in [
        "outcome.config",
        "outcome.decoded",
        "outcome.report",
        "outcome.row",
    ] {
        assert!(expected
            .iter()
            .any(|declaration| declaration.id.as_str() == id));
    }
    for id in UNREACHABLE {
        assert!(!expected
            .iter()
            .any(|declaration| declaration.id.as_str() == id));
    }

    // Every authored root, both unreachable ones, and an unknown root.
    let mut roots = owned.keys().cloned().collect::<Vec<_>>();
    roots.push(hir::DeclarationId::new("outcome.absent"));
    for root in &roots {
        let roots = std::slice::from_ref(root);
        let actual = hir::reachable_authored_types_with_roots(
            &functions,
            &instances,
            &linked.interfaces,
            &borrowed,
            roots,
        );
        let oracle = hir::owned_reachable_types_oracle(
            &functions,
            &instances,
            &linked.interfaces,
            &owned,
            roots,
            &[],
        );
        assert_eq!(actual, oracle, "root {root}");
    }
    let unknown = hir::reachable_authored_types_with_roots(
        &functions,
        &instances,
        &linked.interfaces,
        &borrowed,
        &[hir::DeclarationId::new("outcome.absent")],
    )
    .unwrap_err();
    assert_eq!(unknown.code, "SPX-G173");

    // Relinking publishes the identical complete program, and the borrowed
    // views left every current module declaration and interface unchanged.
    assert_eq!(
        built
            .linked_owned_data_api_program_with_roots("outcome.app", &[])
            .unwrap(),
        linked
    );
    assert_eq!(snapshot(&built.hir.modules), before);
}

#[test]
fn unreachable_duplicate_type_identity_still_refuses_g173_on_both_routes() {
    let mut built = built();
    let (_, functions, instances) = linked_inputs(&built);
    let app = built
        .hir
        .modules
        .iter()
        .position(|module| module.module == "outcome.app")
        .unwrap();
    let spare = built.hir.modules[app]
        .types
        .iter()
        .position(|declaration| declaration.id.as_str() == UNREACHABLE[1])
        .unwrap();
    // Neither copy of the forged identity is reachable from any root.
    built.hir.modules[app].types[spare].id = hir::DeclarationId::new(UNREACHABLE[0]);

    let scalar = reachable_scalar_types(&built.hir.modules, &functions, &instances).unwrap_err();
    assert_eq!(
        scalar,
        vec![graph_error(
            "SPX-G173",
            "workspace scalar type identity is duplicated",
        )]
    );
    assert_eq!(
        Err(scalar),
        old_reachable_scalar_types(&built.hir.modules, &functions, &instances)
    );
    let owned_data = built
        .linked_owned_data_api_program_with_roots("outcome.app", &[])
        .unwrap_err();
    assert_eq!(
        owned_data,
        vec![graph_error(
            "SPX-G173",
            "workspace owned-data type identity is duplicated",
        )]
    );
    assert_eq!(
        Err(owned_data),
        old_owned_data_map(&built.hir.modules).map(|_| ())
    );
}

#[test]
fn borrowed_inventory_removes_transient_clone_backing_and_frees_everything() {
    // Immutable inputs and every ledger Rc are fixed outside each observer.
    let built = built();
    let (_, functions, instances) = linked_inputs(&built);
    let modules = &built.hir.modules;
    let expected = old_reachable_scalar_types(modules, &functions, &instances).unwrap();

    // Actual backing of the owned copies the former inventory made of every
    // declaration (key and value) and every interface.
    let (_, copies) = measure(|| {
        for declaration in modules.iter().flat_map(|module| module.types.iter()) {
            std::hint::black_box((declaration.id.clone(), declaration.clone()));
        }
        for interface in modules.iter().flat_map(|module| &module.interfaces) {
            std::hint::black_box(interface.clone());
        }
    });
    assert!(!copies.invalid && copies.allocated > 0 && copies.live == 0);

    // One observer each spans construction -> publication -> Drop.
    let observe = |old: bool| {
        bounded_output::with_limit_usage_refusal(0, || {
            measure(|| {
                let published = if old {
                    old_reachable_scalar_types(modules, &functions, &instances)
                } else {
                    reachable_scalar_types(modules, &functions, &instances)
                }
                .unwrap();
                assert_eq!(published, expected);
                drop(published);
            })
        })
    };
    let (((), new), new_overflow, new_consumed, new_refusal) = observe(false);
    let (((), old), old_overflow, old_consumed, old_refusal) = observe(true);
    for metrics in [new, old] {
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.freed, metrics.allocated);
    }
    assert!(
        old.allocated - new.allocated >= copies.allocated,
        "removed {} bytes, expected at least the {} copied bytes",
        old.allocated - new.allocated,
        copies.allocated
    );
    assert!(new.highwater <= old.highwater);
    assert!(new.allocations < old.allocations);
    // This route has no reservation payer before or after: zero debit.
    assert_eq!((new_overflow, new_consumed, new_refusal), (false, 0, None));
    assert_eq!((old_overflow, old_consumed, old_refusal), (false, 0, None));

    // Retained ownership: the published owned vector is identical backing.
    let (published, new_kept) = measure(|| reachable_scalar_types(modules, &functions, &instances));
    let (old_published, old_kept) =
        measure(|| old_reachable_scalar_types(modules, &functions, &instances));
    assert!(!new_kept.invalid && !old_kept.invalid);
    assert_eq!(published, old_published);
    assert_eq!(new_kept.live, old_kept.live);
    assert!(new_kept.live > 0);
    drop((published, old_published));
}

#[test]
fn selected_root_uses_the_exact_held_key_and_preserves_detached_text() {
    let key = hir::DeclarationId::new("root.é");
    let available = BTreeMap::from([(key.clone(), ())]);
    let spelling = "root.é".to_owned();
    let (selected, allocation) = measure(|| selected_root_identity(&available, &spelling));
    assert_eq!(selected, key);
    assert_eq!(
        selected.shared_allocation_key(),
        key.shared_allocation_key()
    );
    assert!(!allocation.invalid);
    assert_eq!(allocation.allocated, 0);

    let missing = selected_root_identity(&available, "root.é.missing");
    assert_eq!(missing.as_str(), "root.é.missing");
    assert!(!available.contains_key(&missing));
}
