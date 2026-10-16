//! Full ordinary substitution with the existing replay payer and raw H006 text.
//! The complete original substituter remains independently compiled in HIR.
use super::*;

fn raw(payer: &mut Payer<'_>, message: std::fmt::Arguments<'_>) -> Diagnostic {
    match inventory_text::raw(payer, message) {
        Ok(message) => Diagnostic::io("SPX-H006", message),
        Err(error) => error,
    }
}

pub(super) fn substitute(
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
    payer: &mut Payer<'_>,
) -> Result<ResolvedType, Diagnostic> {
    let function = payer.function;
    inventory_text::current(payer)?;
    payer
        .budget
        .charge(function, 1, "inventory substitution root")?;
    // Preserve the ordinary leaf shortcut; no traversal backing for these roots.
    if let ResolvedType::TypeParameter {
        owner: actual,
        index,
    } = template
    {
        if !inventory_text::owner_matches(actual, owner, payer)? {
            return Err(raw(
                payer,
                format_args!(
                    "type template for `{owner}` contains foreign parameter owner `{actual}`"
                ),
            ));
        }
        let index_usize = usize::try_from(*index).map_err(|_| {
            raw(
                payer,
                format_args!("type parameter index {index} does not fit usize"),
            )
        })?;
        let argument = arguments.get(index_usize).ok_or_else(|| {
            raw(
                payer,
                format_args!("type template for `{owner}` references missing parameter {index}"),
            )
        })?;
        return inventory_clone::clone_type(argument, payer);
    }
    if !inventory_clone::owns_backing(template) {
        return inventory_clone::clone_type(template, payer);
    }
    enum Frame<'a> {
        Enter(&'a ResolvedType),
        Finish(&'a DeclarationId, usize),
        Callable(usize),
    }
    let mut frames = inventory_flags::vector(payer)?;
    scratch_grow(&mut frames, 1, payer)?;
    frames.push(Frame::Enter(template));
    let mut resolved = inventory_flags::vector(payer)?;
    loop {
        inventory_metadata::step(payer, 1)?;
        let Some(frame) = frames.pop() else {
            break;
        };
        payer
            .budget
            .charge(function, 1, "inventory substitution frame")?;
        match frame {
            Frame::Enter(template) => {
                match template {
                    ResolvedType::TypeParameter {
                        owner: actual,
                        index,
                    } => {
                        if !inventory_text::owner_matches(actual, owner, payer)? {
                            return Err(raw(payer, format_args!("type template for `{owner}` contains foreign parameter owner `{actual}`")));
                        }
                        let index_usize = usize::try_from(*index).map_err(|_| {
                            raw(
                                payer,
                                format_args!("type parameter index {index} does not fit usize"),
                            )
                        })?;
                        let argument = arguments.get(index_usize).ok_or_else(|| raw(payer, format_args!("type template for `{owner}` references missing parameter {index}")))?;
                        let argument = inventory_clone::clone_type(argument, payer)?;
                        scratch_grow(&mut resolved, 1, payer)?;
                        resolved.push(argument);
                    }
                    ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } => {
                        let additional = arguments.len().checked_add(1).ok_or_else(|| {
                            payer.construction_refusal(
                                "inventory substitution frame count overflowed",
                            )
                        })?;
                        scratch_grow(&mut frames, additional, payer)?;
                        frames.push(Frame::Finish(declaration, arguments.len()));
                        payer.budget.charge(
                            function,
                            arguments.len(),
                            "inventory substitution child scheduling",
                        )?;
                        frames.extend(arguments.iter().rev().map(Frame::Enter));
                    }
                    ResolvedType::Function { parameters, result } => {
                        let additional = parameters.len().checked_add(2).ok_or_else(|| {
                            payer.construction_refusal(
                                "inventory substitution frame count overflowed",
                            )
                        })?;
                        scratch_grow(&mut frames, additional, payer)?;
                        frames.push(Frame::Callable(parameters.len()));
                        frames.push(Frame::Enter(result));
                        payer.budget.charge(
                            function,
                            parameters.len(),
                            "inventory substitution callable child scheduling",
                        )?;
                        frames.extend(parameters.iter().rev().map(Frame::Enter));
                    }
                    leaf @ (ResolvedType::Never
                    | ResolvedType::Unit
                    | ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::Char
                    | ResolvedType::U8
                    | ResolvedType::Usize
                    | ResolvedType::ArrayU8(_)
                    | ResolvedType::F32
                    | ResolvedType::F64
                    | ResolvedType::Bool
                    | ResolvedType::String
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
                    | ResolvedType::MutFunctionI64
                    | ResolvedType::Bytes
                    | ResolvedType::Str
                    | ResolvedType::SliceU8
                    | ResolvedType::StringMap) => {
                        let leaf = inventory_clone::clone_type(leaf, payer)?;
                        scratch_grow(&mut resolved, 1, payer)?;
                        resolved.push(leaf);
                    }
                }
            }
            Frame::Finish(declaration, count) => {
                let split = resolved.len().checked_sub(count).ok_or_else(|| {
                    raw(
                        payer,
                        format_args!("type substitution traversal is incomplete"),
                    )
                })?;
                let mut arguments = inventory_clone::exact(count, payer)?;
                arguments.extend(resolved.drain(split..));
                scratch_grow(&mut resolved, 1, payer)?;
                resolved.push(ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments,
                });
            }
            Frame::Callable(count) => {
                let result = resolved
                    .pop()
                    .ok_or_else(|| raw(payer, format_args!("incomplete callable substitution")))?;
                let split = resolved
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| raw(payer, format_args!("incomplete callable parameters")))?;
                let mut parameters = inventory_clone::exact(count, payer)?;
                parameters.extend(resolved.drain(split..));
                inventory_metadata::step(payer, 2)?; // box publication and eventual release
                let result = payer.boxed_type(result)?;
                scratch_grow(&mut resolved, 1, payer)?;
                resolved.push(ResolvedType::Function { parameters, result });
            }
        }
    }
    if resolved.len() != 1 {
        return Err(raw(
            payer,
            format_args!("type substitution traversal did not settle"),
        ));
    }
    Ok(resolved
        .pop()
        .expect("substitution result count checked above"))
}

fn scratch_grow<T>(
    values: &mut Vec<T>,
    additional: usize,
    payer: &mut Payer<'_>,
) -> Result<(), Diagnostic> {
    inventory_metadata::step(payer, additional)?; // published slots
    inventory_metadata::step(payer, additional)?; // eventual pop/drop visits
    inventory_flags::grow(values, additional, payer)
}
