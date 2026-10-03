//! The validated cleanup CFG drives physical ownership. Expression evaluators
//! cache each acyclic HIR value; no cleanup ordering is reconstructed from AST.
use super::*;
use semaprax::ast::BinaryOp;
use semaprax::cleanup::FieldLivenessShape;
use semaprax::cleanup_plan::{
    CleanupPlace, CleanupResultSource, CleanupTerminator, CleanupTransition, EdgeCondition,
    ExitContinuation,
};
use semaprax::hir::{
    DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedStatement, ValueId,
};
use std::collections::BTreeMap;
use std::fmt::Write;

struct Emitter<'a> {
    function: &'a ResolvedFunction,
    expressions: Vec<&'a ResolvedExpr>,
    bindings: BTreeMap<ValueId, &'a ResolvedExpr>,
    constructor: &'a DeclarationId,
    method: &'a DeclarationId,
}

pub(super) fn render(
    function: &ResolvedFunction,
    constructor: &DeclarationId,
    method: &DeclarationId,
    lifecycle: &DeclarationId,
    resource: &ResolvedType,
) -> Result<String, Diagnostic> {
    if !function.requires.is_empty()
        || !function.ensures.is_empty()
        || !function.effects.is_empty()
        || function.params.len() > 8
        || function.params.iter().any(|p| p.ty != ResolvedType::I64)
        || !matches!(function.return_type, ResolvedType::I64 | ResolvedType::Bool)
        || !function
            .cleanup_plan
            .entry_state
            .live_owned_parameters
            .is_empty()
        || function.cleanup_plan.slots.len() > 64
    {
        return Err(sdk_error(
            "opaque owner export is outside the experimental profile",
        ));
    }
    let mut emitter = Emitter {
        function,
        expressions: Vec::new(),
        bindings: BTreeMap::new(),
        constructor,
        method,
    };
    emitter.collect(&function.body)?;
    for slot in &function.cleanup_plan.slots {
        if &slot.ty != resource
            || !matches!(&slot.field_liveness_shape, FieldLivenessShape::Leaf { lifecycle: id, .. } if id == lifecycle)
        {
            return Err(sdk_error("opaque owner cleanup slot is unsupported"));
        }
    }
    emitter.render(lifecycle)
}

