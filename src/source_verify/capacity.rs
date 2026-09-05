//! Source-level byte-data storage, call-path, and transcript capacity projection.

use super::*;

mod command_io;

pub(super) fn source_capacity_functions(program: &Program) -> Vec<(Option<&str>, &Function)> {
    let mut functions = program
        .functions
        .iter()
        .map(|function| (None, function))
        .collect::<Vec<_>>();
    for declaration in &program.types {
        if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
            functions.extend(
                methods
                    .iter()
                    .map(|method| (Some(declaration.name.as_str()), method)),
            );
        }
    }
    functions
}

pub(super) struct SourceCapacityContext<'a> {
    pub(super) types: &'a TypeTable<'a>,
    pub(super) ordinary: &'a BTreeMap<&'a str, &'a Function>,
    pub(super) enclosing_class: Option<&'a str>,
}

struct SourceCapacityScope {
    bindings: BTreeMap<String, Type>,
    transcript_roots: BTreeMap<String, crate::byte_data_capacity::TranscriptSource>,
}

enum SourceTranscriptRoots<'a> {
    Borrowed(&'a BTreeMap<String, crate::byte_data_capacity::TranscriptSource>),
    Owned(BTreeMap<String, crate::byte_data_capacity::TranscriptSource>),
}

struct SourceTranscriptScope<'a> {
    roots: SourceTranscriptRoots<'a>,
}

#[cfg(test)]
std::thread_local! {
    static SOURCE_CAPACITY_LIVE_SCOPES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_CAPACITY_PEAK_SCOPES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_TRANSCRIPT_LIVE_SCOPES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_TRANSCRIPT_PEAK_SCOPES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_TRANSCRIPT_OWNED_MAP_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_TRANSCRIPT_LIVE_FRAME_PEAK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_TRANSCRIPT_ROOT_REF_PEAK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_CAPACITY_MATCH_NEXT_FRAME_PEAK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_CAPACITY_MATCH_NEXT_PATH_PEAK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_CAPACITY_MATCH_NEXT_TYPE_PEAK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_TYPE_SCOPE_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_TYPE_SCOPE_COPIED_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl<'a> SourceTranscriptScope<'a> {
    fn borrowed(roots: &'a BTreeMap<String, crate::byte_data_capacity::TranscriptSource>) -> Self {
        Self::new(SourceTranscriptRoots::Borrowed(roots))
    }

    fn owned(roots: BTreeMap<String, crate::byte_data_capacity::TranscriptSource>) -> Self {
        #[cfg(test)]
        SOURCE_TRANSCRIPT_OWNED_MAP_ALLOCATIONS.with(|allocations| {
            allocations.set(
                allocations
                    .get()
                    .checked_add(1)
                    .expect("owned transcript map allocation count overflow"),
            );
        });
        Self::new(SourceTranscriptRoots::Owned(roots))
    }

    fn new(roots: SourceTranscriptRoots<'a>) -> Self {
        #[cfg(test)]
        SOURCE_TRANSCRIPT_LIVE_SCOPES.with(|live| {
            let next = live
                .get()
                .checked_add(1)
                .expect("live transcript scope count overflow");
            live.set(next);
            SOURCE_TRANSCRIPT_PEAK_SCOPES.with(|peak| peak.set(peak.get().max(next)));
        });
        Self { roots }
    }

    fn roots(&self) -> &BTreeMap<String, crate::byte_data_capacity::TranscriptSource> {
        match &self.roots {
            SourceTranscriptRoots::Borrowed(roots) => roots,
            SourceTranscriptRoots::Owned(roots) => roots,
        }
    }

    fn roots_mut(&mut self) -> &mut BTreeMap<String, crate::byte_data_capacity::TranscriptSource> {
        let SourceTranscriptRoots::Owned(roots) = &mut self.roots else {
            unreachable!("only an owned block transcript scope may be mutated");
        };
        roots
    }
}

impl SourceCapacityScope {
    fn new(
        bindings: BTreeMap<String, Type>,
        transcript_roots: BTreeMap<String, crate::byte_data_capacity::TranscriptSource>,
    ) -> Self {
        #[cfg(test)]
        SOURCE_CAPACITY_LIVE_SCOPES.with(|live| {
            let next = live
                .get()
                .checked_add(1)
                .expect("live scope count overflow");
            live.set(next);
            SOURCE_CAPACITY_PEAK_SCOPES.with(|peak| peak.set(peak.get().max(next)));
        });
        Self {
            bindings,
            transcript_roots,
        }
    }
}

#[cfg(test)]
impl Drop for SourceCapacityScope {
    fn drop(&mut self) {
        SOURCE_CAPACITY_LIVE_SCOPES.with(|live| {
            live.set(
                live.get()
                    .checked_sub(1)
                    .expect("live scope count underflow"),
            );
        });
    }
}

#[cfg(test)]
impl Drop for SourceTranscriptScope<'_> {
    fn drop(&mut self) {
        SOURCE_TRANSCRIPT_LIVE_SCOPES.with(|live| {
            live.set(
                live.get()
                    .checked_sub(1)
                    .expect("live transcript scope count underflow"),
            );
        });
    }
}

