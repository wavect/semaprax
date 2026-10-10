//! Borrowed declaration authority for retained workspace Graph selection.
//! Frozen from-parts selectors do not acquire this authority.
use super::*;
use std::fmt::{self, Write};

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
    let Some(origin) = facts.get(declaration.as_str()) else {
        return None;
    };
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
    let Some((owner, record)) = candidates.next() else {
        return None;
    };
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
    let Some((kind, checked)) = module.signature_types.iter().find_map(|(key, value)| {
        matches_text(
            key,
            format_args!("nominal:{}:{}:0:", declaration.as_str().len(), declaration),
        )
        .then_some(value)
    }) else {
        return None;
    };
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
        let Some(field_origin) = facts.get(field.id.as_str()) else {
            return None;
        };
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

pub(super) fn all(
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<Vec<&'static str>, Vec<Diagnostic>> {
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
    for module in modules {
        schemas.push(schema(module, modules, facts)?);
    }
    Ok(schemas)
}

pub(super) fn schema(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<&'static str, Vec<Diagnostic>> {
    if module.native_law {
        return Ok("semaprax.native-law.v1");
    }
    let authority = |function: &hir::ResolvedFunction| {
        hir::vec_loop_renewal::requires_with_admission(function, |operation, element| {
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
        })
    };
    for function in module.functions.iter().chain(
        module
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        let byte = crate::byte_ops::requires_same_owner_set(function);
        let string = !byte && crate::string_ops::replacement::requires(function);
        let ordinary = !byte && !string && authority(function);
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
            &module.types,
            &module.functions,
            &module.function_templates,
            &module.function_instances,
        )
        .map_err(|error| vec![error]);
    }
    graph::graph_schema_from_parts_and_instances_with_renewal_authority(
        &module.interfaces,
        &module.types,
        &module.functions,
        &module.function_templates,
        &module.function_instances,
        Some(&authority),
    )
    .map_err(|error| vec![error])
}

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

    fn build_source(source: &str) -> WorkspaceGraphBuild {
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
                    fields[0].ty = hir::ResolvedType::Bool;
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
                "semaprax.graph.v75"
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
}
