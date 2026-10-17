use super::super::name_equal::construct_transient;
use super::*;
use crate::ast::program_source::ProgramSource;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::{measure, Metrics};
use crate::workspace_graph::index_authored;
use sha2::{Digest, Sha256};

const LIMIT: usize = 32 * 1024 * 1024;
const PROVIDER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/std/collections/src/collections.spx"
));
const IMPORTS: &str = "module directory.app;\nuse function @id(\"std.collections.vec.with-capacity\") from std.collections as with_capacity;\nuse function @id(\"std.collections.vec.push\") from std.collections as push;\nuse function @id(\"std.collections.vec.get\") from std.collections as get;\n";
const MAIN: &str = "@id(\"directory.main\") fn main()->i64 {let a=with_capacity<i64>(2usize);let b=push<i64>(a,42);get<i64>(b,0usize)}";

fn programs(caller: &str) -> [Program; 2] {
    let caller = if caller == IMPORTS {
        // Imports-only text is deliberately not admitted source. Preserve its
        // original complete parser refusal before constructing an AST-only
        // synthetic-main input, outside both old/current observation windows.
        let diagnostic = crate::parse(IMPORTS, "caller.spx").unwrap_err();
        assert_eq!(diagnostic.code, "SPX-P101");
        assert_eq!(diagnostic.severity, crate::diagnostic::Severity::Error);
        assert_eq!(
            diagnostic.message,
            "a module must declare at least one function"
        );
        assert_eq!(diagnostic.path.as_deref(), Some("caller.spx"));
        assert_eq!(
            diagnostic
                .span
                .as_ref()
                .map(|span| (span.start, span.end, span.line, span.column)),
            Some((IMPORTS.len(), IMPORTS.len(), 5, 1))
        );
        assert_eq!(diagnostic.help, None);
        let source =
            format!("{IMPORTS}@id(\"directory.fixture-anchor\") fn fixture_anchor()->i64 {{0}}");
        let mut program = crate::parse(&source, "caller.spx").unwrap();
        let anchors = std::mem::take(&mut program.functions);
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].stable_id, "directory.fixture-anchor");
        assert_eq!(anchors[0].name, "fixture_anchor");
        drop(anchors);
        assert!(program.functions.is_empty());
        assert_eq!(program.functions.capacity(), 0);
        // Program has no retained function index/directory. index_authored and
        // LayoutRegistry are built by callers only after this removal.
        program
    } else {
        crate::parse(caller, "caller.spx").unwrap()
    };
    [caller, crate::parse(PROVIDER, "provider.spx").unwrap()]
}
fn renamed(caller: &str) -> String {
    caller
        .replace("as with_capacity;", "as make_vec;")
        .replace("with_capacity<i64>", "make_vec<i64>")
        .replace("as push;", "as append;")
        .replace("push<i64>", "append<i64>")
        .replace("as get;", "as read_value;")
        .replace("get<i64>", "read_value<i64>")
}
thread_local! {
    static OBSERVE_PUSH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FIRST_PUSH_REMAINING: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}
