//! Bounded structural access and boundary facts; never an execution trace.
use serde_json::{json, Value};
use std::collections::BTreeMap;

use super::{
    error, ownership, span, ImageFacet, ProjectSemanticImage, MAX_INTERMEDIATE_BYTES, MAX_ITEMS,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, OwnershipMode, Place, PlaceProjection, ResolvedExpr, ResolvedExprKind as Expr,
    ResolvedFunction, ResolvedMatchMode, ResolvedStatement as Statement,
};
use crate::workspace_graph::WorkspaceGraphProjectionModule;

const MAX_DEPTH: usize = 256;
const MAX_VISITS: usize = 65_536;
#[derive(Clone, Copy)]
enum Access {
    Observe,
    Consume,
    Borrow,
    Unclassified,
}
impl Access {
    fn name(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Consume => "consume",
            Self::Borrow => "borrow",
            Self::Unclassified => "unclassified",
        }
    }
    fn from_mode(mode: OwnershipMode) -> Self {
        match mode {
            OwnershipMode::Own => Self::Consume,
            OwnershipMode::Borrow => Self::Borrow,
            _ => Self::Observe,
        }
    }
}
#[derive(Clone, Copy)]
struct Context<'a> {
    phase: &'static str,
    depth: usize,
    boundary: Option<&'a ResolvedExpr>,
    access: Access,
}
enum Task<'a> {
    Expression(&'a ResolvedExpr, Context<'a>),
    Statement(&'a Statement, &'a ResolvedExpr, usize, Context<'a>),
}

