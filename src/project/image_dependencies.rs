//! Shared bounded declaration-use index over retained compiler-checked HIR.
//! Lazy image state is derived only; no deserialization or source authority.
use super::{ProjectRevision, ProjectSemanticImage};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, PlaceProjection, ResolvedExpr, ResolvedExprKind as Expr, ResolvedMatchPattern,
    ResolvedRecordMatchFieldPattern, ResolvedRecordMatchPatternField, ResolvedStatement,
    ResolvedTypeDeclarationKind,
};
use crate::workspace_graph::WorkspaceGraphProjectionModule;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub const IMAGE_DECLARATION_DEPENDENCIES_SCHEMA: &str =
    "semaprax.image-declaration-dependencies.v1";
pub const MAX_IMAGE_DECLARATION_DEPENDENCIES_BYTES: usize = 8 * 1024 * 1024;
mod navigation;
mod obligations;
pub use navigation::{
    ImageDependencyPageOptions, ImageDependencyView, IMAGE_DEPENDENCY_PAGE_SCHEMA,
    IMAGE_DEPENDENCY_SUMMARY_SCHEMA,
};
pub use obligations::{
    IMAGE_CLEANUP_DEPENDENCIES_SCHEMA, IMAGE_CLEANUP_DEPENDENCIES_VERIFICATION_SCHEMA,
    MAX_IMAGE_CLEANUP_DEPENDENCIES_BYTES,
};
const MAX_ITEMS: usize = 65_536;
const MAX_VISITS: usize = 1_048_576;
const MAX_DEPTH: usize = 256;
const MAX_RETAINED_BYTES: usize = 16 * 1024 * 1024;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub(super) type DependencyCell = OnceLock<Result<DependencyIndex>>;

struct DependencySelection {
    selected: BTreeSet<String>,
    ordinals: BTreeSet<usize>,
    users: BTreeSet<String>,
    closure: BTreeSet<String>,
    calls: Vec<usize>,
}