// Test-only observation after an actual owning push, never a forecasted count.
pub(in crate::workspace_graph::expected_projection::borrowed_imports::typed) fn observe_owned_push()
{
    if OBSERVE_PUSH.with(std::cell::Cell::get) {
        FIRST_PUSH_REMAINING.with(|first| {
            if first.get().is_none() {
                first.set(crate::bounded_output::active_remaining());
            }
        });
    }
}
#[derive(Debug)]
struct Observed {
    accepted: bool,
    overflowed: bool,
    paid: usize,
    first: Option<ReservationRefusal>,
    first_owned_paid: Option<usize>,
    metrics: Metrics,
}
fn observe(programs: &[Program], current: bool, limit: usize) -> Observed {
    let authored = index_authored(programs).unwrap();
    FIRST_PUSH_REMAINING.with(|first| first.set(None));
    OBSERVE_PUSH.with(|active| active.set(current));
    let ((ok, metrics), overflowed, paid, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let registry = LayoutRegistry::new(programs, &authored);
            let mut owner = match Receiver::acquire() {
                Ok(owner) => owner,
                Err(_) => return false,
            };
            let result = if current {
                construct_transient(
                    &programs[0],
                    &authored,
                    programs,
                    &registry,
                    None,
                    &mut owner,
                )
            } else {
                super::before_owned_directory::construct_transient(
                    &programs[0],
                    &authored,
                    programs,
                    &registry,
                    None,
                    &mut owner,
                )
            };
            let ok = result.is_ok();
            drop(result);
            drop(owner);
            drop(registry);
            ok
        })
    });
    OBSERVE_PUSH.with(|active| active.set(false));
    let first_owned_paid =
        FIRST_PUSH_REMAINING.with(|first| first.get().map(|remaining| limit - remaining));
    Observed {
        first_owned_paid,
        accepted: ok && !overflowed,
        overflowed,
        paid,
        first,
        metrics,
    }
}
fn settled(value: &Observed) {
    assert!(
        !value.metrics.invalid
            && value.metrics.live == 0
            && value.metrics.allocated == value.metrics.freed,
        "{value:?}"
    );
}

