use super::*;
use crate::ast::program_source::ProgramSource;
#[derive(Clone, Copy)]
enum Frame<'a> {
    Function(&'a dyn ProgramSource, &'a Function, usize),
    Expression(&'a dyn ProgramSource, &'a Expr, usize),
}

pub(crate) fn nonallocating(program: &dyn ProgramSource, target: &Function) -> bool {
    if !program.function_refs().contains_unaliased_payload(target) {
        return false;
    }
    nonallocating_with(
        program,
        target,
        |owner, name| {
            owner
                .function_refs()
                .unaliased_payload_by_name(name)
                .map(|f| (owner, f))
        },
        ast_param,
        super::super::ast_result_admitted,
        program
            .functions()
            .len()
            .saturating_add(program.module_uses().len()),
    )
}

/// The supplied lookup must authenticate actual provider bodies. Frames retain
/// that body's own source module, including for transitive parameter admission.
pub(crate) fn nonallocating_with<'a>(
    program: &'a dyn ProgramSource,
    target: &'a Function,
    lookup: impl Fn(&'a dyn ProgramSource, &str) -> Option<(&'a dyn ProgramSource, &'a Function)>,
    param_admit: impl Fn(&dyn ProgramSource, ParamMode, &Type) -> bool,
    result_admit: impl Fn(&dyn ProgramSource, &Type) -> bool,
    lookup_work: usize,
) -> bool {
    prove(
        program,
        target,
        &lookup,
        &param_admit,
        &result_admit,
        lookup_work,
    )
    .is_some()
}
fn prove<'a>(
    program: &'a dyn ProgramSource,
    target: &'a Function,
    lookup: &impl Fn(&'a dyn ProgramSource, &str) -> Option<(&'a dyn ProgramSource, &'a Function)>,
    param_admit: &impl Fn(&dyn ProgramSource, ParamMode, &Type) -> bool,
    result_admit: &impl Fn(&dyn ProgramSource, &Type) -> bool,
    lookup_work: usize,
) -> Option<()> {
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<
        [Option<Frame<'_>>; DEPTH],
    >()) {
        return None;
    }
    let mut cursor = Cursor::default();
    prove_with_cursor(
        program,
        target,
        lookup,
        param_admit,
        result_admit,
        lookup_work,
        &mut cursor,
    )
}
pub(super) struct Cursor<'a> {
    frames: [Option<Frame<'a>>; DEPTH],
}
impl Default for Cursor<'_> {
    fn default() -> Self {
        Self {
            frames: [None; DEPTH],
        }
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn prove_with_cursor<'a>(
    program: &'a dyn ProgramSource,
    target: &'a Function,
    lookup: &impl Fn(&'a dyn ProgramSource, &str) -> Option<(&'a dyn ProgramSource, &'a Function)>,
    param_admit: &impl Fn(&dyn ProgramSource, ParamMode, &Type) -> bool,
    result_admit: &impl Fn(&dyn ProgramSource, &Type) -> bool,
    lookup_work: usize,
    cursor: &mut Cursor<'a>,
) -> Option<()> {
    let frames = &mut cursor.frames;
    frames[0] = Some(Frame::Function(program, target, 0));
    let mut depth = 1usize;
    let mut work = WORK;
    while depth != 0 {
        work = work.checked_sub(1)?;
        let current = depth - 1;
        let child = match frames[current]? {
            Frame::Function(owner, function, root) => {
                if root == 0 {
                    work = work.checked_sub(
                        function
                            .params
                            .len()
                            .checked_add(1)?
                            .checked_mul(lookup_work)?,
                    )?;
                }
                if root == 0
                    && (!function.effects.is_empty()
                        || !function.type_parameters.is_empty()
                        || function.yields.is_some()
                        || function.follows.is_some()
                        || !function
                            .params
                            .iter()
                            .all(|p| param_admit(owner, p.mode, &p.ty))
                        || !(function.return_type == Type::Bytes
                            || result_admit(owner, &function.return_type)
                                && function.return_type != Type::String))
                {
                    return None;
                }
                let expression = if root < function.requires.len() {
                    function.requires.get(root)
                } else if root == function.requires.len() {
                    Some(&function.body)
                } else {
                    function.ensures.get(root - function.requires.len() - 1)
                };
                frames[current] = Some(Frame::Function(owner, function, root + 1));
                expression.map(|e| Frame::Expression(owner, e, usize::MAX))
            }
            Frame::Expression(owner, expression, next) => {
                if next == usize::MAX {
                    frames[current] = Some(Frame::Expression(owner, expression, 0));
                    match &expression.kind {
                        ExprKind::String(_)
                        | ExprKind::Closure { .. }
                        | ExprKind::MethodCall { .. }
                        | ExprKind::SuperMethod { .. }
                        | ExprKind::Yield { .. }
                        | ExprKind::Try { .. }
                        | ExprKind::LexicalExit { .. } => return None,
                        ExprKind::ConstructRecord {
                            type_name,
                            type_arguments,
                            ..
                        } => {
                            if !type_arguments.is_empty()
                                || !owner.types().iter().any(|d| {
                                    d.name == *type_name
                                        && crate::source_verify::copy_record_declaration_admitted(d)
                                })
                            {
                                return None;
                            }
                        }
                        ExprKind::ConstructVariant {
                            type_name,
                            type_arguments,
                            ..
                        } => {
                            if !copy_variant(owner, type_name, type_arguments) {
                                return None;
                            }
                        }
                        ExprKind::Block { statements, .. } => {
                            work = work.checked_sub(statements.len())?;
                            if statements
                                .iter()
                                .any(|s| matches!(s, crate::ast::Statement::Unsafe { .. }))
                            {
                                return None;
                            }
                        }
                        ExprKind::Call {
                            name,
                            type_arguments,
                            args,
                        } => {
                            if name == crate::vec_field::NAME {
                                if type_arguments.len() != 1
                                    || args.len() != 3
                                    || !matches!(&args[2].kind, ExprKind::String(field) if !field.is_empty())
                                {
                                    return None;
                                }
                            } else if let Some(op) = crate::byte_ops::by_name(name) {
                                if !byte_operation(op)
                                    || !type_arguments.is_empty()
                                    || args.len() != op.arity()
                                {
                                    return None;
                                }
                            } else if let Some(op) = crate::str_ops::by_name(name) {
                                if !borrowed_text_operation(op)
                                    || !type_arguments.is_empty()
                                    || args.len() != op.arity()
                                {
                                    return None;
                                }
                            } else if let Some(op) = crate::string_ops::by_name(name) {
                                if !text_operation(op) {
                                    return None;
                                }
                            } else if let Some(op) = crate::vec_ops::by_name(name) {
                                if !vec_operation(op) {
                                    return None;
                                }
                            } else {
                                work = work.checked_sub(lookup_work.checked_add(depth)?)?;
                                let (provider, callee) = lookup(owner, name)?;
                                if !type_arguments.is_empty() || args.len() != callee.params.len()
                                    || frames[..depth].iter().any(|f| matches!(f, Some(Frame::Function(_, active, _)) if active.stable_id == callee.stable_id)) { return None; }
                                frames[current] = Some(Frame::Expression(owner, expression, 0));
                                if depth == DEPTH {
                                    return None;
                                }
                                frames[depth] = Some(Frame::Function(provider, callee, 0));
                                depth += 1;
                                continue;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                // Block and Match child selection traverses an actual prefix.
                work = work.checked_sub(next.checked_add(1)?.checked_mul(2)?)?;
                frames[current] = Some(Frame::Expression(owner, expression, next + 1));
                if next == 2
                    && matches!(&expression.kind, ExprKind::Call { name, .. } if name == crate::vec_field::NAME)
                {
                    continue;
                }
                expression
                    .child(next)
                    .map(|e| Frame::Expression(owner, e, usize::MAX))
            }
        };
        if let Some(child) = child {
            if depth == DEPTH {
                return None;
            }
            frames[depth] = Some(child);
            depth += 1;
        } else {
            frames[current] = None;
            depth -= 1;
        }
    }
    Some(())
}

fn copy_variant(program: &dyn ProgramSource, name: &str, arguments: &[Type]) -> bool {
    if matches!(name, "Option" | "Result") {
        return arguments.len() == usize::from(name == "Result") + 1
            && arguments
                .iter()
                .all(crate::source_verify::is_scalar_source_type);
    }
    program.types().iter().any(|d| d.name == name && d.type_parameters.is_empty()
        && arguments.is_empty() && matches!(&d.kind, crate::ast::TypeDeclarationKind::Variant { cases }
            if cases.iter().all(|c| c.fields.iter().all(|f| crate::source_verify::is_scalar_source_type(&f.ty)))))
}
