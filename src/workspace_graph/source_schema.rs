//! Borrowed declaration authority for retained workspace Graph selection.
//! Frozen from-parts selectors do not acquire this authority.
use super::*;
use std::fmt::{self, Write};
mod byte_helper;
mod nominal_context;
mod scalar_lexical;
mod static_text;
pub(super) use byte_helper::Proofs;
pub(super) use static_text::REPORT;

struct MatchText<'a>(&'a str);
impl Write for MatchText<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0 = self.0.strip_prefix(text).ok_or(fmt::Error)?;
        Ok(())
    }
}

fn matches_text(text: &str, arguments: fmt::Arguments<'_>) -> bool {
    let mut matcher = MatchText(text);
    matcher.write_fmt(arguments).is_ok() && matcher.0.is_empty()
}

fn scalar_layout(ty: &hir::ResolvedType) -> Option<&'static str> {
    use hir::ResolvedType::*;
    Some(match ty {
        I64 => "scalar:i64",
        I32 => "scalar:i32",
        U8 => "scalar:u8",
        Usize => "scalar:usize",
        Char => "scalar:char",
        F32 => "scalar:f32",
        F64 => "scalar:f64",
        Bool => "scalar:bool",
        _ => return None,
    })
}

fn record_mode(
    ty: &hir::ResolvedType,
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Option<hir::OwnershipMode> {
    let hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let origin = facts.get(declaration.as_str())?;
    if origin.kind != hir::DeclarationKind::Record
        || origin.origin != hir::IdentityOrigin::Explicit
        || origin.owner.is_some()
    {
        return None;
    }
    let mut candidates = modules
        .iter()
        .flat_map(|owner| owner.types.iter().map(move |item| (owner, item)))
        .filter(|(_, item)| item.id == *declaration);
    let (owner, record) = candidates.next()?;
    if candidates.next().is_some()
        || origin.path.as_deref() != Some(owner.path.as_str())
        || origin.module.as_deref() != Some(owner.module.as_str())
        || !record.type_parameters.is_empty()
    {
        return None;
    }
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return None;
    };
    if !(1..=hir::copy_record_collection::MAX_FIELDS).contains(&fields.len()) {
        return None;
    }
    let (kind, checked) = module.signature_types.iter().find_map(|(key, value)| {
        matches_text(
            key,
            format_args!("nominal:{}:{}:0:", declaration.as_str().len(), declaration),
        )
        .then_some(value)
    })?;
    if *kind != hir::DeclarationKind::Record || checked.contains_resource || !checked.sized {
        return None;
    }
    let mut layout = MatchText(&checked.layout_key);
    if write!(
        layout,
        "record:{}:{}:{}:",
        declaration.as_str().len(),
        declaration,
        fields.len()
    )
    .is_err()
    {
        return None;
    }
    let mut owned_count = 0usize;
    let mut all_fields_explicit = true;
    for (position, field) in fields.iter().enumerate() {
        let field_origin = facts.get(field.id.as_str())?;
        if field_origin.kind != hir::DeclarationKind::Field
            || field_origin.owner.as_deref() != Some(declaration.as_str())
            || field_origin.path.as_deref() != Some(owner.path.as_str())
            || field_origin.module.as_deref() != Some(owner.module.as_str())
            || fields[..position]
                .iter()
                .any(|previous| previous.id == field.id)
        {
            return None;
        }
        let key = match field.ty {
            hir::ResolvedType::String => {
                owned_count += 1;
                "owned:string"
            }
            hir::ResolvedType::Bytes => {
                owned_count += 1;
                "owned:bytes"
            }
            _ => scalar_layout(&field.ty)?,
        };
        all_fields_explicit &= field_origin.origin == hir::IdentityOrigin::Explicit;
        if owned_count > hir::owned_leaf_collection::MAX_OWNED_FIELDS {
            return None;
        }
        if write!(
            layout,
            "{}:{}:{}:{}",
            field.id.as_str().len(),
            field.id,
            key.len(),
            key
        )
        .is_err()
        {
            return None;
        }
    }
    if !layout.0.is_empty()
        || checked.copy != (owned_count == 0)
        || checked.needs_drop != (owned_count != 0)
        || (owned_count != 0 && !all_fields_explicit)
    {
        return None;
    }
    Some(if owned_count == 0 {
        hir::OwnershipMode::Value
    } else {
        hir::OwnershipMode::Own
    })
}

