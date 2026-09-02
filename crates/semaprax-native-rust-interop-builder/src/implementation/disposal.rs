//! Iterative disposal of a resolved program. The frame stack is
//! preallocated so teardown cannot grow the stack or the ledger.

use super::*;

pub(super) enum ResolvedDisposeFrame {
    ExprBox(Box<ResolvedExpr>),
    Exprs(Vec<ResolvedExpr>),
    Statements(Vec<ResolvedStatement>),
    Fields(Vec<crate::hir::ResolvedFieldInitializer>),
    Arms(Vec<crate::hir::ResolvedMatchArm>),
    RecordPatternFields(Vec<crate::hir::ResolvedRecordMatchPatternField>),
    VariantPatternFields(Vec<crate::hir::ResolvedMatchPatternField>),
    Type(ResolvedType),
    Types(Vec<ResolvedType>),
    Shape(semaprax::cleanup::FieldLivenessShape),
    Shapes(Vec<semaprax::cleanup::FieldLiveness>),
}

const _: () = assert!(std::mem::size_of::<ResolvedDisposeFrame>() == 56);

pub(super) struct ResolvedProgramOwner {
    program: Option<ResolvedProgram>,
    frames: Vec<ResolvedDisposeFrame>,
}

// Keeping the source owner inline preserves the already-censused HIR disposal
// allocation contract; boxing it would introduce a new unaccounted allocation.
#[allow(clippy::large_enum_variant)]
pub(super) enum PhaseAResolved<'a> {
    Source(ResolvedProgramOwner),
    Project(&'a ResolvedProgram),
}

impl PhaseAResolved<'_> {
    pub(super) fn program(&self) -> &ResolvedProgram {
        match self {
            Self::Source(owner) => owner.program(),
            Self::Project(program) => program,
        }
    }
}

impl ResolvedProgramOwner {
    pub(super) fn new(
        program: ResolvedProgram,
        frames: Vec<ResolvedDisposeFrame>,
        capacity: usize,
    ) -> Self {
        if frames.capacity() != capacity || !frames.is_empty() {
            std::process::abort();
        }
        note_resolved_dispose_capacity(0, capacity);
        Self {
            program: Some(program),
            frames,
        }
    }

    pub(super) fn program(&self) -> &ResolvedProgram {
        self.program.as_ref().expect("resolved program retained")
    }
}

fn disposal_push(frames: &mut Vec<ResolvedDisposeFrame>, frame: ResolvedDisposeFrame) {
    if frames.len() == frames.capacity() {
        // The owner is created only after the admitted-depth census reserved
        // this fixed workspace. Exhaustion is an internal invariant failure;
        // aborting avoids both recursive fallback and allocation during Drop.
        std::process::abort();
    }
    frames.push(frame);
    note_resolved_dispose_high_water(frames.len());
}

