//! Read-only, paginated facets derived from retained validated module HIR.
//! Opaque references bind selection; they confer no authority or secrecy.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::ProjectSemanticImage;
use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedFunction};
use crate::workspace_graph::WorkspaceGraphProjectionModule;

pub const IMAGE_FUNCTION_SUMMARY_SCHEMA: &str = "semaprax.image-function-summary.v1";
pub const IMAGE_FACET_SCHEMA: &str = "semaprax.image-facet.v1";
mod instances;
mod relationships;
pub use instances::{IMAGE_FUNCTION_INSTANCES_SCHEMA, IMAGE_INSTANCE_FACET_SCHEMA};

const MAX_ITEMS: usize = 65_536;
const MAX_INTERMEDIATE_BYTES: usize = 16 * 1024 * 1024;
const MAX_REPORT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageFacet {
    Signature,
    Contracts,
    Callers,
    Ownership,
    Loans,
    Cleanup,
    Relationships,
    DataAccess,
    UnsafeBoundaries,
}
impl ImageFacet {
    pub const ALL: [Self; 9] = [
        Self::Signature,
        Self::Contracts,
        Self::Callers,
        Self::Ownership,
        Self::Loans,
        Self::Cleanup,
        Self::Relationships,
        Self::DataAccess,
        Self::UnsafeBoundaries,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Signature => "signature",
            Self::Contracts => "contracts",
            Self::Callers => "callers",
            Self::Ownership => "ownership",
            Self::Loans => "loans",
            Self::Cleanup => "cleanup",
            Self::Relationships => "relationships",
            Self::DataAccess => "data-access",
            Self::UnsafeBoundaries => "unsafe-boundaries",
        }
    }
    pub fn parse(name: &str) -> Result<Self, Vec<Diagnostic>> {
        Self::ALL
            .into_iter()
            .find(|facet| facet.name() == name)
            .ok_or_else(|| error("SPX-G227", "unknown semantic image facet"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageFacetOptions {
    page_size: usize,
    max_bytes: usize,
}
impl ImageFacetOptions {
    pub fn new(page_size: usize, max_bytes: usize) -> Result<Self, Vec<Diagnostic>> {
        if !(1..=128).contains(&page_size) || !(1024..=MAX_REPORT_BYTES).contains(&max_bytes) {
            return Err(error(
                "SPX-G228",
                "semantic image facet options exceed their bounds",
            ));
        }
        Ok(Self {
            page_size,
            max_bytes,
        })
    }

    pub(crate) fn page_size(self) -> usize {
        self.page_size
    }

    pub(crate) fn max_bytes(self) -> usize {
        self.max_bytes
    }
}
impl Default for ImageFacetOptions {
    fn default() -> Self {
        Self {
            page_size: 32,
            max_bytes: 65_536,
        }
    }
}

impl ProjectSemanticImage {
    /// Compact signature and available facet handles, without changing Image v1.
    pub fn function_summary(
        &self,
        expected_image_digest: &str,
        id: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected_image_digest)?;
        let (module, function) = self.facet_function(id)?;
        let handles = ImageFacet::ALL
            .into_iter()
            .map(|facet| json!({"facet": facet.name(), "handle": self.facet_handle(id, facet)}))
            .collect::<Vec<_>>();
        report(
            json!({
                "schema": IMAGE_FUNCTION_SUMMARY_SCHEMA,
                "image_revision": self.image_digest(), "project_revision": self.revision().project_revision(),
                "id": id, "name": function.name, "path": module.path(), "module": module.module(),
                "source_revision": module.source_revision(), "span": span(function.span),
                "parameter_count": function.params.len(), "return_type_id": function.return_type.identity_key(),
                "effects": function.effects, "requires_count": function.requires.len(), "ensures_count": function.ensures.len(),
                "facets": handles, "evidence_class": "descriptive_projection_of_validated_hir",
                "source_authority": false, "target_execution": false,
            }),
            65_536,
        )
    }

    /// Expand only an exact summary handle. Cursors bind image, target, facet,
    /// page size and offset; they are not mutable server state or permissions.
    #[allow(clippy::too_many_arguments)]
    pub fn expand_facet(
        &self,
        expected_image_digest: &str,
        id: &str,
        facet: ImageFacet,
        handle: &str,
        cursor: Option<&str>,
        options: ImageFacetOptions,
    ) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected_image_digest)?;
        let (module, function) = self.facet_function(id)?;
        if handle.len() > 71 || cursor.is_some_and(|value| value.len() > 100) {
            return Err(error(
                "SPX-G228",
                "semantic image facet reference exceeds its byte bound",
            ));
        }
        if handle != self.facet_handle(id, facet) {
            return Err(error(
                "SPX-G229",
                "semantic image facet handle is stale or unknown",
            ));
        }
        let offset = match cursor {
            None => 0,
            Some(value) => {
                let offset = value
                    .split_once(':')
                    .and_then(|(text, _)| text.parse::<usize>().ok())
                    .filter(|offset| *offset > 0 && *offset <= MAX_ITEMS)
                    .ok_or_else(|| error("SPX-G229", "semantic image facet cursor is invalid"))?;
                if offset % options.page_size != 0
                    || value != cursor_for(handle, offset, options.page_size)
                {
                    return Err(error(
                        "SPX-G229",
                        "semantic image facet cursor is stale or mismatched",
                    ));
                }
                offset
            }
        };
        let items = self.facet_items(module, function, facet)?;
        if items.len() > MAX_ITEMS {
            return Err(error(
                "SPX-G228",
                "semantic image facet inventory exceeds its bound",
            ));
        }
        if offset > items.len() || (cursor.is_some() && offset == items.len()) {
            return Err(error(
                "SPX-G229",
                "semantic image facet cursor is outside the inventory",
            ));
        }
        let total = items.len();
        let end = offset.saturating_add(options.page_size).min(total);
        let page = items
            .into_iter()
            .skip(offset)
            .take(options.page_size)
            .collect::<Vec<_>>();
        let next = (end < total).then(|| cursor_for(handle, end, options.page_size));
        report(
            json!({
                "schema": IMAGE_FACET_SCHEMA, "image_revision": self.image_digest(),
                "project_revision": self.revision().project_revision(), "target": id, "facet": facet.name(), "handle": handle,
                "path": module.path(), "source_revision": module.source_revision(),
                "offset": offset, "total_items": total, "items": page, "next_cursor": next,
                "evidence_class": "descriptive_projection_of_validated_hir",
                "nonclaims": ["no_source_or_commit_authority", "no_runtime_liveness_or_target_execution", "no_test_coverage_inference", "no_external_or_dynamic_callers"],
            }),
            options.max_bytes,
        )
    }

    fn facet_function(
        &self,
        id: &str,
    ) -> Result<(&WorkspaceGraphProjectionModule, &ResolvedFunction), Vec<Diagnostic>> {
        if id.len() > 4096 {
            return Err(error(
                "SPX-G228",
                "semantic image facet target exceeds its byte bound",
            ));
        }
        if id.is_empty() || id.contains('\0') {
            return Err(error("SPX-G227", "invalid semantic image facet target"));
        }
        if self.revision().semantic.image_symbol(id).is_none() {
            return Err(error(
                "SPX-G227",
                "semantic image facet function is unavailable",
            ));
        }
        self.revision()
            .semantic
            .image_modules()
            .iter()
            .find_map(|module| {
                module
                    .functions()
                    .iter()
                    .find(|function| function.id.as_str() == id)
                    .map(|function| (module, function))
            })
            .ok_or_else(|| {
                error(
                    "SPX-G227",
                    "semantic image facets require a declared resolved function",
                )
            })
    }
    fn facet_handle(&self, id: &str, facet: ImageFacet) -> String {
        bound_hash(
            b"semaprax.image-facet-handle.v1\0",
            &[self.image_digest(), id, facet.name()],
        )
    }

    pub(super) fn facet_items(
        &self,
        module: &WorkspaceGraphProjectionModule,
        function: &ResolvedFunction,
        facet: ImageFacet,
    ) -> Result<Vec<Value>, Vec<Diagnostic>> {
        Ok(match facet {
            ImageFacet::DataAccess | ImageFacet::UnsafeBoundaries => {
                relationships::items(self, module, function, facet)?
            }
            ImageFacet::Signature => {
                let mut items = function.params.iter().enumerate().map(|(index, param)| json!({"kind":"parameter", "index": index, "id":param.id.as_str(), "name":param.name, "type_id":param.ty.identity_key(), "ownership":ownership(param.ownership), "span":span(param.span)})).collect::<Vec<_>>();
                items.push(json!({"kind":"result", "id":function.result_id.as_str(), "type_id":function.return_type.identity_key()}));
                items.extend(
                    function
                        .effects
                        .iter()
                        .map(|effect| json!({"kind":"effect_requirement", "capability":effect})),
                );
                items.extend(
                    module
                        .permits()
                        .iter()
                        .map(|permit| json!({"kind":"module_permit", "capability":permit})),
                );
                items
            }
            ImageFacet::Contracts => {
                let mut items = Vec::new();
                for (phase, expressions) in [
                    ("requires", &function.requires),
                    ("ensures", &function.ensures),
                ] {
                    for (index, expression) in expressions.iter().enumerate() {
                        let (result, overflow) =
                            crate::bounded_output::with_limit(MAX_INTERMEDIATE_BYTES, || {
                                crate::graph::agent_contract_expr_json(expression)
                            });
                        if overflow {
                            return Err(error(
                                "SPX-G228",
                                "semantic image contract exceeds its rendering bound",
                            ));
                        }
                        let expression_graph =
                            trusted_json(&result.map_err(|diagnostic| vec![diagnostic])?)?;
                        items.push(json!({"phase":phase, "index":index, "expression_id":expression.id.as_str(), "type_id":expression.ty.identity_key(), "span":span(expression.span), "expression":expression_graph}));
                    }
                }
                items
            }
            ImageFacet::Callers => {
                let mut items = Vec::new();
                for caller_module in self.revision().semantic.image_modules() {
                    for caller in caller_module.functions() {
                        append_caller(
                            &mut items,
                            caller_module,
                            caller.id.as_str(),
                            &caller.requires,
                            &caller.body,
                            &caller.ensures,
                            function.id.as_str(),
                            module.module(),
                        );
                    }
                    for caller in caller_module.function_templates() {
                        append_caller(
                            &mut items,
                            caller_module,
                            caller.id.as_str(),
                            &caller.requires,
                            &caller.body,
                            &caller.ensures,
                            function.id.as_str(),
                            module.module(),
                        );
                    }
                }
                items.sort_by(|left, right| {
                    left["caller"]
                        .as_str()
                        .cmp(&right["caller"].as_str())
                        .then_with(|| left["phase"].as_str().cmp(&right["phase"].as_str()))
                });
                items
            }
            ImageFacet::Ownership => {
                let mut items = vec![
                    json!({"kind":"inventory", "schema":function.cleanup.schema, "structural_slot_count":function.cleanup.slots.len(), "flag_count":function.cleanup.flags.len(), "live_owned_parameter_slots":function.cleanup.entry_state.live_owned_parameters.iter().map(|id| id.0).collect::<Vec<_>>(), "conditional_owned_parameter_count":function.cleanup.entry_state.conditional_owned_parameters.len(), "order_meaning":"structural_discovery_not_runtime_destruction"}),
                ];
                items.extend(function.params.iter().map(|param| json!({"kind":"parameter_ownership", "id":param.id.as_str(), "mode":ownership(param.ownership), "type_id":param.ty.identity_key()})));
                items.extend(function.cleanup.slots.iter().map(|slot| json!({"kind":"structural_slot", "id":slot.id.0, "discovery_index":slot.discovery_index, "type_id":slot.ty.identity_key()})));
                items
            }
            ImageFacet::Loans => plan_items(
                || crate::graph_loan::loan_plan_json(&function.loan_plan),
                &["loans", "endpoints", "edges"],
            )?,
            ImageFacet::Cleanup => plan_items(
                || crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan),
                &[
                    "slots",
                    "status_sources",
                    "blocks",
                    "edges",
                    "regions",
                    "exits",
                ],
            )?,
            ImageFacet::Relationships => {
                let revision = self.revision();
                let id = function.id.as_str();
                vec![
                    json!({"kind":"project_profile_admission", "project_schema":revision.manifest().schema(), "profile":revision.manifest().profile(), "admitted":true, "basis":"retained_ProjectRevision_admission", "native_target_check":"not_performed", "wasm_target_check":"not_performed"}),
                    json!({"kind":"entry_relationship", "entry_module":revision.manifest().entry(), "declared_in_entry_module":module.module() == revision.manifest().entry(), "in_entry_closure":revision.entry_program().functions.iter().any(|item| item.id.as_str() == id)}),
                    json!({"kind":"test_relationship", "test_module":revision.manifest().test_module(), "declared_in_test_module":module.module() == revision.manifest().test_module(), "in_test_closure":revision.test_program().functions.iter().any(|item| item.id.as_str() == id), "coverage":"not_inferred", "executed":false}),
                    json!({"kind":"export_relationship", "selected_web_export":revision.manifest().web_exports().iter().any(|export| export == id), "artifact_emitted":false}),
                ]
            }
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn append_caller(
    items: &mut Vec<Value>,
    module: &WorkspaceGraphProjectionModule,
    caller: &str,
    requires: &[ResolvedExpr],
    body: &ResolvedExpr,
    ensures: &[ResolvedExpr],
    target: &str,
    target_module: &str,
) {
    for (phase, expressions) in [
        ("requires", requires),
        ("body", std::slice::from_ref(body)),
        ("ensures", ensures),
    ] {
        let mut calls = 0usize;
        for expression in expressions {
            crate::hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                if callee.as_str() == target {
                    calls += 1;
                }
            });
        }
        if calls > 0 {
            items.push(json!({"caller":caller, "path":module.path(), "module":module.module(), "phase":phase, "call_sites":calls, "cross_file":module.module() != target_module}));
        }
    }
}

fn plan_items(
    render: impl FnOnce() -> String,
    arrays: &[&str],
) -> Result<Vec<Value>, Vec<Diagnostic>> {
    let (text, overflow) = crate::bounded_output::with_limit(MAX_INTERMEDIATE_BYTES, render);
    if overflow {
        return Err(error(
            "SPX-G228",
            "semantic image proof plan exceeds its rendering bound",
        ));
    }
    let mut plan = trusted_json(&text)?;
    let mut items = Vec::new();
    for field in arrays {
        if let Some(values) = plan.get_mut(*field).and_then(Value::as_array_mut) {
            for (index, value) in std::mem::take(values).into_iter().enumerate() {
                if items.len() >= MAX_ITEMS - 1 {
                    return Err(error(
                        "SPX-G228",
                        "semantic image proof inventory exceeds its item bound",
                    ));
                }
                items.push(json!({"section":field, "index":index, "value":value}));
            }
        }
        plan.as_object_mut()
            .expect("compiler-owned plan is an object")
            .remove(*field);
    }
    items.insert(0, json!({"section":"header", "value":plan}));
    Ok(items)
}
fn trusted_json(text: &str) -> Result<Value, Vec<Diagnostic>> {
    serde_json::from_str(text)
        .map_err(|_| error("SPX-G227", "compiler-owned facet projection is invalid"))
}
fn span(span: crate::ast::Span) -> Value {
    json!({"start":span.start, "end":span.end, "line":span.line, "column":span.column})
}
fn ownership(mode: OwnershipMode) -> &'static str {
    match mode {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}
fn cursor_for(handle: &str, offset: usize, page_size: usize) -> String {
    format!(
        "{}:{}",
        offset,
        bound_hash(
            b"semaprax.image-facet-cursor.v1\0",
            &[handle, &offset.to_string(), &page_size.to_string()]
        )
    )
}
fn bound_hash(domain: &[u8], fields: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    for field in fields {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}
fn report(value: Value, max_bytes: usize) -> Result<String, Vec<Diagnostic>> {
    super::image::render(value, false, max_bytes).map_err(|_| {
        error(
            "SPX-G228",
            "semantic image facet response exceeds its byte bound",
        )
    })
}
fn error(code: &'static str, message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(code, message)]
}