#[cfg(test)]
pub(super) fn reset_source_capacity_scope_peak() {
    SOURCE_CAPACITY_LIVE_SCOPES.with(|live| assert_eq!(live.get(), 0));
    SOURCE_CAPACITY_PEAK_SCOPES.with(|peak| peak.set(0));
    SOURCE_CAPACITY_MATCH_NEXT_FRAME_PEAK.with(|peak| peak.set(0));
    SOURCE_CAPACITY_MATCH_NEXT_PATH_PEAK.with(|peak| peak.set(0));
    SOURCE_CAPACITY_MATCH_NEXT_TYPE_PEAK.with(|peak| peak.set(0));
    SOURCE_TYPE_SCOPE_ALLOCATIONS.with(|count| count.set(0));
    SOURCE_TYPE_SCOPE_COPIED_ENTRIES.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn source_capacity_scope_peak() -> usize {
    SOURCE_CAPACITY_PEAK_SCOPES.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn source_capacity_scope_live() -> usize {
    SOURCE_CAPACITY_LIVE_SCOPES.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn source_capacity_match_next_scratch_peak() -> (usize, usize, usize) {
    (
        SOURCE_CAPACITY_MATCH_NEXT_FRAME_PEAK.with(std::cell::Cell::get),
        SOURCE_CAPACITY_MATCH_NEXT_PATH_PEAK.with(std::cell::Cell::get),
        SOURCE_CAPACITY_MATCH_NEXT_TYPE_PEAK.with(std::cell::Cell::get),
    )
}

#[cfg(test)]
pub(super) fn source_type_scope_copy_totals() -> (usize, usize) {
    (
        SOURCE_TYPE_SCOPE_ALLOCATIONS.with(std::cell::Cell::get),
        SOURCE_TYPE_SCOPE_COPIED_ENTRIES.with(std::cell::Cell::get),
    )
}

#[cfg(test)]
pub(super) fn reset_source_transcript_scope_peak() {
    SOURCE_TRANSCRIPT_LIVE_SCOPES.with(|live| assert_eq!(live.get(), 0));
    SOURCE_TRANSCRIPT_PEAK_SCOPES.with(|peak| peak.set(0));
    SOURCE_TRANSCRIPT_OWNED_MAP_ALLOCATIONS.with(|allocations| allocations.set(0));
    SOURCE_TRANSCRIPT_LIVE_FRAME_PEAK.with(|peak| peak.set(0));
    SOURCE_TRANSCRIPT_ROOT_REF_PEAK.with(|peak| peak.set(0));
}

#[cfg(test)]
pub(super) fn source_transcript_scope_peak() -> usize {
    SOURCE_TRANSCRIPT_PEAK_SCOPES.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn source_transcript_scope_live() -> usize {
    SOURCE_TRANSCRIPT_LIVE_SCOPES.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn source_transcript_owned_map_allocations() -> usize {
    SOURCE_TRANSCRIPT_OWNED_MAP_ALLOCATIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn source_transcript_frame_scratch_peak() -> (usize, usize) {
    (
        SOURCE_TRANSCRIPT_LIVE_FRAME_PEAK.with(std::cell::Cell::get),
        SOURCE_TRANSCRIPT_ROOT_REF_PEAK.with(std::cell::Cell::get),
    )
}

fn source_array_payload(types: &TypeTable<'_>, ty: &Type) -> Result<u32, ()> {
    let mut total = 0_u32;
    let mut pending = vec![ty.clone()];
    let mut expanded = 0_usize;
    while let Some(ty) = pending.pop() {
        expanded = expanded.checked_add(1).ok_or(())?;
        if expanded > 65_536 {
            return Err(());
        }
        match ty {
            Type::ArrayU8(length) => total = total.checked_add(length).ok_or(())?,
            Type::Named { .. } => {
                if let Some(fields) = types.record_fields(&ty) {
                    for field in fields.iter().rev() {
                        pending.push(types.record_field_type(&ty, field).ok_or(())?);
                    }
                }
            }
            Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::String
            | Type::Bytes
            | Type::Str
            | Type::SliceU8 => {}
        }
    }
    Ok(total)
}

fn source_capacity_super_method<'a>(
    context: &SourceCapacityContext<'a>,
    method: &str,
) -> Option<&'a Function> {
    let class = context.enclosing_class?;
    let declaration = context.types.declaration(class)?;
    let Type::Named { name: parent, .. } = declaration.extends.as_ref()? else {
        return None;
    };
    resolve_class_method(context.types, parent, method).map(|(_, function)| function)
}

pub(super) fn source_capacity_expr_type(
    expression: &Expr,
    bindings: &BTreeMap<String, Type>,
    context: &SourceCapacityContext<'_>,
) -> Option<Type> {
    type OwnedBindings = std::rc::Rc<std::cell::RefCell<BTreeMap<String, Type>>>;
    enum Bindings<'a> {
        Borrowed(&'a BTreeMap<String, Type>),
        Owned(OwnedBindings),
    }

    impl Bindings<'_> {
        fn cloned_map(&self) -> BTreeMap<String, Type> {
            match self {
                Self::Borrowed(bindings) => (*bindings).clone(),
                Self::Owned(bindings) => bindings.borrow().clone(),
            }
        }
    }

    enum Continuation<'a> {
        Method(&'a str),
        Project(&'a str),
        Block {
            statements: &'a [Statement],
            next: usize,
            tail: &'a Expr,
            bindings: OwnedBindings,
            pending_name: Option<&'a str>,
        },
    }

    let immediate = |expression: &Expr, bindings: &BTreeMap<String, Type>| match &expression.kind {
        ExprKind::Int(_) => Some(Type::I64),
        ExprKind::Int32(_) => Some(Type::I32),
        ExprKind::Char(_) => Some(Type::Char),
        ExprKind::Uint8(_) => Some(Type::U8),
        ExprKind::Usize(_) => Some(Type::Usize),
        ExprKind::ArrayU8(values) => u32::try_from(values.len()).ok().map(Type::ArrayU8),
        ExprKind::RepeatArrayU8 { count, .. } => Some(Type::ArrayU8(*count)),
        ExprKind::Float32(_) => Some(Type::F32),
        ExprKind::Float64(_) => Some(Type::F64),
        ExprKind::Bool(_) => Some(Type::Bool),
        ExprKind::String(_) => Some(Type::String),
        ExprKind::Var(name) => bindings.get(name).cloned(),
        ExprKind::Call { name, .. } => crate::byte_ops::by_name(name)
            .map(crate::byte_ops::ByteOp::ast_return_type)
            .or_else(|| {
                crate::host_io_ops::by_name(name).map(crate::host_io_ops::HostIoOp::ast_return_type)
            })
            .or_else(|| {
                crate::command_io_ops::by_name(name).map(crate::command_io_ops::ast_return_type)
            })
            .or_else(|| {
                context
                    .ordinary
                    .get(name.as_str())
                    .map(|item| item.return_type.clone())
            }),
        ExprKind::MethodCall { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Try { .. }
        | ExprKind::Binary { .. }
        | ExprKind::If { .. }
        | ExprKind::UpdateRecord { .. }
        | ExprKind::Project { .. }
        | ExprKind::Block { .. }
        | ExprKind::Match { .. } => None,
        ExprKind::SuperMethod { method, .. } => source_capacity_super_method(context, method)
            .map(|function| function.return_type.clone()),
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            ..
        } => Some(Type::Named {
            name: type_name.clone(),
            arguments: type_arguments.clone(),
        }),
    };

    let mut current = expression;
    let mut current_bindings = Bindings::Borrowed(bindings);
    let mut continuations = Vec::new();
    let mut result;
    loop {
        match &current.kind {
            ExprKind::MethodCall {
                receiver, method, ..
            } => {
                continuations.push(Continuation::Method(method));
                current = receiver;
                continue;
            }
            ExprKind::Unary { value, .. } | ExprKind::Try { operand: value } => {
                current = value;
                continue;
            }
            ExprKind::Binary { op, left, .. } => {
                result = match op {
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
                    | BinaryOp::And
                    | BinaryOp::Or => Some(Type::Bool),
                    _ => {
                        current = left;
                        continue;
                    }
                };
            }
            ExprKind::If { then_branch, .. } => {
                current = then_branch;
                continue;
            }
            ExprKind::UpdateRecord { base, .. } => {
                current = base;
                continue;
            }
            ExprKind::Project { base, field, .. } => {
                continuations.push(Continuation::Project(field));
                current = base;
                continue;
            }
            ExprKind::Block { statements, tail } => {
                let copied = current_bindings.cloned_map();
                #[cfg(test)]
                {
                    SOURCE_TYPE_SCOPE_ALLOCATIONS.with(|count| {
                        count.set(
                            count
                                .get()
                                .checked_add(1)
                                .expect("scope allocation overflow"),
                        );
                    });
                    SOURCE_TYPE_SCOPE_COPIED_ENTRIES.with(|count| {
                        count.set(
                            count
                                .get()
                                .checked_add(copied.len())
                                .expect("copied scope entry count overflow"),
                        );
                    });
                }
                continuations.push(Continuation::Block {
                    statements,
                    next: 0,
                    tail,
                    bindings: std::rc::Rc::new(std::cell::RefCell::new(copied)),
                    pending_name: None,
                });
                result = None;
            }
            ExprKind::Match { arms, .. } => {
                let arm = arms.first()?;
                current = &arm.value;
                continue;
            }
            _ => {
                result = match &current_bindings {
                    Bindings::Borrowed(bindings) => immediate(current, bindings),
                    Bindings::Owned(bindings) => immediate(current, &bindings.borrow()),
                }
            }
        }

        loop {
            let Some(continuation) = continuations.pop() else {
                return result;
            };
            match continuation {
                Continuation::Method(method) => {
                    result = result.and_then(|ty| {
                        let Type::Named { name, .. } = ty else {
                            return None;
                        };
                        resolve_class_method(context.types, &name, method)
                            .map(|(_, function)| function.return_type.clone())
                    });
                }
                Continuation::Project(field) => {
                    result = result.and_then(|base_ty| {
                        let declaration = context
                            .types
                            .record_fields(&base_ty)?
                            .iter()
                            .find(|candidate| candidate.name == field)?;
                        context.types.record_field_type(&base_ty, declaration)
                    });
                }
                Continuation::Block {
                    statements,
                    mut next,
                    tail,
                    bindings,
                    pending_name,
                } => {
                    if let (Some(name), Some(ty)) = (pending_name, result.take()) {
                        bindings.borrow_mut().insert(name.to_owned(), ty);
                    }
                    let mut pending = None;
                    while let Some(statement) = statements.get(next) {
                        next += 1;
                        let Statement::Let {
                            name,
                            declared,
                            value,
                            ..
                        } = statement
                        else {
                            continue;
                        };
                        if let Some(ty) = declared {
                            bindings.borrow_mut().insert(name.clone(), ty.clone());
                            continue;
                        }
                        pending = Some((name.as_str(), value));
                        break;
                    }
                    if let Some((name, value)) = pending {
                        continuations.push(Continuation::Block {
                            statements,
                            next,
                            tail,
                            bindings: std::rc::Rc::clone(&bindings),
                            pending_name: Some(name),
                        });
                        current = value;
                        current_bindings = Bindings::Owned(bindings);
                        break;
                    }
                    current = tail;
                    current_bindings = Bindings::Owned(bindings);
                    break;
                }
            }
        }
    }
}

fn source_capacity_slot(
    slots: &mut Vec<crate::byte_data_capacity::ArrayStorageSlot>,
    types: &TypeTable<'_>,
    identity: String,
    kind: crate::byte_data_capacity::ArrayStorageKind,
    ty: &Type,
) -> Result<(), ()> {
    let length = source_array_payload(types, ty)?;
    if length != 0 || matches!(ty, Type::ArrayU8(0)) {
        slots.push(crate::byte_data_capacity::ArrayStorageSlot {
            identity,
            kind,
            length,
        });
    }
    Ok(())
}

pub(super) fn source_transcript_source_from_roots(
    expression: &Expr,
    roots: &BTreeMap<String, crate::byte_data_capacity::TranscriptSource>,
) -> crate::byte_data_capacity::TranscriptSource {
    use crate::byte_data_capacity::TranscriptSource;
    type RootsRef<'roots> = std::rc::Rc<std::cell::RefCell<SourceTranscriptScope<'roots>>>;
    enum Frame<'expr, 'roots> {
        Visit(&'expr Expr, RootsRef<'roots>),
        If,
        MatchNext {
            arms: &'expr [crate::ast::MatchArm],
            next: usize,
            roots: RootsRef<'roots>,
            source: Option<TranscriptSource>,
        },
        Block {
            statements: &'expr [Statement],
            next: usize,
            tail: &'expr Expr,
            roots: RootsRef<'roots>,
            declared: BTreeSet<String>,
        },
        BlockLet {
            name: &'expr str,
            statements: &'expr [Statement],
            next: usize,
            tail: &'expr Expr,
            roots: RootsRef<'roots>,
            declared: BTreeSet<String>,
        },
    }

    let mut frames = vec![Frame::Visit(
        expression,
        std::rc::Rc::new(std::cell::RefCell::new(SourceTranscriptScope::borrowed(
            roots,
        ))),
    )];
    let mut results = Vec::new();
    while let Some(frame) = frames.pop() {
        #[cfg(test)]
        {
            let live_frames = frames.len().saturating_add(1);
            let root_refs = frames
                .iter()
                .chain(std::iter::once(&frame))
                .filter(|frame| {
                    matches!(
                        frame,
                        Frame::Visit(_, _)
                            | Frame::MatchNext { .. }
                            | Frame::Block { .. }
                            | Frame::BlockLet { .. }
                    )
                })
                .count();
            SOURCE_TRANSCRIPT_LIVE_FRAME_PEAK.with(|peak| peak.set(peak.get().max(live_frames)));
            SOURCE_TRANSCRIPT_ROOT_REF_PEAK.with(|peak| peak.set(peak.get().max(root_refs)));
        }
        match frame {
            Frame::Visit(expression, roots) => match &expression.kind {
                ExprKind::Var(name) => results.push(
                    roots
                        .borrow()
                        .roots()
                        .get(name)
                        .copied()
                        .unwrap_or(TranscriptSource::Unknown),
                ),
                ExprKind::ArrayU8(values) => results.push(
                    u64::try_from(values.len())
                        .map(TranscriptSource::Fixed)
                        .unwrap_or(TranscriptSource::Unknown),
                ),
                ExprKind::RepeatArrayU8 { count, .. } => {
                    results.push(TranscriptSource::Fixed(u64::from(*count)));
                }
                ExprKind::String(value) => results.push(
                    u64::try_from(value.len())
                        .map(TranscriptSource::Fixed)
                        .unwrap_or(TranscriptSource::Unknown),
                ),
                ExprKind::Call { name, .. } if name == crate::command_io_ops::ARG_UTF8_NAME => {
                    results.push(TranscriptSource::CommandArguments);
                }
                ExprKind::Call { name, .. } if name == crate::command_io_ops::STDIN_READ_NAME => {
                    results.push(TranscriptSource::Stdin);
                }
                ExprKind::Call { name, args, .. }
                    if matches!(
                        name.as_str(),
                        crate::byte_ops::ARRAY_AS_SLICE_NAME
                            | crate::byte_ops::STR_AS_BYTES_NAME
                            | crate::byte_ops::BYTES_AS_SLICE_NAME
                    ) =>
                {
                    if let Some(argument) = args.first() {
                        frames.push(Frame::Visit(argument, roots));
                    } else {
                        results.push(TranscriptSource::Unknown);
                    }
                }
                ExprKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    frames.push(Frame::If);
                    frames.push(Frame::Visit(else_branch, std::rc::Rc::clone(&roots)));
                    frames.push(Frame::Visit(then_branch, roots));
                }
                ExprKind::Block { statements, tail } => {
                    let local = {
                        let roots = roots.borrow();
                        std::rc::Rc::new(std::cell::RefCell::new(SourceTranscriptScope::owned(
                            roots.roots().clone(),
                        )))
                    };
                    frames.push(Frame::Block {
                        statements,
                        next: 0,
                        tail,
                        roots: local,
                        declared: BTreeSet::new(),
                    });
                }
                ExprKind::Match { arms, .. } => {
                    frames.push(Frame::MatchNext {
                        arms,
                        next: 0,
                        roots,
                        source: None,
                    });
                }
                _ => results.push(TranscriptSource::Unknown),
            },
            Frame::If => {
                let else_source = results.pop().unwrap_or(TranscriptSource::Unknown);
                let then_source = results.pop().unwrap_or(TranscriptSource::Unknown);
                results.push(if then_source == else_source {
                    then_source
                } else {
                    TranscriptSource::Unknown
                });
            }
            Frame::MatchNext {
                arms,
                next,
                roots,
                mut source,
            } => {
                if next != 0 {
                    let candidate = results.pop().unwrap_or(TranscriptSource::Unknown);
                    match source {
                        None => source = Some(candidate),
                        Some(previous) if previous == candidate => {}
                        Some(_) => {
                            results.push(TranscriptSource::Unknown);
                            continue;
                        }
                    }
                }
                let Some(arm) = arms.get(next) else {
                    results.push(source.unwrap_or(TranscriptSource::Unknown));
                    continue;
                };
                frames.push(Frame::MatchNext {
                    arms,
                    next: next + 1,
                    roots: std::rc::Rc::clone(&roots),
                    source,
                });
                frames.push(Frame::Visit(&arm.value, roots));
            }
            Frame::Block {
                statements,
                mut next,
                tail,
                roots,
                declared,
            } => {
                let mut pending = None;
                while let Some(statement) = statements.get(next) {
                    next += 1;
                    match statement {
                        Statement::Let { name, value, .. } => {
                            pending = Some((name.as_str(), value));
                            break;
                        }
                        Statement::Assign { name, .. } => {
                            roots
                                .borrow_mut()
                                .roots_mut()
                                .insert(name.clone(), TranscriptSource::Unknown);
                        }
                        Statement::Unsafe { .. } | Statement::While { .. } => {}
                    }
                }
                if let Some((name, value)) = pending {
                    frames.push(Frame::BlockLet {
                        name,
                        statements,
                        next,
                        tail,
                        roots: roots.clone(),
                        declared,
                    });
                    frames.push(Frame::Visit(value, roots));
                } else {
                    frames.push(Frame::Visit(tail, roots));
                }
            }
            Frame::BlockLet {
                name,
                statements,
                next,
                tail,
                roots,
                mut declared,
            } => {
                let source = results.pop().unwrap_or(TranscriptSource::Unknown);
                roots.borrow_mut().roots_mut().insert(
                    name.to_owned(),
                    if declared.insert(name.to_owned()) {
                        source
                    } else {
                        TranscriptSource::Unknown
                    },
                );
                frames.push(Frame::Block {
                    statements,
                    next,
                    tail,
                    roots,
                    declared,
                });
            }
        }
    }
    results.pop().unwrap_or(TranscriptSource::Unknown)
}

fn source_capacity_pattern_slots(
    pattern: &MatchPattern,
    expected: &Type,
    path: &str,
    bindings: &mut BTreeMap<String, Type>,
    slots: &mut Vec<crate::byte_data_capacity::ArrayStorageSlot>,
    types: &TypeTable<'_>,
) -> Result<(), ()> {
    match pattern {
        MatchPattern::Binding { name, .. } => {
            source_capacity_slot(
                slots,
                types,
                format!("{path}.binding.{name}"),
                crate::byte_data_capacity::ArrayStorageKind::Binding,
                expected,
            )?;
            bindings.insert(name.clone(), expected.clone());
        }
        MatchPattern::Record { fields, .. } => {
            let mut pending = Vec::new();
            let declared = types.record_fields(expected).ok_or(())?;
            for (index, field) in fields.iter().enumerate().rev() {
                let declaration = declared
                    .iter()
                    .find(|candidate| candidate.name == field.name)
                    .ok_or(())?;
                let field_ty = types.record_field_type(expected, declaration).ok_or(())?;
                pending.push((&field.pattern, field_ty, format!("{path}.field.{index}")));
            }
            while let Some((pattern, field_ty, field_path)) = pending.pop() {
                match pattern {
                    RecordMatchFieldPattern::Binding { name, .. } => {
                        source_capacity_slot(
                            slots,
                            types,
                            format!("{field_path}.binding.{name}"),
                            crate::byte_data_capacity::ArrayStorageKind::Binding,
                            &field_ty,
                        )?;
                        bindings.insert(name.clone(), field_ty);
                    }
                    RecordMatchFieldPattern::Record { fields, .. } => {
                        let declared = types.record_fields(&field_ty).ok_or(())?;
                        for (index, field) in fields.iter().enumerate().rev() {
                            let declaration = declared
                                .iter()
                                .find(|candidate| candidate.name == field.name)
                                .ok_or(())?;
                            let nested_ty =
                                types.record_field_type(&field_ty, declaration).ok_or(())?;
                            pending.push((
                                &field.pattern,
                                nested_ty,
                                format!("{field_path}.field.{index}"),
                            ));
                        }
                    }
                    RecordMatchFieldPattern::Wildcard { .. } => {}
                }
            }
        }
        MatchPattern::Variant { .. }
        | MatchPattern::Wildcard { .. }
        | MatchPattern::Literal { .. }
        | MatchPattern::Or { .. } => {}
    }
    Ok(())
}

fn source_capacity_pattern_transcript_bindings(
    pattern: &MatchPattern,
    roots: &mut BTreeMap<String, crate::byte_data_capacity::TranscriptSource>,
) {
    use crate::byte_data_capacity::TranscriptSource;
    match pattern {
        MatchPattern::Binding { name, .. } => {
            roots.insert(name.clone(), TranscriptSource::Unknown);
        }
        MatchPattern::Variant { fields, .. } => {
            for field in fields {
                roots.insert(field.binding.clone(), TranscriptSource::Unknown);
            }
        }
        MatchPattern::Record { fields, .. } => {
            let mut pending = fields
                .iter()
                .rev()
                .map(|field| &field.pattern)
                .collect::<Vec<_>>();
            while let Some(pattern) = pending.pop() {
                match pattern {
                    RecordMatchFieldPattern::Binding { name, .. } => {
                        roots.insert(name.clone(), TranscriptSource::Unknown);
                    }
                    RecordMatchFieldPattern::Record { fields, .. } => {
                        pending.extend(fields.iter().rev().map(|field| &field.pattern));
                    }
                    RecordMatchFieldPattern::Wildcard { .. } => {}
                }
            }
        }
        MatchPattern::Wildcard { .. } | MatchPattern::Literal { .. } | MatchPattern::Or { .. } => {}
    }
}

fn source_capacity_expr(
    expression: &Expr,
    path: &str,
    bindings: &mut BTreeMap<String, Type>,
    transcript_roots: &mut BTreeMap<String, crate::byte_data_capacity::TranscriptSource>,
    slots: &mut Vec<crate::byte_data_capacity::ArrayStorageSlot>,
    context: &SourceCapacityContext<'_>,
    direct_destination: bool,
) -> Result<crate::byte_data_capacity::CapacityFlow, ()> {
    use crate::byte_data_capacity::{ArrayStorageKind, CapacityFlow, TranscriptSource};

    type ScopeRef = std::rc::Rc<std::cell::RefCell<SourceCapacityScope>>;
    enum BlockUpdate<'a> {
        None,
        Let {
            name: &'a str,
            ty: Option<Type>,
            source: TranscriptSource,
        },
        Assign(&'a str),
    }
    enum Frame<'a> {
        Visit {
            expression: &'a Expr,
            path: String,
            scope: ScopeRef,
            direct_destination: bool,
        },
        Argument {
            expression: &'a Expr,
            path: String,
            slot: Option<(String, Type)>,
            scope: ScopeRef,
        },
        Sequence(usize),
        Alternative(usize),
        Loop,
        Block {
            statements: &'a [Statement],
            next: usize,
            tail: &'a Expr,
            path: String,
            scope: ScopeRef,
            direct_destination: bool,
        },
        BlockAfter {
            statements: &'a [Statement],
            next: usize,
            tail: &'a Expr,
            path: String,
            scope: ScopeRef,
            direct_destination: bool,
            update: BlockUpdate<'a>,
        },
        MatchNext {
            arms: &'a [crate::ast::MatchArm],
            next: usize,
            path: String,
            scope: ScopeRef,
            scrutinee_ty: Option<Type>,
            direct_destination: bool,
        },
        Match(usize),
        Emit(CapacityFlow),
    }

    fn sequence(children: Vec<CapacityFlow>) -> CapacityFlow {
        if children.is_empty() {
            CapacityFlow::Empty
        } else {
            CapacityFlow::Sequence(children)
        }
    }

    let root_scope = std::rc::Rc::new(std::cell::RefCell::new(SourceCapacityScope::new(
        bindings.clone(),
        transcript_roots.clone(),
    )));
    let clone_scope = |scope: &ScopeRef| {
        let scope = scope.borrow();
        std::rc::Rc::new(std::cell::RefCell::new(SourceCapacityScope::new(
            scope.bindings.clone(),
            scope.transcript_roots.clone(),
        )))
    };
    let mut frames = vec![Frame::Visit {
        expression,
        path: path.to_owned(),
        scope: root_scope,
        direct_destination,
    }];
    let mut results = Vec::new();
    while let Some(frame) = frames.pop() {
        #[cfg(test)]
        {
            let (frame_count, path_bytes, type_bytes) = frames
                .iter()
                .chain(std::iter::once(&frame))
                .fold((0_usize, 0_usize, 0_usize), |totals, frame| {
                    let Frame::MatchNext {
                        path, scrutinee_ty, ..
                    } = frame
                    else {
                        return totals;
                    };
                    (
                        totals.0.saturating_add(1),
                        totals.1.saturating_add(path.capacity()),
                        totals.2.saturating_add(
                            scrutinee_ty.as_ref().map_or(0, ast_type_owned_capacity),
                        ),
                    )
                });
            SOURCE_CAPACITY_MATCH_NEXT_FRAME_PEAK
                .with(|peak| peak.set(peak.get().max(frame_count)));
            SOURCE_CAPACITY_MATCH_NEXT_PATH_PEAK.with(|peak| peak.set(peak.get().max(path_bytes)));
            SOURCE_CAPACITY_MATCH_NEXT_TYPE_PEAK.with(|peak| peak.set(peak.get().max(type_bytes)));
        }
        match frame {
            Frame::Visit {
                expression,
                path,
                scope,
                direct_destination,
            } => {
                let expression_ty = {
                    let scope = scope.borrow();
                    source_capacity_expr_type(expression, &scope.bindings, context)
                };
                if let Some(ty) = expression_ty {
                    let payload = source_array_payload(context.types, &ty)?;
                    if payload != 0 || matches!(ty, Type::ArrayU8(0)) {
                        let kind = match &expression.kind {
                            ExprKind::Call { .. }
                            | ExprKind::MethodCall { .. }
                            | ExprKind::SuperMethod { .. } => Some(ArrayStorageKind::CallStaging),
                            ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. }
                                if direct_destination =>
                            {
                                None
                            }
                            ExprKind::Var(_) => None,
                            ExprKind::Block { .. }
                            | ExprKind::If { .. }
                            | ExprKind::Match { .. } => None,
                            _ => Some(ArrayStorageKind::Temporary),
                        };
                        if let Some(kind) = kind {
                            slots.push(crate::byte_data_capacity::ArrayStorageSlot {
                                identity: path.clone(),
                                kind,
                                length: payload,
                            });
                        }
                    }
                }
                match &expression.kind {
                    ExprKind::Call { name, args, .. } => {
                        let target = context.ordinary.get(name.as_str()).copied();
                        let effect = if name == crate::byte_ops::COPY_NAME {
                            Some(CapacityFlow::BytesCopy {
                                site: path.clone(),
                                conservative_payload_bytes:
                                    crate::byte_data_capacity::MAX_ARRAY_BYTES,
                            })
                        } else if name == crate::host_io_ops::STDOUT_WRITE_NAME {
                            Some(CapacityFlow::StdoutWrite {
                                site: path.clone(),
                                source: {
                                    let scope = scope.borrow();
                                    source_transcript_source_from_roots(
                                        &args[0],
                                        &scope.transcript_roots,
                                    )
                                },
                            })
                        } else if let Some(operation) = crate::command_io_ops::by_name(name) {
                            command_io::flow(
                                operation,
                                &path,
                                args,
                                &scope.borrow().transcript_roots,
                            )
                        } else {
                            target.map(|target| CapacityFlow::Call {
                                site: path.clone(),
                                callee: target.stable_id.clone(),
                            })
                        };
                        let count = args.len() + usize::from(effect.is_some());
                        frames.push(Frame::Sequence(count));
                        if let Some(effect) = effect {
                            frames.push(Frame::Emit(effect));
                        }
                        for (index, argument) in args.iter().enumerate().rev() {
                            frames.push(Frame::Argument {
                                expression: argument,
                                path: format!("{path}.arg.{index}.value"),
                                slot: target
                                    .and_then(|function| function.params.get(index))
                                    .map(|param| (format!("{path}.arg.{index}"), param.ty.clone())),
                                scope: std::rc::Rc::clone(&scope),
                            });
                        }
                    }
                    ExprKind::MethodCall {
                        receiver,
                        method,
                        args,
                        ..
                    } => {
                        let target = {
                            let scope = scope.borrow();
                            source_capacity_expr_type(receiver, &scope.bindings, context)
                        }
                        .and_then(|ty| {
                            let Type::Named { name, .. } = ty else {
                                return None;
                            };
                            resolve_class_method(context.types, &name, method)
                                .map(|(_, function)| function)
                        });
                        let effect = target.map(|target| CapacityFlow::Call {
                            site: path.clone(),
                            callee: target.stable_id.clone(),
                        });
                        frames.push(Frame::Sequence(
                            1 + args.len() + usize::from(effect.is_some()),
                        ));
                        if let Some(effect) = effect {
                            frames.push(Frame::Emit(effect));
                        }
                        for (index, argument) in args.iter().enumerate().rev() {
                            frames.push(Frame::Argument {
                                expression: argument,
                                path: format!("{path}.arg.{index}.value"),
                                slot: target.and_then(|function| {
                                    function.params.get(index + 1).map(|param| {
                                        (format!("{path}.arg.{index}"), param.ty.clone())
                                    })
                                }),
                                scope: std::rc::Rc::clone(&scope),
                            });
                        }
                        frames.push(Frame::Argument {
                            expression: receiver,
                            path: format!("{path}.receiver.value"),
                            slot: target.and_then(|function| {
                                function
                                    .params
                                    .first()
                                    .map(|param| (format!("{path}.receiver"), param.ty.clone()))
                            }),
                            scope: std::rc::Rc::clone(&scope),
                        });
                    }
                    ExprKind::SuperMethod { method, args, .. } => {
                        let target = source_capacity_super_method(context, method);
                        if let Some(param) = target.and_then(|function| function.params.first()) {
                            source_capacity_slot(
                                slots,
                                context.types,
                                format!("{path}.receiver"),
                                ArrayStorageKind::CallStaging,
                                &param.ty,
                            )?;
                        }
                        let effect = target.map(|target| CapacityFlow::Call {
                            site: path.clone(),
                            callee: target.stable_id.clone(),
                        });
                        frames.push(Frame::Sequence(args.len() + usize::from(effect.is_some())));
                        if let Some(effect) = effect {
                            frames.push(Frame::Emit(effect));
                        }
                        for (index, argument) in args.iter().enumerate().rev() {
                            frames.push(Frame::Argument {
                                expression: argument,
                                path: format!("{path}.arg.{index}.value"),
                                slot: target.and_then(|function| {
                                    function.params.get(index + 1).map(|param| {
                                        (format!("{path}.arg.{index}"), param.ty.clone())
                                    })
                                }),
                                scope: std::rc::Rc::clone(&scope),
                            });
                        }
                    }
                    ExprKind::Unary { value, .. } | ExprKind::Try { operand: value } => {
                        frames.push(Frame::Visit {
                            expression: value,
                            path: format!("{path}.operand"),
                            scope: std::rc::Rc::clone(&scope),
                            direct_destination: false,
                        });
                    }
                    ExprKind::Binary { left, right, .. } => {
                        frames.push(Frame::Sequence(2));
                        frames.push(Frame::Visit {
                            expression: right,
                            path: format!("{path}.right"),
                            scope: std::rc::Rc::clone(&scope),
                            direct_destination: false,
                        });
                        frames.push(Frame::Visit {
                            expression: left,
                            path: format!("{path}.left"),
                            scope,
                            direct_destination: false,
                        });
                    }
                    ExprKind::Block { statements, tail } => {
                        let local = clone_scope(&scope);
                        frames.push(Frame::Block {
                            statements,
                            next: 0,
                            tail,
                            path,
                            scope: local,
                            direct_destination,
                        });
                    }
                    ExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        frames.push(Frame::Sequence(2));
                        frames.push(Frame::Alternative(2));
                        frames.push(Frame::Visit {
                            expression: else_branch,
                            path: format!("{path}.else"),
                            scope: std::rc::Rc::clone(&scope),
                            direct_destination,
                        });
                        frames.push(Frame::Visit {
                            expression: then_branch,
                            path: format!("{path}.then"),
                            scope: std::rc::Rc::clone(&scope),
                            direct_destination,
                        });
                        frames.push(Frame::Visit {
                            expression: condition,
                            path: format!("{path}.condition"),
                            scope: std::rc::Rc::clone(&scope),
                            direct_destination: false,
                        });
                    }
                    ExprKind::ConstructRecord { fields, .. }
                    | ExprKind::ConstructVariant { fields, .. } => {
                        frames.push(Frame::Sequence(fields.len()));
                        for (index, field) in fields.iter().enumerate().rev() {
                            frames.push(Frame::Visit {
                                expression: &field.value,
                                path: format!("{path}.field.{index}"),
                                scope: std::rc::Rc::clone(&scope),
                                direct_destination: false,
                            });
                        }
                    }
                    ExprKind::Match {
                        scrutinee, arms, ..
                    } => {
                        let scrutinee_ty = {
                            let scope = scope.borrow();
                            source_capacity_expr_type(scrutinee, &scope.bindings, context)
                        };
                        frames.push(Frame::Match(arms.len()));
                        frames.push(Frame::Visit {
                            expression: scrutinee,
                            path: format!("{path}.scrutinee"),
                            scope: std::rc::Rc::clone(&scope),
                            direct_destination: false,
                        });
                        frames.push(Frame::MatchNext {
                            arms,
                            next: 0,
                            path: path.clone(),
                            scope: std::rc::Rc::clone(&scope),
                            scrutinee_ty,
                            direct_destination,
                        });
                    }
                    ExprKind::UpdateRecord { base, fields } => {
                        frames.push(Frame::Sequence(1 + fields.len()));
                        for (index, field) in fields.iter().enumerate().rev() {
                            frames.push(Frame::Visit {
                                expression: &field.value,
                                path: format!("{path}.field.{index}"),
                                scope: std::rc::Rc::clone(&scope),
                                direct_destination: false,
                            });
                        }
                        frames.push(Frame::Visit {
                            expression: base,
                            path: format!("{path}.base"),
                            scope: std::rc::Rc::clone(&scope),
                            direct_destination: false,
                        });
                    }
                    ExprKind::Project { base, .. } => frames.push(Frame::Visit {
                        expression: base,
                        path: format!("{path}.base"),
                        scope: std::rc::Rc::clone(&scope),
                        direct_destination: false,
                    }),
                    ExprKind::Int(_)
                    | ExprKind::Int32(_)
                    | ExprKind::Char(_)
                    | ExprKind::Uint8(_)
                    | ExprKind::Usize(_)
                    | ExprKind::ArrayU8(_)
                    | ExprKind::RepeatArrayU8 { .. }
                    | ExprKind::Float32(_)
                    | ExprKind::Float64(_)
                    | ExprKind::Bool(_)
                    | ExprKind::String(_)
                    | ExprKind::Var(_) => results.push(CapacityFlow::Empty),
                }
            }
            Frame::Argument {
                expression,
                path,
                slot,
                scope,
            } => {
                if let Some((identity, ty)) = slot {
                    source_capacity_slot(
                        slots,
                        context.types,
                        identity,
                        ArrayStorageKind::CallStaging,
                        &ty,
                    )?;
                }
                frames.push(Frame::Visit {
                    expression,
                    path,
                    scope,
                    direct_destination: false,
                });
            }
            Frame::Sequence(count) | Frame::Alternative(count) => {
                let start = results.len().checked_sub(count).ok_or(())?;
                let children = results.drain(start..).collect::<Vec<_>>();
                results.push(if matches!(frame, Frame::Sequence(_)) {
                    sequence(children)
                } else {
                    CapacityFlow::Alternative(children)
                });
            }
            Frame::Loop => {
                let body = results.pop().ok_or(())?;
                let condition = results.pop().ok_or(())?;
                results.push(CapacityFlow::Loop {
                    condition: Box::new(condition),
                    body: Box::new(body),
                });
            }
            Frame::Block {
                statements,
                next,
                tail,
                path,
                scope,
                direct_destination,
            } => {
                if let Some(statement) = statements.get(next) {
                    let statement_path = format!("{path}.s{next}");
                    let (expression, child_path, child_direct, update) = match statement {
                        Statement::Let {
                            name,
                            declared,
                            value,
                            ..
                        } => {
                            let ty = declared.clone().or_else(|| {
                                let scope = scope.borrow();
                                source_capacity_expr_type(value, &scope.bindings, context)
                            });
                            if let Some(ty) = &ty {
                                source_capacity_slot(
                                    slots,
                                    context.types,
                                    format!("{statement_path}.binding.{name}"),
                                    ArrayStorageKind::Binding,
                                    ty,
                                )?;
                            }
                            let source = {
                                let scope = scope.borrow();
                                source_transcript_source_from_roots(value, &scope.transcript_roots)
                            };
                            (
                                value,
                                format!("{statement_path}.value"),
                                true,
                                BlockUpdate::Let { name, ty, source },
                            )
                        }
                        Statement::Assign { name, value, .. } => (
                            value,
                            format!("{statement_path}.value"),
                            true,
                            BlockUpdate::Assign(name),
                        ),
                        Statement::Unsafe { body, .. } => (
                            body.as_ref(),
                            format!("{statement_path}.body"),
                            true,
                            BlockUpdate::None,
                        ),
                        Statement::While {
                            condition, body, ..
                        } => {
                            frames.push(Frame::BlockAfter {
                                statements,
                                next: next + 1,
                                tail,
                                path,
                                scope: std::rc::Rc::clone(&scope),
                                direct_destination,
                                update: BlockUpdate::None,
                            });
                            frames.push(Frame::Loop);
                            frames.push(Frame::Visit {
                                expression: body,
                                path: format!("{statement_path}.body"),
                                scope: std::rc::Rc::clone(&scope),
                                direct_destination: false,
                            });
                            frames.push(Frame::Visit {
                                expression: condition,
                                path: format!("{statement_path}.condition"),
                                scope: std::rc::Rc::clone(&scope),
                                direct_destination: false,
                            });
                            continue;
                        }
                    };
                    frames.push(Frame::BlockAfter {
                        statements,
                        next: next + 1,
                        tail,
                        path,
                        scope: std::rc::Rc::clone(&scope),
                        direct_destination,
                        update,
                    });
                    frames.push(Frame::Visit {
                        expression,
                        path: child_path,
                        scope: std::rc::Rc::clone(&scope),
                        direct_destination: child_direct,
                    });
                } else {
                    frames.push(Frame::Sequence(statements.len() + 1));
                    frames.push(Frame::Visit {
                        expression: tail,
                        path: format!("{path}.tail"),
                        scope,
                        direct_destination,
                    });
                }
            }
            Frame::BlockAfter {
                statements,
                next,
                tail,
                path,
                scope,
                direct_destination,
                update,
            } => {
                match update {
                    BlockUpdate::None => {}
                    BlockUpdate::Let { name, ty, source } => {
                        let mut scope = scope.borrow_mut();
                        if let Some(ty) = ty {
                            scope.bindings.insert(name.to_owned(), ty);
                        }
                        scope.transcript_roots.insert(name.to_owned(), source);
                    }
                    BlockUpdate::Assign(name) => {
                        scope
                            .borrow_mut()
                            .transcript_roots
                            .insert(name.to_owned(), TranscriptSource::Unknown);
                    }
                }
                frames.push(Frame::Block {
                    statements,
                    next,
                    tail,
                    path,
                    scope,
                    direct_destination,
                });
            }
            Frame::MatchNext {
                arms,
                next,
                path,
                scope,
                scrutinee_ty,
                direct_destination,
            } => {
                let Some(arm) = arms.get(next) else {
                    continue;
                };
                let arm_scope = clone_scope(&scope);
                if let Some(scrutinee_ty) = &scrutinee_ty {
                    let mut arm_state = arm_scope.borrow_mut();
                    source_capacity_pattern_slots(
                        &arm.pattern,
                        scrutinee_ty,
                        &format!("{path}.arm.{next}.pattern"),
                        &mut arm_state.bindings,
                        slots,
                        context.types,
                    )?;
                    source_capacity_pattern_transcript_bindings(
                        &arm.pattern,
                        &mut arm_state.transcript_roots,
                    );
                }
                let guard_path = format!("{path}.arm.{next}.guard");
                let value_path = format!("{path}.arm.{next}.value");
                frames.push(Frame::MatchNext {
                    arms,
                    next: next + 1,
                    path,
                    scope,
                    scrutinee_ty,
                    direct_destination,
                });
                frames.push(Frame::Sequence(1 + usize::from(arm.guard.is_some())));
                frames.push(Frame::Visit {
                    expression: &arm.value,
                    path: value_path,
                    scope: std::rc::Rc::clone(&arm_scope),
                    direct_destination,
                });
                if let Some(guard) = &arm.guard {
                    frames.push(Frame::Visit {
                        expression: guard,
                        path: guard_path,
                        scope: std::rc::Rc::clone(&arm_scope),
                        direct_destination: false,
                    });
                }
            }
            Frame::Match(arm_count) => {
                let scrutinee = results.pop().ok_or(())?;
                let start = results.len().checked_sub(arm_count).ok_or(())?;
                let alternatives = results.drain(start..).collect::<Vec<_>>();
                results.push(sequence(vec![
                    scrutinee,
                    CapacityFlow::Alternative(alternatives),
                ]));
            }
            Frame::Emit(flow) => results.push(flow),
        }
    }
    if results.len() == 1 {
        results.pop().ok_or(())
    } else {
        Err(())
    }
}

