//! Complete overlay walk; only default and directory refusal custody changes.
#[path = "typed_cost.rs"]
mod typed_cost;
use super::{
    AuthoredDeclaration, LayoutRegistry, alias_clone, budgeted_format, eligible, graph_error,
    import_stub, iterative, refusal, type_rewrite,
};
use crate::ast::program_source::{SyntheticProgramView, generated_roots::GeneratedRoots};
use crate::ast::{Expr, ExprKind, Function, ModuleUseKind, Program, Type, TypeDeclaration};
use crate::workspace_graph::constructor_failure::{receiving::Receiver, typed::Failure};
use std::collections::BTreeMap;
use typed_cost::{pay_default_teardown, pay_signature, pay_teardown_minimum, pay_type};

pub(in crate::workspace_graph) fn construct_using<'p, 'a>(
    program: &'p Program,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    registry: &LayoutRegistry<'_>,
    core: Option<&mut import_stub::core_proofs::CoreProviderProofs<'a>>,
    owner: &mut Receiver,
) -> Result<SyntheticProgramView<'p>, Failure> {
    owner.entry::<SyntheticProgramView<'p>>()?;
    owner.reserve(0)?;
    if !eligible(program, authored, programs) {
        return Err(Failure::one(graph_error(
            "SPX-G173",
            "imported borrowed projection requires the owned route",
        )));
    }
    let layout = registry.get(program, authored, programs)?;
    let explicit = program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Type)
        .count();
    let type_count = explicit
        .checked_add(layout.entries.len())
        .ok_or_else(refusal)?;
    let recursive = iterative(program, authored);
    if recursive {
        pay_teardown_minimum(owner)?;
    }
    owner.reserve(std::mem::size_of::<GeneratedRoots>())?;
    let mut roots = if recursive {
        GeneratedRoots::iterative(directory::<TypeDeclaration>(type_count, owner)?, Vec::new())
    } else {
        GeneratedRoots::plain(directory::<TypeDeclaration>(type_count, owner)?, Vec::new())
    };
    let types = &mut roots.types;
    for import in program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Type)
    {
        let target = &authored[import.persistent_id.as_str()];
        let declaration = target.ty.expect("validated type target");
        pay_type(declaration, owner)?;
        if recursive {
            typed_cost::teardown_type(declaration, owner)?;
        }
        types.push(alias_clone::type_payload(declaration));
        let copy = types.last_mut().expect("inserted type");
        copy.name = owner.text(&import.alias)?;
        owner.reserve(0)?;
        super::super::super::generic_type_import::receiving::rewrite_declaration_with_layout(
            copy,
            target.module,
            program,
            programs,
            Some(&layout),
            owner,
        )?;
    }
    // Preserve the independent closure's exact order after explicit aliases.
    for entry in &layout.entries {
        pay_type(entry.declaration, owner)?;
        if recursive {
            typed_cost::teardown_type(entry.declaration, owner)?;
        }
        types.push(alias_clone::type_payload(entry.declaration));
        let copy = types.last_mut().expect("inserted private type");
        copy.name = owner.text(&entry.alias)?;
        owner.reserve(0)?;
        super::super::super::generic_type_import::receiving::rewrite_declaration_with_layout(
            copy,
            entry.module,
            program,
            programs,
            Some(&layout),
            owner,
        )?;
    }
    let all_count = program
        .types
        .len()
        .checked_add(types.len())
        .ok_or_else(refusal)?;
    let mut type_index = directory::<(&str, &TypeDeclaration)>(all_count, owner)?;
    type_index.extend(
        program
            .types
            .iter()
            .chain(types.iter())
            .map(|declaration| (declaration.name.as_str(), declaration)),
    );
    type_index.sort_unstable_by(|a, b| a.0.cmp(b.0));
    if type_index.windows(2).any(|items| items[0].0 == items[1].0) {
        return Err(Failure::one(graph_error(
            "SPX-G173",
            "synthetic workspace type-name index is not unique",
        )));
    }
    let needs_main = !program
        .functions
        .iter()
        .any(|function| function.name == "main")
        && !program
            .module_uses
            .iter()
            .any(|import| import.kind == ModuleUseKind::Function && import.alias == "main");
    let imports = program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Function)
        .count();
    roots.functions = directory::<Function>(
        imports
            .checked_add(usize::from(needs_main))
            .ok_or_else(refusal)?,
        owner,
    )?;
    let functions = &mut roots.functions;
    let mut byte_proofs =
        import_stub::core_proofs::Phase::new(authored, programs, core).map_err(Failure::one)?;
    for import in program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Function)
    {
        let target = &authored[import.persistent_id.as_str()];
        let original = target.function.expect("validated function target");
        let retain_body = programs
            .iter()
            .find(|provider| provider.module == target.module)
            .is_some_and(|provider| {
                crate::vec_ops::source_wrapper(provider, original).is_some()
                    || crate::box_ops::source_wrapper(provider, original).is_some()
            });
        if retain_body {
            typed_cost::teardown_function(original, true, owner)?;
        } else {
            pay_signature(original, owner)?;
            if recursive {
                typed_cost::teardown_function(original, false, owner)?;
            }
        }
        let function = if retain_body {
            alias_clone::retained_function(original, &import.alias)
        } else {
            alias_clone::signature_function(original, &import.alias)
        };
        functions.push(function);
        owner.reserve(0)?;
        if retain_body {
            continue;
        }
        let function = functions.last_mut().expect("inserted signature");
        for param in &mut function.params {
            type_rewrite::receiving::rewrite_with_layout(
                &mut param.ty,
                target.module,
                program,
                programs,
                Some(&layout),
                owner,
            )?;
        }
        type_rewrite::receiving::rewrite_with_layout(
            &mut function.return_type,
            target.module,
            program,
            programs,
            Some(&layout),
            owner,
        )?;
        if recursive {
            pay_default_teardown(&function.return_type, &type_index, owner)?;
        }
        function.body = import_stub::typed::default_expr(
            function,
            &type_index,
            super::super::super::owned_function_import::admitted_with_layout(
                program, target, authored, programs, &layout,
            ),
            byte_proofs
                .get()
                .map_err(Failure::one)?
                .authorized(original, target.module)
                .map_err(Failure::one)?,
            owner,
        )?;
    }
    // Same old rule also recognizes an imported alias named main.
    if needs_main && !functions.iter().any(|function| function.name == "main") {
        owner.reserve(std::mem::size_of::<Expr>())?;
        let stable_id =
            budgeted_format(format_args!("workspace.synthetic.main.{}", program.module));
        owner.reserve(0)?;
        let name = owner.text("main")?;
        functions.push(Function {
            stable_id,
            explicit_id: true,
            name,
            name_span: crate::ast::Span::default(),
            type_parameters: Vec::new(),
            params: Vec::new(),
            return_type: Type::I64,
            effects: Vec::new(),
            yields: None,
            follows: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            body: Expr {
                kind: ExprKind::Int(0),
                span: crate::ast::Span::default(),
            },
            span: crate::ast::Span::default(),
        });
        owner.reserve(0)?;
    }
    drop(type_index);
    // Infallible bounded string helpers record refusal and return empty text.
    // A generated view must never publish that partial construction as success.
    owner.reserve(0)?;
    let result = SyntheticProgramView::with_generated_owner(program, roots).ok_or_else(refusal)?;
    byte_proofs.complete().map_err(Failure::one)?;
    Ok(result)
}

