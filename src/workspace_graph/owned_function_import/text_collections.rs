//! Private text/collection imports; public and frozen Project selectors stay separate.
use super::*;

pub(super) fn admitted(
    caller: &Program,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    layout: &crate::workspace_graph::expected_projection::private_layout::LayoutClosure<'_>,
) -> Result<bool, ()> {
    let Some(function) = target.function else {
        return Ok(false);
    };
    let signature_root = target.explicit
        && caller.module_uses.iter().any(|u| {
            u.kind == ModuleUseKind::Function
                && u.persistent_id == function.stable_id
                && u.target_module == target.module
        })
        && authored
            .get(function.stable_id.as_str())
            .is_some_and(|entry| {
                entry
                    .function
                    .is_some_and(|held| std::ptr::eq(held, function))
                    && entry.module == target.module
                    && entry.explicit
            });
    if !function.type_parameters.is_empty()
        || function.yields.is_some()
        || function.follows.is_some()
    {
        return Ok(false);
    }
    let refused = std::cell::Cell::new(false);
    let shape = |ty: &Type| {
        if scalar(ty) || matches!(ty, Type::ArrayU8(_)) {
            return Some(false);
        }
        if matches!(ty, Type::String | Type::Bytes) || crate::map_ops::ast_collection(ty) {
            return Some(true);
        }
        if signature_root {
            match layout.signature_owner(caller, target, ty, authored, programs) {
                Ok(Some(owner)) => return Some(owner),
                Err(_) => {
                    refused.set(true);
                    return None;
                }
                Ok(None) => {}
            }
            // A private nominal root must never fall back to the allocating
            // explicit-import scanner if its independent closure refused.
            if crate::workspace_graph::expected_projection::private_layout::signature_is_private(
                caller,
                target.module,
                ty,
                programs,
            ) {
                refused.set(true);
                return None;
            }
        }
        shape(
            target.module,
            ty,
            caller,
            authored,
            programs,
            signature_root,
        )
    };
    let added = |ty: &Type| {
        crate::map_ops::ast_collection(ty)
            || *ty == Type::String
            || (matches!(ty, Type::Named { .. }) && shape(ty).is_some())
    };
    let selected = added(&function.return_type) || function.params.iter().any(|p| added(&p.ty));
    let admitted = selected
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
        });
    if refused.get() {
        Err(())
    } else {
        Ok(admitted)
    }
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
    shape(target.module, &ty, caller, authored, programs, false).is_some()
}

