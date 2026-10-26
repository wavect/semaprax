use super::super::{
    active_builder_limit, budgeted_format, graph_error, import_stub, limit_error, private_layout,
    type_rewrite, AuthoredDeclaration,
};
use super::{construct, eligible, reserve_builder_structure, LayoutRegistry};
use crate::ast::program_source::ProgramSource;
use crate::ast::{
    Expr, ExprKind, Function, ModuleUse, ModuleUseKind, Program, Type, TypeDeclaration,
};
use crate::bounded_output::with_limit_usage_refusal;
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::{import_validation, index_authored, index_modules};
use std::collections::BTreeMap;
#[path = "tests/initialization.rs"]
mod initialization;
#[path = "tests/old.rs"]
mod old;
const PROVIDER: &str = include_str!("../private_layout/function_provider.spx");
const CALLER: &str = include_str!("../private_layout/function_caller.spx");
const SCALAR_PROVIDER: &str = "module imported.provider; @id(\"helper\") fn helper(value:i64)->i64 requires value>=0 ensures result>=0 {value+1}";
const SCALAR_CALLER: &str = "module imported.app; use function @id(\"helper\") from imported.provider as increment; @id(\"app.main\") fn main()->i64 {increment(41)}";
fn sources(caller: &str, provider: &str) -> [Program; 2] {
    [
        crate::parse(caller, "caller.spx").unwrap(),
        crate::parse(provider, "provider.spx").unwrap(),
    ]
}
// The actual module resolver verifies every fresh caller projection inside
// both linked Vec/Box wrapper scopes. Retained provider wrapper bodies keep
// their reserved std identities, so oracle and view use that same boundary;
// outside it the same projection remains refused.
fn linked<T>(operation: impl FnOnce() -> T) -> T {
    crate::vec_ops::with_authenticated_linked_source(|| {
        crate::box_ops::with_authenticated_linked_source(operation)
    })
}
fn parity(caller: &str, provider: &str) {
    let programs = sources(caller, provider);
    let authored = index_authored(&programs).unwrap();
    let modules = index_modules(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    import_validation::validate_uses_with_registry(&programs, &modules, &authored, Some(&registry))
        .unwrap();
    assert!(eligible(&programs[0], &authored, &programs));
    let before = old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap();
    let view = construct(&programs[0], &authored, &programs, &registry).unwrap();
    assert_eq!(
        view.types().iter().collect::<Vec<_>>(),
        before.types.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        view.functions().iter().collect::<Vec<_>>(),
        before.functions.iter().collect::<Vec<_>>()
    );
    for (ordinal, function) in programs[0].functions.iter().enumerate() {
        assert!(std::ptr::eq(
            view.functions().get(ordinal).unwrap(),
            function
        ));
        assert!(std::ptr::eq(
            &view.functions().get(ordinal).unwrap().body,
            &function.body
        ));
    }
    for (ordinal, declaration) in programs[0].types.iter().enumerate() {
        assert!(std::ptr::eq(
            view.types().get(ordinal).unwrap(),
            declaration
        ));
    }
    let layout = registry.get(&programs[0], &authored, &programs).unwrap();
    let expected = linked(|| {
        hir::resolve_with_function_reuse(
            &before,
            None,
            None,
            None,
            layout.presentation(&programs[0], &before).unwrap(),
        )
    });
    let actual =
        linked(|| hir::resolve_source(&view, layout.presentation(&programs[0], &view).unwrap()));
    let (expected, _) = expected.unwrap();
    let (actual, _) = actual.unwrap();
    assert_eq!(expected, actual);
    hir::validate(&actual).unwrap();
    let generated_type = view
        .types()
        .get(programs[0].types.len())
        .map(|value| value.stable_id.as_ptr());
    let generated_function = view
        .functions()
        .get(programs[0].functions.len())
        .map(|value| value.stable_id.as_ptr());
    // The genuine cache publication still owns its authored declarations. It
    // moves generated aliases/prototypes, without a second overlay payload copy.
    let key = super::super::borrowed_leaf::Input::Borrowed(view)
        .into_checked_key()
        .unwrap();
    assert_eq!(key, before);
    assert_eq!(
        crate::format::canonical(&key),
        crate::format::canonical(&before)
    );
    // Graph construction reruns source verification, so it needs the same
    // authenticated linked origin as both full HIR receivers above.
    let (actual_graph, expected_graph) = linked(|| {
        (
            crate::graph::to_json(&key).unwrap(),
            crate::graph::to_json(&before).unwrap(),
        )
    });
    assert_eq!(actual_graph, expected_graph);
    if let Some(pointer) = generated_type {
        assert_eq!(
            key.types[programs[0].types.len()].stable_id.as_ptr(),
            pointer
        );
    }
    if let Some(pointer) = generated_function {
        assert_eq!(
            key.functions[programs[0].functions.len()]
                .stable_id
                .as_ptr(),
            pointer
        );
    }
}
#[test]
fn imports_aliases_private_layouts_defaults_and_cache_publication_keep_full_old_projection() {
    parity(SCALAR_CALLER, SCALAR_PROVIDER);
    for arity in [0, 1, 2, 17] {
        let params = (0..arity)
            .map(|index| format!("value{index}:i64"))
            .collect::<Vec<_>>()
            .join(",");
        let args = (0..arity)
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let provider =
            format!("module imported.provider; @id(\"helper\") fn helper({params})->i64 {{42}}");
        let caller = format!("module imported.app; use function @id(\"helper\") from imported.provider as helper; @id(\"app.main\") fn main()->i64 {{helper({args})}}");
        parity(&caller, &provider);
    }
    // A parsed module declares at least one function (SPX-P101). Keep the
    // inert authored declaration in the reserved-main hostile input and the
    // separate admitted caller that exercises generated synthetic main.
    // Retain the original imported-main input as an explicit hostile alias:
    // ordinary import validation reserves main before projection construction.
    let alias_main = sources(
        "module imported.app; use function @id(\"helper\") from imported.provider as main; @id(\"app.inert\") fn inert()->i64 {0}",
        "module imported.provider; @id(\"helper\") fn helper()->i64 {42}",
    );
    let authored = index_authored(&alias_main).unwrap();
    let modules = index_modules(&alias_main).unwrap();
    let registry = LayoutRegistry::new(&alias_main, &authored);
    let errors = import_validation::validate_uses_with_registry(
        &alias_main,
        &modules,
        &authored,
        Some(&registry),
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "SPX-G172");
    parity(
        &SCALAR_CALLER.replace(
            " @id(\"app.main\") fn main()->i64 {increment(41)}",
            " @id(\"app.inert\") fn inert()->i64 {0}",
        ),
        SCALAR_PROVIDER,
    );
    parity(CALLER, PROVIDER);
    parity(&CALLER.replacen("module function_layout.app;", "module function_layout.app; use type @id(\"report\") from function_layout.provider as ImportedReport;", 1), PROVIDER);
    // Actual authored generic main/instance bodies are borrowed; imported
    // generic prototypes also use their exact ordinary classifier.
    parity(
        &SCALAR_CALLER
            .replace(
                "@id(\"app.main\")",
                "@id(\"identity\") fn identity<T>(value:T)->T {value} @id(\"app.main\")",
            )
            .replace("increment(41)", "identity<i64>(increment(41))"),
        SCALAR_PROVIDER,
    );
}
#[test]
fn duplicate_aliases_identities_and_late_wrong_types_preserve_old_diagnostics() {
    for caller in [
        format!("{SCALAR_CALLER} @id(\"other\") fn increment(value:i64)->i64 {{value}}"),
        format!("{SCALAR_CALLER} @id(\"app.main\") fn duplicate()->i64 {{0}}"),
        SCALAR_CALLER.replace("increment(41)", "increment(true)"),
        CALLER.replace("let report = make();", "let report:Report = make();"),
    ] {
        let provider = if caller.contains("function_layout") {
            PROVIDER
        } else {
            SCALAR_PROVIDER
        };
        let programs = sources(&caller, provider);
        // Project index rejects duplicate persistent IDs independently, before
        // a synthetic view can become lookup authority.
        if let Err(errors) = index_authored(&programs) {
            assert!(errors.iter().any(|error| error.code.starts_with("SPX-G")));
            continue;
        }
        let authored = index_authored(&programs).unwrap();
        let registry = LayoutRegistry::new(&programs, &authored);
        let before = old::construct(&programs[0], &authored, &programs, Some(&registry));
        let after = construct(&programs[0], &authored, &programs, &registry);
        match (before, after) {
            (Ok(before), Ok(view)) => {
                let layout = registry.get(&programs[0], &authored, &programs).unwrap();
                let old = hir::resolve_with_function_reuse(
                    &before,
                    None,
                    None,
                    None,
                    layout.presentation(&programs[0], &before).unwrap(),
                )
                .err()
                .unwrap();
                let new =
                    hir::resolve_source(&view, layout.presentation(&programs[0], &view).unwrap())
                        .err()
                        .unwrap();
                assert_eq!(old, new);
            }
            (Err(before), Err(after)) => assert_eq!(before, after),
            _ => panic!("owned/view constructor verdict differs"),
        };
    }
}
#[test]
fn complete_active_payer_resolution_and_drop_keep_exact_short_and_sticky_refusal() {
    for (caller, provider) in [(SCALAR_CALLER, SCALAR_PROVIDER), (CALLER, PROVIDER)] {
        let programs = sources(caller, provider);
        initialization::complete_original_receiver_before_measurement(&programs);
        let authored = index_authored(&programs).unwrap();
        let observe = |limit, retain| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let registry = LayoutRegistry::new(&programs, &authored);
                    let result = (|| {
                        // Original complete HIR and cleanup bound remains paid. This
                        // owner changes only the actual source overlay receiver.
                        let costs = super::super::synthetic_builder_bytes_scoped_with_registry(
                            &programs[0],
                            &authored,
                            &programs,
                            None,
                            4,
                            true,
                            Some(&registry),
                        )?;
                        reserve_builder_structure(costs.raw_clone_and_hir)?;
                        let view = construct(&programs[0], &authored, &programs, &registry)?;
                        let layout = registry.get(&programs[0], &authored, &programs)?;
                        let resolved =
                            hir::resolve_source(&view, layout.presentation(&programs[0], &view)?)?
                                .0;
                        hir::validate(&resolved).map_err(|error| vec![error])?;
                        drop(resolved);
                        if retain {
                            drop(
                                super::super::borrowed_leaf::Input::Borrowed(view)
                                    .into_checked_key()?,
                            );
                        } else {
                            drop(view);
                        }
                        Ok::<(), Vec<Diagnostic>>(())
                    })();
                    if result.is_err() {
                        assert!(reserve_builder_structure(0).is_err());
                    }
                    drop(result);
                    drop(registry);
                })
            })
        };
        for retain in [false, true] {
            let (((), actual), refused, required, _) = observe(usize::MAX, retain);
            assert!(
                !refused && !actual.invalid && actual.live == 0,
                "full imported receiver: {actual:?}; first invalid reason: {:?}",
                crate::test_allocation_observer::invalid_reason()
            );
            assert_eq!(actual.allocated, actual.freed);
            assert!(actual.allocated <= required && actual.highwater <= required);
            let (((), exact), refused, paid, _) = observe(required, retain);
            assert!(
                !refused && !exact.invalid && exact.live == 0 && paid == required,
                "exact imported receiver: {exact:?}; first invalid reason: {:?}",
                crate::test_allocation_observer::invalid_reason()
            );
            let (((), short), refused, _, receipt) = observe(required - 1, retain);
            assert!(
                refused && receipt.is_some() && !short.invalid && short.live == 0,
                "short imported receiver: {short:?}; first invalid reason: {:?}",
                crate::test_allocation_observer::invalid_reason()
            );
        }
    }
}
#[test]
fn plain_overlay_drop_requests_no_new_backing_and_full_route_keeps_old_verdicts() {
    let programs = sources(CALLER, PROVIDER);
    let authored = index_authored(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    let view = construct(&programs[0], &authored, &programs, &registry).unwrap();
    let (_, dropped) = measure(|| drop(view));
    assert!(!dropped.invalid && dropped.allocations == 0 && dropped.reallocations == 0);
    let generic = sources(
        SCALAR_CALLER,
        "module imported.provider; @id(\"helper\") fn helper<T>(value:T)->T {value}",
    );
    let map = index_authored(&generic).unwrap();
    assert!(eligible(&generic[0], &map, &generic));
    let registry = LayoutRegistry::new(&generic, &map);
    let before = old::construct(&generic[0], &map, &generic, Some(&registry));
    let after =
        super::super::borrowed_leaf::Input::construct(&generic[0], &map, &generic, &registry, true)
            .and_then(|input| {
                assert!(input.owned().is_none());
                input.into_checked_key()
            });
    assert_eq!(before, after);
    let invariant = sources(SCALAR_CALLER, "module imported.provider; @id(\"row\") record Row { @id(\"row.x\") x:i64, } requires x>=0 @id(\"helper\") fn helper(value:i64)->i64 {value}");
    let map = index_authored(&invariant).unwrap();
    assert!(eligible(&invariant[0], &map, &invariant));
    let registry = LayoutRegistry::new(&invariant, &map);
    let before = old::construct(&invariant[0], &map, &invariant, Some(&registry));
    let after = super::super::borrowed_leaf::Input::construct(
        &invariant[0],
        &map,
        &invariant,
        &registry,
        true,
    )
    .and_then(|input| {
        assert!(input.owned().is_none());
        input.into_checked_key()
    });
    assert_eq!(before, after);
}
#[test]
fn literal_full_old_layout_removal_is_observed_without_fitting_discovery_counts() {
    let programs = sources(
        "module imported.app; use function @id(\"helper\") from imported.provider as factory; @id(\"app.inert\") fn inert()->i64 {0}",
        "module imported.provider; @id(\"helper\") fn helper()->i64 {42}",
    );
    let authored = index_authored(&programs).unwrap();
    let observe = |borrow| {
        measure(|| {
            let registry = LayoutRegistry::new(&programs, &authored);
            if borrow {
                drop(construct(&programs[0], &authored, &programs, &registry).unwrap());
            } else {
                drop(old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap());
            }
            drop(registry);
        })
    };
    let ((), before) = observe(false);
    let ((), after) = observe(true);
    // Independently saved old source has one inert authored function (required
    // by SPX-P101), one import and two generated functions. Old Program::clone
    // obtains the exact capacity1 function directory; Vec::push then
    // reallocates to capacity4 for this nonbyte <=1024-byte element and the
    // synthetic main fits. The overlay borrows the authored function and
    // obtains only the exact capacity2 generated directory.
    assert!(
        std::mem::size_of::<Function>() <= 1024
            && std::mem::size_of::<Expr>() <= 1024
            && std::mem::size_of::<Type>() <= 1024
    );
    let strings = "caller.spx".len()
        + "imported.app".len()
        + "helper".len()
        + "imported.provider".len()
        + "factory".len();
    let directories = (1 + 4 - 2) * std::mem::size_of::<Function>();
    // Old clones the inert function's stable id, name and `{0}` Block's boxed
    // leaf tail; empty signature/contract Vecs and the inline i64 own none.
    let inert = "app.inert".len() + "inert".len() + std::mem::size_of::<Expr>();
    let removed = strings + std::mem::size_of::<ModuleUse>()
        + directories
        + inert
        // The saved signature receiver clones the provider name before
        // replacing it. The actual current Program Drop uses inline scratch.
        + "helper".len();
    assert_eq!(before.allocated, after.allocated + removed);
    assert_eq!(before.allocations, after.allocations + 5 + 1 + 3 + 1 + 1);
    for observed in [before, after] {
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
#[test]
fn literal_imported_directory_replacements_and_authored_payloads_are_removed() {
    use std::fmt::Write;
    let mut caller = String::from("module indexed.app;");
    let mut provider = String::from("module indexed.provider;");
    let mut imported_string_bytes = 0;
    for ordinal in 0..17 {
        let id = format!("row.{ordinal}");
        let alias = format!("Imported{ordinal}");
        imported_string_bytes += id.len() + alias.len() + "indexed.provider".len();
        write!(
            caller,
            " use type @id(\"{id}\") from indexed.provider as {alias};"
        )
        .unwrap();
        write!(
            provider,
            " @id(\"{id}\") record Row{ordinal} {{ @id(\"{id}.x\") x:i64, }}"
        )
        .unwrap();
    }
    caller.push_str(" @id(\"local\") record Local { @id(\"local.x\") x:i64, } @id(\"app.main\") fn main()->i64 {0}");
    // SPX-P101: the provider declares one inert function after its seventeen
    // records. Neither producer clones a provider function for type-only
    // imports, and the shared registry runs identically in both observations,
    // so the independent caller-side accounting below is unchanged.
    provider.push_str(" @id(\"indexed.provider.inert\") fn inert()->i64 {0}");
    let programs = sources(&caller, &provider);
    let authored = index_authored(&programs).unwrap();
    let observe = |borrow| {
        measure(|| {
            let registry = LayoutRegistry::new(&programs, &authored);
            if borrow {
                drop(construct(&programs[0], &authored, &programs, &registry).unwrap());
            } else {
                drop(old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap());
            }
            drop(registry);
        })
    };
    let ((), old) = observe(false);
    let ((), current) = observe(true);
    // The saved old actual producer starts with one cloned authored type,
    // then pushes seventeen imports: requested capacities 1,4,8,16,32.
    // The view keeps the authored type borrowed and acquires capacity17.
    assert!(
        std::mem::size_of::<TypeDeclaration>() <= 1024
            && std::mem::size_of::<Expr>() <= 1024
            && std::mem::size_of::<Type>() <= 1024
    );
    let directories = (1 + 4 + 8 + 16 + 32 - 17) * std::mem::size_of::<TypeDeclaration>();
    let authored = std::mem::size_of::<Function>()
        + std::mem::size_of::<Expr>()
        + std::mem::size_of::<crate::ast::FieldDeclaration>()
        + "app.main".len()
        + "main".len()
        + "local".len()
        + "Local".len()
        + "local.x".len()
        + "x".len();
    let discarded_sidecars = 17 * std::mem::size_of::<ModuleUse>()
        + imported_string_bytes
        + "caller.spx".len()
        + "indexed.app".len();
    // Nineteen scalar roots fill the actual inline16 Type frontier and
    // spill three values into one capacity4 Vec. Expression scratch stays inline.
    let drop_backing = 4 * std::mem::size_of::<Type>();
    // The original type clone owns RowN before replacing it with ImportedN.
    let discarded_names: usize = (0..17).map(|ordinal| format!("Row{ordinal}").len()).sum();
    assert_eq!(
        old.allocated,
        current.allocated
            + directories
            + authored
            + discarded_sidecars
            + drop_backing
            + discarded_names
    );
    assert_eq!(old.reallocations, current.reallocations + 4);
    for observed in [old, current] {
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    // The combined type-name index is one capacity18 allocation in the actual
    // producer. Isolate that same directory receiver for exact/one-short/sticky
    // backing without attributing any fabricated old growth to the source.
    let bytes = 18 * std::mem::size_of::<(&str, &TypeDeclaration)>();
    for limit in [bytes, bytes - 1] {
        let ((success, metrics), refused, charged, receipt) =
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let result = super::directory::<(&str, &TypeDeclaration)>(18);
                    let success = result.is_ok();
                    if !success {
                        assert!(reserve_builder_structure(0).is_err());
                    }
                    drop(result);
                    success
                })
            });
        assert!(!metrics.invalid && metrics.live == 0);
        if limit == bytes {
            assert!(success && !refused && charged == bytes);
            assert_eq!(metrics.allocated, bytes);
        } else {
            assert!(!success && refused && receipt.is_some());
        }
    }
}

#[path = "tests/alias_helpers.rs"]
mod alias_helpers;
#[path = "tests/full.rs"]
mod full;
