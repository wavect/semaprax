//! Separate a one-owner Matcher claim from unchanged mixed-resource examples.
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn conformance_manifests(scratch: &Path, manifest: &Path, module: &str) -> Vec<PathBuf> {
    if module != "std.pattern" {
        return super::logging::conformance_manifests(scratch, manifest, module);
    }
    let directory = scratch.join("pattern-one-owner");
    std::fs::create_dir_all(directory.join("src")).unwrap();
    std::fs::write(
        directory.join("semaprax.toml"),
        r#"schema = "semaprax.manifest.v1"

[package]
name = "pattern-one-owner"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "fixture.pattern.examples"
sources = ["src/examples.spx", "src/tests.spx"]
tests = ["fixture.pattern.tests"]

[exports]
web = []

[dependencies]
std.pattern = "=0.1.0"
"#,
    )
    .unwrap();
    for (name, source) in [
        ("examples", include_str!("pattern_examples.spx")),
        ("tests", include_str!("pattern_tests.spx")),
    ] {
        let program = semaprax::parse(source, format!("{name}.spx")).unwrap();
        std::fs::write(
            directory.join(format!("src/{name}.spx")),
            semaprax::format::canonical(&program),
        )
        .unwrap();
    }
    // The original full fixtures, including Strings and simultaneous Matchers,
    // remain first. The strict single-slot witness is additional coverage.
    vec![manifest.to_path_buf(), directory.join("semaprax.toml")]
}

fn leaves(shape: &semaprax::cleanup::FieldLivenessShape) -> (usize, usize) {
    use semaprax::cleanup::FieldLivenessShape;
    match shape {
        FieldLivenessShape::NoDrop => (0, 0),
        FieldLivenessShape::Leaf { lifecycle, .. } => match lifecycle.as_str() {
            "core.bytes.drop" => (1, 0),
            "core.string.drop" => (0, 1),
            other => panic!("unexpected pattern lifecycle {other}"),
        },
        FieldLivenessShape::Record { fields, .. } => {
            fields.iter().fold((0, 0), |(bytes, strings), field| {
                let next = leaves(&field.shape);
                (bytes + next.0, strings + next.1)
            })
        }
        _ => panic!("pattern ownership witness changed its closed shape"),
    }
}

pub(super) fn live_bound(
    manifest: &Path,
    role: &str,
    program: &semaprax::hir::ResolvedProgram,
) -> usize {
    if manifest.parent().unwrap().file_name().unwrap() == "pattern-one-owner" {
        return 1;
    }
    // Snapshot HIR has independently authenticated every inventory and plan.
    // No cleanup vectors are reordered or used as physical cleanup authority.
    let function_name = if role == "tests" {
        "test_truncated_escape_offsets"
    } else {
        "headers"
    };
    let function = program
        .functions
        .iter()
        .find(|function| function.name == function_name)
        .expect("original pattern ownership witness must exist");
    let (bytes, strings) = function
        .cleanup
        .slots
        .iter()
        .filter(|slot| {
            matches!(
                slot.origin,
                semaprax::cleanup::CleanupStorageOrigin::Binding { .. }
            )
        })
        .fold((0, 0), |(bytes, strings), slot| {
            let next = leaves(&slot.shape);
            (bytes + next.0, strings + next.1)
        });
    if role == "tests" {
        // Four independent Matchers and four Strings are held simultaneously.
        assert_eq!((bytes, strings), (4, 4));
        bytes + strings
    } else {
        // The single String remains live across the whole-owner Matcher renewals.
        assert_eq!(strings, 1);
        assert!(bytes >= 1);
        2
    }
}

pub(super) fn assert_adjacent_refusal(
    scratch: &Path,
    wasm_path: &Path,
    live_bound: usize,
    expected: i64,
) {
    assert!(live_bound > 0);
    let script = scratch.join("pattern-adjacent-refusal.mjs");
    std::fs::write(
        &script,
        super::wasm_host::wasm_conformance_js(
            &wasm_path.file_name().unwrap().to_string_lossy(),
            live_bound - 1,
            expected,
            None,
        ),
    )
    .unwrap();
    let output = Command::new("node")
        .arg(script.file_name().unwrap())
        .current_dir(scratch)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "adjacent lower arena bound must refuse"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Error: owned Bytes live entry limit exceeded"),
        "adjacent lower bound must reach the real allocation refusal: {}",
        String::from_utf8_lossy(&output.stderr),
    );
}
