//! Private text/collection imports; public and frozen Project selectors stay separate.
use super::*;

pub(super) fn admitted(
    caller: &Program,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    let Some(function) = target.function else {
        return false;
    };
    if !function.type_parameters.is_empty()
        || function.yields.is_some()
        || function.follows.is_some()
    {
        return false;
    }
    let shape = |ty: &Type| shape(target.module, ty, caller, authored, programs);
    let added = |ty: &Type| {
        crate::map_ops::ast_collection(ty)
            || *ty == Type::String
            || (matches!(ty, Type::Named { .. }) && shape(ty).is_some())
    };
    let selected = added(&function.return_type) || function.params.iter().any(|p| added(&p.ty));
    selected
        && shape(&function.return_type).is_some()
        && function.params.iter().all(|p| {
            if crate::map_ops::ast_collection(&p.ty) {
                return matches!(p.mode, ParamMode::Own | ParamMode::Borrow);
            }
            match &p.ty {
                Type::String => matches!(p.mode, ParamMode::Value | ParamMode::Own),
                Type::Bytes => p.mode == ParamMode::Own,
                Type::Str | Type::SliceU8 => p.mode == ParamMode::Borrow,
                ty if scalar(ty) || matches!(ty, Type::ArrayU8(_)) => p.mode == ParamMode::Value,
                _ => match shape(&p.ty) {
                    Some(true) => matches!(p.mode, ParamMode::Own | ParamMode::Borrow),
                    Some(false) => p.mode == ParamMode::Value,
                    None => false,
                },
            }
        })
}

pub(super) fn record_import(
    caller: &Program,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    let Some(declaration) = target.ty else {
        return false;
    };
    let ty = Type::Named {
        name: declaration.name.clone(),
        arguments: Vec::new(),
    };
    shape(target.module, &ty, caller, authored, programs).is_some()
}

