//! Actual signature rewriting borrows identities and uses the original receiver.
//! The whole previous rewriting and identity receivers remain independent.
use super::record_element;
use crate::ast::{ModuleUseKind, Program, Type};
use crate::diagnostic::Diagnostic;
use crate::workspace_graph::constructor_failure::{
    receiving::{Receiver, Setup},
    typed::Failure,
};
use crate::workspace_graph::expected_projection::private_layout::LayoutClosure;
use std::mem::{size_of, size_of_val};

// The same provider/local/import ordering as the complete original resolve_type_id.
// This returns only a source-held identity, never a new import or layout grant.
fn identity<'p>(
    module: &str,
    name: &str,
    programs: &'p [Program],
    owner: &mut Receiver,
) -> Result<Option<&'p str>, Setup> {
    owner.reserve(
        size_of::<Result<Option<&str>, Setup>>()
            + size_of_val(&module)
            + size_of_val(&name)
            + size_of_val(&programs)
            + size_of_val(&owner),
    )?;
    let Some(program) = programs.iter().find(|item| item.module == module) else {
        return Ok(None);
    };
    if let Some(local) = program.types.iter().find(|item| item.name == name) {
        return Ok(Some(local.stable_id.as_str()));
    }
    Ok(program
        .module_uses
        .iter()
        .find(|item| item.kind == ModuleUseKind::Type && item.alias == name)
        .map(|item| item.persistent_id.as_str()))
}

fn vector_element<'t>(
    ty: &'t Type,
    module: &str,
    caller: &Program,
    programs: &[Program],
    layout: Option<&LayoutClosure<'_>>,
    owner: &mut Receiver,
) -> Result<Option<&'t Type>, Failure> {
    owner.entry::<Option<&Type>>()?;
    owner.reserve(
        size_of_val(&ty)
            + size_of_val(&module)
            + size_of_val(&caller)
            + size_of_val(&programs)
            + size_of_val(&layout),
    )?;
    let Type::Named { name, arguments } = ty else {
        return Ok(None);
    };
    let [element] = arguments.as_slice() else {
        return Ok(None);
    };
    let Some(provider) = programs.iter().find(|program| program.module == module) else {
        return Ok(None);
    };
    let id = identity(module, name, programs, owner)?.or_else(|| {
        crate::prelude::declarations_for_program(provider)
            .iter()
            .find(|declaration| declaration.name == *name)
            .map(|declaration| declaration.stable_id.as_str())
    });
    if id != Some(crate::prelude::VEC_ID) {
        return Ok(None);
    }
    if crate::vec_ops::ast_vec_element_is_admitted(element) || *element == Type::String {
        return Ok(Some(element));
    }
    let Type::Named { name, arguments } = element else {
        return Ok(None);
    };
    if !arguments.is_empty() {
        return Ok(None);
    }
    let Some(id) = identity(module, name, programs, owner)? else {
        return Ok(None);
    };
    let Some(declaration) = programs
        .iter()
        .flat_map(|program| &program.types)
        .find(|declaration| declaration.stable_id == id)
    else {
        return Ok(None);
    };
    Ok((record_element(declaration)
        && (caller
            .module_uses
            .iter()
            .any(|item| item.kind == ModuleUseKind::Type && item.persistent_id == id)
            || layout.is_some_and(|l| l.alias(caller, id).is_some())))
    .then_some(element))
}

fn missing(id: &str, owner: &mut Receiver) -> Result<Diagnostic, Failure> {
    owner.entry::<Diagnostic>()?;
    owner.reserve(
        size_of_val(&id)
            + size_of::<Diagnostic>()
            + 2 * size_of::<&str>()
            + size_of::<String>()
            + size_of::<usize>()
            + size_of::<Option<usize>>(),
    )?;
    let prefix = "cross-file signature type `";
    let suffix = "` is not explicitly imported";
    let bytes = prefix
        .len()
        .checked_add(id.len())
        .and_then(|n| n.checked_add(suffix.len()))
        .ok_or_else(|| owner.limit())?;
    owner.reserve(bytes)?;
    let mut message = String::with_capacity(bytes);
    if message.capacity() != bytes {
        return Err(owner.physical().into());
    }
    message.push_str(prefix);
    message.push_str(id);
    message.push_str(suffix);
    Ok(Diagnostic::io("SPX-G172", message))
}

