//! Retained concrete instances, kept distinct from authored template identities.
use super::*;
use crate::hir::{self, ResolvedFunctionInstance, ResolvedFunctionTemplate};

pub const IMAGE_FUNCTION_INSTANCES_SCHEMA: &str = "semaprax.image-function-instances.v1";
pub const IMAGE_INSTANCE_FACET_SCHEMA: &str = "semaprax.image-instance-facet.v1";
const MAX_INSTANCE_BYTES: usize = 65_536;
const MAX_DEPTH: usize = 256;
const EVIDENCE: &str = "descriptive_projection_of_retained_generic_instance_hir";
const NONCLAIMS: [&str; 5] = [
    "no_source_or_commit_authority",
    "no_target_execution_or_test_coverage",
    "retained_instances_not_all_possible_instantiations",
    "template_spans_are_source_provenance_not_executed_sites",
    "no_external_or_dynamic_callers",
];
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

impl ProjectSemanticImage {
    pub fn function_instances(
        &self,
        expected_image: &str,
        template_id: &str,
        cursor: Option<&str>,
        options: ImageFacetOptions,
    ) -> Result<String> {
        self.require_digest(expected_image)?;
        let (module, template) = template(self, template_id)?;
        let handle = bound_hash(
            b"semaprax.image-function-instances-handle.v1\0",
            &[self.image_digest(), template_id],
        );
        let offset = offset(&handle, cursor, options)?;
        let instances = inventory(module, template)?;
        check_offset(offset, instances.len(), cursor)?;
        let end = offset
            .saturating_add(options.page_size)
            .min(instances.len());
        let mut rows = Vec::new();
        let mut bytes = 0;
        for instance in &instances[offset..end] {
            let facets = ImageFacet::ALL.into_iter().map(|facet| json!({"facet":facet.name(),"handle":instance_handle(self,template_id,instance.id.as_str(),facet)})).collect::<Vec<_>>();
            push(
                &mut rows,
                json!({"instance_id":instance.id.as_str(),"type_arguments":type_keys(&instance.type_arguments)?,
                "parameter_count":instance.function.params.len(),"return_type_id":type_keys(std::slice::from_ref(&instance.function.return_type))?[0],
                "effects":instance.function.effects,"requires_count":instance.function.requires.len(),"ensures_count":instance.function.ensures.len(),"facets":facets}),
                &mut bytes,
            )?;
        }
        report(
            json!({"schema":IMAGE_FUNCTION_INSTANCES_SCHEMA,"image_revision":self.image_digest(),"project_revision":self.revision().project_revision(),
            "template_id":template_id,"name":template.name,"path":module.path(),"module":module.module(),"source_revision":module.source_revision(),"source_digest":module.source_digest(),
            "template_span":span(template.span),"type_parameter_count":template.type_parameters.len(),"handle":handle,"offset":offset,"total_instances":instances.len(),
            "instances":rows,"next_cursor":(end<instances.len()).then(|| cursor_for_instance(&handle,end,options.page_size)),
            "evidence_class":EVIDENCE,"source_authority":false,"target_execution":false,"nonclaims":NONCLAIMS}),
            options.max_bytes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn expand_instance_facet(
        &self,
        expected_image: &str,
        template_id: &str,
        instance_id: &str,
        facet: ImageFacet,
        handle: &str,
        cursor: Option<&str>,
        options: ImageFacetOptions,
    ) -> Result<String> {
        self.require_digest(expected_image)?;
        selector(instance_id, MAX_INSTANCE_BYTES)?;
        let (module, template) = template(self, template_id)?;
        let instances = inventory(module, template)?;
        let instance = instances
            .into_iter()
            .find(|i| i.id.as_str() == instance_id)
            .ok_or_else(|| {
                error(
                    "SPX-G227",
                    "retained function instance is unavailable for this template",
                )
            })?;
        if handle.len() > 71 {
            return Err(error("SPX-G228", "instance facet handle exceeds its bound"));
        }
        if handle != instance_handle(self, template_id, instance_id, facet) {
            return Err(error(
                "SPX-G229",
                "instance facet handle is stale or mismatched",
            ));
        }
        let offset = offset(handle, cursor, options)?;
        let items = match facet {
            ImageFacet::Callers => callers(self, module, instance)?,
            ImageFacet::Relationships => relationships(self, instance),
            _ => self.facet_items(module, &instance.function, facet)?,
        };
        if items.len() > MAX_ITEMS {
            return Err(error(
                "SPX-G228",
                "instance facet inventory exceeds its bound",
            ));
        }
        check_offset(offset, items.len(), cursor)?;
        let total = items.len();
        let end = offset.saturating_add(options.page_size).min(total);
        let page = items
            .into_iter()
            .skip(offset)
            .take(options.page_size)
            .collect::<Vec<_>>();
        report(
            json!({"schema":IMAGE_INSTANCE_FACET_SCHEMA,"image_revision":self.image_digest(),"project_revision":self.revision().project_revision(),
            "template_id":template_id,"instance_id":instance_id,"type_arguments":type_keys(&instance.type_arguments)?,"facet":facet.name(),"handle":handle,
            "path":module.path(),"module":module.module(),"source_revision":module.source_revision(),"source_digest":module.source_digest(),"template_span":span(template.span),
            "offset":offset,"total_items":total,"items":page,"next_cursor":(end<total).then(||cursor_for_instance(handle,end,options.page_size)),
            "evidence_class":EVIDENCE,"source_authority":false,"target_execution":false,"nonclaims":NONCLAIMS}),
            options.max_bytes,
        )
    }
}

fn selector(value: &str, max: usize) -> Result<()> {
    if value.len() > max {
        return Err(error(
            "SPX-G228",
            "generic instance selector exceeds its byte bound",
        ));
    }
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(error("SPX-G227", "generic instance selector is invalid"));
    }
    Ok(())
}
fn template<'a>(
    image: &'a ProjectSemanticImage,
    id: &str,
) -> Result<(
    &'a WorkspaceGraphProjectionModule,
    &'a ResolvedFunctionTemplate,
)> {
    selector(id, 4096)?;
    if image.revision().semantic.image_symbol(id).is_none() {
        return Err(error("SPX-G227", "generic template is unavailable"));
    }
    let mut found = None;
    for module in image.revision().semantic.image_modules() {
        for template in module
            .function_templates()
            .iter()
            .filter(|t| t.id.as_str() == id)
        {
            if found.replace((module, template)).is_some() {
                return Err(error(
                    "SPX-G227",
                    "generic template source owner is ambiguous",
                ));
            }
        }
    }
    found.ok_or_else(|| {
        error(
            "SPX-G227",
            "selection requires an authored retained generic template",
        )
    })
}
fn inventory<'a>(
    module: &'a WorkspaceGraphProjectionModule,
    template: &ResolvedFunctionTemplate,
) -> Result<Vec<&'a ResolvedFunctionInstance>> {
    if module.function_instances().len() > MAX_ITEMS {
        return Err(error(
            "SPX-G228",
            "retained instance inventory exceeds its bound",
        ));
    }
    let mut selected = Vec::new();
    for instance in module
        .function_instances()
        .iter()
        .filter(|i| i.template == template.id)
    {
        selector(instance.id.as_str(), MAX_INSTANCE_BYTES)?;
        if instance.function.id != template.id
            || instance.type_arguments.len() != template.type_parameters.len()
        {
            return Err(error(
                "SPX-G227",
                "retained instance disagrees with its template owner",
            ));
        }
        let (id, overflow) = crate::bounded_output::with_limit(MAX_INTERMEDIATE_BYTES, || {
            hir::FunctionInstanceId::derive(&instance.template, &instance.type_arguments)
        });
        if overflow {
            return Err(error(
                "SPX-G228",
                "instance identity derivation exceeds its bound",
            ));
        }
        if id != instance.id {
            return Err(error(
                "SPX-G227",
                "retained instance identity disagrees with its exact type arguments",
            ));
        }
        selected.push(instance);
    }
    selected.sort_by(|a, b| a.id.cmp(&b.id));
    if selected.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err(error(
            "SPX-G227",
            "retained instance identity is duplicated",
        ));
    }
    Ok(selected)
}
fn type_keys(types: &[hir::ResolvedType]) -> Result<Vec<String>> {
    if types.len() > MAX_ITEMS {
        return Err(error(
            "SPX-G228",
            "instance type inventory exceeds its bound",
        ));
    }
    let mut keys = Vec::new();
    let mut bytes = 0usize;
    for ty in types {
        let (key, overflow) =
            crate::bounded_output::with_limit(MAX_INTERMEDIATE_BYTES - bytes, || ty.identity_key());
        if overflow || key.len() > MAX_INTERMEDIATE_BYTES - bytes {
            return Err(error(
                "SPX-G228",
                "instance type identities exceed their bound",
            ));
        }
        bytes += key.len();
        keys.push(key);
    }
    Ok(keys)
}
fn instance_handle(
    image: &ProjectSemanticImage,
    template: &str,
    instance: &str,
    facet: ImageFacet,
) -> String {
    bound_hash(
        b"semaprax.image-instance-facet-handle.v1\0",
        &[image.image_digest(), template, instance, facet.name()],
    )
}
fn cursor_for_instance(handle: &str, offset: usize, page_size: usize) -> String {
    format!(
        "{}:{}",
        offset,
        bound_hash(
            b"semaprax.image-instance-cursor.v1\0",
            &[handle, &offset.to_string(), &page_size.to_string()]
        )
    )
}
fn offset(handle: &str, cursor: Option<&str>, options: ImageFacetOptions) -> Result<usize> {
    let Some(cursor) = cursor else { return Ok(0) };
    if cursor.len() > 100 {
        return Err(error("SPX-G228", "instance cursor exceeds its byte bound"));
    }
    let offset = cursor
        .split_once(':')
        .and_then(|(n, _)| n.parse::<usize>().ok())
        .filter(|n| *n > 0 && *n <= MAX_ITEMS)
        .ok_or_else(|| error("SPX-G229", "instance cursor is invalid"))?;
    if offset % options.page_size != 0
        || cursor != cursor_for_instance(handle, offset, options.page_size)
    {
        return Err(error("SPX-G229", "instance cursor is stale or mismatched"));
    }
    Ok(offset)
}
fn check_offset(offset: usize, total: usize, cursor: Option<&str>) -> Result<()> {
    if offset > total || (cursor.is_some() && offset == total) {
        return Err(error(
            "SPX-G229",
            "instance cursor is outside its inventory",
        ));
    }
    Ok(())
}
fn push(items: &mut Vec<Value>, row: Value, bytes: &mut usize) -> Result<()> {
    let size = serde_json::to_vec(&row)
        .map_err(|_| error("SPX-G227", "instance projection is not JSON"))?
        .len();
    *bytes = bytes
        .checked_add(size)
        .ok_or_else(|| error("SPX-G228", "instance projection accounting overflow"))?;
    if items.len() >= MAX_ITEMS || *bytes > MAX_INTERMEDIATE_BYTES {
        return Err(error(
            "SPX-G228",
            "instance projection exceeds its inventory bound",
        ));
    }
    items.push(row);
    Ok(())
}

