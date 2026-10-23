//! A local record signature authenticates only a callee-ID constructor.
//! It grants no nominal, field, expression, cleanup or import-edge discount.
use super::super::{Context, Failure, Invoice, MAX_FRAMES};
use crate::ast::{Function, ModuleUse, Param, Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::workspace_graph::AuthoredKind;

type Ancestry<'p> = [Option<&'p TypeDeclaration>; MAX_FRAMES];
type ResultBool = Result<bool, Failure>;
// Every receiving boundary below is paid before forming/traversing it. The
// ancestry owns immutable loans; pointer equality detects a cycle only AFTER
// declaration/source/provider authentication, never grants identity authority.
type Signature<'p, 'a> = (
    Context<'p, 'a>,
    Context<'p, 'a>,
    &'p Function,
    &'p Function,
    &'p Invoice,
    &'p Invoice,
    Ancestry<'p>,
    Ancestry<'p>,
    &'p Vec<crate::ast::TypeParameterDeclaration>,
    &'p Vec<crate::ast::TypeParameterDeclaration>,
    &'p mut Ancestry<'p>,
    &'p mut Ancestry<'p>,
    &'p Vec<Param>,
    &'p Vec<Param>,
    &'p [Param],
    &'p [Param],
    &'p [Param],
    &'p [Param],
    std::slice::Iter<'p, Param>,
    std::slice::Iter<'p, Param>,
    &'p Type,
    &'p Type,
    ResultBool,
    ResultBool,
    bool,
    bool,
    bool,
    bool,
);
type Parameter<'p> = (
    &'p mut std::slice::Iter<'p, Param>,
    &'p mut std::slice::Iter<'p, Param>,
    Option<&'p Param>,
    Option<&'p Param>,
    &'p Param,
    &'p Param,
    &'p Type,
    &'p Type,
    ResultBool,
    ResultBool,
);
type Visit<'p, 'a> = (
    Context<'p, 'a>,
    Context<'p, 'a>,
    &'p Type,
    &'p Type,
    &'p Invoice,
    &'p Invoice,
    &'p mut Ancestry<'p>,
    &'p mut Ancestry<'p>,
    usize,
    usize,
    usize,
    usize,
    bool,
    bool,
    bool,
    bool,
    ResultBool,
    ResultBool,
    &'p String,
    &'p String,
    &'p Vec<Type>,
    &'p Vec<Type>,
    &'p Vec<ModuleUse>,
    &'p Vec<ModuleUse>,
    &'p [ModuleUse],
    &'p [ModuleUse],
    &'p [ModuleUse],
    &'p [ModuleUse],
    std::slice::Iter<'p, ModuleUse>,
    std::slice::Iter<'p, ModuleUse>,
    &'p Vec<TypeDeclaration>,
    &'p Vec<TypeDeclaration>,
    &'p [TypeDeclaration],
    &'p [TypeDeclaration],
    &'p [TypeDeclaration],
    &'p [TypeDeclaration],
    std::slice::Iter<'p, TypeDeclaration>,
    std::slice::Iter<'p, TypeDeclaration>,
    Option<&'p TypeDeclaration>,
    Option<&'p TypeDeclaration>,
    Option<&'p TypeDeclaration>,
    Option<&'p TypeDeclaration>,
);
// String equality/length receivers, iterator result and next receivers are
// independent from the held declaration/alias loans and from their payloads.
type NameStep<'p, T> = (
    &'p mut std::slice::Iter<'p, T>,
    &'p mut std::slice::Iter<'p, T>,
    Option<&'p T>,
    Option<&'p T>,
    &'p T,
    &'p T,
    &'p String,
    &'p String,
    &'p String,
    &'p String,
    &'p String,
    &'p String,
    &'p String,
    &'p String,
    &'p str,
    &'p str,
    &'p str,
    &'p str,
    &'p str,
    &'p str,
    &'p str,
    &'p str,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    bool,
    bool,
    ResultBool,
    ResultBool,
);

