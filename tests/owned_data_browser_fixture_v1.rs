//! Compiler-side admission and package checks for the provisioned browser subject.
//! These tests author no source execution, publication, or browser operation.

use std::path::Path;

use semaprax::project::{
    with_authenticated_project, ProjectManifest, PublicApiParameterType, PublicApiResultType,
    MAX_PROJECT_NPM_BUILD_BYTES, PROJECT_NPM_BUILD_SCHEMA_V7, PROJECT_SCHEMA_V8,
    PUBLIC_OWNED_DATA_API_SCHEMA,
};

const DIRECTORY: &str = "platform-tests/owned-data-browser-v1/project";
const FILES: [(&str, &str); 3] = [
    (
        "semaprax.toml",
        include_str!("../platform-tests/owned-data-browser-v1/project/semaprax.toml"),
    ),
    (
        "src/app.spx",
        include_str!("../platform-tests/owned-data-browser-v1/project/src/app.spx"),
    ),
    (
        "src/tests.spx",
        include_str!("../platform-tests/owned-data-browser-v1/project/src/tests.spx"),
    ),
];

#[test]
fn fixed_browser_project_has_exact_owned_api_and_verified_six_artifact_package() {
    use PublicApiParameterType::{Bool, BorrowSliceU8, BorrowStr, I64};

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(DIRECTORY);
    for (relative, expected) in FILES {
        assert_eq!(
            std::fs::read(root.join(relative)).unwrap(),
            expected.as_bytes()
        );
        if relative.ends_with(".spx") {
            let parsed = semaprax::parse(expected, root.join(relative)).unwrap();
            let canonical = semaprax::format::canonical(&parsed);
            let replay = semaprax::parse(&canonical, root.join(relative)).unwrap();
            assert_eq!(semaprax::format::canonical(&replay), canonical);
        }
    }
    let manifest = ProjectManifest::parse(FILES[0].1).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V8);
    assert_eq!(manifest.to_canonical_toml(), FILES[0].1);
    assert_eq!(manifest.sources(), &["src/app.spx", "src/tests.spx"]);
    assert_eq!(manifest.entry(), "owned_browser.app");
    assert_eq!(manifest.test_module(), "owned_browser.tests");

    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let descriptor = snapshot.public_api_descriptor()?;
        assert_eq!(descriptor.schema(), PUBLIC_OWNED_DATA_API_SCHEMA);
        assert_eq!(descriptor.project_schema(), PROJECT_SCHEMA_V8);
        let expected: [(&str, &[PublicApiParameterType]); 4] = [
            ("frame.fail-after", &[BorrowSliceU8, I64]),
            ("frame.fail-before", &[BorrowSliceU8, I64]),
            ("frame.mixed", &[Bool, BorrowStr, BorrowSliceU8]),
            ("frame.payload", &[BorrowSliceU8]),
        ];
        assert_eq!(descriptor.exports().len(), expected.len());
        for (export, (id, parameters)) in descriptor.exports().iter().zip(expected) {
            assert_eq!(export.stable_id().as_str(), id);
            assert_eq!(export.typescript_name(), id);
            assert_eq!(export.result(), PublicApiResultType::OwnedBytes);
            assert_eq!(
                export
                    .parameters()
                    .iter()
                    .map(|value| value.ty())
                    .collect::<Vec<_>>(),
                parameters
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
    for (relative, expected) in FILES {
        assert_eq!(
            std::fs::read(root.join(relative)).unwrap(),
            expected.as_bytes()
        );
    }
}
