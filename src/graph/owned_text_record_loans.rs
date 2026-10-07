//! Graph v69 composes bounded owned String records with authenticated loans.
use super::*;

fn string_record_slot(function: &ResolvedFunction, types: &[hir::ResolvedTypeDeclaration]) -> bool {
    if function.loan_plan.loans.is_empty() {
        return false;
    }
    function.cleanup_plan.slots.iter().any(|slot| {
        let hir::ResolvedType::Nominal {
            declaration,
            arguments,
        } = &slot.ty
        else {
            return false;
        };
        if !arguments.is_empty()
            || types.iter().all(|ty| {
                ty.id != *declaration
                    || !ty.type_parameters.is_empty()
                    || !matches!(ty.kind, hir::ResolvedTypeDeclarationKind::Record { .. })
            })
        {
            return false;
        }
        let crate::cleanup::FieldLivenessShape::Record {
            declaration: root, ..
        } = &slot.field_liveness_shape
        else {
            return false;
        };
        if root != declaration {
            return false;
        }
        let mut pending = vec![&slot.field_liveness_shape];
        while let Some(shape) = pending.pop() {
            match shape {
                crate::cleanup::FieldLivenessShape::Leaf { lifecycle, .. }
                    if matches!(
                        lifecycle.as_str(),
                        crate::cleanup::STRING_DROP_LIFECYCLE_ID
                            | crate::map_ops::DROP_ID
                            | crate::string_ops::MAP_DROP_LIFECYCLE_ID
                    ) =>
                {
                    return true
                }
                crate::cleanup::FieldLivenessShape::Record { fields, .. } => {
                    pending.extend(fields.iter().map(|field| &field.shape))
                }
                _ => {}
            }
        }
        false
    })
}

pub(super) fn requires(program: &ResolvedProgram) -> bool {
    !super::native_import::declares_native_rust_import(&program.interfaces)
        && program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function))
            .any(|function| {
                string_record_slot(function, &program.types)
                    && function.cleanup_plan.slots.iter().any(|slot| {
                        hir::owned_text_record::admitted(&slot.ty, &program.declarations)
                    })
            })
}

pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if requires(program) {
        hir::validate(program)?;
        crate::loan_plan::validate_program(program)?;
        Ok("semaprax.graph.v69")
    } else {
        super::string_replacement::graph_schema(program)
    }
}

pub(crate) fn graph_schema_from_parts_and_instances(
    interfaces: &[hir::ResolvedInterface],
    types: &[hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    templates: &[hir::ResolvedFunctionTemplate],
    instances: &[hir::ResolvedFunctionInstance],
) -> Result<&'static str, Diagnostic> {
    // This metadata selector has no independent execution authority. The
    // ordinary workspace boundary validates every retained complete program.
    if !super::native_import::declares_native_rust_import(interfaces)
        && functions
            .iter()
            .chain(instances.iter().map(|i| &i.function))
            .any(|function| string_record_slot(function, types))
    {
        Ok("semaprax.graph.v69")
    } else {
        super::string_replacement::graph_schema_from_parts_and_instances(
            interfaces, types, functions, templates, instances,
        )
    }
}

pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph =
        super::string_replacement::graph_json(program, revision, functions, types, view)?;
    if !requires(program) {
        return Ok(graph);
    }
    hir::validate(program)?;
    crate::loan_plan::validate_program(program)?;
    let document: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let previous = document["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(previous));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io("SPX-G411", "noncanonical checked graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v69\"");
    graph.pop();
    graph.push_str(",\"owned_text_record_loans\":{\"schema\":\"semaprax.owned-text-record-loans.v1\",\"authority\":false}}");
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = r#"
module test.owned_text_graph;
@id("text.record") record Text {@id("text.title") title:string,}
@id("text.read") fn read(text:borrow Text)->i64 {string_len(text.title)}
@id("text.main") fn main()->i64 {let text=Text{title:"tag"};read(text)+read(text)}
"#;
    #[test]
    fn closed_collection_record_loans_select_v69_without_a_string_sibling() {
        for field in ["Map<i64,string>", "Set<i64>", "Map<string,i64>"] {
            let source = format!(
                r#"module test.collection_graph;
@id("collection.record") record Carrier {{@id("collection.value") value:{field},}}
@id("collection.read") fn read(carrier:borrow Carrier)->i64 {{7}}
@id("collection.main") fn main()->i64 {{let carrier=Carrier{{value:{constructor}}};read(carrier)+read(carrier)}}"#,
                constructor = match field {
                    "Map<i64,string>" => "map_new<i64,string>(1usize)",
                    "Set<i64>" => "set_new<i64>(1usize)",
                    _ => "map_new(1usize)",
                }
            );
            let program = crate::check(&source, "collection-record-graph.spx").unwrap();
            let graph = crate::graph::to_json(&program).unwrap();
            let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
            assert_eq!(document["schema"], "semaprax.graph.v69", "{field}");
            assert!(crate::graph::to_legacy_json(&program)
                .unwrap_err()
                .iter()
                .any(|d| d.code == "SPX-G410"));
        }
    }
    #[test]
    fn exact_text_record_loans_select_v69_and_frozen_graph_refuses_them() {
        let source = crate::check(SOURCE, "owned-text-graph.spx").unwrap();
        let canonical = crate::format::canonical(&source);
        let reparsed = crate::check(&canonical, "owned-text-graph.spx").unwrap();
        let graph = crate::graph::to_json(&source).unwrap();
        assert_eq!(graph, crate::graph::to_json(&reparsed).unwrap());
        let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
        assert_eq!(document["schema"], "semaprax.graph.v69");
        assert_eq!(document["owned_text_record_loans"]["authority"], false);
        assert!(crate::graph::to_legacy_json(&source)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "SPX-G410"));
        assert!(crate::graph::reject_evidence_schema("semaprax.graph.v69").is_err());
        let program = hir::resolve(&source).unwrap();
        assert_eq!(graph_schema(&program).unwrap(), "semaprax.graph.v69");
        assert_eq!(
            graph_schema_from_parts_and_instances(
                &program.interfaces,
                &program.types,
                &program.functions,
                &program.function_templates,
                &program.function_instances
            )
            .unwrap(),
            "semaprax.graph.v69"
        );
        let mut forged = program.clone();
        let foreign = forged
            .functions
            .iter()
            .find(|f| f.id.as_str() == "text.read")
            .unwrap()
            .params[0]
            .id
            .clone();
        let loan = forged
            .functions
            .iter_mut()
            .flat_map(|f| &mut f.loan_plan.loans)
            .next()
            .unwrap();
        loan.origin.root = foreign;
        assert!(graph_schema(&forged).is_err());
        assert!(crate::graph::to_hir_json(&forged, "forged-revision").is_err());
    }
}