pub(super) fn ordinary(
    context: Context<'_, '_>,
    function: &Function,
    invoice: &Invoice,
) -> ResultBool {
    invoice.pay::<Signature<'_, '_>>()?;
    if !function.explicit_id
        || !function.type_parameters.is_empty()
        || matches!(function.yields, Some(_))
        || matches!(function.follows, Some(_))
    {
        return Ok(false);
    }
    let mut ancestry = [None; MAX_FRAMES];
    if !visit(context, &function.return_type, invoice, &mut ancestry, 0)? {
        return Ok(false);
    }
    let mut parameters = function.params.iter();
    loop {
        invoice.pay::<Parameter<'_>>()?;
        let Some(parameter) = parameters.next() else {
            return Ok(true);
        };
        invoice.work(1)?;
        if !visit(context, &parameter.ty, invoice, &mut ancestry, 0)? {
            return Ok(false);
        }
    }
}

fn visit<'p>(
    context: Context<'p, '_>,
    ty: &Type,
    invoice: &Invoice,
    ancestry: &mut Ancestry<'p>,
    depth: usize,
) -> ResultBool {
    invoice.pay::<Visit<'_, '_>>()?;
    invoice.work(1)?;
    let (name, arguments) = match ty {
        Type::I64
        | Type::I32
        | Type::U8
        | Type::Usize
        | Type::Char
        | Type::F32
        | Type::F64
        | Type::Bool
        | Type::String
        | Type::Bytes
        | Type::Str
        | Type::SliceU8 => return Ok(true),
        Type::Named { name, arguments } => (name, arguments),
        _ => return Ok(false),
    };
    if !arguments.is_empty() || depth >= MAX_FRAMES {
        return Ok(false);
    }
    let mut aliases = context.program.module_uses.iter();
    loop {
        invoice.pay::<NameStep<'_, ModuleUse>>()?;
        let Some(alias) = aliases.next() else {
            break;
        };
        invoice.work(
            name.len()
                .saturating_add(alias.alias.len())
                .saturating_add(1),
        )?;
        if str::eq(name.as_str(), alias.alias.as_str()) {
            return Ok(false);
        }
    }
    let mut declarations = context.program.types.iter();
    let mut found = None;
    loop {
        invoice.pay::<NameStep<'_, TypeDeclaration>>()?;
        let Some(declaration) = declarations.next() else {
            break;
        };
        invoice.work(
            name.len()
                .saturating_add(declaration.name.len())
                .saturating_add(1),
        )?;
        if str::eq(name.as_str(), declaration.name.as_str()) {
            if matches!(found, Some(_)) {
                return Ok(false);
            }
            found = Some(declaration);
        }
    }
    let Some(declaration) = found else {
        return Ok(false);
    };
    if !authenticate(context, declaration, invoice)? {
        return Ok(false);
    }
    // A complete current source loan is now established. The ancestry only
    // rejects re-entry into that exact immutable declaration on this path.
    let mut index = 0;
    while index < depth {
        invoice.pay::<(
            usize,
            usize,
            usize,
            usize,
            bool,
            bool,
            &Option<&TypeDeclaration>,
            &Option<&TypeDeclaration>,
            &TypeDeclaration,
            &TypeDeclaration,
            &TypeDeclaration,
            &TypeDeclaration,
        )>()?;
        invoice.work(1)?;
        if matches!(ancestry[index], Some(prior) if std::ptr::eq(prior, declaration)) {
            return Ok(false);
        }
        index += 1;
    }
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return Ok(false);
    };
    invoice.pay::<(
        &TypeDeclarationKind,
        &TypeDeclarationKind,
        &Vec<crate::ast::FieldDeclaration>,
        &Vec<crate::ast::FieldDeclaration>,
        &[crate::ast::FieldDeclaration],
        &[crate::ast::FieldDeclaration],
        &[crate::ast::FieldDeclaration],
        &[crate::ast::FieldDeclaration],
        std::slice::Iter<'_, crate::ast::FieldDeclaration>,
        std::slice::Iter<'_, crate::ast::FieldDeclaration>,
        Option<&TypeDeclaration>,
        Option<&TypeDeclaration>,
        &mut Option<&TypeDeclaration>,
        &mut Option<&TypeDeclaration>,
    )>()?;
    ancestry[depth] = Some(declaration);
    let mut fields = fields.iter();
    loop {
        invoice.pay::<(
            &mut std::slice::Iter<'_, crate::ast::FieldDeclaration>,
            &mut std::slice::Iter<'_, crate::ast::FieldDeclaration>,
            Option<&crate::ast::FieldDeclaration>,
            Option<&crate::ast::FieldDeclaration>,
            &crate::ast::FieldDeclaration,
            &crate::ast::FieldDeclaration,
        )>()?;
        invoice.pay::<(&Type, &Type, ResultBool, ResultBool, usize, usize)>()?;
        let Some(field) = fields.next() else {
            break;
        };
        invoice.work(1)?;
        if !visit(context, &field.ty, invoice, ancestry, depth + 1)? {
            ancestry[depth] = None;
            return Ok(false);
        }
    }
    ancestry[depth] = None;
    Ok(true)
}