// Re-derive a bounded record closure. Only its root is source-visible; descendants
// remain private layouts and are copied under independently fresh synthetic names.
// The boolean distinguishes owners from Copy records; no generic substitution occurs.
fn shape<'a>(
    module: &'a str,
    root: &'a Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    signature_root: bool,
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
            Frame::Enter(module, ty, depth)
                if vector(module, ty, caller, authored, programs, depth > 1)
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
                if depth == 1
                    && !signature_root
                    && caller.module != target.module
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
                            vector(target.module, &payload.ty, caller, authored, programs, true)
                        });
                    let string_payload = matches!(payloads.as_slice(), [payload]
                        if payload.ty == Type::String);
                    let stream_payload = matches!(payloads.as_slice(), [bytes, length]
                        if bytes.ty == Type::Bytes && length.ty == Type::Usize);
                    if !record_payload && !vector_payload && !string_payload && !stream_payload {
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
    descendant: bool,
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
        && (descendant
            || caller.module == target.module
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
        let Ok(layout) =
            crate::workspace_graph::expected_projection::private_layout::LayoutClosure::derive(
                &programs[1],
                &authored,
                &programs,
            )
        else {
            return false;
        };
        admitted(
            &programs[1],
            authored.get("inspect").unwrap(),
            &authored,
            &programs,
            &layout,
        )
        .unwrap_or(false)
    }
    #[test]
    fn nested_outcome_imports_rederive_vector_elements_and_explicit_root() {
        assert!(accepts(PROVIDER, APP));
        assert!(accepts(
            PROVIDER,
            &APP.replace("use type @id(\"item\") from imports.provider as Item;", "")
        ));
        assert!(!accepts(
            PROVIDER,
            &APP.replace(
                "use type @id(\"outcome\") from imports.provider as Outcome;",
                ""
            )
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
    fn nested_string_config_and_row_vector_rederive_private_layout_closure() {
        // Same ordinary carrier composition as the native codec fault witness:
        // scalar refusal / Report containing a String Config and flat String rows.
        let provider = PROVIDER
            .replace(
                "@id(\"report\") record Report",
                "@id(\"config\") record Config { @id(\"config.label\") label:string, @id(\"config.retry\") retry:usize, }\n@id(\"report\") record Report",
            )
            .replace(
                "@id(\"report.items\") items: Vec<Item>,",
                "@id(\"report.config\") config:Config, @id(\"report.items\") items: Vec<Item>,",
            );
        let config_import = "use type @id(\"config\") from imports.provider as Config;";
        let app = APP.replace(
            "use type @id(\"report\")",
            &format!("{config_import}\nuse type @id(\"report\")"),
        );
        assert!(accepts(&provider, &app));
        for required in [
            config_import,
            "use type @id(\"item\") from imports.provider as Item;",
            "use type @id(\"report\") from imports.provider as Report;",
        ] {
            assert!(app.contains(required));
            assert!(accepts(&provider, &app.replace(required, "")));
        }
        for forged in [
            provider.replace("label:string", "label:str"),
            provider.replace("items: Vec<Item>", "items: Vec<Vec<Item>>"),
            provider.replace("@id(\"config.label\") ", ""),
            provider.replace("config:Config", "config:Outcome"),
        ] {
            assert_ne!(forged, provider);
            assert!(!accepts(&forged, &app));
        }
    }

    #[test]
    fn stream_input_import_replays_exact_owner_length_and_error_shape() {
        let provider = r#"module imports.provider;
@id("outcome") variant Input {
 @id("ready") Ready { @id("ready.bytes") bytes:Bytes, @id("ready.length") length:usize, },
 @id("error") Error { @id("error.code") code:i64, @id("error.offset") offset:usize, @id("error.field") field:i64, },
}
@id("inspect") fn inspect()->Input {Input::Error{code:1,offset:0usize,field:0}}
"#;
        let app = r#"module imports.app;
use type @id("outcome") from imports.provider as Input;
@id("app.main") fn main()->i64{0}
"#;
        assert!(accepts(provider, app));
        for forged in [
            provider.replace("bytes:Bytes", "bytes:Slice<u8>"),
            provider.replace("length:usize", "length:i64"),
            provider.replace(
                "bytes:Bytes, @id(\"ready.length\") length:usize",
                "bytes:usize, @id(\"ready.length\") length:Bytes",
            ),
            provider.replace("offset:usize", "offset:i64"),
            provider.replace("@id(\"ready.bytes\") ", ""),
        ] {
            assert_ne!(forged, provider);
            assert!(!accepts(&forged, app));
        }
        assert!(!accepts(
            provider,
            &app.replace(
                "use type @id(\"outcome\") from imports.provider as Input;",
                ""
            )
        ));
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
        // A vector-payload outcome is not an inferred private-layout root
        // (Private Nominal Layout Closure v1: only records and the exact
        // record-payload refusal outcome are); its Item element keeps the
        // explicit import route, so the layout closure refuses the omission.
        assert!(!accepts(
            provider,
            &app.replace("use type @id(\"item\") from imports.provider as Item;", "")
        ));
        assert!(!accepts(
            provider,
            &app.replace(
                "use type @id(\"outcome\") from imports.provider as Outcome;",
                ""
            )
        ));
        assert!(!accepts(&provider.replace("@id(\"item.text\") ", ""), app));
        assert!(!accepts(
            &provider.replace("fn inspect()", "fn inspect<T>()"),
            app
        ));
    }
}