#[test]
fn borrowed_owned_mixed_and_synthetic_main_keep_zero_or_original_capacity_and_full_hir_parity() {
    let caller = format!("{IMPORTS}{MAIN}");
    let owned = renamed(&caller);
    let mixed = caller
        .replace("as get;", "as read_value;")
        .replace("get<i64>", "read_value<i64>");
    // Paired legal consumers of the SAME provider identity, with distinct
    // local aliases. Duplicate IDs in one input remain source-rejected S102.
    let mut same_provider_bindings = [false; 2];
    let mut canonical_metadata = None;
    for (source, expected_owned, expected_capacity) in [
        (caller.as_str(), 0usize, 0usize),
        (owned.as_str(), 3usize, 3usize),
        (mixed.as_str(), 1usize, 3usize),
        (IMPORTS, 1usize, 4usize),
    ] {
        let programs = programs(source);
        let authored = index_authored(&programs).unwrap();
        let ((), overflowed, _, first) = with_limit_usage_refusal(LIMIT, || {
            let registry = LayoutRegistry::new(&programs, &authored);
            let mut owner = Receiver::acquire().unwrap();
            let old = super::before_owned_directory::construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner,
            )
            .unwrap();
            let current = construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner,
            )
            .unwrap();
            assert_eq!(current.functions.roots.functions.len(), expected_owned);
            assert_eq!(
                current.functions.roots.functions.capacity(),
                expected_capacity
            );
            assert_eq!(
                old.functions().iter().collect::<Vec<_>>(),
                current.functions().iter().collect::<Vec<_>>()
            );
            assert_eq!(
                old.types().iter().collect::<Vec<_>>(),
                current.types().iter().collect::<Vec<_>>()
            );
            let layout = registry.get(&programs[0], &authored, &programs).unwrap();
            crate::vec_ops::with_authenticated_linked_source(|| {
                let old_result = crate::hir::identity_resolution::resolve_source_nominal(
                    &old,
                    layout.presentation(&programs[0], &old).unwrap(),
                    crate::workspace_graph::reserve_builder_structure,
                );
                let current_result = crate::hir::identity_resolution::resolve_source_nominal(
                    &current,
                    layout.presentation(&programs[0], &current).unwrap(),
                    crate::workspace_graph::reserve_builder_structure,
                );
                let (expected, actual) = match (old_result, current_result) {
                    (Ok(expected), Ok(actual)) => (expected.0, actual.0),
                    (Err(crate::hir::identity_resolution::Failure::Source(old_errors)),
                     Err(crate::hir::identity_resolution::Failure::Source(current_errors))) => {
                        assert_old_alias_refusal(&old, &old_errors);
                        assert_eq!(old_errors, current_errors);
                        // Both historical owning-name routes are retained as
                        // actual failures. This additional oracle authenticates
                        // every produced field, then separates declaration and
                        // binding for the genuine current import-as success.
                        let oracle = provider_binding_oracle(&old, &programs);
                        let production = crate::workspace_graph::expected_projection::borrowed_imports::typed::alias::construct_transient(&programs[0], &authored, &programs, &registry, None, &mut owner).unwrap();
                        let expected = crate::hir::identity_resolution::resolve_source_nominal(
                            &oracle, layout.presentation(&programs[0], &oracle).unwrap(),
                            crate::workspace_graph::reserve_builder_structure).unwrap().0;
                        let actual = crate::hir::identity_resolution::resolve_source_nominal(
                            &production, layout.presentation(&programs[0], &production).unwrap(),
                            crate::workspace_graph::reserve_builder_structure).unwrap().0;
                        (expected, actual)
                    }
                    other => panic!(
                        "old/current historical outcome changed: {:?}",
                        (other.0.as_ref().map(|_| ()), other.1.as_ref().map(|_| ()))
                    ),
                };
                assert_eq!(expected, actual);
                // Genuine parsed renamed providers keep canonical metadata and
                // local lookup independently, including the all/mixed cases.
                for import in &programs[0].module_uses {
                    let provider = programs[1].functions.iter()
                        .find(|function| function.stable_id == import.persistent_id).unwrap();
                    let id = actual.declarations.function_id(&import.alias).unwrap();
                    assert_eq!(id.as_str(), import.persistent_id);
                    assert_eq!(actual.declarations.declaration(id).unwrap().name, provider.name);
                    assert_eq!(actual.function_templates.iter()
                        .find(|template| template.id == *id).unwrap().name, provider.name);
                    assert_eq!(actual.function_templates.iter()
                        .filter(|template| template.id == *id).count(), 1);
                    if id.as_str() == "std.collections.vec.with-capacity" {
                        let metadata = (actual.declarations.declaration(id).unwrap().clone(),
                            actual.function_templates.iter().find(|template| template.id == *id).unwrap().clone());
                        if let Some(first) = &canonical_metadata { assert_eq!(first, &metadata); }
                        else { canonical_metadata = Some(metadata); }
                        if import.alias == "with_capacity" { same_provider_bindings[0] = true; }
                        if import.alias == "make_vec" { same_provider_bindings[1] = true; }
                    }
                }
                crate::hir::validate(&actual).unwrap();
            });
        });
        assert!(!overflowed && first.is_none());
    }
    assert_eq!(same_provider_bindings, [true, true]);
    // This broader sealed presentation can hold ordinary generic AND
    // monomorphic aliases; neither may inherit the opposite naming policy.
    // Fixture construction is outside the paid resolver invocation.
    {
        use crate::ast::program_source::function_alias::{FunctionDirectory, FunctionEntry, FunctionView, TransientFunctions};
        use crate::ast::program_source::generated_roots::GeneratedRoots;
        let provider = crate::parse("module ordinary.provider; @id(\"ordinary.generic\") fn generic<T>(value:T)->T{value} @id(\"ordinary.plain\") fn plain(value:i64)->i64{value}", "ordinary-provider.spx").unwrap();
        let caller = crate::parse("module ordinary.consumer; @id(\"ordinary.main\") fn main()->i64 {local_generic<i64>(7)+local_plain(8)}", "ordinary-consumer.spx").unwrap();
        let directory = FunctionDirectory {
            roots: GeneratedRoots::plain(Vec::new(), Vec::new()),
            entries: vec![
                FunctionEntry::Borrowed(FunctionView::checked(&provider, &provider.functions[0], "local_generic").unwrap()),
                FunctionEntry::Borrowed(FunctionView::checked(&provider, &provider.functions[1], "local_plain").unwrap()),
            ],
        };
        let input = TransientFunctions::new(&caller, directory);
        let ((), overflowed, _, first) = with_limit_usage_refusal(LIMIT, || {
            let resolved = crate::hir::identity_resolution::resolve_source_nominal(
                &input, None, crate::workspace_graph::reserve_builder_structure).unwrap().0;
            let generic = resolved.declarations.function_id("local_generic").unwrap();
            let plain = resolved.declarations.function_id("local_plain").unwrap();
            assert_eq!(generic.as_str(), "ordinary.generic");
            assert_eq!(plain.as_str(), "ordinary.plain");
            assert_eq!(resolved.declarations.declaration(generic).unwrap().name, "generic");
            assert_eq!(resolved.function_templates.iter().find(|item| item.id == *generic).unwrap().name, "generic");
            assert_eq!(resolved.declarations.declaration(plain).unwrap().name, "local_plain");
            assert_eq!(resolved.functions.iter().find(|item| item.id == *plain).unwrap().name, "local_plain");
            crate::hir::validate(&resolved).unwrap();
            drop(resolved);
        });
        assert!(!overflowed && first.is_none());
    }
}