fn authenticate(
    context: Context<'_, '_>,
    declaration: &TypeDeclaration,
    invoice: &Invoice,
) -> ResultBool {
    use crate::workspace_graph::AuthoredDeclaration;
    type Auth<'p, 'a> = (
        Context<'p, 'a>,
        Context<'p, 'a>,
        &'p TypeDeclaration,
        &'p TypeDeclaration,
        &'p Invoice,
        &'p Invoice,
        &'p Program,
        &'p Program,
        &'p [Program],
        &'p [Program],
        std::slice::Iter<'p, Program>,
        std::slice::Iter<'p, Program>,
        &'p mut std::slice::Iter<'p, Program>,
        &'p mut std::slice::Iter<'p, Program>,
        Option<&'p Program>,
        Option<&'p Program>,
        bool,
        bool,
        bool,
        bool,
        &'p Vec<crate::ast::TypeParameterDeclaration>,
        &'p Vec<crate::ast::TypeParameterDeclaration>,
        &'p str,
        &'p str,
        &'p str,
        &'p str,
        &'p str,
        &'p str,
        &'p str,
        &'p str,
        &'p String,
        &'p String,
        usize,
        usize,
        usize,
        usize,
        &'p super::super::Index<'a>,
        &'p super::super::Index<'a>,
        &'p str,
        &'p str,
        &'p str,
        &'p str,
        &'p String,
        &'p String,
        Option<&'p AuthoredDeclaration<'a>>,
        Option<&'p AuthoredDeclaration<'a>>,
        &'p AuthoredDeclaration<'a>,
        &'p AuthoredDeclaration<'a>,
        Option<&'p TypeDeclaration>,
        Option<&'p TypeDeclaration>,
        &'p TypeDeclaration,
        &'p TypeDeclaration,
        ResultBool,
        ResultBool,
        usize,
        usize,
        usize,
        usize,
    );
    invoice.pay::<Auth<'_, '_>>()?;
    let mut providers = context.programs.iter();
    let mut present = false;
    loop {
        invoice.pay::<(
            &mut std::slice::Iter<'_, Program>,
            &mut std::slice::Iter<'_, Program>,
            Option<&Program>,
            Option<&Program>,
            &Program,
            &Program,
            &Program,
            &Program,
            *const Program,
            *const Program,
            bool,
            bool,
        )>()?;
        let Some(provider) = providers.next() else {
            break;
        };
        invoice.work(1)?;
        if std::ptr::eq(provider, context.program) {
            present = true;
            break;
        }
    }
    if !present
        || !declaration.explicit_id
        || !declaration.type_parameters.is_empty()
        || matches!(declaration.extends, Some(_))
        || matches!(declaration.invariants, Some(_))
    {
        return Ok(false);
    }
    invoice.work(
        declaration
            .stable_id
            .len()
            .saturating_add(context.authored.len())
            .saturating_add(1),
    )?;
    let Some(target) = context.authored.get(declaration.stable_id.as_str()) else {
        return Ok(false);
    };
    let Some(held) = target.ty else {
        return Ok(false);
    };
    Ok(matches!(target.kind, AuthoredKind::Type)
        && target.explicit
        && target.module == context.program.module
        && target.path == context.program.path
        && std::ptr::eq(held, declaration))
}

#[cfg(test)]
#[path = "record_signature_tests.rs"]
pub(in crate::workspace_graph::expected_projection) mod tests;