// Re-derive a bounded explicit record closure, including each exposed type import.
// The boolean distinguishes owners from Copy records; no generic substitution occurs.
fn shape<'a>(
    module: &'a str,
    root: &'a Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Option<bool> {
    enum Frame<'a> {
        Enter(&'a str, &'a Type, usize),
        Leave(String),
    }
    let mut pending = vec![Frame::Enter(module, root, 1)];
    let mut active = BTreeSet::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut owns = false;
    let mut outcome = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(module, ty, _)
                if vector(module, ty, caller, authored, programs)
                    || crate::map_ops::ast_collection(ty)
                    || matches!(ty, Type::String | Type::Bytes) =>
            {
                owns = true;
                leaves += 1;
            }
            Frame::Enter(_, ty, depth)
                if scalar(ty) || (depth == 1 && matches!(ty, Type::ArrayU8(_))) => {}
            Frame::Enter(module, Type::Named { name, arguments }, depth) => {
                if !arguments.is_empty() || depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                    return None;
                }
                let id = resolve_type_id(module, name, programs)?;
                let target = authored.get(id.as_str())?;
                let declaration = target.ty?;
                if !target.explicit
                    || !declaration.explicit_id
                    || !declaration.type_parameters.is_empty()
                    || !declaration.invariants().is_empty()
                    || !active.insert(id.clone())
                {
                    return None;
                }
                if caller.module != target.module
                    && !caller
                        .module_uses
                        .iter()
                        .any(|u| u.kind == ModuleUseKind::Type && u.persistent_id == id)
                {
                    return None;
                }
                // A conditional owner is admitted only at the signature root.
                // Its finite record/vector/String payload and scalar refusal
                // are independently replayed; no borrowed or generic result escapes.
                if let TypeDeclarationKind::Variant { cases } = &declaration.kind {
                    if depth != 1
                        || outcome
                        || cases.len() != 2
                        || cases.iter().any(|case| {
                            !case.explicit_id || case.fields.iter().any(|field| !field.explicit_id)
                        })
                    {
                        return None;
                    }
                    let error = |case: &crate::ast::VariantCaseDeclaration| {
                        matches!(case.fields.as_slice(), [code, offset, field]
                        if code.ty == Type::I64 && offset.ty == Type::Usize && field.ty == Type::I64)
                    };
                    let payloads = if error(&cases[1]) {
                        &cases[0].fields
                    } else if error(&cases[0]) {
                        &cases[1].fields
                    } else {
                        // The response encoder returns exactly one owned String
                        // or one scalar required-length refusal, never a view.
                        let encoded = |case: &crate::ast::VariantCaseDeclaration| matches!(case.fields.as_slice(), [text] if text.ty == Type::String);
                        let refused = |case: &crate::ast::VariantCaseDeclaration| matches!(case.fields.as_slice(), [required] if required.ty == Type::Usize);
                        if encoded(&cases[0]) && refused(&cases[1]) {
                            &cases[0].fields
                        } else if encoded(&cases[1]) && refused(&cases[0]) {
                            &cases[1].fields
                        } else {
                            return None;
                        }
                    };
                    let record_payload = matches!(payloads.as_slice(), [payload]
                        if matches!(&payload.ty, Type::Named { name, arguments }
                            if arguments.is_empty() && resolve_type_id(target.module, name, programs)
                                .and_then(|id| authored.get(id.as_str()))
                                .and_then(|target| target.ty)
                                .is_some_and(|ty| matches!(&ty.kind, TypeDeclarationKind::Record { .. }))));
                    let vector_payload = (1..=2).contains(&payloads.len())
                        && payloads.iter().all(|payload| {
                            vector(target.module, &payload.ty, caller, authored, programs)
                        });
                    let string_payload = matches!(payloads.as_slice(), [payload]
                        if payload.ty == Type::String);
                    if !record_payload && !vector_payload && !string_payload {
                        return None;
                    }
                    outcome = true;
                    fields += cases.iter().map(|case| case.fields.len()).sum::<usize>();
                    pending.push(Frame::Leave(id));
                    pending.extend(
                        payloads
                            .iter()
                            .rev()
                            .map(|payload| Frame::Enter(target.module, &payload.ty, depth + 1)),
                    );
                    continue;
                }
                let TypeDeclarationKind::Record { fields: declared } = &declaration.kind else {
                    return None;
                };
                if declared.iter().any(|field| !field.explicit_id)
                    || (outcome
                        && declared
                            .iter()
                            .any(|field| crate::map_ops::ast_collection(&field.ty)))
                {
                    return None;
                }
                fields += declared.len();
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return None;
                }
                pending.push(Frame::Leave(id));
                pending.extend(
                    declared
                        .iter()
                        .rev()
                        .map(|f| Frame::Enter(target.module, &f.ty, depth + 1)),
                );
            }
            Frame::Leave(id) => {
                active.remove(&id);
            }
            _ => return None,
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return None;
        }
    }
    Some(owns)
}

// Vec element admission stays flat and independent of the enclosing record.
fn vector(
    module: &str,
    ty: &Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    let Some(provider) = programs.iter().find(|program| program.module == module) else {
        return false;
    };
    let identity = resolve_type_id(module, name, programs).or_else(|| {
        crate::prelude::declarations_for_program(provider)
            .iter()
            .find(|declaration| declaration.name == *name)
            .map(|declaration| declaration.stable_id.clone())
    });
    if identity.as_deref() != Some(crate::prelude::VEC_ID) {
        return false;
    }
    let [element] = arguments.as_slice() else {
        return false;
    };
    if crate::vec_ops::ast_vec_element_is_admitted(element) || *element == Type::String {
        return true;
    }
    let Type::Named { name, arguments } = element else {
        return false;
    };
    if !arguments.is_empty() {
        return false;
    }
    let Some(id) = resolve_type_id(module, name, programs) else {
        return false;
    };
    let Some(target) = authored.get(id.as_str()) else {
        return false;
    };
    let Some(declaration) = target.ty else {
        return false;
    };
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return false;
    };
    let owned = fields
        .iter()
        .filter(|field| matches!(field.ty, Type::String | Type::Bytes))
        .count();
    target.explicit
        && declaration.explicit_id
        && declaration.type_parameters.is_empty()
        && declaration.invariants().is_empty()
        && (1..=8).contains(&fields.len())
        && owned <= 2
        && fields.iter().all(|field| {
            field.explicit_id
                && (scalar(&field.ty) || matches!(field.ty, Type::String | Type::Bytes))
        })
        && (caller.module == target.module
            || caller
                .module_uses
                .iter()
                .any(|u| u.kind == ModuleUseKind::Type && u.persistent_id == id))
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROVIDER: &str = r#"
module imports.provider;
@id("item") record Item { @id("item.text") text: string, @id("item.n") n: i64, }
@id("report") record Report { @id("report.items") items: Vec<Item>, }
@id("outcome") variant Outcome {
 @id("ok") Ok { @id("ok.report") report: Report, },
 @id("err") Err { @id("err.code") code: i64, @id("err.offset") offset: usize, @id("err.field") field: i64, },
}
@id("inspect") fn inspect(value: borrow Outcome) -> i64 { 0 }
"#;
    const APP: &str = r#"
