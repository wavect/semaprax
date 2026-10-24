//! Complete predecessor provider and immutable validation handoff controls.
use super::emit_provider;
use crate::codegen::{emit_hir_c_with_labels, reject_native_rust_for_native, NativeOutputProfile};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};
use crate::project::PublicApiSubject;
use std::collections::HashMap;
fn emit_hir_c_for_owned_data_provider(program: &ResolvedProgram) -> Result<String, Diagnostic> {
    reject_native_rust_for_native(program)?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::OwnedDataProvider,
        None,
    )
}
fn emit_hir_c_for_owned_utf8_provider(program: &ResolvedProgram) -> Result<String, Diagnostic> {
    reject_native_rust_for_native(program)?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::OwnedUtf8Provider,
        None,
    )
}

mod before {
    use super::super::{
        emit_export, emit_provider_runtime_profile, provider_error, NativeOwnedDataProviderArtifact,
    };
    use crate::diagnostic::Diagnostic;
    use crate::hir::ResolvedProgram;
    use crate::project::{
        replay_public_api_descriptor, PublicApiResultType, PublicApiSubject,
        PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
    };
    use std::fmt::Write as _;
    include!("emit_provider_before.rs");
}

fn program(utf8: bool) -> ResolvedProgram {
    let body = if utf8 {
        "@id(\"export\") fn exported() -> string { \"hello世界\" }"
    } else {
        "@id(\"export\") fn exported(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }"
    };
    let source = format!("module provider.receipt; {body} @id(\"main\") fn main()->i64 {{0}}");
    let parsed = crate::check(&source, "provider-receipt.spx").unwrap();
    crate::hir::resolve(&parsed).unwrap()
}
fn subject(utf8: bool) -> PublicApiSubject<'static> {
    const DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    PublicApiSubject {
        project_schema: if utf8 {
            "semaprax.project.v10"
        } else {
            "semaprax.project.v8"
        },
        project_revision: DIGEST,
        workspace_revision: DIGEST,
        project_graph_digest: DIGEST,
    }
}
#[test]
fn complete_owned_provider_bytes_remove_exactly_two_full_validators_for_both_profiles() {
    for utf8 in [false, true] {
        let program = program(utf8);
        let selected = vec!["export".to_owned()];
        let descriptor =
            crate::project::derive_public_api_descriptor(&program, &selected, subject(utf8))
                .unwrap();
        let bytes = descriptor.canonical_bytes();
        let digest = descriptor.digest();
        let limit = 64 * 1024 * 1024;
        let (checked, refused, validation_bytes, _) =
            crate::bounded_output::with_limit_usage_refusal(limit, || {
                crate::hir::validate(&program)
            });
        checked.unwrap();
        assert!(!refused && validation_bytes > 0);
        let (old, refused, old_bytes, _) =
            crate::bounded_output::with_limit_usage_refusal(limit, || {
                before::emit_provider(&program, &selected, subject(utf8), &bytes, &digest, true)
            });
        let old = old.unwrap();
        assert!(!refused);
        let (new, refused, new_bytes, _) =
            crate::bounded_output::with_limit_usage_refusal(limit, || {
                emit_provider(&program, &selected, subject(utf8), &bytes, &digest, true)
            });
        assert_eq!(new.unwrap(), old);
        assert!(!refused);
        assert_eq!(old_bytes - new_bytes, 2 * validation_bytes);
        const FLOOR: usize = 2 * 1024 * 1024;
        for limit in [FLOOR, new_bytes + FLOOR - 1, new_bytes + FLOOR] {
            let (output, refused, paid, first) =
                crate::bounded_output::with_limit_usage_refusal(limit, || {
                    assert!(crate::bounded_output::set_active_floor(FLOOR));
                    emit_provider(&program, &selected, subject(utf8), &bytes, &digest, true)
                });
            if limit == new_bytes + FLOOR {
                assert!(!refused && first.is_none());
                assert_eq!(output.unwrap(), old);
                assert_eq!(paid, new_bytes);
            } else {
                assert!(output.is_err(), "provider must refuse limit {limit}");
                assert!(paid <= limit - FLOOR);
                assert!(refused && first.is_some());
            }
        }
    }
}
#[test]
fn hostile_hir_and_descriptor_drift_keep_the_complete_predecessor_diagnostics() {
    for utf8 in [false, true] {
        let program = program(utf8);
        let selected = vec!["export".to_owned()];
        let descriptor =
            crate::project::derive_public_api_descriptor(&program, &selected, subject(utf8))
                .unwrap();
        let bytes = descriptor.canonical_bytes();
        let digest = descriptor.digest();
        for drift in 0..3 {
            let mut hostile = program.clone();
            match drift {
                0 => hostile.entrypoint = DeclarationId::new("missing.main"),
                1 => hostile.functions[0].cleanup_plan.blocks.clear(),
                2 => hostile.functions[0].id = DeclarationId::new("forged\0identity"),
                _ => unreachable!(),
            }
            let expected = crate::hir::validate(&hostile).unwrap_err();
            assert_eq!(
                before::emit_provider(
                    &hostile,
                    &selected,
                    subject(utf8),
                    b"foreign descriptor",
                    "foreign digest",
                    true
                )
                .unwrap_err(),
                expected
            );
            assert_eq!(
                emit_provider(
                    &hostile,
                    &selected,
                    subject(utf8),
                    b"foreign descriptor",
                    "foreign digest",
                    true
                )
                .unwrap_err(),
                expected
            );
        }
        for (submitted, submitted_digest) in [
            (&bytes[..bytes.len() - 1], digest.as_str()),
            (b"foreign descriptor".as_slice(), digest.as_str()),
            (
                bytes.as_slice(),
                "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            ),
        ] {
            assert_eq!(
                emit_provider(
                    &program,
                    &selected,
                    subject(utf8),
                    submitted,
                    submitted_digest,
                    true
                )
                .unwrap_err(),
                before::emit_provider(
                    &program,
                    &selected,
                    subject(utf8),
                    submitted,
                    submitted_digest,
                    true
                )
                .unwrap_err()
            );
        }
        let (old, old_refused, _, old_first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                before::emit_provider(&program, &selected, subject(utf8), &bytes, &digest, true)
            });
        let (new, new_refused, _, new_first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                emit_provider(&program, &selected, subject(utf8), &bytes, &digest, true)
            });
        assert_eq!(new.unwrap_err(), old.unwrap_err());
        assert!(old_refused && new_refused);
        assert_eq!(new_first, old_first);
    }
}