/// Typed twin for the change view. Builder-structure refusals stay legacy
/// owned Vecs, including `reserve_builder_structure`'s own limit Vec; only a
/// selected replay refusal carries the caller's paid `form` report.
pub(super) fn all(
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    form: crate::cleanup_plan::SelectedReplayReportForm,
) -> Result<Vec<&'static str>, ViewRefusal> {
    let bytes = modules
        .len()
        .checked_mul(std::mem::size_of::<&'static str>())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(bytes)?;
    let mut schemas = Vec::with_capacity(modules.len());
    let allocated = schemas.capacity() * std::mem::size_of::<&'static str>();
    if allocated > bytes {
        reserve_builder_structure(allocated - bytes)?;
    }
    let mut proofs = byte_helper::Proofs::new(modules, facts);
    for module in modules {
        schemas.push(schema_with_proofs_paid(
            module,
            modules,
            facts,
            &mut proofs,
            form,
        )?);
    }
    Ok(schemas)
}

#[cfg(test)]
pub(super) fn schema(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<&'static str, Vec<Diagnostic>> {
    schema_with_proofs(
        module,
        modules,
        facts,
        &mut byte_helper::Proofs::new(modules, facts),
    )
}

/// Legacy pass-through boundary: a paid G410 report moves out unchanged; an
/// unpaid carrier refusal reaches the typed unpaid boundary, and only its
/// named `into_unfunded` allocates the same public literal here.
#[cfg(test)]
pub(super) fn schema_with_proofs<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    proofs: &mut byte_helper::Proofs<'a>,
) -> Result<&'static str, Vec<Diagnostic>> {
    schema_with_proofs_paid(module, modules, facts, proofs, static_text::REPORT)
        .map_err(ViewRefusal::into_legacy)
}

/// The caller's `form` is acquired as a paid carrier on each selected replay
/// ledger. Every other refusal here remains a legacy owned Vec.
pub(super) fn schema_with_proofs_paid<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    proofs: &mut byte_helper::Proofs<'a>,
    form: crate::cleanup_plan::SelectedReplayReportForm,
) -> Result<&'static str, ViewRefusal> {
    schema_receiving_paid(module, modules, facts, proofs, form, None)
}

/// With the receiving caller's own current sources, a module holding the
/// closed scalar lexical inventory is selected only through its consumed
/// receipt. Without sources, or without that inventory, every module keeps
/// its existing route, including the frozen from-parts G410 refusal.
pub(super) fn schema_receiving_paid<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    proofs: &mut byte_helper::Proofs<'a>,
    form: crate::cleanup_plan::SelectedReplayReportForm,
    sources: Option<&dyn ReceivingSources>,
) -> Result<&'static str, ViewRefusal> {
    if module.native_law {
        return Ok("semaprax.native-law.v1");
    }
    let static_schema = static_text::schema(module, facts)?;
    if let (None, Some(sources)) = (static_schema, sources) {
        if let Some(received) = scalar_lexical::receive_paid(module, modules, facts, sources, form)
            .map_err(ViewRefusal::Replay)?
        {
            // The receipt names this exact module; any other module is an
            // impossible caller error kept at the unfunded legacy literal.
            return received
                .select(module)
                .ok_or_else(|| ViewRefusal::Legacy(static_text::REPORT.legacy()));
        }
    }
    let mut lexical_static = false;
    if static_schema.is_some() {
        lexical_static = static_text::replay_lexical_paid(module, modules, facts, form)
            .map_err(ViewRefusal::Replay)?;
    }
    select(
        module,
        modules,
        facts,
        proofs,
        static_schema,
        lexical_static,
    )
    .map_err(ViewRefusal::Legacy)
}

