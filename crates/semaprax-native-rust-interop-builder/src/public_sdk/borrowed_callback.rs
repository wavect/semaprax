//! Synchronous Rust scopes for actual parameter-rooted source closures.
use super::*;
use semaprax::ast::{ExprKind, ParamMode, Span, Statement, Type};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeBorrowedCallbackProjection {
    pub source_revision: String,
    pub c_source: String,
    pub safe_rust: String,
}

/// Render an inert projection of one checked source entry that creates and
/// invokes a borrowed closure. The entry is called inside each Rust invocation;
/// the generated callable cannot escape its synchronous borrowed scope.
pub fn prepare_native_rust_borrowed_callback(
    source: &str,
    path: &Path,
    entry_id: &str,
) -> Result<NativeBorrowedCallbackProjection, Vec<Diagnostic>> {
    let fail = |message: &str| vec![Diagnostic::error("SPX-B154", message, Span::default())];
    if source.len() > MAX_SOURCE_BYTES {
        return Err(fail("borrowed callback source exceeds its bound"));
    }
    let program = semaprax::check(source, path)?;
    let entry = program
        .functions
        .iter()
        .find(|f| f.stable_id == entry_id)
        .ok_or_else(|| fail("borrowed callback entry is absent"))?;
    if !entry.explicit_id
        || !entry.type_parameters.is_empty()
        || entry.params.len() != 2
        || entry.params[0].ty != Type::Str
        || entry.params[0].mode != ParamMode::Borrow
        || entry.params[1].ty != Type::I64
        || entry.params[1].mode != ParamMode::Value
        || entry.return_type != Type::I64
        || !entry.effects.is_empty()
        || entry.yields.is_some()
        || !program.interfaces.is_empty()
    {
        return Err(fail(
            "borrowed callback entry requires pure fn(borrow str,i64)->i64",
        ));
    }
    let ExprKind::Block { statements, tail } = &entry.body.kind else {
        return Err(fail(
            "borrowed callback entry requires direct creation and invocation",
        ));
    };
    let [Statement::Let {
        name,
        mutable: false,
        value,
        ..
    }] = statements.as_slice()
    else {
        return Err(fail(
            "borrowed callback entry requires one immutable closure local",
        ));
    };
    let ExprKind::Call {
        name: called,
        type_arguments,
        args,
    } = &tail.kind
    else {
        return Err(fail("borrowed callback entry must invoke its closure"));
    };
    if called != name
        || !type_arguments.is_empty()
        || args.len() != 1
        || !matches!(&args[0].kind,ExprKind::Var(argument) if argument==&entry.params[1].name)
        || !matches!(
            &value.kind,
            ExprKind::Closure {
                owning: false,
                retained: false,
                mutable: false,
                ..
            }
        )
    {
        return Err(fail(
            "borrowed callback entry must invoke its exact closure argument",
        ));
    }
    let resolved = semaprax::hir::resolve(&program)?;
    semaprax::hir::validate(&resolved).map_err(|e| vec![e])?;
    let closure = semaprax::hir::closure::inventory(&resolved)
        .into_iter()
        .find(|e| e.span == value.span)
        .ok_or_else(|| fail("borrowed source closure is absent"))?;
    let semaprax::hir::ResolvedExprKind::Closure { captures, .. } = &closure.kind else {
        unreachable!()
    };
    if captures.len() != 1
        || captures[0].binding.ty != semaprax::hir::ResolvedType::Str
        || captures[0].binding.ownership != semaprax::hir::OwnershipMode::Borrow
        || captures[0].binding.name != entry.params[0].name
    {
        return Err(fail(
            "borrowed callback must capture the exact shared source parameter",
        ));
    }
    semaprax::hir::closure::closure_function(&resolved, closure).map_err(|e| vec![e])?;
    let source_revision = domain_digest(
        b"semaprax.borrowed-callback-source.v1\0",
        semaprax::format::canonical(&program).as_bytes(),
    );
    let mut c_source = String::from("#define SPX_NO_ENTRY_WRAPPER 1\n");
    c_source.push_str(&semaprax::codegen::emit_c(&program).map_err(|e| vec![e])?);
    let symbol = format!(
        "spx_decl_{}",
        entry_id
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    c_source
        .push_str(&include_str!("borrowed_callback_c.template").replace("ENTRY_SYMBOL", &symbol));
    Ok(NativeBorrowedCallbackProjection {
        source_revision,
        c_source,
        safe_rust: include_str!("borrowed_callback_rust.template").into(),
    })
}