module imports.app;
use type @id("item") from imports.provider as Item;
use type @id("report") from imports.provider as Report;
use type @id("outcome") from imports.provider as Outcome;
@id("app.main") fn main() -> i64 { 0 }
"#;
    fn accepts(provider: &str, app: &str) -> bool {
        let programs = [
            crate::parse(provider, "provider.spx").unwrap(),
            crate::parse(app, "app.spx").unwrap(),
        ];
        let authored = super::super::super::index_authored(&programs).unwrap();
        admitted(
            &programs[1],
            authored.get("inspect").unwrap(),
            &authored,
            &programs,
        )
    }
    #[test]
    fn nested_outcome_imports_rederive_vector_elements_and_direct_types() {
        assert!(accepts(PROVIDER, APP));
        assert!(!accepts(
            PROVIDER,
            &APP.replace("use type @id(\"item\") from imports.provider as Item;", "")
        ));
        assert!(!accepts(
            &PROVIDER.replace("text: string", "text: Vec<string>"),
            APP
        ));
        assert!(!accepts(
            &PROVIDER.replace("offset: usize", "offset: i64"),
            APP
        ));
        assert!(!accepts(&PROVIDER.replace("@id(\"ok.report\") ", ""), APP));
    }

    #[test]
    fn string_response_import_preserves_exact_scalar_refusal_and_excludes_views() {
        let provider = r#"module imports.provider;
@id("outcome") variant Encoded {
 @id("encoded") Encoded { @id("encoded.text") text:string, },
 @id("refused") Refused { @id("refused.required") required:usize, },
}
@id("inspect") fn inspect()->Encoded { Encoded::Refused{required:0usize} }
"#;
        let app = r#"module imports.app;
use type @id("outcome") from imports.provider as Outcome;
@id("app.main") fn main()->i64{0}
"#;
        assert!(accepts(provider, app));
        assert!(!accepts(
            &provider.replace("required:usize", "required:i64"),
            app
        ));
        assert!(!accepts(&provider.replace("text:string", "text:str"), app));
        assert!(!accepts(
            &provider.replace("@id(\"encoded.text\") ", ""),
            app
        ));
        assert!(!accepts(
            provider,
            &app.replace(
                "use type @id(\"outcome\") from imports.provider as Outcome;",
                ""
            )
        ));
    }
    #[test]
    fn decoded_vector_import_replays_flat_elements_and_refuses_nested_or_missing_types() {
        let provider = r#"module imports.provider;
@id("item") record Item { @id("item.text") text:string, @id("item.n") n:i64, }
@id("outcome") variant Decoded {
 @id("ok") Ready { @id("ok.words") words:Vec<string>, @id("ok.rows") rows:Vec<Item>, },
 @id("err") Error { @id("err.code") code:i64, @id("err.offset") offset:usize, @id("err.field") field:i64, },
}
@id("inspect") fn inspect()->Decoded { Decoded::Error{code:1,offset:0usize,field:0} }
"#;
        let app = r#"module imports.app;
use type @id("item") from imports.provider as Item;
use type @id("outcome") from imports.provider as Outcome;
@id("app.main") fn main()->i64{0}
"#;
        assert!(accepts(provider, app));
        assert!(!accepts(
            &provider.replace("text:string", "text:Vec<string>"),
            app
        ));
        assert!(!accepts(
            &provider.replace("offset:usize", "offset:i64"),
            app
        ));
        assert!(!accepts(
            provider,
            &app.replace("use type @id(\"item\") from imports.provider as Item;", "")
        ));
        assert!(!accepts(
            &provider.replace("fn inspect()", "fn inspect<T>()"),
            app
        ));
    }
}
