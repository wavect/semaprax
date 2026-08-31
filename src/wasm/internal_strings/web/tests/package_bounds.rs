//! Private renderer/final-guard accounting, not source admission or publication.
use super::super::{package_size, render, PACKAGE_LIMIT, SOURCE_LIMIT};
use super::{INVENTORY, SOURCE};
use crate::wasm::internal_strings::{emit_module, InternalStringOptions};

fn total(files: &[(&str, Vec<u8>)]) -> usize {
    files
        .iter()
        .try_fold(0usize, |sum, (_, bytes)| sum.checked_add(bytes.len()))
        .unwrap()
}

#[test]
fn synthetic_module_name_exercises_manifest_inclusive_renderer_limit() {
    let program = crate::check(SOURCE, "package-bounds.spx").unwrap();
    let revision = crate::graph::revision(&program);
    let module = emit_module(
        &program,
        &["main".to_owned()],
        InternalStringOptions::default(),
    )
    .unwrap();
    let frozen = (
        module.wasm_bytes().to_vec(),
        module.descriptor().to_owned(),
        module.runtime_source().to_owned(),
    );

    // Only this private renderer argument is synthetic. It does not purport to
    // match SOURCE's module declaration or pass the bounded source route. The
    // real emitted module and its exact descriptor/runtime are never forged.
    let baseline = render::artifacts("m", SOURCE, &revision, &module).unwrap();
    assert_eq!(
        baseline.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        INVENTORY
    );
    assert_eq!(PACKAGE_LIMIT, 32 * 1024 * 1024);
    let fixed_overhead = total(&baseline).checked_sub(1).unwrap();
    let exact_name_length = PACKAGE_LIMIT.checked_sub(fixed_overhead).unwrap();
    assert!(exact_name_length > SOURCE_LIMIT);
    let non_manifest_total = baseline
        .iter()
        .filter(|(name, _)| *name != "semaprax.manifest.json")
        .map(|(_, bytes)| bytes.len())
        .sum::<usize>();
    assert!(non_manifest_total < PACKAGE_LIMIT);

    // ASCII 'm' needs no JSON escaping. Replacing the one-byte baseline name
    // therefore adds exactly name_length - 1 bytes, with no feedback loop or
    // search using a renderer result to tune the desired boundary.
    for extra in [0usize, 1] {
        let name = "m".repeat(exact_name_length + extra);
        let files = render::artifacts(&name, SOURCE, &revision, &module).unwrap();
        assert_eq!(files.len(), 8);
        assert_eq!(total(&files), 33_554_432 + extra);
        let mut unchanged = 0;
        for ((name, bytes), (original_name, original_bytes)) in files.iter().zip(&baseline) {
            assert_eq!(name, original_name);
            if *name != "semaprax.manifest.json" {
                assert_eq!(bytes, original_bytes);
                unchanged += 1;
            } else {
                assert_eq!(
                    bytes.len(),
                    original_bytes.len() + exact_name_length + extra - 1
                );
            }
        }
        assert_eq!(unchanged, 7);
        // This is the same final guard used after rendering in build(). No
        // filesystem publisher is called for these synthetic manifest facts.
        let result = package_size(files.iter().map(|(_, bytes)| bytes.len()));
        if extra == 0 {
            result.unwrap();
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.code, "SPX-W111");
            assert_eq!(
                error.message,
                "internal String Web package exceeds 33554432 bytes"
            );
        }
        assert_eq!(module.wasm_bytes(), frozen.0);
        assert_eq!(module.descriptor(), frozen.1);
        assert_eq!(module.runtime_source(), frozen.2);
    }
}
