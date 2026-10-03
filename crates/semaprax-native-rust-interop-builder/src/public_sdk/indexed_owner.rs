//! Selected-index admission and Project packaging for the narrow owner profile.
use super::*;
use semaprax::hir::{OwnershipMode, ResolvedImportResultKind, ResolvedProgram, ResolvedType};
use semaprax_rust_api_index::{
    ApiItem, ItemKind, Receiver, RustApiIndex, TypeRecordKind, Visibility,
};

/// Recheck the complete public type closure, not only a matching method name.
pub(super) fn require_owner_type(index: &RustApiIndex, item: &ApiItem) -> Result<(), &'static str> {
    let (path, _) = item
        .path
        .rsplit_once("::")
        .ok_or("selected owner type path is absent")?;
    let ty = index
        .types()
        .iter()
        .find(|ty| ty.path == path)
        .ok_or("selected owner type is absent from index")?;
    if item.kind != ItemKind::InherentMethod
        || !matches!(item.receiver, Receiver::None | Receiver::Owned)
        || !item.generics.parameters.is_empty()
        || !item.generics.where_predicates.is_empty()
        || !item.closure_complete
        || !item.type_roots.iter().any(|root| root == path)
        || !item.reachable_types.iter().any(|root| root == path)
        || ty.kind != TypeRecordKind::Struct
        || ty.visibility != Visibility::Public
        || !ty.generics.parameters.is_empty()
        || !ty.generics.where_predicates.is_empty()
    {
        return Err(
            "selected owner requires one public nongeneric indexed struct and inherent method",
        );
    }
    Ok(())
}

