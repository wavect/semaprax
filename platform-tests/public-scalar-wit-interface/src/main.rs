use std::path::{Path, PathBuf};
use std::sync::Arc;

use semaprax::project::{
    ProjectRevision, ScalarWitInterfaceArtifactV1, ScalarWitTypeV1, with_authenticated_project,
};
use wit_parser::{Resolve, Type, TypeDefKind, WorldItem};

const EXPECTED_FUNCTIONS: &[(&str, &[ScalarWitTypeV1], ScalarWitTypeV1)] = &[
    (
        "spx-7769742e612d62",
        &[ScalarWitTypeV1::I64],
        ScalarWitTypeV1::I64,
    ),
    (
        "spx-7769742e612e62",
        &[ScalarWitTypeV1::I64],
        ScalarWitTypeV1::I64,
    ),
    (
        "spx-7769742e615f62",
        &[ScalarWitTypeV1::I64],
        ScalarWitTypeV1::I64,
    ),
    (
        "spx-7769742e626f6f6c",
        &[ScalarWitTypeV1::Bool],
        ScalarWitTypeV1::Bool,
    ),
    (
        "spx-7769742e6569676874",
        &[
            ScalarWitTypeV1::I64,
            ScalarWitTypeV1::I64,
            ScalarWitTypeV1::I64,
            ScalarWitTypeV1::I64,
            ScalarWitTypeV1::I64,
            ScalarWitTypeV1::I64,
            ScalarWitTypeV1::I64,
            ScalarWitTypeV1::I64,
        ],
        ScalarWitTypeV1::I64,
    ),
    ("spx-7769742e7a65726f", &[], ScalarWitTypeV1::I64),
];

fn fixture_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/project_scalar_wit_interface_v1/semaprax.toml")
        .canonicalize()
        .expect("the checked-in scalar WIT Project fixture must resolve")
}

