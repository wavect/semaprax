//! Supported package interface selection; never a source-privacy boundary.
use super::{LibrarySource, PackageMetadata};
use semaprax::ast::{Type, TypeDeclarationKind};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn parse(package: &serde_json::Value) -> Result<Option<Vec<String>>, &'static str> {
    let Some(value) = package.get("api") else {
        return Ok(None);
    };
    let entries = value.as_array().ok_or("package api must be an array")?;
    if entries.is_empty() {
        return Err("package api must not be empty");
    }
    let mut ids = Vec::new();
    for entry in entries {
        let id = entry
            .as_str()
            .ok_or("package api identities must be strings")?;
        if id.is_empty() || id.len() > 128 {
            return Err("package api identity extent is invalid");
        }
        if ids
            .last()
            .is_some_and(|previous: &String| previous.as_str() >= id)
        {
            return Err("package api identities must be strictly sorted and unique");
        }
        ids.push(id.to_owned());
    }
    Ok(Some(ids))
}

pub(super) fn selected(package: &PackageMetadata, id: &str) -> bool {
    package
        .api
        .as_ref()
        .is_none_or(|ids| ids.iter().any(|entry| entry == id))
}

pub(super) fn validate(package: &PackageMetadata, libraries: &[LibrarySource]) {
    let Some(api) = &package.api else { return };
    let declared: BTreeSet<&str> = libraries
        .iter()
        .flat_map(|library| {
            library
                .program
                .functions
                .iter()
                .map(|function| function.stable_id.as_str())
                .chain(library.program.types.iter().map(|ty| ty.stable_id.as_str()))
        })
        .collect();
    for id in api {
        assert!(
            id.starts_with(&format!("{}.", package.module)) && declared.contains(id.as_str()),
            "{}: selected API identity `{id}` must resolve to a declaration in the package",
            package.module
        );
    }
    for library in libraries {
        let nominals: BTreeMap<&str, &str> = library
            .program
            .types
            .iter()
            .map(|ty| (ty.name.as_str(), ty.stable_id.as_str()))
            .collect();
        for function in &library.program.functions {
            if selected(package, &function.stable_id) {
                for ty in function
                    .params
                    .iter()
                    .map(|param| &param.ty)
                    .chain(std::iter::once(&function.return_type))
                {
                    require_nominals(package, ty, &nominals);
                }
            }
        }
        for declaration in &library.program.types {
            if !selected(package, &declaration.stable_id) {
                continue;
            }
            if let Some(parent) = &declaration.extends {
                require_nominals(package, parent, &nominals);
            }
            match &declaration.kind {
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        require_nominals(package, &field.ty, &nominals)
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        for field in &case.fields {
                            require_nominals(package, &field.ty, &nominals)
                        }
                    }
                }
                TypeDeclarationKind::Resource { .. } => {}
            }
            if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
                for method in methods {
                    for ty in method
                        .params
                        .iter()
                        .map(|param| &param.ty)
                        .chain(std::iter::once(&method.return_type))
                    {
                        require_nominals(package, ty, &nominals);
                    }
                }
            }
        }
    }
}

#[test]
fn api_inventory_refuses_unknown_ids_and_omitted_signature_nominals() {
    let source = r#"module std.inventory;
@id("std.inventory.carrier")
record Carrier { @id("std.inventory.carrier.value") value: i64, }
@id("std.inventory.observe")
fn observe(carrier: borrow Carrier) -> i64 { match borrow carrier { Carrier { value } => value, } }
"#;
    let library = LibrarySource {
        path: "inventory.spx".into(),
        program: semaprax::parse(source, "inventory.spx").unwrap(),
        source: source.into(),
    };
    let package = |api: Vec<&str>| PackageMetadata {
        directory: "inventory".into(),
        module: "std.inventory".into(),
        tier: "alloc".into(),
        targets: vec!["interpreter".into()],
        status: "partial".into(),
        api: Some(api.into_iter().map(str::to_owned).collect()),
    };
    let libraries = [library];
    for ids in [
        vec!["std.inventory.missing"],
        vec!["std.inventory.observe"],
        vec!["other.observe"],
    ] {
        assert!(std::panic::catch_unwind(|| validate(&package(ids), &libraries)).is_err());
    }
    validate(
        &package(vec!["std.inventory.carrier", "std.inventory.observe"]),
        &libraries,
    );
    let mut legacy = package(vec![]);
    legacy.api = None;
    assert!(selected(&legacy, "std.inventory.observe"));
    assert!(selected(&legacy, "std.inventory.carrier"));
    validate(&legacy, &libraries);
}

fn require_nominals(package: &PackageMetadata, ty: &Type, nominals: &BTreeMap<&str, &str>) {
    match ty {
        Type::Named { name, arguments } => {
            if let Some(id) = nominals.get(name.as_str()) {
                assert!(
                    selected(package, id),
                    "{}: nominal signature identity `{id}` is absent from API inventory",
                    package.module
                );
            }
            for argument in arguments {
                require_nominals(package, argument, nominals)
            }
        }
        Type::Function { parameters, result } => {
            for parameter in parameters {
                require_nominals(package, parameter, nominals)
            }
            require_nominals(package, result, nominals);
        }
        _ => {}
    }
}

#[test]
fn api_inventory_refuses_empty_reordered_duplicate_and_nonstrings() {
    for value in [
        serde_json::json!([]),
        serde_json::json!(["std.x.z", "std.x.a"]),
        serde_json::json!(["std.x.a", "std.x.a"]),
        serde_json::json!([1]),
        serde_json::Value::Null,
    ] {
        assert!(parse(&serde_json::json!({"api":value})).is_err());
    }
    assert_eq!(parse(&serde_json::json!({})).unwrap(), None);
    assert_eq!(
        parse(&serde_json::json!({"api":["std.x.a","std.x.b"]})).unwrap(),
        Some(vec!["std.x.a".into(), "std.x.b".into()])
    );
}
