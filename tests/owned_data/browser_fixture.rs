//! Compiler-side admission and package checks for the provisioned browser subject.
//! These tests author no source execution, publication, or browser operation.

use std::path::Path;

use semaprax::hir::{ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedStatement, ValueId};
use semaprax::project::{
    with_authenticated_project, ProjectManifest, PublicApiParameterType, PublicApiResultType,
    MAX_PROJECT_NPM_BUILD_BYTES, PROJECT_NPM_BUILD_SCHEMA_V7, PROJECT_SCHEMA_V8,
    PUBLIC_OWNED_DATA_API_SCHEMA,
};

const DIRECTORY: &str = "platform-tests/owned-data-browser-v1/project";
const FILES: [(&str, &str); 3] = [
    (
        "semaprax.toml",
        include_str!("../../platform-tests/owned-data-browser-v1/project/semaprax.toml"),
    ),
    (
        "src/app.spx",
        include_str!("../../platform-tests/owned-data-browser-v1/project/src/app.spx"),
    ),
    (
        "src/tests.spx",
        include_str!("../../platform-tests/owned-data-browser-v1/project/src/tests.spx"),
    ),
];
const VARIANT_DIRECTORY: &str = "platform-tests/owned-data-browser-v1/variant-project";
const VARIANT_FILES: [(&str, &str); 3] = [
    (
        "semaprax.toml",
        include_str!("../../platform-tests/owned-data-browser-v1/variant-project/semaprax.toml"),
    ),
    (
        "src/app.spx",
        include_str!("../../platform-tests/owned-data-browser-v1/variant-project/src/app.spx"),
    ),
    (
        "src/tests.spx",
        include_str!("../../platform-tests/owned-data-browser-v1/variant-project/src/tests.spx"),
    ),
];

#[test]
fn fixed_browser_project_has_exact_owned_api_and_verified_six_artifact_package() {
    verify_project(DIRECTORY, FILES, false);
}

#[test]
fn staged_variant_browser_project_has_exact_api_and_verified_six_artifact_package() {
    verify_project(VARIANT_DIRECTORY, VARIANT_FILES, true);
}

fn verify_project(directory: &str, files: [(&str, &str); 3], variants: bool) {
    use PublicApiParameterType::{Bool, BorrowSliceU8, BorrowStr, I64};
    use PublicApiResultType::{OptionOwnedBytes, OwnedBytes, ResultOwnedBytesI64};

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(directory);
    for (relative, expected) in files {
        assert_eq!(
            std::fs::read(root.join(relative)).unwrap(),
            expected.as_bytes()
        );
        if relative.ends_with(".spx") {
            let parsed = semaprax::parse(expected, root.join(relative)).unwrap();
            let canonical = semaprax::format::canonical(&parsed);
            let replay = semaprax::parse(&canonical, root.join(relative)).unwrap();
            assert_eq!(semaprax::format::canonical(&replay), canonical);
            if !variants && relative == "src/tests.spx" {
                // This module intentionally imports the real scalar payload check;
                // only the Workspace Graph can resolve its retained subject.
                for source in [&parsed, &replay] {
                    let errors = semaprax::graph::to_json(source).unwrap_err();
                    assert_eq!(errors.len(), 1);
                    assert_eq!(errors[0].code, "SPX-G172");
                    assert_eq!(
                        errors[0].message,
                        "source module imports require Workspace Semantic Graph resolution"
                    );
                }
            } else {
                assert!(relative == "src/app.spx" || variants && relative == "src/tests.spx");
                assert_eq!(
                    semaprax::graph::to_json(&parsed).unwrap(),
                    semaprax::graph::to_json(&replay).unwrap()
                );
            }
        }
    }
    let manifest = ProjectManifest::parse(files[0].1).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V8);
    assert_eq!(manifest.to_canonical_toml(), files[0].1);
    assert_eq!(manifest.sources(), &["src/app.spx", "src/tests.spx"]);
    assert_eq!(
        manifest.entry(),
        if variants {
            "owned_browser_variants.app"
        } else {
            "owned_browser.app"
        }
    );
    assert_eq!(
        manifest.test_module(),
        if variants {
            "owned_browser_variants.tests"
        } else {
            "owned_browser.tests"
        }
    );

    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        if variants {
            staged_before_variant(snapshot.entry_program());
        }
        let descriptor = snapshot.public_api_descriptor()?;
        let revision = snapshot.retain_revision();
        let workspace_graph: serde_json::Value =
            serde_json::from_str(snapshot.semantic_graph()).unwrap();
        assert_eq!(
            workspace_graph["workspace_revision"],
            snapshot.workspace_revision()
        );
        assert_eq!(
            workspace_graph["graph_digest"],
            revision.semantic_graph_digest()
        );
        assert_eq!(
            descriptor.project_graph_digest(),
            revision.semantic_graph_digest()
        );
        assert_eq!(descriptor.schema(), PUBLIC_OWNED_DATA_API_SCHEMA);
        assert_eq!(descriptor.project_schema(), PROJECT_SCHEMA_V8);
        let expected: &[(&str, &[PublicApiParameterType], PublicApiResultType)] = if variants {
            &[
                ("frame.maybe", &[BorrowSliceU8, Bool], OptionOwnedBytes),
                ("frame.result", &[BorrowSliceU8, Bool], ResultOwnedBytesI64),
            ]
        } else {
            &[
                ("frame.fail-after", &[BorrowSliceU8, I64], OwnedBytes),
                ("frame.fail-before", &[BorrowSliceU8, I64], OwnedBytes),
                ("frame.mixed", &[Bool, BorrowStr, BorrowSliceU8], OwnedBytes),
                ("frame.payload", &[BorrowSliceU8], OwnedBytes),
            ]
        };
        assert_eq!(descriptor.exports().len(), expected.len());
        for (export, (id, parameters, result)) in descriptor.exports().iter().zip(expected) {
            assert_eq!(export.stable_id().as_str(), *id);
            assert_eq!(export.typescript_name(), *id);
            assert_eq!(export.result(), *result);
            assert_eq!(
                export
                    .parameters()
                    .iter()
                    .map(|value| value.ty())
                    .collect::<Vec<_>>(),
                *parameters
            );
        }
        let build = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        build.verify().unwrap();
        build.verify_public_api_descriptor(&descriptor).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
        assert_eq!(envelope["schema"], PROJECT_NPM_BUILD_SCHEMA_V7);
        let rows = envelope["artifacts"].as_array().unwrap();
        let names = [
            "app.wasm",
            "semaprax.js",
            "semaprax.bindings.js",
            "semaprax.bindings.d.ts",
            "semaprax.api.json",
            "package.json",
        ];
        assert_eq!(rows.len(), names.len());
        for (row, name) in rows.iter().zip(names) {
            assert_eq!(row["path"], name);
            assert!(!row["hex"].as_str().unwrap().is_empty());
        }
        let hex = rows[4]["hex"].as_str().unwrap();
        let metadata = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let metadata: serde_json::Value = serde_json::from_slice(&metadata).unwrap();
        assert_eq!(metadata["schema"], "semaprax.owned-data-api.v1");
        assert_eq!(
            metadata["descriptor"],
            String::from_utf8(descriptor.canonical_bytes()).unwrap()
        );
        assert_eq!(metadata["descriptor_digest"], descriptor.digest());
        Ok(())
    })
    .unwrap();
    for (relative, expected) in files {
        assert_eq!(
            std::fs::read(root.join(relative)).unwrap(),
            expected.as_bytes()
        );
    }
}