/// Same exact-count directory as the independent old helper. The initialized
/// return, Vec header, layout and actual backing are paid by the original
/// receiver before allocation; a failed reservation propagates only Copy Setup.
fn directory<T>(count: usize, owner: &mut Receiver) -> Result<Vec<T>, Failure> {
    use std::mem::size_of;
    owner.entry::<Vec<T>>()?;
    owner.reserve(
        size_of::<Vec<T>>()
            + size_of::<usize>()
            + size_of::<std::alloc::Layout>()
            + size_of::<Result<std::alloc::Layout, std::alloc::LayoutError>>(),
    )?;
    let layout = match std::alloc::Layout::array::<T>(count) {
        Ok(layout) => layout,
        Err(_) => return Err(owner.physical().into()),
    };
    owner.reserve(layout.size())?;
    let values = Vec::with_capacity(count);
    if values.capacity() != count {
        return Err(owner.physical().into());
    }
    Ok(values)
}

#[cfg(test)]
mod directory_tests {
    use super::{Failure, Receiver, directory};
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::{current_metrics, measure};
    use crate::workspace_graph::constructor_failure::receiving::Setup;
    const LIMIT: usize = 1024 * 1024;

    #[test]
    fn typed_overlay_directories_match_original_values_and_whole_drop_at_every_byte_frontier() {
        for count in [0, 1, 4, 9] {
            let (((), physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                measure(|| {
                    let mut owner = Receiver::acquire().unwrap();
                    let mut values = directory::<u64>(count, &mut owner).unwrap();
                    assert_eq!(values.capacity(), count);
                    for index in 0..count {
                        values.push(index as u64);
                    }
                    assert_eq!(values.as_slice(), (0..count as u64).collect::<Vec<_>>());
                    drop(values);
                    drop(owner);
                })
            });
            assert!(!refused && first.is_none());
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
            // Measure the owning helper without test-only comparison allocation.
            let ((), refused, exact, first) = with_limit_usage_refusal(LIMIT, || {
                let mut owner = Receiver::acquire().unwrap();
                drop(directory::<u64>(count, &mut owner).unwrap());
                drop(owner);
            });
            assert!(!refused && first.is_none());
            for limit in 0..=exact {
                let (((), physical), refused, used, first) = with_limit_usage_refusal(
                    limit,
                    || {
                        measure(|| match Receiver::acquire() {
                            Err(Setup::Budget { .. }) => {}
                            Err(other) => panic!("unexpected owner failure: {other:?}"),
                            Ok(mut owner) => {
                                match directory::<u64>(count, &mut owner) {
                                    Ok(values) => {
                                        assert_eq!(limit, exact);
                                        assert_eq!(values.capacity(), count);
                                        drop(values);
                                    }
                                    Err(Failure::Setup(selected @ Setup::Budget { .. })) => {
                                        let stop = current_metrics().allocated;
                                        assert!(
                                            matches!(directory::<u64>(count, &mut owner), Err(Failure::Setup(next)) if next == selected)
                                        );
                                        drop(owner);
                                        assert_eq!(current_metrics().allocated, stop);
                                        return;
                                    }
                                    Err(other) => panic!("unexpected directory failure: {other:?}"),
                                }
                                drop(owner);
                            }
                        })
                    },
                );
                assert_eq!(refused, limit != exact);
                assert_eq!(first.is_some(), refused);
                assert!(used <= limit);
                assert!(
                    !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
                );
                assert!(physical.allocated <= used);
            }
            let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                let mut original = super::super::directory::<u64>(count).unwrap();
                for index in 0..count {
                    original.push(index as u64);
                }
                assert_eq!(original.capacity(), count);
                assert!(original.iter().copied().eq(0..count as u64));
                drop(original);
            });
            assert!(!refused && first.is_none());
        }
    }

    #[test]
    fn typed_overlay_directory_foreign_restored_and_overflow_exits_keep_original_first_setup() {
        let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let mut owner = Receiver::acquire().unwrap();
            let (((), physical), refused, used, first) = with_limit_usage_refusal(LIMIT, || {
                measure(|| {
                    assert!(matches!(
                        directory::<u64>(1, &mut owner),
                        Err(Failure::Setup(Setup::Detached))
                    ));
                })
            });
            assert!(!refused && used == 0 && first.is_none());
            assert_eq!(physical.allocated, 0);
            let ((), physical) = measure(|| {
                assert!(matches!(
                    directory::<u64>(usize::MAX, &mut owner),
                    Err(Failure::Setup(Setup::Detached))
                ));
                drop(owner);
            });
            assert_eq!(physical.allocated, 0);
        });
        assert!(refused && first.is_some());
        let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let mut owner = Receiver::acquire().unwrap();
            let ((), physical) = measure(|| {
                let selected = match directory::<u64>(usize::MAX, &mut owner).unwrap_err() {
                    Failure::Setup(selected @ Setup::Budget { .. }) => selected,
                    other => panic!("unexpected overflow failure: {other:?}"),
                };
                assert!(
                    matches!(directory::<u64>(1, &mut owner), Err(Failure::Setup(next)) if next == selected)
                );
                drop(owner);
            });
            assert_eq!(physical.allocated, 0);
        });
        assert!(refused);
        assert_eq!(first.unwrap().requested, usize::MAX);
    }

    #[test]
    fn typed_overlay_directory_complete_source_inverses_and_three_live_calls_are_bound() {
        use sha2::{Digest, Sha256};
        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-directory-propagation-825/source-inverses.json"
        )))
        .unwrap();
        let sources = [
            (
                include_str!("typed.rs"),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-directory-propagation-825/original/00.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/CONSTRUCTOR-FAILURE-RECEIVING-V1.md"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-directory-propagation-825/original/01.before"
                )),
            ),
            (
                include_str!("../borrowed_imports.rs"),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-directory-propagation-825/original/02.before"
                )),
            ),
        ];
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), sources.len());
        for (row, (current, original)) in rows.iter().zip(sources) {
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
                ),
                row["current_sha256"].as_str().unwrap()
            );
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
                ),
                row["original_sha256"].as_str().unwrap()
            );
            let mut restored = current.to_owned();
            for edit in row["edits"].as_array().unwrap() {
                let new = edit["new"].as_str().unwrap();
                assert_eq!(restored.matches(new).count(), 1);
                restored = restored.replacen(new, edit["before"].as_str().unwrap(), 1);
            }
            assert_eq!(restored, original);
        }
        let live = sources[0].0.split("#[cfg(test)]").next().unwrap();
        assert_eq!(
            live.matches("directory::<TypeDeclaration>(type_count, owner)?")
                .count(),
            2
        );
        assert!(live.contains("directory::<(&str, &TypeDeclaration)>(all_count, owner)?"));
        assert!(live.contains("roots.functions = directory::<Function>"));
        assert!(!live.contains("budgeted_format, directory"));
    }
}
