//! Copy frame observations never replace current byte-range provenance.
use super::{packages, root, run_examples_and_conformance, temporary};
use semaprax::{hir, project};
#[path = "ndjson_failure.rs"]
mod failure;

#[test]
fn bounded_ndjson_framing_and_structured_faults_execute_on_all_three_backends() {
    run_examples_and_conformance(
        packages()
            .into_iter()
            .filter(|p| p.module == "std.data.ndjson")
            .collect(),
    );
}

#[test]
fn ndjson_checked_cache_preserves_the_authenticated_project_graph() {
    let manifest = root().join("std/data-ndjson/semaprax.toml");
    project::with_authenticated_project(&manifest, |snapshot| {
        let sources = snapshot
            .manifest()
            .sources()
            .iter()
            .map(|path| {
                let source = snapshot
                    .sources()
                    .iter()
                    .find(|source| source.path() == path)
                    .expect("authenticated authored NDJSON source");
                project::ProjectFrontendSource::new(source.path(), source.source()).unwrap()
            })
            .collect::<Vec<_>>();
        let mut cache = project::ProjectFrontendCache::new_with_semantic_cache();
        let cold = cache.build(snapshot.manifest(), &sources)?;
        let warm = cache.build(snapshot.manifest(), &sources)?;
        let work: serde_json::Value = serde_json::from_str(warm.to_json()).unwrap();
        assert_eq!(work["work"]["checked_HIR_reused"], snapshot.sources().len());
        assert_eq!(work["work"]["modules_resolved"], 0);
        for build in [&cold, &warm] {
            let expected = snapshot.semantic_graph();
            let actual = build.revision().semantic_graph();
            assert!(actual == expected, "checked NDJSON Project graph changed");
            hir::validate(build.revision().test_program()).unwrap();
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn ndjson_receiving_range_rejects_foreign_root_type_operation_and_stale_producer() {
    let manifest = root().join("std/data-ndjson/semaprax.toml");
    project::with_authenticated_project(&manifest, |snapshot| {
        let original = snapshot.test_program();
        hir::validate(original).unwrap();
        let foreign = original
            .functions
            .iter()
            .find(|f| f.id.as_str() == "std.data.ndjson.frame-at")
            .unwrap()
            .params[0]
            .id
            .clone();
        for mutation in 0..4 {
            let mut forged = original.clone();
            let function = forged
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "std.data.ndjson.basic-fault")
                .unwrap();
            let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
                panic!("ordinary receiver block")
            };
            let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
                panic!("actual receiving line binding")
            };
            let hir::ResolvedExprKind::ByteRange {
                operation,
                source,
                start,
                end,
            } = &mut value.kind
            else {
                panic!("ordinary byte-range receiving seam")
            };
            match mutation {
                0 => {
                    let hir::ResolvedExprKind::Place(place) = &mut source.kind else {
                        panic!("named same-body source")
                    };
                    place.root = foreign.clone();
                }
                1 => source.ty = hir::ResolvedType::Bytes,
                2 => *operation = hir::DeclarationId::new("core.bytes.as-slice"),
                3 => end.id = start.id.clone(),
                _ => unreachable!(),
            }
            assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn ndjson_forged_frame_contract_is_sticky_and_failed_ranges_never_publish() {
    let scratch = temporary("ndjson-receiving-refusal");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    let manifest = std::fs::read_to_string(root().join("std/data-ndjson/semaprax.toml")).unwrap();
    std::fs::write(scratch.join("semaprax.toml"), manifest).unwrap();
    std::fs::copy(
        root().join("std/data-ndjson/src/ndjson.spx"),
        scratch.join("src/ndjson.spx"),
    )
    .unwrap();
    std::fs::copy(
        root().join("std/data-ndjson/src/examples.spx"),
        scratch.join("src/examples.spx"),
    )
    .unwrap();
    for frame in [
        "ordinal:0usize,start:0usize,end:99usize,next:99usize",
        "ordinal:1usize,start:0usize,end:2usize,next:3usize",
        "ordinal:0usize,start:3usize,end:5usize,next:5usize",
    ] {
        let source = format!("module std.data.ndjson.tests; use type @id(\"std.data.ndjson.frame\") from std.data.ndjson as Frame; use type @id(\"std.data.ndjson.fault\") from std.data.ndjson as Fault; use function @id(\"std.data.ndjson.basic-fault\") from std.data.ndjson as basic_fault; @id(\"std.data.ndjson.tests.main\") fn main()->i64 {{let raw=[123u8,125u8,10u8,123u8,125u8];let body=array_as_slice(raw);let fault=basic_fault(body,Frame{{{frame}}});fault.category}}");
        let parsed = semaprax::parse(&source, "forged-frame.spx").unwrap();
        std::fs::write(
            scratch.join("src/tests.spx"),
            semaprax::format::canonical(&parsed),
        )
        .unwrap();
        project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
            for _ in 0..3 {
                let result = snapshot.execute_test(&project::ProjectExecutionOptions::default())?;
                let project::ProjectExecutionOutcome::LanguageFailure(status) = result.outcome()
                else {
                    panic!("frame refusal cannot publish category: {result:?}")
                };
                assert_eq!(
                    status.domain_id(),
                    semaprax::conformance::CONTRACT_STATUS_DOMAIN_V1
                );
                assert_eq!(
                    status.code(),
                    semaprax::conformance::CONTRACT_REQUIRES_FALSE_CODE
                );
            }
            failure::check(snapshot.test_program(), &scratch)?;
            Ok(())
        })
        .unwrap();
    }
    // Actual package global-fault exact boundary, under the unchanged owning maximum envelope.
    let exact = r#"module std.data.ndjson.tests; use type @id("std.data.ndjson.fault") from std.data.ndjson as Fault; use function @id("std.data.ndjson.global-fault") from std.data.ndjson as global_fault; @id("std.data.ndjson.tests.main") fn main()->i64 {let raw=bytes_zeroed(65536usize);let fault=global_fault(bytes_as_slice(raw));if fault.category==0 && fault.record_index == -1 {0}else{1}}"#;
    std::fs::write(
        scratch.join("src/tests.spx"),
        semaprax::format::canonical(&semaprax::parse(exact, "exact-frame-body.spx").unwrap()),
    )
    .unwrap();
    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        let options = project::ProjectExecutionOptions::new(1_048_576, 160_000_000).unwrap();
        let result = snapshot.execute_test(&options)?;
        assert!(
            matches!(
                result.outcome(),
                project::ProjectExecutionOutcome::Returned(0)
            ),
            "exact global boundary: {result:?}"
        );
        // This closure has owned Bytes imports, so use the existing byte
        // host with exact one-live-Bytes bound rather than the Vec-only host.
        for (program, expected_peak) in
            [(snapshot.entry_program(), 0), (snapshot.test_program(), 1)]
        {
            let c = semaprax::codegen::emit_hir_c(program).map_err(|error| vec![error])?;
            assert!(!c.contains("memcpy(result, source"));
            for optimization in ["-O0", "-O2"] {
                super::compile_and_run_c(&c, &scratch, optimization, "0");
            }
            let wasm =
                semaprax::wasm::emit_resolved_module(program).map_err(|error| vec![error])?;
            std::fs::write(scratch.join("exact-ndjson.wasm"), wasm).unwrap();
            std::fs::write(
                scratch.join("exact-ndjson.mjs"),
                super::wasm_host::wasm_conformance_js(
                    "exact-ndjson.wasm",
                    1,
                    0,
                    Some(expected_peak),
                ),
            )
            .unwrap();
            let output = std::process::Command::new("node")
                .arg("exact-ndjson.mjs")
                .current_dir(&scratch)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(scratch).unwrap();
}