pub(super) fn build(
    program: &ResolvedProgram,
    subject: &project::ProjectSdkSubject,
    selections: &[IndexedProjectScalarSelection<'_>],
    output: &Path,
) -> Result<NativeRustSdkBundle, Vec<Diagnostic>> {
    if selections.len() != 2 || subject.exports.len() != 1 || !subject.capabilities.is_empty() {
        return Err(vec![sdk_error(
            "indexed owner Project requires one export and two selected imports",
        )]);
    }
    let manifest = semaprax::project::ProjectManifest::parse(&subject.manifest)?;
    if !manifest.rust_dependencies().is_empty() {
        return Err(vec![sdk_error(
            "experimental owner package admits only the explicit self-contained selected source",
        )]);
    }
    let first = selections[0].selection;
    let mut binding_rows = Vec::new();
    let mut owner_identity: Option<(String, String)> = None;
    let mut constructor = false;
    let mut consuming = false;
    let mut signature_checks = String::new();
    let mut ordered = selections.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|s| s.selection.import_id);
    for selected in ordered {
        let selection = selected.selection;
        let package = selection.package;
        if selection.index_bytes != first.index_bytes
            || selection.package_source_bytes != first.package_source_bytes
            || package.cargo_alias != first.package.cargo_alias
            || package.target != target_triple().unwrap_or("")
        {
            return Err(vec![sdk_error(
                "indexed owner methods must select the same exact package and target",
            )]);
        }
        let import = program
            .interfaces
            .iter()
            .flat_map(|i| &i.imports)
            .find(|i| i.id.as_str() == selection.import_id)
            .ok_or_else(|| vec![sdk_error("indexed owner import is absent")])?;
        let located = |code, message| {
            vec![Diagnostic::error(code, message, import.span).at_path(selected.source_path)]
        };
        let index = RustApiIndex::replay(selection.index_bytes)
            .map_err(|_| located("SPX-B142", "selected owner index replay failed"))?;
        index
            .require_package_identity(
                package.name,
                package.version,
                package.source_sha256,
                package.target,
                package.feature_digest,
            )
            .and_then(|_| index.require_cargo_alias_identity(package.cargo_alias))
            .and_then(|_| index.require_stable_compiler_identity(package.stable_rustc_version))
            .map_err(|_| located("SPX-B142", "selected owner package identity drifted"))?;
        let path = import
            .rust_path
            .as_deref()
            .ok_or_else(|| located("SPX-B143", "selected owner Rust path is absent"))?;
        let items = index
            .select_supported(&[path])
            .map_err(|_| located("SPX-B141", "selected owner method is unavailable"))?;
        let item = items[0];
        require_owner_type(&index, item).map_err(|message| located("SPX-B145", message))?;
        if path.split("::").next() != Some(package.cargo_alias) {
            return Err(located(
                "SPX-B143",
                "selected owner path disagrees with Cargo alias",
            ));
        }
        let mut source_program =
            semaprax::parse(selected.source, selected.source_path).map_err(|e| vec![e])?;
        let declaration = source_program
            .interfaces
            .iter_mut()
            .flat_map(|i| &mut i.imports)
            .find(|i| i.stable_id == selection.import_id)
            .ok_or_else(|| located("SPX-B143", "selected owner source import is absent"))?;
        let bound = semaprax::native_rust_binding::bind_selected_owner_signature(
            declaration,
            &source_program.types,
            &item.signature,
            index.digest(),
            if item.receiver == Receiver::Owned {
                "owned"
            } else {
                "none"
            },
        )
        .map_err(|e| vec![e.at_path(selected.source_path)])?;
        if !bound {
            return Err(located(
                "SPX-B145",
                "selected owner signature is outside the profile",
            ));
        }

        if !import.index_selected
            || import.selected_index_digest.as_deref() != Some(index.digest())
            || import.selected_receiver.as_deref().unwrap_or("none")
                != if item.receiver == Receiver::Owned {
                    "owned"
                } else {
                    "none"
                }
        {
            return Err(located(
                "SPX-B146",
                "selected owner binding has drifted from its index",
            ));
        }
        let resource = match (&import.result.kind, item.receiver) {
            (ResolvedImportResultKind::OwnedResource { resource }, Receiver::None)
                if !constructor =>
            {
                constructor = true;
                resource
            }
            (ResolvedImportResultKind::Bool, Receiver::Owned) if !consuming => {
                consuming = true;
                match import.parameters.first() {
                    Some(p) if p.ownership == OwnershipMode::Own => match &p.ty {
                        ResolvedType::Nominal {
                            declaration,
                            arguments,
                        } if arguments.is_empty() => declaration,
                        _ => {
                            return Err(located(
                                "SPX-B145",
                                "selected owner receiver is not a resource",
                            ))
                        }
                    },
                    _ => {
                        return Err(located(
                            "SPX-B145",
                            "selected owner receiver has no ownership",
                        ))
                    }
                }
            }
            _ => {
                return Err(located(
                    "SPX-B145",
                    "selected owner constructor/method pair is invalid",
                ))
            }
        };
        let type_path = path.rsplit_once("::").expect("checked path").0;
        let tokens = semaprax::native_rust_binding::rust_api_path_tokens(path)
            .ok_or_else(|| located("SPX-B143", "selected owner path cannot be emitted"))?;
        let type_tokens = semaprax::native_rust_binding::rust_api_path_tokens(type_path)
            .ok_or_else(|| located("SPX-B143", "selected owner type cannot be emitted"))?;
        if item.receiver == Receiver::Owned {
            writeln!(
                signature_checks,
                "const _: fn({type_tokens},i64)->bool={tokens};"
            )
            .expect("writing checked signature");
        } else {
            writeln!(
                signature_checks,
                "const _: fn(i64)->{type_tokens}={tokens};"
            )
            .expect("writing checked signature");
        }
        let identity = (resource.as_str().to_owned(), type_path.to_owned());
        if owner_identity
            .as_ref()
            .is_some_and(|previous| previous != &identity)
        {
            return Err(located(
                "SPX-B143",
                "selected owner resource and Rust type identities disagree",
            ));
        }
        owner_identity = Some(identity);
        binding_rows.push(serde_json::json!({"id":import.id.as_str(),"rust_path":path,"signature":item.signature,"receiver":import.selected_receiver.as_deref().unwrap_or("none")}));
    }
    if !constructor || !consuming {
        return Err(vec![sdk_error(
            "selected owner constructor/method pair is incomplete",
        )]);
    }
    binding_rows.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let generated = owner_sdk::render(program, &subject.exports[0].id).map_err(|e| vec![e])?;
    let alias = semaprax::native_rust_binding::rust_api_path_tokens(first.package.cargo_alias)
        .ok_or_else(|| vec![sdk_error("selected owner Cargo alias is invalid")])?;
    let source = std::str::from_utf8(first.package_source_bytes)
        .map_err(|_| vec![sdk_error("selected owner source is not UTF-8")])?;
    let library = format!(
        "mod {alias}{{\n{source}\n}}\n{signature_checks}\n{}",
        generated.rust_adapter
    );
    let provider = generated
        .c_source
        .replace("#include \"owner.h\"", &generated.header);
    let (resource, type_path) = owner_identity.expect("checked owner pair");
    let descriptor = serde_json::json!({
        "schema":"semaprax.native-rust-opaque-owner-descriptor.experimental.v1",
        "target":first.package.target,"stable_rustc_version":first.package.stable_rustc_version,
        "subject":serde_json::from_str::<Value>(&subject.canonical).expect("checked Project subject"),
        "binding":{"resource_id":resource,"rust_type":type_path,"package_name":first.package.name,"package_version":first.package.version,"cargo_alias":first.package.cargo_alias,"package_source_sha256":first.package.source_sha256,"index_digest":RustApiIndex::replay(first.index_bytes).expect("replayed index").digest(),"imports":binding_rows},
        "provider_sha256":raw_digest(provider.as_bytes()),"header_sha256":raw_digest(generated.header.as_bytes()),"rust_sha256":raw_digest(library.as_bytes()),
    });
    let mut descriptor = serde_json::to_vec(&descriptor).expect("JSON value serializes");
    descriptor.push(b'\n');
    let bundle = semaprax_native_rust_owned_data_package::build_opaque_owner_package(
        semaprax_native_rust_owned_data_package::OpaqueOwnerPackagePlan {
            descriptor,
            provider_c: provider.into_bytes(),
            header: generated.header.into_bytes(),
            rust_source: library.into_bytes(),
        },
        output,
    )
    .map_err(|error| {
        vec![Diagnostic::io(
            "SPX-B142",
            format!("experimental owner package failed: {error:?}"),
        )]
    })?;
    Ok(NativeRustSdkBundle {
        output_directory: bundle.output_directory().into(),
        manifest_path: bundle.manifest_path().into(),
        manifest_digest: bundle.manifest_digest().into(),
        crate_name: bundle.crate_name().into(),
        target_triple: bundle.target_triple().into(),
    })
}