impl Drop for ResolvedProgramOwner {
    fn drop(&mut self) {
        let Some(program) = self.program.take() else {
            return;
        };
        let ResolvedProgram {
            module,
            permits,
            entrypoint,
            declarations,
            types,
            interfaces,
            function_templates,
            functions,
            function_instances,
        } = program;
        // Scalars, strings, declarations, and non-recursive declaration
        // containers may drop directly after every recursive HIR tree has
        // been moved into the preallocated disposal machine.
        for interface in interfaces {
            for import in interface.imports {
                for parameter in import.parameters {
                    disposal_push(&mut self.frames, ResolvedDisposeFrame::Type(parameter.ty));
                    drain_disposal_frames(&mut self.frames, None);
                }
            }
        }
        crate::hir::dispose_declaration_index_for_private_contract(declarations, |ty| {
            disposal_push(&mut self.frames, ResolvedDisposeFrame::Type(ty));
            drain_disposal_frames(&mut self.frames, None);
        });
        drop((module, permits, entrypoint));
        for declaration in types {
            match declaration.kind {
                crate::hir::ResolvedTypeDeclarationKind::Resource { .. } => {}
                crate::hir::ResolvedTypeDeclarationKind::Record { fields }
                | crate::hir::ResolvedTypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        disposal_push(&mut self.frames, ResolvedDisposeFrame::Type(field.ty));
                        drain_disposal_frames(&mut self.frames, None);
                    }
                }
                crate::hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        for field in case.fields {
                            disposal_push(&mut self.frames, ResolvedDisposeFrame::Type(field.ty));
                            drain_disposal_frames(&mut self.frames, None);
                        }
                    }
                }
            }
        }
        for template in function_templates {
            disposal_push(
                &mut self.frames,
                ResolvedDisposeFrame::Type(template.return_type),
            );
            drain_disposal_frames(&mut self.frames, None);
            disposal_push(
                &mut self.frames,
                ResolvedDisposeFrame::Exprs(template.requires),
            );
            drain_disposal_frames(&mut self.frames, None);
            disposal_push(
                &mut self.frames,
                ResolvedDisposeFrame::Exprs(template.ensures),
            );
            drain_disposal_frames(&mut self.frames, None);
            for parameter in template.params {
                disposal_push(&mut self.frames, ResolvedDisposeFrame::Type(parameter.ty));
                drain_disposal_frames(&mut self.frames, None);
            }
            drain_disposal_frames(&mut self.frames, Some(template.body));
        }
        for function in functions {
            push_function_for_disposal(&mut self.frames, function);
        }
        for instance in function_instances {
            disposal_push(
                &mut self.frames,
                ResolvedDisposeFrame::Types(instance.type_arguments),
            );
            drain_disposal_frames(&mut self.frames, None);
            push_function_for_disposal(&mut self.frames, instance.function);
        }
        drain_disposal_frames(&mut self.frames, None);
        note_resolved_dispose_capacity(1, self.frames.capacity());
        note_resolved_dispose_completion();
    }
}

fn push_function_for_disposal(frames: &mut Vec<ResolvedDisposeFrame>, function: ResolvedFunction) {
    disposal_push(frames, ResolvedDisposeFrame::Type(function.return_type));
    drain_disposal_frames(frames, None);
    disposal_push(frames, ResolvedDisposeFrame::Exprs(function.requires));
    drain_disposal_frames(frames, None);
    disposal_push(frames, ResolvedDisposeFrame::Exprs(function.ensures));
    drain_disposal_frames(frames, None);
    for parameter in function.params {
        disposal_push(frames, ResolvedDisposeFrame::Type(parameter.ty));
        drain_disposal_frames(frames, None);
    }
    for slot in function.cleanup.slots {
        disposal_push(frames, ResolvedDisposeFrame::Type(slot.ty));
        drain_disposal_frames(frames, None);
        disposal_push(frames, ResolvedDisposeFrame::Shape(slot.shape));
        drain_disposal_frames(frames, None);
    }
    for slot in function.cleanup_plan.slots {
        disposal_push(frames, ResolvedDisposeFrame::Type(slot.ty));
        drain_disposal_frames(frames, None);
        disposal_push(
            frames,
            ResolvedDisposeFrame::Shape(slot.field_liveness_shape),
        );
        drain_disposal_frames(frames, None);
    }
    for block in function.cleanup_plan.blocks {
        for transition in block.transitions {
            if let crate::cleanup_plan::CleanupTransition::StageCopyResult { source } = transition {
                match source {
                    crate::cleanup_plan::StagedCopyResultSource::Body { instance, .. } => {
                        disposal_push(frames, ResolvedDisposeFrame::Type(instance));
                        drain_disposal_frames(frames, None);
                    }
                    crate::cleanup_plan::StagedCopyResultSource::TryResidual {
                        source_instance,
                        target_instance,
                        ..
                    }
                    | crate::cleanup_plan::StagedCopyResultSource::TryOptionNone {
                        source_instance,
                        target_instance,
                        ..
                    } => {
                        disposal_push(frames, ResolvedDisposeFrame::Type(source_instance));
                        drain_disposal_frames(frames, None);
                        disposal_push(frames, ResolvedDisposeFrame::Type(target_instance));
                        drain_disposal_frames(frames, None);
                    }
                }
            }
        }
    }
    drain_disposal_frames(frames, Some(function.body));
}