#[derive(Default)]
pub(super) struct DependencyIndex {
    cleanup: OnceLock<Result<obligations::CleanupDependencyIndex>>,
    typed: BTreeMap<String, Value>,
    members: BTreeMap<String, Vec<String>>,
    rows: Vec<Value>,
    sites_by_id: BTreeMap<String, Vec<usize>>,
    callers: BTreeMap<String, BTreeSet<String>>,
    call_sites: Vec<Value>,
    functions: BTreeSet<String>,
    visits: usize,
    calls: usize,
    pattern_visits: usize,
    type_items: usize,
    retained_bytes: usize,
}
impl DependencyIndex {
    fn charge(&mut self, bytes: usize) -> Result<()> {
        self.retained_bytes = self
            .retained_bytes
            .checked_add(bytes)
            .ok_or_else(|| capacity("dependency index allocation accounting overflow"))?;
        if self.retained_bytes > MAX_RETAINED_BYTES {
            return Err(capacity("dependency index exceeds its retained byte bound"));
        }
        Ok(())
    }
    fn charge_value(&mut self, value: &Value) -> Result<()> {
        self.charge(value_bytes(value)?)?;
        // Vec/map carriers and conservative node overhead are charged in
        // addition to every owned Value/string below. This is not an RSS claim.
        self.charge(128)
    }
    fn typed_entry(&mut self, id: &str, value: Value, members: Vec<String>) -> Result<()> {
        // Checked classes retain inherited prefix fields in each class view.
        // They are the same declaration, not additional authored identities.
        if let Some(previous) = self.typed.get(id) {
            return if previous == &value && self.members.get(id) == Some(&members) {
                Ok(())
            } else {
                Err(invalid(
                    "dependency typed declaration identity is ambiguous",
                ))
            };
        }
        if self.type_items >= MAX_ITEMS {
            return Err(capacity("dependency typed index exceeds its item bound"));
        }
        self.type_items += 1;
        self.charge_value(&value)?;
        self.charge(
            id.len().saturating_mul(2) + 256 + members.iter().map(|s| s.len() + 64).sum::<usize>(),
        )?;
        if self.typed.insert(id.to_owned(), value).is_some() {
            return Err(invalid(
                "dependency typed declaration identity is ambiguous",
            ));
        }
        self.members.insert(id.to_owned(), members);
        Ok(())
    }
    pub(super) fn build(revision: &ProjectRevision) -> Result<Self> {
        let mut index = Self::default();
        for module in revision.semantic.image_modules() {
            for ty in module.types() {
                let mut fields = Vec::new();
                let mut members = Vec::new();
                let mut temporary = 0usize;
                let mut append_field = |field: &hir::ResolvedFieldDeclaration,
                                        _owner: &str,
                                        index: &mut Self|
                 -> Result<()> {
                    let value = field_value(field);
                    temporary = temporary
                        .checked_add(value_bytes(&value)? + 128)
                        .ok_or_else(|| {
                            capacity("dependency type projection accounting overflow")
                        })?;
                    if temporary > MAX_RETAINED_BYTES || fields.len() >= MAX_ITEMS {
                        return Err(capacity("dependency type projection exceeds its bound"));
                    }
                    index.typed_entry(
                        field.id.as_str(),
                        json!({"kind":"field","owner":revision.semantic.image_symbol(field.id.as_str()).map(|symbol|symbol["owner"].clone()).unwrap_or(Value::Null),"field":value}),
                        Vec::new(),
                    )?;
                    fields.push(value);
                    members.push(field.id.as_str().to_owned());
                    Ok(())
                };
                match &ty.kind {
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                        for field in fields {
                            append_field(field, ty.id.as_str(), &mut index)?;
                        }
                    }
                    ResolvedTypeDeclarationKind::Variant { cases } => {
                        for case in cases {
                            for field in &case.fields {
                                append_field(field, case.id.as_str(), &mut index)?;
                            }
                            let values = case.fields.iter().map(field_value).collect::<Vec<_>>();
                            index.typed_entry(case.id.as_str(),json!({"kind":"case","owner":ty.id.as_str(),"id":case.id.as_str(),"name":case.name,"index":case.index,"fields":values}),
                                case.fields.iter().map(|field|field.id.as_str().to_owned()).collect())?;
                        }
                    }
                    ResolvedTypeDeclarationKind::Resource { .. } => {}
                }
                let kind = match &ty.kind {
                    ResolvedTypeDeclarationKind::Record { .. } => "record",
                    ResolvedTypeDeclarationKind::Class { .. } => "class",
                    ResolvedTypeDeclarationKind::Variant { .. } => "variant",
                    ResolvedTypeDeclarationKind::Resource { .. } => "resource",
                };
                if let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind {
                    members.extend(cases.iter().map(|case| case.id.as_str().to_owned()));
                }
                if ty.type_parameters.len() > MAX_ITEMS - index.type_items {
                    return Err(capacity(
                        "dependency type parameter index exceeds its bound",
                    ));
                }
                index.type_items += ty.type_parameters.len();
                index.typed_entry(ty.id.as_str(),json!({"kind":kind,"id":ty.id.as_str(),"name":ty.name,"fields":fields,
                    "type_parameters":ty.type_parameters.iter().map(|parameter|json!({"name":parameter.name,"index":parameter.index})).collect::<Vec<_>>()}),members)?;
            }
            for function in module.functions() {
                index.function(function.id.as_str())?;
                scan_function(
                    &mut index,
                    module,
                    function.id.as_str(),
                    &function.requires,
                    &function.body,
                    &function.ensures,
                )?;
            }
            for function in module.function_templates() {
                index.function(function.id.as_str())?;
                scan_function(
                    &mut index,
                    module,
                    function.id.as_str(),
                    &function.requires,
                    &function.body,
                    &function.ensures,
                )?;
            }
        }
        Ok(index)
    }
    fn function(&mut self, id: &str) -> Result<()> {
        if self.functions.len() >= MAX_ITEMS {
            return Err(capacity("dependency callable index exceeds its bound"));
        }
        self.charge(id.len() + 128)?;
        if !self.functions.insert(id.to_owned()) {
            return Err(invalid("dependency callable identity is ambiguous"));
        }
        Ok(())
    }
    fn site(
        &mut self,
        module: &WorkspaceGraphProjectionModule,
        function: &str,
        phase: &str,
        node: &ResolvedExpr,
        id: &str,
        access: &str,
    ) -> Result<()> {
        if self.rows.len() >= MAX_ITEMS {
            return Err(capacity("dependency field sites exceed their bound"));
        }
        let mut row = site_origin(module, function, phase, node);
        row["field_or_type_id"] = json!(id);
        row["access"] = json!(access);
        row["reason"] = json!(access);
        self.charge_value(&row)?;
        if !self.sites_by_id.contains_key(id) {
            self.charge(id.len() + 128)?;
        }
        self.charge(2 * std::mem::size_of::<usize>())?;
        self.sites_by_id
            .entry(id.to_owned())
            .or_default()
            .push(self.rows.len());
        self.rows.push(row);
        Ok(())
    }
    fn call(
        &mut self,
        module: &WorkspaceGraphProjectionModule,
        function: &str,
        phase: &str,
        node: &ResolvedExpr,
    ) -> Result<()> {
        let Expr::Call {
            callee,
            type_arguments,
            instance,
            ..
        } = &node.kind
        else {
            return Ok(());
        };
        self.calls += 1;
        if self.calls > MAX_ITEMS {
            return Err(capacity("dependency call-site index exceeds its bound"));
        }
        if !self.callers.contains_key(callee.as_str()) {
            self.charge(callee.as_str().len() + 128)?;
        }
        if !self
            .callers
            .get(callee.as_str())
            .is_some_and(|callers| callers.contains(function))
        {
            self.charge(function.len() + 128)?;
        }
        self.callers
            .entry(callee.as_str().to_owned())
            .or_default()
            .insert(function.to_owned());
        let mut row = site_origin(module, function, phase, node);
        row["callee_id"] = json!(callee.as_str());
        row["instance_id"] = json!(instance.as_ref().map(|instance| instance.as_str()));
        row["type_arguments"] = json!(type_arguments
            .iter()
            .map(hir::ResolvedType::identity_key)
            .collect::<Vec<_>>());
        row["reason"] = json!("retained_direct_call_to_persistent_callee");
        self.charge_value(&row)?;
        self.call_sites.push(row);
        Ok(())
    }
    pub(super) fn typed_declaration(&self, target: &str) -> Value {
        self.typed.get(target).cloned().unwrap_or(Value::Null)
    }
    fn selected(&self, target: &str, complete: bool) -> BTreeSet<String> {
        let mut selected = BTreeSet::from([target.to_owned()]);
        if complete {
            if let Some(members) = self.members.get(target) {
                selected.extend(members.iter().cloned());
            }
        } else if let Some(fields) = self
            .typed
            .get(target)
            .and_then(|typed| typed["fields"].as_array())
        {
            selected.extend(
                fields
                    .iter()
                    .filter_map(|field| field["id"].as_str())
                    .map(str::to_owned),
            );
        }
        selected
    }
    pub(super) fn reverse_relationships(
        &self,
        revision: &ProjectRevision,
        target: &str,
    ) -> Result<Value> {
        let selection = self.selection(target, false)?;
        Ok(self.relationships(revision, &selection, false))
    }
    fn selection(&self, target: &str, complete: bool) -> Result<DependencySelection> {
        let selected = self.selected(target, complete);
        let mut ordinals = BTreeSet::new();
        for id in &selected {
            if let Some(sites) = self.sites_by_id.get(id) {
                ordinals.extend(sites.iter().copied());
            }
        }
        let mut users = BTreeSet::new();
        for ordinal in &ordinals {
            let row = &self.rows[*ordinal];
            users.insert(
                row["function_id"]
                    .as_str()
                    .expect("compiler site has owner")
                    .to_owned(),
            );
        }
        let mut closure = users.clone();
        if self.functions.contains(target) {
            closure.insert(target.to_owned());
        }
        let mut pending = closure.iter().cloned().collect::<Vec<_>>();
        while let Some(id) = pending.pop() {
            if let Some(callers) = self.callers.get(&id) {
                for caller in callers {
                    if closure.insert(caller.clone()) {
                        pending.push(caller.clone());
                    }
                }
            }
            if closure.len() > MAX_ITEMS {
                return Err(capacity("dependency caller closure exceeds its bound"));
            }
        }
        let calls = self
            .call_sites
            .iter()
            .enumerate()
            .filter_map(|(ordinal, site)| {
                (closure.contains(site["function_id"].as_str().unwrap_or(""))
                    && closure.contains(site["callee_id"].as_str().unwrap_or("")))
                .then_some(ordinal)
            })
            .collect();
        Ok(DependencySelection {
            selected,
            ordinals,
            users,
            closure,
            calls,
        })
    }
    fn relationships(
        &self,
        revision: &ProjectRevision,
        selection: &DependencySelection,
        complete: bool,
    ) -> Value {
        let rows = selection.ordinals.iter().map(|ordinal| {
            let row = &self.rows[*ordinal];
            if complete {row.clone()} else {json!({"field_or_type_id":row["field_or_type_id"],"function_id":row["function_id"],"path":row["path"],"phase":row["phase"],"expression_id":row["expression_id"],"access":row["access"]})}
        }).collect::<Vec<_>>();
        let users = &selection.users;
        let closure = &selection.closure;
        let test_root = revision.test_program().entrypoint.as_str();
        json!({"direct_field_sites":rows,"direct_field_user_functions":users,
            "reverse_callable_closure":closure,"declared_test_root":test_root,"test_reachable":closure.contains(test_root),
            "basis":"retained_HIR_field_ID_accesses_and_local_or_imported_direct_calls","coverage":"not_inferred","executed":false,
            "limitations":["no_external_or_dynamic_callers","aggregate_whole_value_reads_not_expanded_to_every_leaf","no_runtime_liveness_or_path_feasibility"]})
    }
}