pub(super) fn items(
    image: &ProjectSemanticImage,
    module: &WorkspaceGraphProjectionModule,
    function: &ResolvedFunction,
    facet: ImageFacet,
) -> Result<Vec<Value>, Vec<Diagnostic>> {
    let mut output = Collector {
        image,
        module,
        function,
        facet,
        items: Vec::new(),
        bytes: 0,
    };
    let mut call_targets = BTreeMap::new();
    for source in image.revision().semantic.image_modules() {
        for function in source.functions() {
            if call_targets.len() >= MAX_VISITS {
                return Err(limit());
            }
            call_targets.insert(function.id.as_str(), function);
        }
        for instance in source.function_instances() {
            if call_targets.len() >= MAX_VISITS {
                return Err(limit());
            }
            call_targets.insert(instance.id.as_str(), &instance.function);
        }
    }
    if function
        .requires
        .len()
        .saturating_add(function.ensures.len())
        .saturating_add(1)
        > MAX_VISITS
    {
        return Err(limit());
    }
    let mut pending = Vec::new();
    for (phase, roots) in [
        ("ensures", function.ensures.as_slice()),
        ("body", std::slice::from_ref(&function.body)),
        ("requires", function.requires.as_slice()),
    ] {
        for expression in roots.iter().rev() {
            pending.push(Task::Expression(
                expression,
                Context {
                    phase,
                    depth: 0,
                    boundary: None,
                    access: if phase == "body" {
                        Access::from_mode(function.body.ownership)
                    } else {
                        Access::Observe
                    },
                },
            ));
        }
    }
    let mut visits = 0;
    while let Some(task) = pending.pop() {
        visits += 1;
        if visits > MAX_VISITS {
            return Err(limit());
        }
        match task {
            Task::Statement(statement, block, index, context) => {
                if context.depth > MAX_DEPTH {
                    return Err(limit());
                }
                match statement {
                    Statement::Let {
                        binding,
                        value,
                        span: location,
                        ..
                    }
                    | Statement::Assign {
                        binding,
                        value,
                        span: location,
                        ..
                    } => {
                        if facet == ImageFacet::DataAccess {
                            let (edge, field, mutable) = match statement {
                                Statement::Let { mutable, .. } => {
                                    ("binding_initialize", None, *mutable)
                                }
                                Statement::Assign { field, .. } => (
                                    "binding_write",
                                    field.as_ref().map(|field| field.as_str()),
                                    true,
                                ),
                                _ => unreachable!(),
                            };
                            output.push(value,context,edge,"resolved_statement_binding",json!({"value_id":binding.id.as_str(),"field_id":field,"binding_ownership":ownership(binding.ownership),"binding_type_id":binding.ty.identity_key(),"mutable":mutable,"container_expression_id":block.id.as_str(),"statement_index":index,"span":span(*location)}))?;
                        }
                        pending.push(Task::Expression(
                            value,
                            Context {
                                access: Access::from_mode(binding.ownership),
                                depth: context.depth + 1,
                                ..context
                            },
                        ));
                    }
                    Statement::Unsafe {
                        audit,
                        body,
                        span: location,
                    } => {
                        if facet == ImageFacet::UnsafeBoundaries {
                            output.push(body,context,"unsafe_audit_boundary","resolved_audited_safe_body",json!({"audit":audit,"body_expression_id":body.id.as_str(),"container_expression_id":block.id.as_str(),"statement_index":index,"span":span(*location),"module_unsafe_permit":module.permits().iter().any(|permit|permit=="unsafe"),"raw_memory_operations":false}))?;
                        }
                        pending.push(Task::Expression(
                            body,
                            Context {
                                boundary: Some(body),
                                access: Access::Observe,
                                depth: context.depth + 1,
                                ..context
                            },
                        ));
                    }
                    Statement::While {
                        condition, body, ..
                    } => {
                        let child = Context {
                            depth: context.depth + 1,
                            access: Access::Observe,
                            ..context
                        };
                        pending.push(Task::Expression(body, child));
                        pending.push(Task::Expression(condition, child));
                    }
                }
            }
            Task::Expression(expression, context) => {
                if context.depth > MAX_DEPTH {
                    return Err(limit());
                }
                let children = match &expression.kind {
                    Expr::Block { statements, .. } => statements.len().saturating_add(1),
                    Expr::Call { args, .. } => args.len(),
                    Expr::NativeRustImportCall(call) => call.args.len(),
                    Expr::HostCommandCall(call) => call.args.len(),
                    Expr::ConstructRecord { fields, .. }
                    | Expr::ConstructVariant { fields, .. } => fields.len(),
                    Expr::UpdateRecord { fields, .. } => fields.len().saturating_add(1),
                    Expr::Match { arms, .. } => arms.len().saturating_mul(2).saturating_add(1),
                    _ => 3,
                };
                if children > MAX_VISITS.saturating_sub(visits.saturating_add(pending.len())) {
                    return Err(limit());
                }
                output.expression(expression, context)?;
                let child = Context {
                    depth: context.depth + 1,
                    access: Access::Observe,
                    ..context
                };
                match &expression.kind {
                    Expr::Block { statements, tail } => {
                        if statements.len() > MAX_VISITS.saturating_sub(pending.len()) {
                            return Err(limit());
                        }
                        pending.push(Task::Expression(
                            tail,
                            Context {
                                access: context.access,
                                ..child
                            },
                        ));
                        for (index, statement) in statements.iter().enumerate().rev() {
                            pending.push(Task::Statement(statement, expression, index, child));
                        }
                    }
                    Expr::Call {
                        callee,
                        instance,
                        args,
                        ..
                    } => {
                        let target = call_targets
                            .get(
                                instance
                                    .as_ref()
                                    .map_or(callee.as_str(), |instance| instance.as_str()),
                            )
                            .copied();
                        for (index, arg) in args.iter().enumerate().rev() {
                            let access = target
                                .and_then(|target| target.params.get(index))
                                .map_or(Access::Unclassified, |param| {
                                    Access::from_mode(param.ownership)
                                });
                            pending.push(Task::Expression(arg, Context { access, ..child }));
                        }
                    }
                    Expr::NativeRustImportCall(call) => {
                        let import = module
                            .interfaces()
                            .iter()
                            .flat_map(|interface| &interface.imports)
                            .find(|item| item.id == call.import);
                        for (index, arg) in call.args.iter().enumerate().rev() {
                            let access = import
                                .and_then(|item| item.parameters.get(index))
                                .map_or(Access::Unclassified, |param| {
                                    Access::from_mode(param.ownership)
                                });
                            pending.push(Task::Expression(arg, Context { access, ..child }));
                        }
                    }
                    Expr::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        pending.push(Task::Expression(
                            else_branch,
                            Context {
                                access: context.access,
                                ..child
                            },
                        ));
                        pending.push(Task::Expression(
                            then_branch,
                            Context {
                                access: context.access,
                                ..child
                            },
                        ));
                        pending.push(Task::Expression(condition, child));
                    }
                    Expr::Match {
                        mode,
                        scrutinee,
                        arms,
                    } => {
                        for arm in arms.iter().rev() {
                            pending.push(Task::Expression(
                                &arm.value,
                                Context {
                                    access: context.access,
                                    ..child
                                },
                            ));
                            if let Some(guard) = &arm.guard {
                                pending.push(Task::Expression(guard, child));
                            }
                        }
                        let access = match mode {
                            ResolvedMatchMode::Own => Access::Consume,
                            ResolvedMatchMode::Borrow => Access::Borrow,
                            ResolvedMatchMode::Value => Access::Observe,
                        };
                        pending.push(Task::Expression(scrutinee, Context { access, ..child }));
                    }
                    Expr::Upcast { source } => pending.push(Task::Expression(
                        source,
                        Context {
                            access: Access::Consume,
                            ..child
                        },
                    )),
                    Expr::ConstructRecord { fields, .. }
                    | Expr::ConstructVariant { fields, .. } => {
                        for field in fields.iter().rev() {
                            pending.push(Task::Expression(
                                &field.value,
                                Context {
                                    access: Access::from_mode(field.value.ownership),
                                    ..child
                                },
                            ));
                        }
                    }
                    Expr::UpdateRecord { base, fields, .. } => {
                        for field in fields.iter().rev() {
                            pending.push(Task::Expression(
                                &field.value,
                                Context {
                                    access: Access::from_mode(field.value.ownership),
                                    ..child
                                },
                            ));
                        }
                        pending.push(Task::Expression(
                            base,
                            Context {
                                access: Access::from_mode(base.ownership),
                                ..child
                            },
                        ));
                    }
                    Expr::Try { operand, .. } | Expr::TryOption { operand, .. } => {
                        pending.push(Task::Expression(
                            operand,
                            Context {
                                access: Access::from_mode(operand.ownership),
                                ..child
                            },
                        ))
                    }
                    _ => {
                        let mut children = Vec::new();
                        hir::push_resolved_expression_children_in_authored_order(
                            expression,
                            &mut children,
                        );
                        pending.extend(
                            children
                                .into_iter()
                                .map(|expression| Task::Expression(expression, child)),
                        );
                    }
                }
            }
        }
        if pending.len() > MAX_VISITS.saturating_sub(visits) {
            return Err(limit());
        }
    }
    Ok(output.items)
}