fn select<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    proofs: &mut byte_helper::Proofs<'a>,
    static_schema: Option<&'static str>,
    lexical_static: bool,
) -> Result<&'static str, Vec<Diagnostic>> {
    let helper_schema = byte_helper::schema_with_proofs(module, modules, facts, proofs)?;
    let mut string_slots = false;
    for function in module.functions.iter().chain(
        module
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            hir::visit_resolved_calls(root, &mut |callee, _, _| {
                string_slots |= matches!(
                    callee.as_str(),
                    crate::vec_ops::string_slot::ID | crate::vec_ops::string_slot::BYTES_ID
                );
            });
        }
    }
    let finite_scratch = module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(crate::finite_byte_scratch::function_requires);
    let admission = |operation: crate::vec_ops::VecOp, element: &hir::ResolvedType| {
        if crate::vec_ops::resolved_element_is_admitted(element)
            && crate::vec_ops::resolved_operation_element_is_admitted(operation, element)
        {
            return Some(hir::OwnershipMode::Value);
        }
        if *element == hir::ResolvedType::String && operation.admits_owned_leaf()
            || *element == hir::ResolvedType::Bytes
                && crate::vec_ops::resolved_operation_element_is_admitted(operation, element)
        {
            return Some(hir::OwnershipMode::Own);
        }
        let mode = record_mode(element, module, modules, facts)?;
        match mode {
            hir::OwnershipMode::Value if !operation.owned_leaf_only() => Some(mode),
            hir::OwnershipMode::Own if operation.admits_owned_leaf() => Some(mode),
            _ => None,
        }
    };
    let authority = |function: &hir::ResolvedFunction| {
        hir::vec_loop_renewal::requires_with_admission(function, admission)
    };
    let consuming = module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(|function| {
            hir::vec_loop_renewal::consuming::requires_with_admission(function, admission)
        });
    // Static modules independently replay the other renewal obligations too.
    if static_schema.is_none() && !consuming && !string_slots {
        if finite_scratch {
            return Ok("semaprax.graph.v78");
        }
        if let Some(schema) = helper_schema {
            return Ok(schema);
        }
    }
    for function in module.functions.iter().chain(
        module
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        if lexical_static && hir::lexical_exit::requires(function).map_err(|error| vec![error])? {
            continue;
        }
        if helper_schema.is_some()
            && function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
        {
            continue;
        }
        let lexical = hir::lexical_exit::requires(function).map_err(|error| vec![error])?;
        if (function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18) != lexical
        {
            return Err(vec![graph_error(
                "SPX-G410",
                "lexical cleanup schema disagrees with authored exits",
            )]);
        }
        let byte = !lexical && crate::byte_ops::requires_same_owner_set(function);
        let string = !lexical && !byte && crate::string_ops::replacement::requires(function);
        let ordinary = !lexical && !byte && !string && authority(function);
        for (schema, expected, message) in [
            (
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17,
                byte,
                "byte-buffer renewal cleanup schema disagrees",
            ),
            (
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V16,
                string,
                "String replacement cleanup schema disagrees",
            ),
            (
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V15,
                ordinary,
                "ordinary Vec renewal cleanup schema disagrees",
            ),
        ] {
            if (function.cleanup_plan.schema == schema) != expected {
                return Err(vec![graph_error("SPX-G410", message)]);
            }
        }
    }
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(|f| {
            std::iter::once(&f.body)
                .chain(&f.requires)
                .chain(&f.ensures)
                .any(crate::vec_field::expression_uses)
        })
    {
        return Ok("semaprax.graph.v85");
    }
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(crate::hir::copy_discard_function_uses)
    {
        return Ok("semaprax.graph.v84");
    }
    // Byte traversal is the outer v83 layer, preserving row-loan v82 facts.
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(crate::hir::byte_traversal_function_uses)
    {
        return Ok("semaprax.graph.v83");
    }
    if graph::row_loan::functions_require(
        module
            .functions
            .iter()
            .chain(module.function_instances.iter().map(|i| &i.function)),
    ) {
        return Ok(graph::row_loan::SCHEMA);
    }
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .try_fold(false, |found, f| {
            crate::hir::vec_constructor::function_uses(f).map(|uses| found || uses)
        })
        .map_err(|e| vec![e])?
    {
        return Ok("semaprax.graph.v81");
    }
    if string_slots {
        return Ok("semaprax.graph.v80");
    }
    if consuming {
        return Ok("semaprax.graph.v79");
    }
    if let Some(schema) = static_schema {
        return Ok(if finite_scratch {
            "semaprax.graph.v78"
        } else {
            schema
        });
    }
    reserve_builder_structure(nominal_context::RECEIVING_CONTROLS)?;
    let types = nominal_context::Types {
        module,
        modules,
        facts,
    };
    if !module
        .functions
        .iter()
        .chain(
            module
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| authority(function) && !hir::vec_loop_renewal::requires(function))
    {
        return graph::graph_schema_from_parts_and_instances(
            &module.interfaces,
            &types,
            &module.functions,
            &module.function_templates,
            &module.function_instances,
        )
        .map_err(|error| vec![error]);
    }
    graph::graph_schema_from_parts_and_instances_with_renewal_authority(
        &module.interfaces,
        &types,
        &module.functions,
        &module.function_templates,
        &module.function_instances,
        Some(&authority),
    )
    .map_err(|error| vec![error])
}