impl ProjectSemanticImage {
    pub(super) fn dependency_index(&self) -> Result<&DependencyIndex> {
        self.dependencies
            .get_or_init(|| DependencyIndex::build(self.revision()))
            .as_ref()
            .map_err(Clone::clone)
    }
    /// Reverse relevance from actual retained declaration identities and HIR
    /// sites. This report neither executes tests nor establishes test coverage.
    pub fn declaration_dependencies(&self, expected_image: &str, target: &str) -> Result<String> {
        self.require_digest(expected_image)?;
        if target.is_empty() || target.len() > 4096 || target.contains('\0') {
            return Err(invalid(
                "dependency target must be a bounded stable identity",
            ));
        }
        let revision = self.revision();
        let declaration = revision.semantic.image_symbol(target).ok_or_else(|| {
            invalid("dependency target is absent from the retained declaration index")
        })?;
        let path = declaration["path"]
            .as_str()
            .ok_or_else(|| invalid("dependency target has no source owner"))?;
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .ok_or_else(|| invalid("dependency source binding is absent"))?;
        let index = self.dependency_index()?;
        let selection = index.selection(target, true)?;
        let relationships = index.relationships(revision, &selection, true);
        let calls = selection
            .calls
            .iter()
            .map(|ordinal| index.call_sites[*ordinal].clone())
            .collect::<Vec<_>>();
        let report = json!({"schema":IMAGE_DECLARATION_DEPENDENCIES_SCHEMA,"image_digest":self.image_digest(),
            "project_revision":revision.project_revision(),"workspace_revision":revision.workspace_revision(),"target":target,
            "declaration":declaration,"source_binding":{"path":path,"module":declaration["module"],"source_revision":source.source_revision(),"source_digest":source.source_digest()},
            "typed_declaration":index.typed_declaration(target),"selected_declaration_ids":index.selected(target,true),
            "relationships":relationships,"direct_call_sites":calls,"evidence_owner":"retained_checked_hir",
            "evidence_class":"descriptive_recomputable_compiler_projection",
            "limits":{"max_report_bytes":MAX_IMAGE_DECLARATION_DEPENDENCIES_BYTES,"max_retained_bytes":MAX_RETAINED_BYTES,"max_sites":MAX_ITEMS,"max_calls":MAX_ITEMS,"max_type_items":MAX_ITEMS,"max_expression_and_pattern_visits":MAX_VISITS,"max_depth":MAX_DEPTH},
            "index_work":{"expression_visits":index.visits,"pattern_visits":index.pattern_visits,"sites":index.rows.len(),"calls":index.calls,"type_items":index.type_items,"charged_retained_bytes":index.retained_bytes},
            "nonclaims":["not_test_coverage","no_execution_or_source_authority","no_external_or_dynamic_callers","no_runtime_liveness_or_path_feasibility","materialized_generic_instances_not_rescanned; template_callee_IDs_used_without_remapping"]});
        super::image::render(report, true, MAX_IMAGE_DECLARATION_DEPENDENCIES_BYTES)
            .map_err(|_| capacity("dependency report exceeds its byte bound"))
    }
}

