//! Test-specific canonical manifest oracle, not an independent semantic verifier.
use semaprax::hir::ResolvedProgram;
use semaprax::project::{PublicApiDescriptor, PublicApiParameterType, PublicApiResultType};

#[path = "../support/owned_utf8_package.rs"]
mod package_files;
pub(super) use package_files::{names, read, verify};

pub(super) fn helper_identities(program: &ResolvedProgram, renamed: bool) {
    for (id, expected_name) in [
        ("helper.left\u{8}\u{c}\u{7f}\u{85}", "finish"),
        (
            "helper.right",
            if renamed { "renamed_finish" } else { "finish" },
        ),
    ] {
        let helper = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap();
        assert_eq!(helper.name, expected_name);
    }
}

pub(super) fn identities(descriptor: &PublicApiDescriptor) {
    let expected = [
        (
            "bytes.raw",
            "spx_bytes_dot_raw",
            PublicApiParameterType::BorrowSliceU8,
            PublicApiResultType::OwnedBytes,
        ),
        (
            "utf8.left",
            "spx_utf8_dot_left",
            PublicApiParameterType::I64,
            PublicApiResultType::OwnedUtf8,
        ),
        (
            "utf8.right",
            "spx_utf8_dot_right",
            PublicApiParameterType::I64,
            PublicApiResultType::OwnedUtf8,
        ),
    ];
    assert_eq!(descriptor.exports().len(), expected.len());
    for (export, (id, method, parameter, result)) in descriptor.exports().iter().zip(expected) {
        assert_eq!(export.stable_id().as_str(), id);
        assert_eq!(export.rust_method_name(), method);
        assert_eq!(export.parameters().len(), 1);
        assert_eq!(export.parameters()[0].ty(), parameter);
        assert_eq!(export.result(), result);
    }
    assert_eq!(descriptor.limits().max_borrowed_input_bytes, 65_536);
    assert_eq!(descriptor.limits().max_owned_output_bytes, 65_536);
}

pub(super) fn rename_preserves_ids(before: &PublicApiDescriptor, after: &PublicApiDescriptor) {
    assert_ne!(before.digest(), after.digest());
    assert_eq!(before.exports(), after.exports());
    let mut before: serde_json::Value = serde_json::from_slice(&before.canonical_bytes()).unwrap();
    let after: serde_json::Value = serde_json::from_slice(&after.canonical_bytes()).unwrap();
    for binding in [
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
    ] {
        assert_ne!(before[binding], after[binding], "unchanged {binding}");
        before[binding] = after[binding].clone();
    }
    // The renamed helper is not an exported presentation field. Every other
    // canonical descriptor fact, including all signatures/limits, must agree.
    assert_eq!(before, after);
}