fn whole_place(expression: &ResolvedExpr, root: &ValueId) {
    let ResolvedExprKind::Place(place) = &expression.kind else {
        panic!("expected the exact retained whole binding");
    };
    assert_eq!(&place.root, root);
    assert!(place.projections.is_empty());
}

fn branch_value(expression: &ResolvedExpr) -> &ResolvedExpr {
    let ResolvedExprKind::Block { statements, tail } = &expression.kind else {
        panic!("expected a branch block");
    };
    assert!(
        statements.is_empty(),
        "no conditional staging in the branch"
    );
    tail
}

fn staged_before_variant(program: &ResolvedProgram) {
    for (id, variant_id, active_case, inactive_case) in [
        (
            "frame.maybe",
            "core.option",
            "core.option.some",
            "core.option.none",
        ),
        (
            "frame.result",
            "core.result",
            "core.result.ok",
            "core.result.err",
        ),
    ] {
        let function = program
            .functions
            .iter()
            .find(|value| value.id.as_str() == id)
            .unwrap();
        assert_eq!(function.params.len(), 2);
        let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
            panic!("expected staged function block");
        };
        assert_eq!(statements.len(), 1);
        let ResolvedStatement::Let {
            binding,
            value,
            mutable,
            ..
        } = &statements[0]
        else {
            panic!("expected one unconditional owned initializer");
        };
        assert!(!mutable);
        assert_eq!(binding.ty, semaprax::hir::ResolvedType::Bytes);
        let ResolvedExprKind::Call { callee, args, .. } = &value.kind else {
            panic!("expected actual bytes_copy before selection");
        };
        assert_eq!(callee.as_str(), "core.bytes.copy");
        assert_eq!(args.len(), 1);
        whole_place(&args[0], &function.params[0].id);
        let ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } = &tail.kind
        else {
            panic!("expected selection after initialized Bytes");
        };
        whole_place(condition, &function.params[1].id);
        for (branch, expected_case, active) in [
            (then_branch, active_case, true),
            (else_branch, inactive_case, false),
        ] {
            let ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } = &branch_value(branch).kind
            else {
                panic!("expected the authenticated compiler-owned variant");
            };
            assert_eq!(variant.as_str(), variant_id);
            assert_eq!(case.as_str(), expected_case);
            if active {
                assert_eq!(fields.len(), 1);
                whole_place(&fields[0].value, &binding.id);
            } else if id == "frame.maybe" {
                assert!(fields.is_empty());
            } else {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].field.as_str(), "core.result.err.error");
                assert_eq!(fields[0].value.ty, semaprax::hir::ResolvedType::I64);
            }
        }
    }
}