impl<'a> Emitter<'a> {
    fn collect(&mut self, expression: &'a ResolvedExpr) -> Result<(), Diagnostic> {
        if self.expressions.len() >= 256 {
            return Err(sdk_error("opaque owner expression limit"));
        }
        self.expressions.push(expression);
        match &expression.kind {
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::Place(place) if place.projections.is_empty() => {}
            ResolvedExprKind::NativeRustImportCall(call)
                if &call.import == self.constructor || &call.import == self.method =>
            {
                for argument in &call.args {
                    self.collect(argument)?;
                }
            }
            ResolvedExprKind::Block { statements, tail } => {
                if expression.id != self.function.body.id && !statements.is_empty() {
                    return Err(sdk_error(
                        "opaque owner nested statement scope is unsupported",
                    ));
                }
                for statement in statements {
                    let ResolvedStatement::Let {
                        binding,
                        mutable: false,
                        value,
                        ..
                    } = statement
                    else {
                        return Err(sdk_error("opaque owner statement is unsupported"));
                    };
                    self.bindings.insert(binding.id.clone(), value);
                    self.collect(value)?;
                }
                self.collect(tail)?;
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect(condition)?;
                self.collect(then_branch)?;
                self.collect(else_branch)?;
            }
            ResolvedExprKind::Binary {
                op: BinaryOp::Div,
                left,
                right,
            } if expression.ty == ResolvedType::I64 => {
                self.collect(left)?;
                self.collect(right)?;
            }
            _ => return Err(sdk_error("opaque owner expression is unsupported")),
        }
        Ok(())
    }
    fn index(&self, id: &semaprax::hir::ExpressionId) -> Result<usize, Diagnostic> {
        self.expressions
            .iter()
            .position(|expression| &expression.id == id)
            .ok_or_else(|| sdk_error("opaque owner cleanup expression is missing"))
    }
    fn slot(&self, place: &CleanupPlace) -> Result<usize, Diagnostic> {
        if !place.projections.is_empty() {
            return Err(sdk_error("opaque owner projection is unsupported"));
        }
        self.function
            .cleanup_plan
            .slots
            .iter()
            .position(|slot| slot.storage == place.storage)
            .ok_or_else(|| sdk_error("opaque owner cleanup slot is missing"))
    }
    fn eval(&self, expression: &ResolvedExpr) -> Result<String, Diagnostic> {
        Ok(format!(
            "spx_eval_{}(context,f)",
            self.index(&expression.id)?
        ))
    }
    fn render(&self, lifecycle: &DeclarationId) -> Result<String, Diagnostic> {
        let mut out = String::from("#include \"owner.h\"\n#include <limits.h>\n#include <stddef.h>\n_Static_assert(sizeof(spx_owner)==24,\"owner wire size\");\ntypedef struct { int64_t scalar; spx_owner owner; int32_t status; uint8_t done; } spx_value;\n");
        writeln!(out, "typedef struct {{ spx_value values[{}]; int64_t args[8]; spx_owner owners[{}]; uint8_t live[{}]; }} spx_frame;", self.expressions.len(), self.function.cleanup_plan.slots.len().max(1), self.function.cleanup_plan.slots.len().max(1)).unwrap();
        for index in 0..self.expressions.len() {
            writeln!(
                out,
                "spx_value spx_eval_{index}(uint64_t context, spx_frame *f);"
            )
            .unwrap();
        }
        for (index, expression) in self.expressions.iter().enumerate() {
            writeln!(out,"spx_value spx_eval_{index}(uint64_t context, spx_frame *f) {{ (void)context; if(f->values[{index}].done) return f->values[{index}]; spx_value v={{0}};").unwrap();
            match &expression.kind {
                ResolvedExprKind::Int(value) => {
                    writeln!(out, "v.scalar=(int64_t)UINT64_C({});", *value as u64).unwrap();
                }
                ResolvedExprKind::Bool(value) => {
                    writeln!(out, "v.scalar={};", u8::from(*value)).unwrap();
                }
                ResolvedExprKind::Place(place) => {
                    if let Some(parameter) = self
                        .function
                        .params
                        .iter()
                        .position(|parameter| parameter.id == place.root)
                    {
                        writeln!(out, "v.scalar=f->args[{parameter}];").unwrap();
                    } else {
                        let value = self
                            .bindings
                            .get(&place.root)
                            .ok_or_else(|| sdk_error("opaque owner binding is missing"))?;
                        writeln!(out, "v={};", self.eval(value)?).unwrap();
                    }
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    for (i, argument) in call.args.iter().enumerate() {
                        writeln!(
                            out,
                            "spx_value a{i}={}; if(a{i}.status) return a{i};",
                            self.eval(argument)?
                        )
                        .unwrap();
                    }
                    if &call.import == self.constructor {
                        out.push_str("v.status=spx_owner_new(context,a0.scalar,&v.owner);\n");
                    } else {
                        out.push_str("uint8_t result=0; v.status=spx_owner_consume(context,a0.owner,a1.scalar,&result); if(!v.status) v.scalar=result;\n");
                    }
                }
                ResolvedExprKind::Block { statements, tail } => {
                    for statement in statements {
                        writeln!(
                            out,
                            "v={}; if(v.status) return v;",
                            self.eval(statement.value())?
                        )
                        .unwrap();
                    }
                    writeln!(out, "v={};", self.eval(tail)?).unwrap();
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    writeln!(
                        out,
                        "v={}; if(v.status) return v; v=v.scalar ? {} : {};",
                        self.eval(condition)?,
                        self.eval(then_branch)?,
                        self.eval(else_branch)?
                    )
                    .unwrap();
                }
                ResolvedExprKind::Binary { left, right, .. } => {
                    writeln!(out,"spx_value left={}; if(left.status) return left; spx_value right={}; if(right.status) v=right; else if(right.scalar==0 || (left.scalar==INT64_MIN && right.scalar==-1)) v.status=8; else v.scalar=left.scalar/right.scalar;",self.eval(left)?,self.eval(right)?).unwrap();
                }
                _ => unreachable!("collected closed expressions"),
            }
            writeln!(out, "v.done=1; f->values[{index}]=v; return v; }}").unwrap();
        }
        let parameters = (0..self.function.params.len())
            .map(|index| format!(", int64_t arg_{index}"))
            .collect::<String>();
        writeln!(out,"int32_t spx_owner_entry(uint64_t context{parameters}, int64_t *result) {{ if(!result) return 3; spx_frame frame={{0}}, *f=&frame; int32_t status=0; spx_value value={{0}}; (void)value;").unwrap();
        for index in 0..self.function.params.len() {
            writeln!(out, "f->args[{index}]=arg_{index};").unwrap();
        }
        writeln!(out, "goto block_{};", self.function.cleanup_plan.entry.0).unwrap();
        for block in &self.function.cleanup_plan.blocks {
            writeln!(out, "block_{}:;", block.id.0).unwrap();
            for transition in &block.transitions {
                self.transition(&mut out, transition)?;
            }
            match &block.terminator {
                CleanupTerminator::Goto(edge) => {
                    writeln!(
                        out,
                        "goto block_{};",
                        self.function.cleanup_plan.edges[edge.0 as usize].to.0
                    )
                    .unwrap();
                }
                CleanupTerminator::Branch(edges) => {
                    for edge in edges {
                        let edge = &self.function.cleanup_plan.edges[edge.0 as usize];
                        let condition = match &edge.condition {
                            EdgeCondition::BooleanResult(id, selected) => format!(
                                "{}spx_eval_{}(context,f).scalar",
                                if *selected { "" } else { "!" },
                                self.index(id)?
                            ),
                            EdgeCondition::StatusZero(source) => format!(
                                "!spx_eval_{}(context,f).status",
                                self.index(&source.expression)?
                            ),
                            EdgeCondition::StatusNonzero(source) => format!(
                                "spx_eval_{}(context,f).status",
                                self.index(&source.expression)?
                            ),
                            _ => {
                                return Err(sdk_error("opaque owner cleanup branch is unsupported"))
                            }
                        };
                        writeln!(out, "if({condition}) goto block_{};", edge.to.0).unwrap();
                    }
                    out.push_str("return 7;\n");
                }
                CleanupTerminator::Exit(exit) => {
                    let exit = &self.function.cleanup_plan.exits[exit.0 as usize];
                    // The vector is already canonical runtime order. Never sort it.
                    for action in &exit.finalize_in_order {
                        if &action.lifecycle_id != lifecycle || action.active_case.is_some() {
                            return Err(sdk_error("opaque owner finalizer is unsupported"));
                        }
                        let slot = self.slot(&action.source)?;
                        writeln!(out,"if(f->live[{slot}]) {{ f->live[{slot}]=0; int32_t dropped=spx_owner_drop(context,f->owners[{slot}]); if(!status) status=dropped; }}").unwrap();
                    }
                    match &exit.continuation {
                        ExitContinuation::Continue(edge) => {
                            writeln!(
                                out,
                                "goto block_{};",
                                self.function.cleanup_plan.edges[edge.0 as usize].to.0
                            )
                            .unwrap();
                        }
                        ExitContinuation::ReturnFailure { .. } => out.push_str("return status;\n"),
                        ExitContinuation::CommitResult {
                            source: CleanupResultSource::Scalar { expression },
                        } => {
                            writeln!(out,"if(status) return status; value=spx_eval_{}(context,f); if(value.status) return value.status; *result=value.scalar; return 0;",self.index(expression)?).unwrap();
                        }
                        _ => {
                            return Err(sdk_error(
                                "opaque owner cleanup continuation is unsupported",
                            ))
                        }
                    }
                }
            }
        }
        out.push_str("}\n");
        Ok(out)
    }
    fn transition(
        &self,
        out: &mut String,
        transition: &CleanupTransition,
    ) -> Result<(), Diagnostic> {
        match transition {
            CleanupTransition::Initialize { at, destination } => {
                let slot = self.slot(destination)?;
                writeln!(out,"value=spx_eval_{}(context,f); if(value.status || f->live[{slot}]) return 7; f->owners[{slot}]=value.owner; f->live[{slot}]=1;",self.index(at)?).unwrap();
            }
            CleanupTransition::Transfer {
                source,
                destination,
                ..
            } => {
                let source = self.slot(source)?;
                let destination = self.slot(destination)?;
                writeln!(out,"if(!f->live[{source}] || f->live[{destination}]) return 7; f->owners[{destination}]=f->owners[{source}]; f->live[{source}]=0; f->live[{destination}]=1;").unwrap();
            }
            CleanupTransition::CallCommit { arguments, .. } => {
                for argument in arguments {
                    let slot = self.slot(&argument.source)?;
                    writeln!(out, "if(!f->live[{slot}]) return 7;").unwrap();
                }
                for argument in arguments {
                    writeln!(out, "f->live[{}]=0;", self.slot(&argument.source)?).unwrap();
                }
            }
            CleanupTransition::SelectFailure { source } => {
                writeln!(
                    out,
                    "if(!status) status=spx_eval_{}(context,f).status;",
                    self.index(&source.expression)?
                )
                .unwrap();
            }
            _ => return Err(sdk_error("opaque owner cleanup transition is unsupported")),
        }
        Ok(())
    }
}
