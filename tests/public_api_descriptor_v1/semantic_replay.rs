//! Authentic descriptors for alternative checked source subjects, not JSON edits.
//! Revision facts are deliberately constant synthetic fixtures: this isolates
//! retained-HIR comparison and makes no authenticated Project-revision claim.
//! Parameter IDs are compiler-derived from function identity and ordinal; these
//! cases do not claim an independently authored parameter-ID-only mutation.

use super::*;
use semaprax::project::{PublicApiDescriptor, PUBLIC_OWNED_UTF8_PROJECT_SCHEMA};

struct Shape<'a> {
    declaration: &'a str,
    parameters: &'a [(&'a str, PublicApiParameterType)],
    result: PublicApiResultType,
}

fn derive(shape: &Shape<'_>, profile: &'static str) -> (hir::ResolvedProgram, PublicApiDescriptor) {
    let source = format!(
        "module replay.subject;\n@id(\"api.selected\") {}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n",
        shape.declaration
    );
    let program = resolve(&source);
    let descriptor = derive_public_api_descriptor(
        &program,
        &["api.selected".to_owned()],
        PublicApiSubject {
            project_schema: profile,
            ..subject()
        },
    )
    .unwrap();
    assert_eq!(descriptor.exports().len(), 1);
    let export = &descriptor.exports()[0];
    assert_eq!(export.stable_id().as_str(), "api.selected");
    assert_eq!(export.typescript_name(), "api.selected");
    assert_eq!(export.rust_method_name(), "spx_api_dot_selected");
    assert_eq!(export.result(), shape.result);
    assert_eq!(
        export
            .parameters()
            .iter()
            .map(|p| (p.source_name(), p.ty()))
            .collect::<Vec<_>>(),
        shape.parameters
    );
    assert_eq!(
        replay_public_api_descriptor(
            &program,
            &["api.selected".to_owned()],
            PublicApiSubject {
                project_schema: profile,
                ..subject()
            },
            &descriptor.canonical_bytes(),
            &descriptor.digest(),
        )
        .unwrap(),
        descriptor
    );
    (program, descriptor)
}

fn pair(left: Shape<'_>, right: Shape<'_>, profile: &'static str, differs: bool) {
    assert_ne!(left.declaration, right.declaration);
    let (left_program, left_descriptor) = derive(&left, profile);
    let (right_program, right_descriptor) = derive(&right, profile);
    assert_eq!(
        left_descriptor.project_schema(),
        right_descriptor.project_schema()
    );
    assert_eq!(
        left_descriptor.project_revision(),
        right_descriptor.project_revision()
    );
    assert_eq!(
        left_descriptor.workspace_revision(),
        right_descriptor.workspace_revision()
    );
    assert_eq!(
        left_descriptor.project_graph_digest(),
        right_descriptor.project_graph_digest()
    );
    assert_eq!(left_descriptor.limits(), right_descriptor.limits());
    // At shared ordinals these identities stay fixed even when names/types move.
    for (left, right) in left_descriptor.exports()[0]
        .parameters()
        .iter()
        .zip(right_descriptor.exports()[0].parameters())
    {
        assert_eq!(left.stable_id(), right.stable_id());
    }
    if differs {
        assert!(left.parameters != right.parameters || left.result != right.result);
        assert_ne!(
            left_descriptor.canonical_bytes(),
            right_descriptor.canonical_bytes()
        );
        assert_ne!(left_descriptor.digest(), right_descriptor.digest());
    } else {
        assert_eq!(left_descriptor, right_descriptor);
        assert_eq!(
            left_descriptor.canonical_bytes(),
            right_descriptor.canonical_bytes()
        );
        assert_eq!(left_descriptor.digest(), right_descriptor.digest());
    }
    for (program, submitted) in [
        (&left_program, &right_descriptor),
        (&right_program, &left_descriptor),
    ] {
        let replay = replay_public_api_descriptor(
            program,
            &["api.selected".to_owned()],
            PublicApiSubject {
                project_schema: profile,
                ..subject()
            },
            &submitted.canonical_bytes(),
            &submitted.digest(),
        );
        if differs {
            let error = replay.unwrap_err();
            assert_eq!(error.code, "SPX-J113");
            assert_eq!(
                error.message,
                "public API descriptor does not replay against the retained subject"
            );
        } else {
            assert_eq!(replay.unwrap(), *submitted);
        }
    }
}