pub(in crate::workspace_graph) fn rewrite_with_layout(
    ty: &mut Type,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    layout: Option<&LayoutClosure<'_>>,
    owner: &mut Receiver,
) -> Result<(), Failure> {
    owner.entry::<()>()?;
    owner.reserve(
        size_of_val(&ty)
            + size_of_val(&target_module)
            + size_of_val(&caller)
            + size_of_val(&programs)
            + size_of_val(&layout),
    )?;
    if vector_element(ty, target_module, caller, programs, layout, owner)?.is_some() {
        let Type::Named { arguments, .. } = ty else {
            unreachable!()
        };
        return rewrite_with_layout(
            &mut arguments[0],
            target_module,
            caller,
            programs,
            layout,
            owner,
        );
    }
    if crate::stdin_stream_ops::ast_is_reader(ty)
        || crate::map_ops::ast_collection(ty)
        || crate::vec_ops::ast_copy_vec(ty)
    {
        return Ok(());
    }
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    if !arguments.is_empty() {
        return Err(Failure::one(Diagnostic::io_static(
            "SPX-G172",
            "generic cross-file types are not admitted",
        )));
    }
    let Some(target_id) = identity(target_module, name, programs, owner)? else {
        return Err(Failure::one(Diagnostic::io_static(
            "SPX-G173",
            "cross-file type identity lookup disagrees",
        )));
    };
    let alias = caller
        .module_uses
        .iter()
        .find(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id)
        .map(|item| item.alias.as_str())
        .or_else(|| layout.and_then(|l| l.alias(caller, target_id)));
    let Some(alias) = alias else {
        return Err(Failure::one(missing(target_id, owner)?));
    };
    // Do not replace the isolated synthetic source value with an empty sentinel.
    // Publication follows complete text acquisition under the same original owner.
    let replacement = owner.text(alias)?;
    *name = replacement;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::{current_metrics, measure};
    const LIMIT: usize = 8 * 1024 * 1024;
    const PROVIDER: &str = r#"module vector.provider;
@id("row") record Row { @id("row.id") id:i64, @id("row.text") text:string, }
@id("inspect") fn inspect(values:borrow Vec<Row>)->usize{0usize}"#;
    const CALLER: &str = r#"module vector.caller;
use type @id("row") from vector.provider as ImportedRow;
@id("main") fn main()->i64{0}"#;
    fn programs(provider: &str, caller: &str) -> [Program; 2] {
        [
            crate::parse(provider, "provider.spx").unwrap(),
            crate::parse(caller, "caller.spx").unwrap(),
        ]
    }
    #[test]
    fn typed_signature_identity_borrows_exact_original_local_import_and_static_prelude_bytes() {
        let mut programs = programs(PROVIDER, CALLER);
        programs[0].types[0].stable_id = "row.λ\0🙂".to_owned();
        for (module, name) in [
            ("vector.provider", "Row"),
            ("vector.caller", "ImportedRow"),
            ("vector.provider", "Missing"),
            ("missing.module", "Row"),
        ] {
            let original = crate::workspace_graph::resolve_type_id(module, name, &programs);
            let (((), physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                measure(|| {
                    let mut owner = Receiver::acquire().unwrap();
                    let stop = current_metrics().allocated;
                    let borrowed = identity(module, name, &programs, &mut owner).unwrap();
                    assert_eq!(borrowed, original.as_deref());
                    if module == "vector.provider" && name == "Row" {
                        assert_eq!(
                            borrowed.unwrap().as_ptr(),
                            programs[0].types[0].stable_id.as_ptr()
                        );
                    }
                    assert_eq!(current_metrics().allocated, stop);
                    drop(owner);
                })
            });
            assert!(!refused && first.is_none());
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
        }
        let original = &programs[0].functions[0].params[0].ty;
        let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let mut owner = Receiver::acquire().unwrap();
            // Static prelude Vec identity has no new String backing either.
            assert!(
                vector_element(
                    original,
                    "vector.provider",
                    &programs[1],
                    &programs,
                    None,
                    &mut owner
                )
                .unwrap()
                .is_none()
            );
            drop(owner);
        });
        assert!(!refused && first.is_none());
    }

    #[test]
    fn typed_signature_rewrite_matches_complete_old_success_refusals_and_all_six_json_fields() {
        for (provider, caller) in [
            (PROVIDER.to_owned(), CALLER.to_owned()),
            (
                PROVIDER.to_owned(),
                CALLER.replace(
                    "use type @id(\"row\") from vector.provider as ImportedRow;",
                    "",
                ),
            ),
            (
                PROVIDER.replace("text:string", "text:Vec<string>"),
                CALLER.to_owned(),
            ),
            (
                format!(
                    "{PROVIDER}\n@id(\"fake.vec\") record Vec {{ @id(\"fake.field\") field:i64, }}"
                ),
                CALLER.to_owned(),
            ),
        ] {
            let programs = programs(&provider, &caller);
            let source = &programs[0].functions[0].params[0].ty;
            let mut expected_type = source.clone();
            let (expected, refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                super::super::rewrite_with_layout(
                    &mut expected_type,
                    "vector.provider",
                    &programs[1],
                    &programs,
                    None,
                )
            });
            assert!(!refused && first.is_none());
            let (((), physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                measure(|| {
                    // Test input ownership and compatibility/JSON output stay in
                    // this observer; no whole-test funding inference is made.
                    let mut actual = source.clone();
                    let mut owner = Receiver::acquire().unwrap();
                    match (
                        rewrite_with_layout(
                            &mut actual,
                            "vector.provider",
                            &programs[1],
                            &programs,
                            None,
                            &mut owner,
                        ),
                        &expected,
                    ) {
                        (Ok(()), Ok(())) => {
                            assert_eq!(actual, expected_type);
                            drop(owner);
                        }
                        (Err(failure), Err(expected)) => {
                            let report = owner.receive_typed(failure).into_legacy_unfunded();
                            assert_eq!(&report, expected);
                            assert_eq!(
                                serde_json::to_value(&report).unwrap(),
                                serde_json::to_value(expected).unwrap()
                            );
                            drop(report);
                        }
                        (result, expected) => {
                            panic!("old/current rewrite disagreement: {result:?} {expected:?}")
                        }
                    }
                    drop(actual);
                })
            });
            assert!(!refused && first.is_none());
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
        }
        let mut dynamic_programs = programs(
            PROVIDER,
            "module vector.caller; @id(\"main\") fn main()->i64{0}",
        );
        dynamic_programs[0].types[0].stable_id = "row.λ\0🙂".to_owned();
        for name in ["Row", "Ghost"] {
            let original = Type::Named {
                name: name.to_owned(),
                arguments: Vec::new(),
            };
            let mut expected = original.clone();
            let expected = super::super::rewrite_with_layout(
                &mut expected,
                "vector.provider",
                &dynamic_programs[1],
                &dynamic_programs,
                None,
            )
            .unwrap_err();
            let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                let mut current = original.clone();
                let mut owner = Receiver::acquire().unwrap();
                let failure = rewrite_with_layout(
                    &mut current,
                    "vector.provider",
                    &dynamic_programs[1],
                    &dynamic_programs,
                    None,
                    &mut owner,
                )
                .unwrap_err();
                assert_eq!(
                    owner.receive_typed(failure).into_legacy_unfunded(),
                    expected
                );
            });
            assert!(!refused && first.is_none());
        }
        let programs = programs(
            include_str!("../private_layout/function_provider.spx"),
            include_str!("../private_layout/function_caller.spx"),
        );
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let registry = crate::workspace_graph::expected_projection::private_layout::registry::LayoutRegistry::new(&programs, &authored);
        let layout = registry.get(&programs[1], &authored, &programs).unwrap();
        assert!(!layout.entries.is_empty());
        for function in &programs[0].functions {
            for source in
                std::iter::once(&function.return_type).chain(function.params.iter().map(|p| &p.ty))
            {
                let mut expected = source.clone();
                super::super::rewrite_with_layout(
                    &mut expected,
                    "function_layout.provider",
                    &programs[1],
                    &programs,
                    Some(&layout),
                )
                .unwrap();
                let (((), physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                    measure(|| {
                        let mut current = source.clone();
                        let mut owner = Receiver::acquire().unwrap();
                        rewrite_with_layout(
                            &mut current,
                            "function_layout.provider",
                            &programs[1],
                            &programs,
                            Some(&layout),
                            &mut owner,
                        )
                        .unwrap();
                        assert_eq!(current, expected);
                        drop(current);
                        drop(owner);
                    })
                });
                assert!(!refused && first.is_none());
                assert!(
                    !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
                );
            }
        }
    }

    #[test]
    fn typed_signature_rewrite_every_byte_frontier_keeps_first_setup_and_no_later_allocation() {
        let programs = programs(PROVIDER, CALLER);
        let source = &programs[0].functions[0].params[0].ty;
        let ((), refused, exact, first) = with_limit_usage_refusal(LIMIT, || {
            let mut ty = source.clone();
            let mut owner = Receiver::acquire().unwrap();
            rewrite_with_layout(
                &mut ty,
                "vector.provider",
                &programs[1],
                &programs,
                None,
                &mut owner,
            )
            .unwrap();
            drop(owner);
        });
        assert!(!refused && first.is_none());
        for limit in 0..=exact {
            let (((), physical), refused, _, first) = with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut ty = source.clone();
                    let mut owner = match Receiver::acquire() {
                        Ok(owner) => owner,
                        Err(Setup::Budget { .. }) => return,
                        Err(other) => panic!("unexpected receiver setup: {other:?}"),
                    };
                    match rewrite_with_layout(
                        &mut ty,
                        "vector.provider",
                        &programs[1],
                        &programs,
                        None,
                        &mut owner,
                    ) {
                        Ok(()) => assert_eq!(limit, exact),
                        Err(Failure::Setup(selected @ Setup::Budget { .. })) => {
                            let stop = current_metrics().allocated;
                            assert!(
                                matches!(rewrite_with_layout(&mut ty, "vector.provider", &programs[1], &programs, None, &mut owner), Err(Failure::Setup(next)) if next == selected)
                            );
                            drop(owner);
                            drop(ty);
                            assert_eq!(current_metrics().allocated, stop);
                            return;
                        }
                        Err(other) => panic!("unexpected typed rewrite failure: {other:?}"),
                    }
                    drop(owner);
                    drop(ty);
                })
            });
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_some(), refused);
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
        }
        let id = "row.λ\0🙂";
        let ((), refused, exact, first) = with_limit_usage_refusal(LIMIT, || {
            let mut owner = Receiver::acquire().unwrap();
            drop(missing(id, &mut owner).unwrap());
            drop(owner);
        });
        assert!(!refused && first.is_none());
        for limit in 0..=exact {
            let (((), physical), refused, _, first) = with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut owner = match Receiver::acquire() {
                        Ok(owner) => owner,
                        Err(Setup::Budget { .. }) => return,
                        Err(other) => panic!("unexpected owner failure: {other:?}"),
                    };
                    match missing(id, &mut owner) {
                        Ok(primary) => {
                            assert_eq!(limit, exact);
                            let stop = current_metrics().allocated;
                            let report = owner
                                .receive_typed(Failure::One(primary))
                                .into_legacy_unfunded();
                            assert_eq!(report.len(), 1);
                            drop(report);
                            assert_eq!(current_metrics().allocated, stop);
                        }
                        Err(Failure::Setup(selected @ Setup::Budget { .. })) => {
                            let stop = current_metrics().allocated;
                            assert!(
                                matches!(missing(id, &mut owner), Err(Failure::Setup(next)) if next == selected)
                            );
                            drop(owner);
                            assert_eq!(current_metrics().allocated, stop);
                        }
                        Err(other) => panic!("unexpected message failure: {other:?}"),
                    }
                })
            });
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_some(), refused);
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
        }
    }

    #[test]
    fn typed_signature_rewrite_foreign_and_restored_owner_cannot_replace_or_mutate_the_source() {
        let programs = programs(PROVIDER, CALLER);
        let source = &programs[0].functions[0].params[0].ty;
        let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let mut ty = source.clone();
            let mut owner = Receiver::acquire().unwrap();
            let (((), physical), refused, used, first) = with_limit_usage_refusal(7, || {
                measure(|| {
                    assert!(matches!(
                        rewrite_with_layout(
                            &mut ty,
                            "vector.provider",
                            &programs[1],
                            &programs,
                            None,
                            &mut owner
                        ),
                        Err(Failure::Setup(Setup::Detached))
                    ));
                })
            });
            assert!(!refused && used == 0 && first.is_none());
            assert_eq!(physical.allocated, 0);
            assert_eq!(&ty, source);
            assert!(matches!(
                rewrite_with_layout(
                    &mut ty,
                    "vector.provider",
                    &programs[1],
                    &programs,
                    None,
                    &mut owner
                ),
                Err(Failure::Setup(Setup::Detached))
            ));
            assert_eq!(&ty, source);
            drop(owner);
        });
        assert!(refused && first.is_some());
    }
    #[test]
    fn typed_signature_rewrite_complete_old_helpers_live_callers_and_whole_source_inverses_are_bound()
     {
        use sha2::{Digest, Sha256};
        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-signature-rewrite-825/source-inverses.json"
        )))
        .unwrap();
        let sources = [
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection/type_rewrite/receiving.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-signature-rewrite-825/original/00.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection/type_rewrite.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-signature-rewrite-825/original/01.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection/borrowed_imports/typed.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-signature-rewrite-825/original/02.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/workspace/semantic_graph/source_inventory.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-signature-rewrite-825/original/03.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/CONSTRUCTOR-FAILURE-RECEIVING-V1.md"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-signature-rewrite-825/original/04.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-signature-rewrite-825/original/05.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection/private_layout.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-signature-rewrite-825/original/06.before"
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
        let live = sources[2].0.split("#[cfg(test)]").next().unwrap();
        assert_eq!(
            live.matches("type_rewrite::receiving::rewrite_with_layout(")
                .count(),
            2
        );
        let current = sources[0].0.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "vec![",
            "budgeted_clone(",
            "budgeted_format(",
            "reserve_builder_structure(",
            "resolve_type_id(",
        ] {
            assert!(!current.contains(forbidden));
        }
        assert!(sources[5].0.contains("fn resolve_type_id("));
        assert!(
            sources[1]
                .0
                .contains("pub(in crate::workspace_graph) fn rewrite_with_layout(")
        );
    }
}