fn drain_disposal_frames(
    frames: &mut Vec<ResolvedDisposeFrame>,
    mut pending_expression: Option<ResolvedExpr>,
) {
    loop {
        if let Some(expression) = pending_expression.take() {
            disposal_push(frames, ResolvedDisposeFrame::Type(expression.ty));
            match expression.kind {
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::ArrayU8(_)
                | ResolvedExprKind::RepeatArrayU8 { .. }
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_)
                | ResolvedExprKind::String(_)
                | ResolvedExprKind::Place(_)
                | ResolvedExprKind::BorrowPlace { .. } => {}
                ResolvedExprKind::ByteRange {
                    source, start, end, ..
                } => {
                    disposal_push(frames, ResolvedDisposeFrame::ExprBox(end));
                    disposal_push(frames, ResolvedDisposeFrame::ExprBox(start));
                    pending_expression = Some(*source);
                }
                ResolvedExprKind::Call {
                    type_arguments,
                    args,
                    ..
                } => {
                    disposal_push(frames, ResolvedDisposeFrame::Types(type_arguments));
                    disposal_push(frames, ResolvedDisposeFrame::Exprs(args));
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    disposal_push(frames, ResolvedDisposeFrame::Exprs(call.args));
                }
                ResolvedExprKind::HostCommandCall(call) => {
                    disposal_push(frames, ResolvedDisposeFrame::Exprs(call.args));
                }
                ResolvedExprKind::Unary { value, .. }
                | ResolvedExprKind::Project { base: value, .. }
                | ResolvedExprKind::Upcast { source: value } => {
                    pending_expression = Some(*value);
                }
                ResolvedExprKind::Binary { left, right, .. } => {
                    disposal_push(frames, ResolvedDisposeFrame::ExprBox(right));
                    pending_expression = Some(*left);
                }
                ResolvedExprKind::Block { statements, tail } => {
                    disposal_push(frames, ResolvedDisposeFrame::ExprBox(tail));
                    disposal_push(frames, ResolvedDisposeFrame::Statements(statements));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    disposal_push(frames, ResolvedDisposeFrame::ExprBox(else_branch));
                    disposal_push(frames, ResolvedDisposeFrame::ExprBox(then_branch));
                    pending_expression = Some(*condition);
                }
                ResolvedExprKind::ConstructRecord { fields, .. }
                | ResolvedExprKind::ConstructVariant { fields, .. } => {
                    disposal_push(frames, ResolvedDisposeFrame::Fields(fields));
                }
                ResolvedExprKind::Match {
                    scrutinee, arms, ..
                } => {
                    disposal_push(frames, ResolvedDisposeFrame::Arms(arms));
                    pending_expression = Some(*scrutinee);
                }
                ResolvedExprKind::Try {
                    operand,
                    residual_type,
                    ..
                }
                | ResolvedExprKind::TryOption {
                    operand,
                    residual_type,
                    ..
                } => {
                    disposal_push(frames, ResolvedDisposeFrame::Type(residual_type));
                    pending_expression = Some(*operand);
                }
                ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                    disposal_push(frames, ResolvedDisposeFrame::Fields(fields));
                    pending_expression = Some(*base);
                }
            }
            continue;
        }
        let Some(frame) = frames.pop() else { break };
        match frame {
            ResolvedDisposeFrame::ExprBox(expression) => pending_expression = Some(*expression),
            ResolvedDisposeFrame::Exprs(mut expressions) => {
                if let Some(expression) = expressions.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Exprs(expressions));
                    pending_expression = Some(expression);
                }
            }
            ResolvedDisposeFrame::Statements(mut statements) => {
                if let Some(statement) = statements.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Statements(statements));
                    match statement {
                        crate::hir::ResolvedStatement::Let { binding, value, .. }
                        | crate::hir::ResolvedStatement::Assign { binding, value, .. } => {
                            disposal_push(frames, ResolvedDisposeFrame::Type(binding.ty));
                            pending_expression = Some(value);
                        }
                        crate::hir::ResolvedStatement::Unsafe { body, .. } => {
                            pending_expression = Some(*body);
                        }
                        crate::hir::ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            disposal_push(frames, ResolvedDisposeFrame::ExprBox(body));
                            pending_expression = Some(*condition);
                        }
                    }
                }
            }
            ResolvedDisposeFrame::Fields(mut fields) => {
                if let Some(field) = fields.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Fields(fields));
                    pending_expression = Some(field.value);
                }
            }
            ResolvedDisposeFrame::Arms(mut arms) => {
                if let Some(arm) = arms.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Arms(arms));
                    dispose_match_pattern(frames, arm.pattern);
                    if let Some(guard) = arm.guard {
                        disposal_push(frames, ResolvedDisposeFrame::ExprBox(guard));
                    }
                    pending_expression = Some(arm.value);
                }
            }
            ResolvedDisposeFrame::RecordPatternFields(mut fields) => {
                if let Some(field) = fields.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::RecordPatternFields(fields));
                    match field.pattern {
                        crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                            disposal_push(frames, ResolvedDisposeFrame::Type(binding.ty));
                        }
                        crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
                        crate::hir::ResolvedRecordMatchFieldPattern::Record {
                            instance,
                            fields,
                            ..
                        } => {
                            disposal_push(frames, ResolvedDisposeFrame::Type(instance));
                            disposal_push(
                                frames,
                                ResolvedDisposeFrame::RecordPatternFields(fields),
                            );
                        }
                    }
                }
            }
            ResolvedDisposeFrame::VariantPatternFields(mut fields) => {
                if let Some(field) = fields.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::VariantPatternFields(fields));
                    disposal_push(frames, ResolvedDisposeFrame::Type(field.binding.ty));
                }
            }
            ResolvedDisposeFrame::Type(ty) => {
                if let ResolvedType::Nominal { arguments, .. } = ty {
                    disposal_push(frames, ResolvedDisposeFrame::Types(arguments));
                }
            }
            ResolvedDisposeFrame::Types(mut types) => {
                if let Some(ty) = types.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Types(types));
                    disposal_push(frames, ResolvedDisposeFrame::Type(ty));
                }
            }
            ResolvedDisposeFrame::Shape(shape) => {
                if let semaprax::cleanup::FieldLivenessShape::Record { fields, .. } = shape {
                    disposal_push(frames, ResolvedDisposeFrame::Shapes(fields));
                }
            }
            ResolvedDisposeFrame::Shapes(mut shapes) => {
                if let Some(field) = shapes.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Shapes(shapes));
                    disposal_push(frames, ResolvedDisposeFrame::Shape(field.shape));
                }
            }
        }
    }
}

fn dispose_match_pattern(
    frames: &mut Vec<ResolvedDisposeFrame>,
    pattern: crate::hir::ResolvedMatchPattern,
) {
    match pattern {
        crate::hir::ResolvedMatchPattern::Wildcard => {}
        crate::hir::ResolvedMatchPattern::Variant { fields, .. } => {
            disposal_push(frames, ResolvedDisposeFrame::VariantPatternFields(fields));
        }
        crate::hir::ResolvedMatchPattern::Record {
            instance, fields, ..
        } => {
            disposal_push(frames, ResolvedDisposeFrame::Type(instance));
            disposal_push(frames, ResolvedDisposeFrame::RecordPatternFields(fields));
        }
        // Refutable Match v1: a binding arm owns the Copy scrutinee; literal
        // and or-patterns own nothing.
        crate::hir::ResolvedMatchPattern::Binding(_) => {}
        crate::hir::ResolvedMatchPattern::Literal(_) | crate::hir::ResolvedMatchPattern::Or(_) => {}
    }
}