fn relationships(image: &ProjectSemanticImage, instance: &ResolvedFunctionInstance) -> Vec<Value> {
    let revision = image.revision();
    vec![
        json!({"kind":"project_profile_admission","project_schema":revision.manifest().schema(),"profile":revision.manifest().profile(),"admitted":true,"basis":"retained_ProjectRevision_admission","native_target_check":"not_performed","wasm_target_check":"not_performed"}),
        json!({"kind":"entry_relationship","entry_module":revision.manifest().entry(),"in_entry_instance_inventory":revision.entry_program().function_instances.iter().any(|i|i.id==instance.id),"executed":false}),
        json!({"kind":"test_relationship","test_module":revision.manifest().test_module(),"in_test_instance_inventory":revision.test_program().function_instances.iter().any(|i|i.id==instance.id),"coverage":"not_inferred","executed":false}),
        json!({"kind":"export_relationship","template_selected_web_export":revision.manifest().web_exports().iter().any(|id|id==instance.template.as_str()),"instance_export":"not_inferred","artifact_emitted":false}),
    ]
}

fn callers(
    image: &ProjectSemanticImage,
    target_module: &WorkspaceGraphProjectionModule,
    target: &ResolvedFunctionInstance,
) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    let mut bytes = 0;
    let mut visits = 0;
    for module in image.revision().semantic.image_modules() {
        for function in module.functions() {
            append_callers(
                &mut rows,
                &mut bytes,
                &mut visits,
                module,
                function,
                None,
                target_module,
                target,
            )?;
        }
        for instance in module.function_instances() {
            append_callers(
                &mut rows,
                &mut bytes,
                &mut visits,
                module,
                &instance.function,
                Some(instance),
                target_module,
                target,
            )?;
        }
    }
    rows.sort_by(|a, b| {
        a["caller_kind"]
            .as_str()
            .cmp(&b["caller_kind"].as_str())
            .then_with(|| a["caller_id"].as_str().cmp(&b["caller_id"].as_str()))
            .then_with(|| a["phase"].as_str().cmp(&b["phase"].as_str()))
    });
    Ok(rows)
}
#[allow(clippy::too_many_arguments)]
fn append_callers(
    rows: &mut Vec<Value>,
    bytes: &mut usize,
    visits: &mut usize,
    module: &WorkspaceGraphProjectionModule,
    function: &ResolvedFunction,
    caller_instance: Option<&ResolvedFunctionInstance>,
    target_module: &WorkspaceGraphProjectionModule,
    target: &ResolvedFunctionInstance,
) -> Result<()> {
    for (phase, roots) in [
        ("requires", function.requires.as_slice()),
        ("body", std::slice::from_ref(&function.body)),
        ("ensures", function.ensures.as_slice()),
    ] {
        let mut count = 0;
        for root in roots {
            let mut pending = vec![(root, 0usize)];
            while let Some((expression, depth)) = pending.pop() {
                *visits += 1;
                if *visits > MAX_ITEMS || depth > MAX_DEPTH {
                    return Err(error(
                        "SPX-G228",
                        "instance callers traversal exceeds its bound",
                    ));
                }
                if let hir::ResolvedExprKind::Call {
                    callee,
                    instance: Some(instance),
                    ..
                } = &expression.kind
                {
                    if instance == &target.id {
                        if callee != &target.template {
                            return Err(error(
                                "SPX-G227",
                                "instance caller disagrees with its template identity",
                            ));
                        }
                        count += 1;
                    }
                }
                let mut children = Vec::new();
                hir::push_resolved_expression_children_in_authored_order(expression, &mut children);
                if children
                    .len()
                    .saturating_add(pending.len())
                    .saturating_add(*visits)
                    > MAX_ITEMS
                {
                    return Err(error(
                        "SPX-G228",
                        "instance callers traversal exceeds its bound",
                    ));
                }
                pending.extend(children.into_iter().map(|child| (child, depth + 1)));
            }
        }
        if count > 0 {
            push(
                rows,
                json!({"caller_kind":if caller_instance.is_some(){"instance"}else{"function"},"caller_id":caller_instance.map_or(function.id.as_str(),|i|i.id.as_str()),
                "caller_template_id":caller_instance.map(|i|i.template.as_str()),"path":module.path(),"module":module.module(),"source_revision":module.source_revision(),"source_digest":module.source_digest(),
                "phase":phase,"call_sites":count,"cross_file":module.module()!=target_module.module(),"basis":"retained_concrete_call_instance_identity"}),
                bytes,
            )?;
        }
    }
    Ok(())
}