// Hostile controls mutate a detached copy through the same explicit paid API
// required of any shared owner. Later forged writes remain ordinary host input.
#[cfg(test)]
fn materialize_host_fields(
    payload: &mut hir::NominalPayload<hir::ResolvedFieldDeclaration>,
) -> &mut Vec<hir::ResolvedFieldDeclaration> {
    let shared = payload.is_shared();
    let (result, refused, used, first) =
        crate::bounded_output::with_limit_usage_refusal(MAX_BUILDER_BYTES, || {
            payload
                .try_make_mut(|bytes| {
                    crate::bounded_output::reserve_active_required(bytes)
                        .then_some(())
                        .ok_or(())
                })
                .map(|_| ())
        });
    assert!(
        !refused && first.is_none(),
        "host materialization must fit the unchanged builder cap"
    );
    if shared {
        assert!(
            used > 0,
            "a shared host payload must explicitly pay for detachment"
        );
    }
    result.expect("explicit paid host materialization");
    // Successful explicit materialization left the legacy owned Vec carrier.
    payload
}

#[cfg(test)]
mod consuming_tests;

#[cfg(test)]
pub(super) use static_text::{with_prepared_test_context, with_test_context};

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> WorkspaceGraphBuild {
        let source = r#"module app.main;
@id("row") record Row { @id("row.n") n:i64, }
@id("main") fn main()->i64 {
 let mut rows=vec_with_capacity<Row>(2usize);
 let mut i=0;
 while i<2 { rows=vec_push<Row>(rows,Row{n:i}); i=i+1; 0 }
 if vec_len<Row>(rows)==2usize {7}else{0}
}
"#;
        build_source(source)
    }

    pub(super) fn build_source(source: &str) -> WorkspaceGraphBuild {
        let sources = [
            ("app/main.spx", source),
            (
                "lib/support.spx",
                "module lib.support; @id(\"ready\") fn ready()->i64 {0}",
            ),
        ]
        .into_iter()
        .map(|(path, source)| WorkspaceSource {
            path: path.to_owned(),
            source: format::canonical(&crate::parse(source, Path::new(path)).unwrap()),
        })
        .collect();
        build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
            .unwrap()
            .0
    }

    #[test]
    fn string_slot_schema_preserves_scalar_renewal_and_authored_lexical_precedence() {
        for lexical in [false, true] {
            let exit = if lexical {
                "if n==0 {return str_len_bytes(vec_str_at(values,0usize));}else{0}"
            } else {
                ""
            };
            let source = format!(
                r#"module app.main;
@id("inspect") fn inspect(n:i64)->i64 {{
 let values=vec_push<string>(vec_with_capacity<string>(1usize),"ok");
 let mut ranks=vec_with_capacity<i64>(1usize); let mut i=0;
 while i<1 {{ ranks=vec_push<i64>(ranks,i); i=i+1; 0 }}
 {exit}
 str_len_bytes(vec_str_at(values,0usize))
}}
@id("main") fn main()->i64 {{inspect(1)}}
"#
            );
            let mut build = build_source(&source);
            let expected = if lexical {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
            } else {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V15
            };
            let function = build
                .hir
                .modules
                .iter()
                .flat_map(|m| &m.functions)
                .find(|f| f.id.as_str() == "inspect")
                .unwrap();
            assert_eq!(function.cleanup_plan.schema, expected);
            assert_eq!(
                build.source_graph_schemas().unwrap()["app/main.spx"],
                "semaprax.graph.v80"
            );
            let forged = if lexical {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V15
            } else {
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
            };
            build
                .hir
                .modules
                .iter_mut()
                .flat_map(|m| &mut m.functions)
                .find(|f| f.id.as_str() == "inspect")
                .unwrap()
                .cleanup_plan
                .schema = forged;
            assert_eq!(
                build.source_graph_schemas().unwrap_err()[0].code,
                "SPX-G410"
            );
        }
    }

    #[test]
    fn nominal_renewal_schema_replays_retained_record_authority_and_preserves_frozen_refusal() {
        let build = build();
        let module = &build.hir.modules[0];
        assert_eq!(
            build.source_graph_schemas().unwrap()["app/main.spx"],
            "semaprax.graph.v66"
        );
        assert_eq!(
            graph::graph_schema_from_parts_and_instances(
                &module.interfaces,
                &module.types,
                &module.functions,
                &module.function_templates,
                &module.function_instances
            )
            .unwrap_err()
            .code,
            "SPX-G410"
        );
        assert_eq!(
            build.into_change_view().unwrap().modules[0].source_graph_schema,
            "semaprax.graph.v66"
        );
    }

    #[test]
    fn nominal_renewal_schema_rejects_field_origin_layout_and_duplicate_authority_drift() {
        for mutation in 0..5 {
            let mut build = build();
            match mutation {
                0 => {
                    build.hir.declarations.remove("row");
                }
                1 => {
                    build.hir.declarations.get_mut("row").unwrap().origin =
                        hir::IdentityOrigin::Automatic;
                }
                2 => {
                    let hir::ResolvedTypeDeclarationKind::Record { fields } =
                        &mut build.hir.modules[0].types[0].kind
                    else {
                        panic!("record")
                    };
                    materialize_host_fields(fields)[0].ty = hir::ResolvedType::Bool;
                }
                3 => {
                    build.hir.modules[0]
                        .signature_types
                        .values_mut()
                        .find(|(kind, _)| *kind == hir::DeclarationKind::Record)
                        .unwrap()
                        .1
                        .layout_key
                        .push('!');
                }
                _ => {
                    let record = build.hir.modules[0].types[0].clone();
                    build.hir.modules[1].types.push(record);
                }
            }
            assert_eq!(
                build.source_graph_schemas().unwrap_err()[0].code,
                "SPX-G410",
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn higher_scoped_field_schema_cannot_mask_forged_string_or_byte_renewal() {
        let source = r#"module app.main;
@id("row") record Row { @id("row.title") title:string, @id("row.marker") marker:i64, }
@id("inspect") fn inspect(values:borrow Vec<Row>,index:usize)->i64 {
 let view=str_as_bytes(vec_field<Row>(values,index,"title"));
 let marker=vec_field<Row>(values,index,"marker");
 marker+i64_from_usize(byte_len(view))
}
@id("main") fn main()->i64 {
 let mut rows=vec_with_capacity<Row>(2usize);
 let mut i=0;
 while i<2 { rows=vec_push<Row>(rows,Row{title:"row",marker:i}); i=i+1; 0 }
 inspect(rows,0usize)
}
"#;
        for forged in [
            crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V16,
            crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17,
        ] {
            let mut build = build_source(source);
            assert_eq!(
                build.source_graph_schemas().unwrap()["app/main.spx"],
                "semaprax.graph.v85"
            );
            build.hir.modules[0]
                .functions
                .iter_mut()
                .find(|function| function.id.as_str() == "inspect")
                .unwrap()
                .cleanup_plan
                .schema = forged;
            assert_eq!(
                build.source_graph_schemas().unwrap_err()[0].code,
                "SPX-G410"
            );
        }
    }
    #[test]
    fn explicit_host_materialization_preserves_complete_original_hostile_controls() {
        use sha2::{Digest, Sha256};
        let sources = [
            (include_str!("source_schema.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/nominal-host-mutators-827-828/original/src__workspace_graph__source_schema.rs")), "389e63a3e6075306619971be242c0b2a186aaa01024ff04206dcef5461e366b7"),
            (include_str!("source_schema/consuming_tests.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/nominal-host-mutators-827-828/original/src__workspace_graph__source_schema__consuming_tests.rs")), "8feff6a9485dd619c77e5459a9c785e8837a7e2905cc8adb99b08dab62d9125d"),
            (include_str!("source_schema/byte_helper/tests.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/nominal-host-mutators-827-828/original/src__workspace_graph__source_schema__byte_helper__tests.rs")), "194518112329c43d5e47f2a15a4387df0b0294182b1f51bb3d63e1f6822fcb3d"),
            (include_str!("source_schema/static_text/tests/retained_lexical_composition.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/nominal-host-mutators-827-828/original/src__workspace_graph__source_schema__static_text__tests__retained_lexical_composition.rs")), "4a3b721df47651668991ca7a357462ffdedc3f792e6b582e8f75e85a3d5995e0"),
            (include_str!("source_schema/static_text/primitive/records/tests.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/nominal-host-mutators-827-828/original/src__workspace_graph__source_schema__static_text__primitive__records__tests.rs")), "cb1a00842c2ee1693200cdecbb6b9d7a7fc6a3a3406964cced2f4752aed32abb"),
        ];
        let inverses: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/nominal-host-mutators-827-828/exact-inverses.json"
        )))
        .unwrap();
        assert_eq!(inverses["files"].as_array().unwrap().len(), sources.len());
        for (index, (current, original, fixed_hash)) in sources.into_iter().enumerate() {
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
                ),
                fixed_hash
            );
            let mut restored = current.to_owned();
            for splice in inverses["files"][index]["splices"]
                .as_array()
                .unwrap()
                .iter()
                .rev()
            {
                let start = splice["start"].as_u64().unwrap() as usize;
                let after = splice["after"].as_str().unwrap();
                assert_eq!(restored.get(start..start + after.len()), Some(after));
                restored.replace_range(
                    start..start + after.len(),
                    splice["before"].as_str().unwrap(),
                );
            }
            assert_eq!(restored, original);
        }
    }
}