pub(super) fn verify_byte_data_capacity(
    program: &Program,
    types: &TypeTable<'_>,
) -> Result<(), crate::byte_data_capacity::CapacityError> {
    use crate::byte_data_capacity::{ArrayStorageKind, CapacityFlow, FunctionCapacityInput};

    let ordinary = program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let inputs = source_capacity_functions(program)
        .into_iter()
        .map(|(enclosing_class, function)| {
            let context = SourceCapacityContext {
                types,
                ordinary: &ordinary,
                enclosing_class,
            };
            let mut slots = Vec::new();
            let mut bindings = BTreeMap::new();
            let mut transcript_roots = function
                .params
                .iter()
                .map(|parameter| {
                    (
                        parameter.name.clone(),
                        crate::byte_data_capacity::TranscriptSource::Unknown,
                    )
                })
                .collect::<BTreeMap<_, _>>();
            for (index, parameter) in function.params.iter().enumerate() {
                source_capacity_slot(
                    &mut slots,
                    types,
                    format!("{}.param.{index}", function.stable_id),
                    ArrayStorageKind::Parameter,
                    &parameter.ty,
                )
                .map_err(|()| source_capacity_invariant(&function.stable_id))?;
                bindings.insert(parameter.name.clone(), parameter.ty.clone());
            }
            source_capacity_slot(
                &mut slots,
                types,
                format!("{}.result", function.stable_id),
                ArrayStorageKind::ProvisionalResult,
                &function.return_type,
            )
            .map_err(|()| source_capacity_invariant(&function.stable_id))?;
            let mut execution = function
                .requires
                .iter()
                .enumerate()
                .map(|(index, expression)| {
                    source_capacity_expr(
                        expression,
                        &format!("{}.requires.{index}", function.stable_id),
                        &mut bindings.clone(),
                        &mut transcript_roots.clone(),
                        &mut slots,
                        &context,
                        false,
                    )
                    .map_err(|()| source_capacity_invariant(&function.stable_id))
                })
                .collect::<Result<Vec<_>, _>>()?;
            execution.push(
                source_capacity_expr(
                    &function.body,
                    &format!("{}.body", function.stable_id),
                    &mut bindings,
                    &mut transcript_roots,
                    &mut slots,
                    &context,
                    true,
                )
                .map_err(|()| source_capacity_invariant(&function.stable_id))?,
            );
            execution.extend(
                function
                    .ensures
                    .iter()
                    .enumerate()
                    .map(|(index, expression)| {
                        source_capacity_expr(
                            expression,
                            &format!("{}.ensures.{index}", function.stable_id),
                            &mut bindings.clone(),
                            &mut transcript_roots.clone(),
                            &mut slots,
                            &context,
                            false,
                        )
                        .map_err(|()| source_capacity_invariant(&function.stable_id))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            Ok(FunctionCapacityInput {
                function: function.stable_id.clone(),
                array_slots: slots,
                execution: CapacityFlow::Sequence(execution),
            })
        })
        .collect::<Result<Vec<_>, crate::byte_data_capacity::CapacityError>>()?;
    crate::byte_data_capacity::analyze(&inputs).map(|_| ())
}

fn source_capacity_invariant(function: &str) -> crate::byte_data_capacity::CapacityError {
    crate::byte_data_capacity::CapacityError {
        diagnostic: crate::byte_data_capacity::CapacityDiagnostic::Invariant,
        function: Some(function.to_owned()),
        detail: "source byte-data capacity projection could not be reconstructed".to_owned(),
    }
}