fn field_value(field: &hir::ResolvedFieldDeclaration) -> Value {
    json!({"id":field.id.as_str(),"name":field.name,"type_id":field.ty.identity_key(),"index":field.index})
}
fn site_origin(
    module: &WorkspaceGraphProjectionModule,
    function: &str,
    phase: &str,
    node: &ResolvedExpr,
) -> Value {
    json!({"function_id":function,"path":module.path(),"module":module.module(),"phase":phase,"expression_id":node.id.as_str(),
        "source_revision":module.source_revision(),"source_digest":module.source_digest(),
        "span":{"start":node.span.start,"end":node.span.end,"line":node.span.line,"column":node.span.column},
        "evidence_owner":"retained_checked_hir"})
}
fn value_bytes(value: &Value) -> Result<usize> {
    let mut pending = vec![(value, 0usize)];
    let mut bytes = 0usize;
    let mut nodes = 0usize;
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        if nodes > MAX_VISITS || depth > MAX_DEPTH {
            return Err(capacity("dependency row exceeds its structural bound"));
        }
        bytes = bytes
            .checked_add(std::mem::size_of::<Value>() + 64)
            .ok_or_else(|| capacity("dependency row allocation accounting overflow"))?;
        match value {
            Value::String(text) => bytes = bytes.saturating_add(text.capacity()),
            Value::Array(values) => {
                bytes = bytes.saturating_add(values.capacity() * std::mem::size_of::<Value>());
                pending.extend(values.iter().map(|value| (value, depth + 1)));
            }
            Value::Object(values) => {
                for (key, value) in values {
                    bytes = bytes.saturating_add(key.capacity() + 128);
                    pending.push((value, depth + 1));
                }
            }
            _ => {}
        }
        if bytes > MAX_RETAINED_BYTES || pending.len() > MAX_VISITS - nodes {
            return Err(capacity("dependency row exceeds its byte or node bound"));
        }
    }
    Ok(bytes)
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G320", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G321", message)]
}