#[test]
fn complete_old_current_allocations_reservations_reallocations_frontiers_and_drop_are_observed() {
    let caller = format!("{IMPORTS}{MAIN}");
    for (name, source) in [
        ("all-borrowed", caller.clone()),
        ("all-owned", renamed(&caller)),
        (
            "mixed",
            caller
                .replace("as get;", "as read_value;")
                .replace("get<i64>", "read_value<i64>"),
        ),
        ("synthetic-main", IMPORTS.to_owned()),
    ] {
        let programs = programs(&source);
        // Initialize unchanged shared metadata using the complete predecessor.
        assert!(observe(&programs, false, LIMIT).accepted);
        let old = observe(&programs, false, LIMIT);
        let current = observe(&programs, true, LIMIT);
        assert!(old.accepted && current.accepted);
        settled(&old);
        settled(&current);
        if name == "all-borrowed" {
            assert_eq!(
                old.metrics.allocated - current.metrics.allocated,
                3 * std::mem::size_of::<Function>()
            );
            assert_eq!(old.metrics.allocations, current.metrics.allocations + 1);
        }
        // The original full-capacity policy is acquired once in every owned lane.
        if name != "all-borrowed" {
            assert_eq!(current.metrics.reallocations, old.metrics.reallocations);
            assert_eq!(current.metrics.allocations, old.metrics.allocations);
            assert_eq!(current.metrics.allocated, old.metrics.allocated);
        }
        eprintln!("847 owned directory {name}: old={old:?}; current={current:?}");
        for limit in [0, current.paid / 2, current.paid - 1, current.paid] {
            let result = observe(&programs, true, limit);
            settled(&result);
            assert_eq!(
                result.accepted,
                limit == current.paid,
                "{name}: limit={limit} {result:?}"
            );
            if limit == current.paid {
                assert!(!result.overflowed && result.first.is_none());
            }
            if limit == current.paid - 1 {
                assert!(result.overflowed);
            }
        }
        if name == "all-owned" {
            let prefix = current
                .first_owned_paid
                .expect("an actual owned payload was initialized");
            assert!(prefix < current.paid);
            let partial = observe(&programs, true, prefix);
            assert!(!partial.accepted && partial.overflowed && partial.first.is_some());
            assert!(partial.metrics.allocated > 0);
            assert_eq!(partial.first_owned_paid, Some(prefix));
            settled(&partial);
            eprintln!("847 after real first owned push limit={prefix}: {partial:?}");
        }
        // Observe rather than equate the changed physical/receiving frontier.
        let old_at_new = observe(&programs, false, current.paid);
        settled(&old_at_new);
        eprintln!(
            "847 owned directory old at new limit={}: {old_at_new:?}",
            current.paid
        );
    }
}