fn retained_revision() -> Arc<ProjectRevision> {
    with_authenticated_project(&fixture_manifest(), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .expect("the checked-in scalar WIT Project must be admitted")
}

/// The public Copy-scalar surface of the interface, as an external consumer
/// must resolve it against the maintained WIT parser's own primitives.
fn scalar_type(ty: ScalarWitTypeV1) -> Type {
    match ty {
        ScalarWitTypeV1::I64 => Type::S64,
        ScalarWitTypeV1::I32 => Type::S32,
        ScalarWitTypeV1::U8 => Type::U8,
        ScalarWitTypeV1::Char => Type::Char,
        ScalarWitTypeV1::F32 => Type::F32,
        ScalarWitTypeV1::F64 => Type::F64,
        ScalarWitTypeV1::Bool => Type::Bool,
    }
}

fn validate_status(resolve: &Resolve, status: wit_parser::TypeId) {
    let TypeDefKind::Record(record) = &resolve.types[status].kind else {
        panic!("status must resolve to a record")
    };
    assert_eq!(
        record
            .fields
            .iter()
            .map(|field| (field.name.as_str(), field.ty))
            .collect::<Vec<_>>(),
        [
            ("domain", Type::String),
            ("code", Type::U32),
            ("class", Type::U8),
            ("retryable", record.fields[3].ty),
        ]
    );
    let Type::Id(retryable) = record.fields[3].ty else {
        panic!("status.retryable must name option<bool>")
    };
    assert_eq!(
        resolve.types[retryable].kind,
        TypeDefKind::Option(Type::Bool)
    );
}

fn validate_wit(artifact: &ScalarWitInterfaceArtifactV1) {
    let mut resolve = Resolve::default();
    let package_id = resolve
        .push_str("project-scalar-v1.wit", artifact.wit())
        .expect("the public artifact must parse with maintained wit-parser");
    let package = &resolve.packages[package_id];
    assert_eq!(package.name.namespace, "semaprax");
    assert_eq!(package.name.name, "project-scalar");
    assert_eq!(
        package.name.version.as_ref().map(ToString::to_string),
        Some("1.0.0".to_owned())
    );
    assert_eq!(package.interfaces.len(), 1);
    assert_eq!(package.worlds.len(), 1);

    let interface_id = package.interfaces["exports"];
    let interface = &resolve.interfaces[interface_id];
    assert_eq!(interface.name.as_deref(), Some("exports"));
    assert_eq!(interface.package, Some(package_id));
    assert_eq!(interface.functions.len(), EXPECTED_FUNCTIONS.len());
    let status = interface.types["status"];
    validate_status(&resolve, status);

    for ((actual_name, function), (expected_name, parameters, result)) in
        interface.functions.iter().zip(EXPECTED_FUNCTIONS)
    {
        assert_eq!(actual_name, expected_name);
        assert_eq!(function.name, *expected_name);
        assert_eq!(function.params.len(), parameters.len());
        for (index, (parameter, expected)) in function.params.iter().zip(*parameters).enumerate() {
            assert_eq!(parameter.name, format!("arg-{index}"));
            assert_eq!(parameter.ty, scalar_type(*expected));
        }
        let Some(Type::Id(result_id)) = function.result else {
            panic!("every public function must return a named result type")
        };
        let TypeDefKind::Result(result_type) = &resolve.types[result_id].kind else {
            panic!("every public function must return result<T,status>")
        };
        assert_eq!(result_type.ok, Some(scalar_type(*result)));
        assert_eq!(result_type.err, Some(Type::Id(status)));
    }

    let world_id = package.worlds["project-scalar-v1"];
    let world = &resolve.worlds[world_id];
    assert_eq!(world.name, "project-scalar-v1");
    assert_eq!(world.package, Some(package_id));
    assert!(world.imports.is_empty());
    assert_eq!(world.exports.len(), 1);
    assert!(matches!(
        world.exports.values().next(),
        Some(WorldItem::Interface { id, .. }) if *id == interface_id
    ));
}

fn parser_hostiles_are_rejected(wit: &str) {
    let duplicate = wit.replacen("spx-7769742e612e62", "spx-7769742e612d62", 1);
    let malformed = wit.replacen("result<s64, status>", "resultx<s64, status>", 1);
    let truncated = wit
        .trim_end()
        .strip_suffix('}')
        .expect("the world must end with a closing brace");
    for hostile in [&duplicate, &malformed, truncated] {
        let mut resolve = Resolve::default();
        assert!(
            resolve.push_str("hostile.wit", hostile).is_err(),
            "maintained WIT parser admitted a targeted hostile"
        );
    }
}

fn descriptor_hostiles_are_rejected(
    revision: &ProjectRevision,
    artifact: &ScalarWitInterfaceArtifactV1,
) {
    let canonical = artifact.canonical_bytes();
    let mut changed = canonical.clone();
    let project_name = b"\"project_name\":\"scalar-wit-interface\"";
    let offset = changed
        .windows(project_name.len())
        .position(|window| window == project_name)
        .map(|start| start + b"\"project_name\":\"".len())
        .expect("descriptor must bind the exact Project name");
    changed[offset] = b'x';
    for hostile in [&changed[..], &canonical[..canonical.len() - 1]] {
        let error = revision
            .replay_scalar_wit_interface_v1(hostile, &artifact.digest())
            .expect_err("changed descriptor unexpectedly replayed");
        assert_eq!(error[0].code, "SPX-WIT111");
    }
}

fn exercise() {
    let revision = retained_revision();
    let artifact = revision
        .scalar_wit_interface_v1()
        .expect("public scalar WIT derivation must succeed");
    let replayed = revision
        .replay_scalar_wit_interface_v1(&artifact.canonical_bytes(), &artifact.digest())
        .expect("exact public scalar WIT replay must succeed");
    assert_eq!(artifact, replayed);
    validate_wit(&artifact);
    parser_hostiles_are_rejected(artifact.wit());
    descriptor_hostiles_are_rejected(&revision, &artifact);
}

fn main() {
    exercise();
}

#[cfg(test)]
mod tests {
    #[test]
    fn public_default_feature_consumer_parses_and_replays_the_exact_interface() {
        super::exercise();
    }
}
