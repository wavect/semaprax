use std::path::Path;

use crate::{hir, parse};

use super::*;

fn resolve(source: &str) -> ResolvedProgram {
    let parsed = parse(source, Path::new("native-resource-test.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn source(resource_name: &str, interface_name: &str, import_name: &str) -> String {
    format!(
        r#"module test.native_resource;
permit {{ io.release }}

@id("token.type")
resource {resource_name} {{
    @id("token.drop")
    drop trivial;
}}

@id("file.type")
resource File {{
    @id("file.drop")
    drop import "file.finalize";
}}

@id("file.host")
interface {interface_name} permits {{ io.release }} {{
    @id("file.finalize")
    import fn {import_name}(file: own File) -> unit
        effects {{ io.release }}
        failure infallible
        consumes file always;
}}

@id("app.main")
fn main() -> i64 {{ 0 }}
"#
    )
}

#[test]
fn display_renames_do_not_change_the_resource_abi() {
    let first = build_resource_abi(&resolve(&source("Token", "FileHost", "finalize"))).unwrap();
    let second = build_resource_abi(&resolve(&source(
        "RenamedToken",
        "RenamedFileHost",
        "renamed_finalize",
    )))
    .unwrap();
    assert_eq!(first, second);
}

#[test]
fn distinct_resource_ids_produce_distinct_wrapper_types() {
    let abi = build_resource_abi(&resolve(&source("Token", "FileHost", "finalize"))).unwrap();
    assert_eq!(abi.resources.len(), 2);
    assert_ne!(abi.resources[0].c_type, abi.resources[1].c_type);
}

#[test]
fn generated_identifiers_fit_the_portable_internal_identifier_budget() {
    let abi = build_resource_abi(&resolve(&source("Token", "FileHost", "finalize"))).unwrap();
    let identifiers = abi
        .resources
        .iter()
        .map(|resource| resource.c_type.as_str())
        .chain(abi.lifecycles.iter().filter_map(|lifecycle| {
            let NativeFinalizerKind::Imported(finalizer) = &lifecycle.kind else {
                return None;
            };
            Some(finalizer.callback_type.as_str())
        }));
    for identifier in identifiers {
        assert!(
            identifier.len() <= 63,
            "identifier `{identifier}` is too long"
        );
        assert!(identifier
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'));
    }
}

#[test]
fn zero_payload_is_never_emitted_as_a_liveness_test() {
    let abi = build_resource_abi(&resolve(&source("Token", "FileHost", "finalize"))).unwrap();
    assert!(abi.declarations.contains("Payload zero is valid"));
    assert!(!abi.declarations.contains("payload =="));
    assert!(!abi.declarations.contains("payload !="));
    assert!(!abi.declarations.contains("NULL"));
}

#[test]
fn emission_is_deterministic() {
    let program = resolve(&source("Token", "FileHost", "finalize"));
    let first = build_resource_abi(&program).unwrap();
    let second = build_resource_abi(&program).unwrap();
    assert_eq!(first.declarations, second.declarations);
    assert_eq!(first.resources, second.resources);
    assert_eq!(first.lifecycles, second.lifecycles);

    let file = first
        .resources
        .iter()
        .find(|resource| resource.resource_id.as_str() == "file.type")
        .unwrap();
    let token = first
        .resources
        .iter()
        .find(|resource| resource.resource_id.as_str() == "token.type")
        .unwrap();
    let file_position = first.declarations.find(&file.c_type).unwrap();
    let token_position = first.declarations.find(&token.c_type).unwrap();
    let callback_position = first
        .lifecycles
        .iter()
        .find_map(|lifecycle| match &lifecycle.kind {
            NativeFinalizerKind::Imported(finalizer) => {
                first.declarations.find(&finalizer.callback_type)
            }
            NativeFinalizerKind::Trivial => None,
        })
        .unwrap();
    assert!(file_position < token_position);
    assert!(token_position < callback_position);
}

#[test]
fn type_selection_rejects_unknown_record_and_generic_shapes() {
    let program = resolve(&format!(
            "{}\n@id(\"record.type\")\nrecord Record {{\n    @id(\"record.value\")\n    value: i64,\n}}\n",
            source("Token", "FileHost", "finalize")
        ));
    let abi = build_resource_abi(&program).unwrap();

    let unknown = ResolvedType::Nominal {
        declaration: DeclarationId::new("unknown.type"),
        arguments: Vec::new(),
    };
    let record = ResolvedType::Nominal {
        declaration: DeclarationId::new("record.type"),
        arguments: Vec::new(),
    };
    let generic_nominal = ResolvedType::Nominal {
        declaration: DeclarationId::new("token.type"),
        arguments: vec![ResolvedType::I64],
    };
    let type_parameter = ResolvedType::TypeParameter {
        owner: DeclarationId::new("generic.owner"),
        index: 0,
    };

    for (ty, expected) in [
        (unknown, "unknown type"),
        (record, "record"),
        (generic_nominal, "generic nominal"),
        (type_parameter, "generic type"),
    ] {
        let diagnostic = abi.c_type(&program, &ty).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(
            diagnostic.message.contains(expected),
            "unexpected diagnostic: {}",
            diagnostic.message
        );
    }
}

#[test]
fn imported_and_trivial_lifecycles_have_distinct_descriptors() {
    let abi = build_resource_abi(&resolve(&source("Token", "FileHost", "finalize"))).unwrap();
    let trivial = abi
        .lifecycles
        .iter()
        .find(|lifecycle| lifecycle.lifecycle_id.as_str() == "token.drop")
        .unwrap();
    assert_eq!(trivial.resource_id.as_str(), "token.type");
    assert!(matches!(trivial.kind, NativeFinalizerKind::Trivial));

    let imported = abi
        .lifecycles
        .iter()
        .find(|lifecycle| lifecycle.lifecycle_id.as_str() == "file.drop")
        .unwrap();
    let NativeFinalizerKind::Imported(finalizer) = &imported.kind else {
        panic!("file lifecycle must have an imported finalizer descriptor");
    };
    assert_eq!(finalizer.import_id.as_str(), "file.finalize");
    assert_eq!(finalizer.import_key, "file.finalize");
    assert!(abi.declarations.contains(&finalizer.callback_type));
    assert!(abi.declarations.contains(&finalizer.binding_field));
}

#[test]
fn identifier_registration_rejects_collisions() {
    let mut identifiers = BTreeMap::new();
    register_identifier(
        &mut identifiers,
        "spx_r_collision",
        "resource `a`".to_owned(),
    )
    .unwrap();
    let diagnostic = register_identifier(
        &mut identifiers,
        "spx_r_collision",
        "resource `b`".to_owned(),
    )
    .unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("identifier collision"));
}