fn scan_function(
    relations: &mut DependencyIndex,
    module: &WorkspaceGraphProjectionModule,
    function: &str,
    requires: &[ResolvedExpr],
    body: &ResolvedExpr,
    ensures: &[ResolvedExpr],
) -> Result<()> {
    for (phase, roots) in [
        ("requires", requires),
        ("body", std::slice::from_ref(body)),
        ("ensures", ensures),
    ] {
        let mut pending = roots.iter().rev().map(|node| (node, 0)).collect::<Vec<_>>();
        while let Some((node, depth)) = pending.pop() {
            relations.visits += 1;
            if relations.visits > MAX_VISITS || depth > MAX_DEPTH {
                return Err(capacity(
                    "declaration dependency HIR relationship traversal exceeds its bound",
                ));
            }
            relations.call(module, function, phase, node)?;
            let mut pattern_visits = relations.pattern_visits;
            let mut access =
                |id: &str, kind: &str| relations.site(module, function, phase, node, id, kind);
            match &node.kind {
                Expr::Place(place) | Expr::BorrowPlace { place, .. } => {
                    for projection in &place.projections {
                        match projection {
                            PlaceProjection::Field(field)
                            | PlaceProjection::VariantField { field, .. } => access(
                                field.as_str(),
                                if matches!(node.kind, Expr::BorrowPlace { .. }) {
                                    "borrow"
                                } else {
                                    "read_or_move"
                                },
                            )?,
                        }
                    }
                }
                Expr::Project { field, .. } => access(field.as_str(), "projection_read")?,
                Expr::ConstructRecord { record, fields } => {
                    access(record.as_str(), "construct_record")?;
                    for field in fields {
                        access(field.field.as_str(), "initialize")?;
                    }
                }
                Expr::ConstructVariant {
                    variant,
                    case,
                    fields,
                } => {
                    access(variant.as_str(), "construct_variant")?;
                    access(case.as_str(), "construct_case")?;
                    for field in fields {
                        access(field.field.as_str(), "initialize")?;
                    }
                }
                Expr::UpdateRecord { fields, .. } => {
                    for field in fields {
                        access(field.field.as_str(), "update_result_field")?;
                    }
                }
                Expr::Block { statements, .. } => {
                    for statement in statements {
                        if let ResolvedStatement::Assign {
                            field: Some(field), ..
                        } = statement
                        {
                            access(field.as_str(), "in_place_write")?;
                        }
                    }
                }
                Expr::Match { arms, .. } => {
                    for arm in arms {
                        pattern_accesses(&arm.pattern, &mut access, 0, &mut pattern_visits)?;
                    }
                }
                _ => {}
            }
            relations.pattern_visits = pattern_visits;
            if relations.visits.saturating_add(relations.pattern_visits) > MAX_VISITS {
                return Err(capacity(
                    "dependency combined expression/pattern visits exceed their bound",
                ));
            }
            let mut children = Vec::new();
            hir::push_resolved_expression_children_in_authored_order(node, &mut children);
            if children.len() > MAX_VISITS.saturating_sub(relations.visits + pending.len()) {
                return Err(capacity(
                    "declaration dependency pending HIR inventory exceeds its bound",
                ));
            }
            pending.extend(children.into_iter().map(|child| (child, depth + 1)));
        }
    }
    Ok(())
}
fn pattern_accesses(
    pattern: &ResolvedMatchPattern,
    access: &mut impl FnMut(&str, &str) -> Result<()>,
    depth: usize,
    visits: &mut usize,
) -> Result<()> {
    *visits += 1;
    if *visits > MAX_ITEMS {
        return Err(capacity(
            "declaration dependency pattern item inventory exceeds its bound",
        ));
    }
    if depth > MAX_DEPTH {
        return Err(capacity(
            "declaration dependency pattern depth exceeds its bound",
        ));
    }
    match pattern {
        ResolvedMatchPattern::Record { record, fields, .. } => {
            access(record.as_str(), "record_pattern")?;
            record_pattern_accesses(fields, access, depth + 1, visits)?;
        }
        ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } => {
            access(variant.as_str(), "variant_pattern")?;
            access(case.as_str(), "case_pattern")?;
            for field in fields {
                *visits += 1;
                if *visits > MAX_ITEMS {
                    return Err(capacity(
                        "declaration dependency pattern field inventory exceeds its bound",
                    ));
                }
                access(field.field.as_str(), "pattern_bind")?;
            }
        }
        ResolvedMatchPattern::Or(alternatives) => {
            for alternative in alternatives {
                pattern_accesses(alternative, access, depth + 1, visits)?;
            }
        }
        ResolvedMatchPattern::Wildcard
        | ResolvedMatchPattern::Literal(_)
        | ResolvedMatchPattern::Binding(_) => {}
    }
    Ok(())
}
fn record_pattern_accesses(
    fields: &[ResolvedRecordMatchPatternField],
    access: &mut impl FnMut(&str, &str) -> Result<()>,
    depth: usize,
    visits: &mut usize,
) -> Result<()> {
    *visits += 1;
    if *visits > MAX_ITEMS {
        return Err(capacity(
            "declaration dependency pattern item inventory exceeds its bound",
        ));
    }
    if depth > MAX_DEPTH {
        return Err(capacity(
            "declaration dependency record pattern depth exceeds its bound",
        ));
    }
    for field in fields {
        *visits += 1;
        if *visits > MAX_ITEMS {
            return Err(capacity(
                "declaration dependency pattern field inventory exceeds its bound",
            ));
        }
        match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(_) => {
                access(field.field.as_str(), "pattern_bind")?
            }
            ResolvedRecordMatchFieldPattern::Wildcard => {
                access(field.field.as_str(), "pattern_ignore")?
            }
            ResolvedRecordMatchFieldPattern::Record { record, fields, .. } => {
                access(field.field.as_str(), "nested_pattern")?;
                access(record.as_str(), "record_pattern")?;
                record_pattern_accesses(fields, access, depth + 1, visits)?;
            }
        }
    }
    Ok(())
}