#[test]
fn reserved_alias_diagnostics_and_sticky_partial_owned_failure_remain_exact() {
    let caller = format!("{IMPORTS}{MAIN}");
    let mut programs = programs(&caller);
    programs[0].module_uses[2].alias = "string_from_utf8".to_owned();
    let authored = index_authored(&programs).unwrap();
    let ((), overflowed, _, _) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::before_owned_directory::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let current = construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let expected = crate::source_verify::verify(&old);
        let actual = crate::source_verify::verify(&current);
        assert_eq!(expected, actual);
        assert!(actual
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-S113"));
    });
    assert!(!overflowed);
    let programs = programs_for_owned(&caller);
    let authored = index_authored(&programs).unwrap();
    let ((), overflowed, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        // Select the original sticky refusal, then demand a new constructor.
        assert!(owner.reserve(LIMIT).is_err());
        let before = crate::bounded_output::active_remaining();
        let ((), metrics) = measure(|| {
            assert!(construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner
            )
            .is_err());
        });
        assert_eq!(before, crate::bounded_output::active_remaining());
        assert!(!metrics.invalid && metrics.allocated == 0);
    });
    assert!(overflowed && first.is_some());
}
fn programs_for_owned(caller: &str) -> [Program; 2] {
    programs(&renamed(caller))
}

// Authenticate the lazy successor, then restore the exact private e8 checkpoint.
// Its complete earlier custody table remains unchanged and runs independently.
fn lazy_previous_source(source: &str, current: &str) -> String {
    let current = crate::source_reflows_merge::maina3_847_input(source, current);
    let manifest: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/owned-function-directory-lazy-847/source-custody.json"
    )))
    .unwrap();
    for row in manifest["changed"].as_array().unwrap() {
        if row["source"].as_str().unwrap() != source {
            continue;
        }
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            row["sha256"]
        );
        let before = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(row["before"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
            ),
            row["before_sha256"]
        );
        return before;
    }
    current.to_owned()
}

// Restore only exact authenticated complete immediate predecessor inputs before
// the original 847 source closure's independent inverses run. Tables unchanged.
pub(super) fn previous_source(source: &str, current: &str) -> String {
    let current = lazy_previous_source(source, current);
    let manifest: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/owned-function-directory-847/source-custody.json"
    )))
    .unwrap();
    for row in manifest["changed"].as_array().unwrap() {
        if row["source"].as_str().unwrap() != source {
            continue;
        }
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            row["sha256"]
        );
        let before = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(row["before"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
            ),
            row["before_sha256"]
        );
        return before;
    }
    current.to_owned()
}
#[test]
fn complete_immediate_directory_producer_and_expanded_acquisition_remain_source_bound() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/owned-function-directory-847/source-custody.json"
    )))
    .unwrap();
    for section in ["changed", "added"] {
        for row in manifest[section].as_array().unwrap() {
            let path = row["source"].as_str().unwrap();
            let source = std::fs::read_to_string(root.join(path)).unwrap();
            let source = lazy_previous_source(path, &source);
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
                ),
                row["sha256"]
            );
        }
    }
    let successor: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/owned-function-directory-lazy-847/source-custody.json"
    )))
    .unwrap();
    for row in successor["changed"].as_array().unwrap() {
        let path = row["source"].as_str().unwrap();
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        lazy_previous_source(path, &source);
    }
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/owned-function-directory-847/alias.before.rs.txt"
    ));
    let expected = original
        .split_once("#[cfg(test)]\n#[path = \"alias/tests.rs\"]")
        .unwrap()
        .0
        .replace(
            "super::super::super::super::",
            "super::super::super::super::super::",
        )
        .replace("use super::super::{", "use super::super::super::{")
        .replace("use super::typed_cost;", "use super::super::typed_cost;");
    assert_eq!(
        format!("{}\n", expected.trim_end()),
        include_str!("before_owned_directory.rs")
    );
    let tests = crate::source_reflows_merge::maina3_847_input("src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs", include_str!("tests.rs")).replace(
        "        let current = super::owned_growth_tests::previous_source(source, &current);\n",
        "",
    );
    assert_eq!(
        tests,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/owned-function-directory-847/tests.before.rs.txt"
        ))
    );
}


