//! Real Project admission and private host publication; no target code runs.
use super::*;
use semaprax::project::{with_authenticated_project, ProjectNpmBuild};

#[path = "../../../../../tests/support/owned_tuple_product.rs"]
mod tuple;
#[path = "../../../../../tests/support/owned_utf8_product.rs"]
mod utf8;

#[test]
fn all_three_owned_profiles_publish_exact_inline_artifacts_and_preserve_sources() {
    for version in [8, 9, 10] {
        let root = fixture();
        let source = root.join("project");
        fs::create_dir(&source).unwrap();
        let manifest = if version == 10 {
            utf8::write_project(&source, false)
        } else {
            tuple::write_project(&source, version == 9)
        };
        let mut files = vec![];
        let source_names: &[&str] = if version == 10 {
            &[
                "semaprax.toml",
                "src/app.spx",
                "src/left.spx",
                "src/right.spx",
                "src/tests.spx",
            ]
        } else {
            &["semaprax.toml", "src/app.spx", "src/tests.spx"]
        };
        for name in source_names {
            files.push((
                format!("project/{name}"),
                fs::read(source.join(name)).unwrap(),
            ));
        }
        let output = root.join("package");
        with_authenticated_project(&manifest, |snapshot| {
            let build = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
            build.verify().unwrap();
            ProjectNpmBuild::inspect_envelope(build.envelope(), MAX_PROJECT_NPM_BUILD_BYTES)
                .unwrap();
            let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
            assert_eq!(
                envelope["project_schema"],
                format!("semaprax.project.v{version}")
            );
            assert_eq!(
                envelope["schema"],
                format!("semaprax.project-npm-build.v{}", version - 1)
            );
            let rows = envelope["artifacts"].as_array().unwrap();
            assert_eq!(rows.len(), NAMES.len());
            crate::build_owned_npm(snapshot, &output)?;
            for (row, name) in rows.iter().zip(NAMES) {
                assert_eq!(row["path"], name);
                let hex = row["hex"].as_str().unwrap();
                assert_eq!(hex.len() % 2, 0);
                let bytes = (0..hex.len())
                    .step_by(2)
                    .map(|offset| u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(fs::read(output.join(name)).unwrap(), bytes);
                files.push((format!("package/{name}"), bytes));
            }
            let metadata: serde_json::Value =
                serde_json::from_slice(&fs::read(output.join("semaprax.api.json")).unwrap())
                    .unwrap();
            let (descriptor, digest) = match version {
                8 => {
                    let value = snapshot.public_api_descriptor()?;
                    (value.canonical_bytes(), value.digest())
                }
                9 => {
                    let value = snapshot.flat_owned_record_api_descriptor()?;
                    (value.canonical_bytes(), value.digest())
                }
                10 => {
                    let value = snapshot.owned_utf8_api_descriptor()?;
                    (value.canonical_bytes(), value.digest())
                }
                _ => unreachable!(),
            };
            assert_eq!(
                metadata["descriptor"].as_str().unwrap().as_bytes(),
                descriptor
            );
            assert_eq!(metadata["descriptor_digest"], digest);
            let errors = crate::build_owned_npm(snapshot, &output).unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "SPX-W120");
            // The registry/root path remains unavailable even though the full
            // private host just published successfully on this same machine.
            let errors = snapshot.build_npm(&root.join("standalone")).unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "SPX-W120");
            assert!(!root.join("standalone").exists());
            Ok(())
        })
        .unwrap();
        // Complete preflight checks both source bytes and all published bytes,
        // and rejects leftover stages or any unexpected final inventory.
        finish(
            &root,
            &["project".into(), "project/src".into(), "package".into()],
            &files,
        );
    }
}