struct Collector<'a> {
    image: &'a ProjectSemanticImage,
    module: &'a WorkspaceGraphProjectionModule,
    function: &'a ResolvedFunction,
    facet: ImageFacet,
    items: Vec<Value>,
    bytes: usize,
}
impl Collector<'_> {
    fn push(
        &mut self,
        expression: &ResolvedExpr,
        context: Context<'_>,
        edge: &str,
        reason: &str,
        facts: Value,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.items.len() >= MAX_ITEMS {
            return Err(limit());
        }
        let mut row = json!({"schema":"semaprax.image-hir-relationship.v1","image_revision":self.image.image_digest(),"project_revision":self.image.revision().project_revision(),"function_id":self.function.id.as_str(),"path":self.module.path(),"module":self.module.module(),"source_revision":self.module.source_revision(),"source_digest":self.module.source_digest(),"expression_id":expression.id.as_str(),"span":span(expression.span),"phase":context.phase,"edge_kind":edge,"reason":reason,"evidence_owner":"retained_validated_module_hir","evidence_class":"structural_source_fact","expression_ownership":ownership(expression.ownership),"type_id":expression.ty.identity_key(),"use_context":context.access.name(),"enclosing_unsafe_body":context.boundary.map(|body|body.id.as_str()),"runtime_execution":false});
        row.as_object_mut()
            .unwrap()
            .extend(facts.as_object().unwrap().clone());
        let rendered = super::report(
            row.clone(),
            MAX_INTERMEDIATE_BYTES.saturating_sub(self.bytes),
        )?;
        self.bytes = self.bytes.checked_add(rendered.len()).ok_or_else(limit)?;
        self.items.push(row);
        Ok(())
    }
    fn expression(
        &mut self,
        expression: &ResolvedExpr,
        context: Context<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.facet == ImageFacet::DataAccess {
            match &expression.kind {
                Expr::Place(place) => {
                    let edge = if expression.ownership == OwnershipMode::Own
                        && matches!(context.access, Access::Consume)
                    {
                        "place_move"
                    } else if matches!(context.access, Access::Borrow) {
                        "place_borrow"
                    } else {
                        "place_read"
                    };
                    self.push(
                        expression,
                        context,
                        edge,
                        "resolved_place_and_explicit_use_context",
                        place_facts(place),
                    )?;
                }
                Expr::BorrowPlace { operation, place } => {
                    let mut facts = place_facts(place);
                    facts["operation_id"] = json!(operation.as_str());
                    self.push(
                        expression,
                        context,
                        "place_borrow",
                        "compiler_owned_nonconsuming_borrow_place",
                        facts,
                    )?;
                }
                Expr::Project { base, field } => self.push(
                    expression,
                    context,
                    "field_projection",
                    "resolved_expression_projection",
                    json!({"base_expression_id":base.id.as_str(),"field_id":field.as_str()}),
                )?,
                Expr::ConstructRecord { fields, .. }
                | Expr::ConstructVariant { fields, .. }
                | Expr::UpdateRecord { fields, .. } => {
                    for (index, field) in fields.iter().enumerate() {
                        self.push(expression,context,"field_initialize","constructed_result_field_not_in_place_store",json!({"field_id":field.field.as_str(),"initializer_expression_id":field.value.id.as_str(),"field_index":index}))?;
                    }
                }
                _ => {}
            }
        } else if let Expr::NativeRustImportCall(call) = &expression.kind {
            let import = self
                .module
                .interfaces()
                .iter()
                .flat_map(|interface| &interface.imports)
                .find(|item| item.id == call.import);
            let descriptor=import.map(|item|json!({"import_id":item.id.as_str(),"interface_id":item.interface.as_str(),"import_key":item.import_key,"native_rust":item.native_rust,"declared_effects":item.effects,"required_authority":item.required_authority,"parameters":item.parameters.iter().enumerate().map(|(index,param)|json!({"index":index,"type_id":param.ty.identity_key(),"ownership":ownership(param.ownership),"consumes_on_failure":param.consumes_on_failure})).collect::<Vec<_>>(),"result_ownership":ownership(item.result.ownership),"result_producer":item.result.producer,"result_ownership_transfer":item.result.ownership_transfer,"span":span(item.span)}));
            self.push(expression,context,"native_rust_import_call","resolved_direct_import_call_no_transitive_effect_inference",json!({"import_id":call.import.as_str(),"import_expression_id":call.expression.as_str(),"arguments":call.args.iter().enumerate().map(|(index,arg)|json!({"index":index,"expression_id":arg.id.as_str(),"ownership":ownership(arg.ownership),"type_id":arg.ty.identity_key()})).collect::<Vec<_>>(),"declaration":descriptor,"declaration_available":import.is_some(),"host_authority_granted":false}))?;
        }
        Ok(())
    }
}
fn place_facts(place: &Place) -> Value {
    json!({"value_id":place.root.as_str(),"projections":place.projections.iter().map(|part|match part {PlaceProjection::Field(field)=>json!({"kind":"field","field_id":field.as_str()}),PlaceProjection::VariantField {case,field}=>json!({"kind":"variant_field","case_id":case.as_str(),"field_id":field.as_str()})}).collect::<Vec<_>>()})
}
fn limit() -> Vec<Diagnostic> {
    error(
        "SPX-G228",
        "semantic image HIR relationship traversal exceeds its structural or byte bound",
    )
}