/// Additional, test-only provider/binding oracle. This is not the historical
/// receiver and is never inside its paid/allocation observation window. Every
/// declaration produced by the old route is authenticated completely before
/// any original provider payload can stand in for its local binding spelling.
pub(super) fn provider_binding_oracle<'p>(
    old: &dyn ProgramSource,
    programs: &'p [Program],
) -> crate::ast::program_source::function_alias::TransientFunctions<'p> {
    use crate::ast::program_source::function_alias::{FunctionDirectory, FunctionEntry, FunctionView, TransientFunctions};
    use crate::ast::program_source::{generated_roots::GeneratedRoots, FunctionSlices};
    let authored = &programs[0];
    assert_eq!(old.path(), authored.path);
    assert_eq!(old.module(), authored.module);
    assert!(old.functions().len() <= 8 && old.types().len() <= 16);
    for (index, function) in authored.functions.iter().enumerate() {
        assert_eq!(old.functions().get(index), Some(function));
    }
    let mut directory = FunctionDirectory {
        roots: GeneratedRoots::plain(old.types().iter().skip(authored.types.len()).cloned().collect(), Vec::new()),
        entries: Vec::new(),
    };
    for function in old.functions().iter().skip(authored.functions.len()) {
        let imported = authored.module_uses.iter().find(|import| {
            import.kind == crate::ast::ModuleUseKind::Function
                && import.persistent_id == function.stable_id
                && import.alias == function.name
        });
        let provider = imported.and_then(|import| programs.iter().find(|provider| provider.module == import.target_module));
        let original = provider.and_then(|provider| provider.functions.iter().find(|candidate| candidate.stable_id == function.stable_id));
        match (imported, provider, original) {
            (Some(import), Some(provider), Some(original))
                if crate::vec_ops::source_wrapper(provider, original).is_some()
                    || crate::box_ops::source_wrapper(provider, original).is_some() => {
                directory.entries.push(FunctionEntry::Borrowed(FunctionView::checked(provider, original, &import.alias).unwrap()));
            }
            _ => {
                directory.entries.push(FunctionEntry::Owned(directory.roots.functions.len()));
                directory.roots.functions.push(function.clone());
            }
        }
        let views = FunctionSlices::projected(&[], &directory).unwrap();
        assert!(views.get(views.len() - 1).unwrap().same_ordinary_declaration(function), "complete historical payload differs from authenticated provider/binding");
    }
    let bridge = TransientFunctions::new(authored, directory);
    assert_eq!(old.types().iter().collect::<Vec<_>>(), bridge.types().iter().collect::<Vec<_>>());
    assert_eq!(old.permits(), bridge.permits());
    assert_eq!(old.interfaces(), bridge.interfaces());
    assert_eq!(old.protocols(), bridge.protocols());
    assert_eq!(old.implementations(), bridge.implementations());
    assert_eq!(old.session_protocols(), bridge.session_protocols());
    assert_eq!(old.agents(), bridge.agents());
    bridge
}

/// Retain the old receiver's actual limitation as a complete diagnostic oracle.
/// Local import aliases are valid syntax; their old owning materialization
/// changed reserved declaration names and was not a successful HIR subject.
pub(super) fn assert_old_alias_refusal(
    old: &dyn ProgramSource,
    diagnostics: &[crate::diagnostic::Diagnostic],
) {
    let mut expected = Vec::new();
    for function in old.function_refs() {
        let kind = if crate::vec_ops::wrapper_by_id(&function.payload().stable_id)
            .is_some_and(|op| Some(function.name()) != crate::vec_ops::wrapper_name(op)) {
            Some(("SPX-T283", "vector"))
        } else if crate::box_ops::wrapper_by_id(&function.payload().stable_id)
            .is_some_and(|op| function.name() != crate::box_ops::wrapper_name(op)) {
            Some(("SPX-T286", "box"))
        } else { None };
        if let Some((code, kind)) = kind {
            // The old route's binding name is deliberately distinct from the
            // exact reserved wrapper declaration name; no forged success here.
            let mut diagnostic = crate::diagnostic::Diagnostic::error(code,
                format!("`{}.{}` must be one exact transparent compiler-owned {kind} wrapper", old.module(), function.name()),
                function.payload().span);
            diagnostic.path = Some(old.path().to_owned());
            expected.push(diagnostic);
        }
    }
    assert!(!expected.is_empty());
    assert_eq!(diagnostics, expected.as_slice());
}