#[test]
fn authentic_signature_counterparts_reach_retained_hir_rejection_in_both_profiles() {
    use PublicApiParameterType::{Bool, BorrowSliceU8, BorrowStr, I64};
    use PublicApiResultType::{OptionOwnedBytes, OwnedBytes, ResultOwnedBytesI64};
    for profile in [
        PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
    ] {
        let scalar = |declaration, parameters| Shape {
            declaration,
            parameters,
            result: PublicApiResultType::I64,
        };
        pair(
            scalar("fn value(number: i64) -> i64 { 0 }", &[("number", I64)]),
            scalar("fn value(number: bool) -> i64 { 0 }", &[("number", Bool)]),
            profile,
            true,
        );
        pair(
            scalar(
                "fn value(input: borrow str) -> i64 { 0 }",
                &[("input", BorrowStr)],
            ),
            scalar(
                "fn value(input: borrow Slice<u8>) -> i64 { 0 }",
                &[("input", BorrowSliceU8)],
            ),
            profile,
            true,
        );
        pair(
            scalar("fn value(number: i64) -> i64 { 0 }", &[("number", I64)]),
            scalar("fn value(renamed: i64) -> i64 { 0 }", &[("renamed", I64)]),
            profile,
            true,
        );
        pair(
            scalar(
                "fn value(number: i64, flag: bool) -> i64 { 0 }",
                &[("number", I64), ("flag", Bool)],
            ),
            scalar(
                "fn value(flag: bool, number: i64) -> i64 { 0 }",
                &[("flag", Bool), ("number", I64)],
            ),
            profile,
            true,
        );
        pair(
            scalar("fn value(number: i64) -> i64 { 0 }", &[("number", I64)]),
            scalar(
                "fn value(number: i64, flag: bool) -> i64 { 0 }",
                &[("number", I64), ("flag", Bool)],
            ),
            profile,
            true,
        );
        pair(
            scalar("fn value() -> i64 { 0 }", &[]),
            Shape {
                declaration: "fn value() -> bool { true }",
                parameters: &[],
                result: PublicApiResultType::Bool,
            },
            profile,
            true,
        );
        pair(
            scalar("fn value() -> i64 { 0 }", &[]),
            Shape {
                declaration: "fn value() -> usize { 0usize }",
                parameters: &[],
                result: PublicApiResultType::Usize,
            },
            profile,
            true,
        );
        for (declaration, result) in [
            ("fn value(input: borrow Slice<u8>) -> Option<Bytes> { Option<Bytes>::Some { value: bytes_copy(input) } }", OptionOwnedBytes),
            ("fn value(input: borrow Slice<u8>) -> Result<Bytes, i64> { Result<Bytes, i64>::Ok { value: bytes_copy(input) } }", ResultOwnedBytesI64),
        ] {
            pair(Shape { declaration: "fn value(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }", parameters: &[("input", BorrowSliceU8)], result: OwnedBytes },
                Shape { declaration, parameters: &[("input", BorrowSliceU8)], result }, profile, true);
        }
    }
}

#[test]
fn authentic_owned_utf8_is_not_interchangeable_with_owned_bytes() {
    let parameters = &[("input", PublicApiParameterType::BorrowSliceU8)];
    pair(
        Shape {
            declaration: "fn value(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }",
            parameters,
            result: PublicApiResultType::OwnedBytes,
        },
        Shape {
            declaration: "fn value(input: borrow Slice<u8>) -> string { \"hello\" }",
            parameters,
            result: PublicApiResultType::OwnedUtf8,
        },
        PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
        true,
    );
}

#[test]
fn body_and_function_display_changes_preserve_descriptor_with_fixed_subject_facts() {
    for profile in [
        PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
    ] {
        for counterpart in [
            "fn value(number: i64) -> i64 { number + 1 }",
            "fn renamed(number: i64) -> i64 { number }",
        ] {
            let shape = |declaration| Shape {
                declaration,
                parameters: &[("number", PublicApiParameterType::I64)],
                result: PublicApiResultType::I64,
            };
            pair(
                shape("fn value(number: i64) -> i64 { number }"),
                shape(counterpart),
                profile,
                false,
            );
        }
    }
}
