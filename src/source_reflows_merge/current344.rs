// Exact direct-reader bridges discovered by the current344 owning run.
// No compiler, budget, source authority, or executable qualification is granted.
use sha2::{Digest, Sha256};

const DATA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/current344-input-composition-20261016/bridges.json"
));
const DATA_LENGTH: usize = 230623;
const DATA_SHA256: [u8; 32] = [
    82, 230, 195, 37, 70, 229, 47, 60, 175, 27, 76, 223, 170, 76, 12, 252, 162, 136, 44, 204, 57,
    205, 121, 60, 185, 202, 42, 194, 202, 52, 247, 141,
];

fn assert_digest(source: &str, expected: &serde_json::Value) {
    let expected = expected.as_str().expect("fixed whole-source SHA256");
    assert_eq!(expected.len(), 64);
    let bytes: Vec<u8> = (0..64)
        .step_by(2)
        .map(|index| u8::from_str_radix(&expected[index..index + 2], 16).unwrap())
        .collect();
    assert_eq!(Sha256::digest(source.as_bytes()).as_slice(), bytes);
}

// Closed whole-current fronts preserve the incoming main source-stage contract.
fn bf8_current_input(source: &str, current: &str) -> String {
    let (compiled, length, sha, prior_sha, edits): (&str, usize, &str, &str, &[(usize, &str, &str)]) = match source {
        "src/hir/ids.rs" => (include_str!("../hir/ids.rs"), 23422, "1eceacba63ce7ce7cbeb99ebe1ecfa05b3bdc25b679c6f74fadaa0b5b3a0284e", "5e56cda454fa3f14f61e2db5f410eea13ac7795d0887e680348eefdc64e80e64", &[
            (23379, "\n#[cfg(test)]\nmod lowering_identity_tests;\n", ""),
        ]),
        "tests/native/string_settlement/codec_allocation/string_vectors.rs" => (include_str!("../../tests/native/string_settlement/codec_allocation/string_vectors.rs"), 28714, "b723bc31c6b6e3440da6eb95fc732d0469b8df33f14160309060ffca2727257a", "8fc18b5920eb5f4a5c5d43c886f327b32e6f0ead6570d1640e43db0e5f0cfddb", &[
            (21173, "\n// Exact public-workspace host for production-library paid attribution. This\n// does not assert admission when the unchanged 64MiB builder refuses.\n#[cfg(feature = \"unstable-workflow-profiling\")]\n#[test]\nfn catalog_normalizer_public_workspace_reports_actual_paid_validation_phases() {\n    use semaprax::workflow_profile::plan_comparison::{\n        Binding, Build, Capture, PaidPhase, PhaseOrigin, PhasePayer,\n    };\n    use sha2::{Digest, Sha256};\n    const MANIFEST: &str =\n        include_str!(\"../../../../examples/catalog-normalizer-project/semaprax.toml\");\n    const SOURCES: &[(&str, &str)] = &[\n        (\n            \"src/app.spx\",\n            include_str!(\"../../../../examples/catalog-normalizer-project/src/app.spx\"),\n        ),\n        (\n            \"src/batch.spx\",\n            include_str!(\"../../../../examples/catalog-normalizer-project/src/batch.spx\"),\n        ),\n        (\n            \"src/enrichment.spx\",\n            include_str!(\"../../../../examples/catalog-normalizer-project/src/enrichment.spx\"),\n        ),\n        (\n            \"src/limits.spx\",\n            include_str!(\"../../../../examples/catalog-normalizer-project/src/limits.spx\"),\n        ),\n        (\n            \"src/record.spx\",\n            include_str!(\"../../../../examples/catalog-normalizer-project/src/record.spx\"),\n        ),\n        (\n            \"src/tests.spx\",\n            include_str!(\"../../../../examples/catalog-normalizer-project/src/tests.spx\"),\n        ),\n        (\n            \"dependencies/std.data.json.dec/0.1.0/dec.spx\",\n            include_str!(\"../../../../std/data-json-dec/src/dec.spx\"),\n        ),\n        (\n            \"dependencies/std.data.json.doc/0.1.0/doc.spx\",\n            include_str!(\"../../../../std/data-json-doc/src/doc.spx\"),\n        ),\n        (\n            \"dependencies/std.io/0.1.0/io.spx\",\n            include_str!(\"../../../../std/io/src/io.spx\"),\n        ),\n        (\n            \"dependencies/std.data.ndjson/0.1.0/ndjson.spx\",\n            include_str!(\"../../../../std/data-ndjson/src/ndjson.spx\"),\n        ),\n        (\n            \"dependencies/std.io.lines/0.1.0/lines.spx\",\n            include_str!(\"../../../../std/io-lines/src/lines.spx\"),\n        ),\n        (\n            \"dependencies/std.data.json.utf8/0.1.0/utf8.spx\",\n            include_str!(\"../../../../std/data-json-utf8/src/utf8.spx\"),\n        ),\n    ];\n    let manifest = project::ProjectManifest::parse(MANIFEST).unwrap();\n    assert_eq!(manifest.sources().len(), 6);\n    for (path, _) in &SOURCES[..6] {\n        assert!(manifest.sources().iter().any(|p| p.as_str() == *path));\n    }\n    assert_eq!(SOURCES.len(), 12);\n    let root = std::env::temp_dir().join(format!(\n        \"native-normalizer-paid-{}-{}\",\n        std::process::id(),\n        SERIAL.fetch_add(1, Ordering::Relaxed)\n    ));\n    fs::create_dir(&root).unwrap();\n    let root = fs::canonicalize(root).unwrap();\n    struct Remove(std::path::PathBuf);\n    impl Drop for Remove {\n        fn drop(&mut self) {\n            let _ = std::fs::remove_dir_all(&self.0);\n        }\n    }\n    let _remove = Remove(root.clone());\n    fs::write(root.join(\"semaprax.toml\"), MANIFEST).unwrap();\n    let mut ordered = SOURCES.iter().copied().collect::<Vec<_>>();\n    ordered.sort_by(|left, right| left.0.cmp(right.0));\n    let mut digest = Sha256::new();\n    digest.update((MANIFEST.len() as u64).to_le_bytes());\n    digest.update(MANIFEST.as_bytes());\n    let mut files = Vec::new();\n    for (path, source) in ordered {\n        let destination = root.join(path);\n        fs::create_dir_all(destination.parent().unwrap()).unwrap();\n        fs::write(&destination, source).unwrap();\n        let actual = fs::read(&destination).unwrap();\n        assert_eq!(actual, source.as_bytes());\n        digest.update((path.len() as u64).to_le_bytes());\n        digest.update(path.as_bytes());\n        digest.update((actual.len() as u64).to_le_bytes());\n        digest.update(&actual);\n        eprintln!(\n            \"NORMALIZER_PAID_INPUT path={path} bytes={} sha256={:x}\",\n            actual.len(),\n            semaprax::digest_hex::LowerHex(Sha256::digest(&actual))\n        );\n        files.push(serde_json::json!({\"path\":path}));\n    }\n    let paths = root.join(\"paths.json\");\n    fs::write(\n        &paths,\n        format!(\n            \"{{\\\"schema\\\":\\\"semaprax.workspace-semantic-path-set.v1\\\",\\\"files\\\":{}}}\\n\",\n            serde_json::to_string(&files).unwrap()\n        ),\n    )\n    .unwrap();\n    let capture = Capture::begin(Binding::caller_labelled(\"catalog-normalizer exact12/public-semantic-workspace/default64MiB; revision externally bound\", digest.finalize().into())).unwrap();\n    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {\n        semaprax::semantic_workspace::initialize(&root, &paths)\n    }));\n    // The public route has returned or unwound and all compiler owners dropped.\n    let report = capture.finish().unwrap();\n    eprintln!(\n        \"NORMALIZER_PAID_PHASES default_builder_cap=67108864 inclusive_not_additive=true report={report:?}\"\n    );\n    let result = match outcome {\n        Ok(result) => result,\n        Err(payload) => std::panic::resume_unwind(payload),\n    };\n    assert_eq!(report.build, Build::InstrumentedProductionAlgorithm);\n    let exclusive = &report.resolved_core;\n    assert_eq!(exclusive.algorithm, semaprax::workflow_profile::plan_comparison::CoreDebitAlgorithm::InstrumentedProductionAlgorithm);\n    assert!(\n        exclusive.complete && !exclusive.overflow && !exclusive.lost && !exclusive.invalid,\n        \"{exclusive:?}\"\n    );\n    assert!(exclusive.entered_attempts > 0);\n    assert_eq!(\n        exclusive.final_attempt_ordinal,\n        Some(exclusive.entered_attempts - 1)\n    );\n    for attempt in &exclusive.attempts[..exclusive.stored_attempts] {\n        assert!(attempt.complete && !attempt.invalid, \"{attempt:?}\");\n        assert_eq!(attempt.stage_sum(), Some(attempt.consumed));\n        assert_eq!(attempt.operation_sum(), Some(attempt.consumed));\n    }\n    eprintln!(\n        \"NORMALIZER_EXCLUSIVE_CORE_DEBITS reservations_not_allocations=true attempts_nonadditive=true final_attempt_explicit=true report={exclusive:?}\"\n    );\n    assert!(report.paid_phases.balanced && !report.paid_phases.overflow);\n    for phase in [\n        PaidPhase::CrosscheckEnumerated,\n        PaidPhase::CrosscheckFactored,\n    ] {\n        for payer in [\n            PhasePayer::ActiveBuilder,\n            PhasePayer::Unobserved,\n            PhasePayer::Oracle,\n            PhasePayer::Selected,\n            PhasePayer::Unknown,\n        ] {\n            assert_eq!(\n                report\n                    .paid_phases\n                    .row(phase, payer, PhaseOrigin::TestCrosscheck)\n                    .entered,\n                0,\n                \"production library has no cfgtest crosscheck\"\n            );\n        }\n    }\n    let validation = report.paid_phases.row(\n        PaidPhase::CleanupValidation,\n        PhasePayer::ActiveBuilder,\n        PhaseOrigin::Shipping,\n    );\n    assert!(\n        validation.entered > 0 && validation.ended == validation.entered,\n        \"{report:?}\"\n    );\n    assert!(\n        validation.known_paid > 0 && validation.unknown == 0,\n        \"{report:?}\"\n    );\n    match result {\n        Ok(revision) => assert!(!revision.is_empty()),\n        Err(errors) => {\n            eprintln!(\"NORMALIZER_PAID_REAL_REFUSAL {errors:?}\");\n            assert!(\n                !errors.is_empty() && errors.iter().all(|e| e.code == \"SPX-G171\"),\n                \"{errors:?}\"\n            );\n        }\n    }\n}\n", ""),
            (17702, "        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let generated = observe_slot_copies(observed_generated_v2(policy, true));\n", ""),
            (17568, "        #[cfg(not(feature = \"unstable-workflow-profiling\"))]\n", ""),
            (13176, "        project::JsonCodecProfile::NestedResponseV2 {\n            max_string_bytes: 64,\n            max_array_items: 256,\n        },\n        false,\n    ));\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let generated = observe_slot_copies(observed_generated_v2(\n", ""),
            (13065, "    #[cfg(not(feature = \"unstable-workflow-profiling\"))]\n", ""),
            (10336, "    #[cfg(feature = \"unstable-workflow-profiling\")]\n    generated_project_fingerprints(&root, \"final-app-written\", response, invalid);\n", ""),
            (9881, "        #[cfg(feature = \"unstable-workflow-profiling\")]\n        generated_project_fingerprints(\n            &root,\n            if matches!(&policy, project::JsonCodecProfile::NestedRequest { .. }) {\n                \"request-codec-written\"\n            } else {\n                \"response-codec-written\"\n            },\n            response,\n            invalid,\n        );\n", ""),
            (8824, "    #[cfg(feature = \"unstable-workflow-profiling\")]\n    generated_project_fingerprints(&root, \"initial-written\", response, invalid);\n", ""),
            (3387, "// Diagnostic build only: all compiler owners unwind/drop before reporting.\n// This labels the fixture bytes, not an authenticated repository revision.\n#[cfg(feature = \"unstable-workflow-profiling\")]\nfn observed_generated_v2(response: project::JsonCodecProfile, invalid: bool) -> String {\n    eprintln!(\n        \"PLAN_GENERATED_PROJECT_SOURCE_EXPECTED checkpoints=initial-written,request-codec-written,response-codec-written,final-app-written; only reached checkpoints emit four actual byte hashes; absent checkpoints remain unobserved\"\n    );\n    use semaprax::workflow_profile::plan_comparison::{Binding, Capture};\n    use sha2::{Digest, Sha256};\n    let mut digest = Sha256::new();\n    digest.update(SCHEMA.replace(\"Vec<Row>\", \"Vec<string>\").as_bytes());\n    digest.update(include_bytes!(\"../bulk_utf8/semaprax.toml\"));\n    digest.update(INPUT_V2.as_bytes());\n    digest.update(OUTPUT_V2.as_bytes());\n    digest.update(b\"request=NestedRequest/64/256; generated-v2-v1\");\n    digest.update(app_v2(invalid).as_bytes());\n    digest.update(format!(\"{response:?}/{invalid}\").as_bytes());\n    let capture = match Capture::begin(Binding::caller_labelled(\n        \"native/string-vectors/generated-v2; revision=unknown (bind external round manifest); caller-labelled\",\n        digest.finalize().into(),\n    )) {\n        Ok(capture) => capture,\n        Err(error) => {\n            eprintln!(\"PLAN_COMPARISON_DIAGNOSTIC_REFUSED {error:?}\");\n            return generated_v2(response, invalid);\n        }\n    };\n    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {\n        generated_v2(response, invalid)\n    }));\n    match capture.finish() {\n        Ok(report) => {\n            use semaprax::workflow_profile::plan_comparison::Site;\n            eprintln!(\n        \"PLAN_COMPARISON_DIAGNOSTIC build={:?} generation={} P={} N={} STEP={} LOAD={} EMPTY={} TAIL={} left_head={} right_head={} left_exhaustion={} right_exhaustion={} terminal={} started={} ended={} success={} unwind={} counts_complete={} detail_complete={} overflow={}\",\n        report.build, report.generation,\n        report.counts.at(Site::PairPrepared), report.counts.at(Site::Round),\n        report.counts.at(Site::StepAdmitted), report.counts.at(Site::LoadAdmitted),\n        report.counts.at(Site::EmptySegment), report.counts.at(Site::TailAdmitted),\n        report.counts.at(Site::LeftHead), report.counts.at(Site::RightHead),\n        report.counts.at(Site::LeftExhausted), report.counts.at(Site::RightExhausted),\n        report.counts.at(Site::TerminalCompared), report.counts.started, report.counts.ended,\n        report.counts.succeeded, report.counts.unwound,\n        report.counts_complete, report.detail_complete, report.overflow,\n    );\n            eprintln!(\"PLAN_COMPARISON_DIAGNOSTIC_DETAILS {report:?}\");\n        }\n        Err(error) => eprintln!(\"PLAN_COMPARISON_DIAGNOSTIC_INCOMPLETE {error:?}\"),\n    }\n    match result {\n        Ok(value) => value,\n        Err(payload) => std::panic::resume_unwind(payload),\n    }\n}\n// Host-fixture diagnostics only: hash actual written bytes before each compiler\n// admission. Unreached checkpoints have no equality claim. This does not emit\n// compiler events, change generated source, or enter the generated-C allocator.\n#[cfg(feature = \"unstable-workflow-profiling\")]\nfn generated_project_fingerprints(\n    root: &std::path::Path,\n    checkpoint: &'static str,\n    response: project::JsonCodecProfile,\n    invalid: bool,\n) {\n    use sha2::{Digest, Sha256};\n    for relative_path in [\n        \"semaprax.toml\",\n        \"src/schema.spx\",\n        \"src/app.spx\",\n        \"src/tests.spx\",\n    ] {\n        let bytes = fs::read(root.join(relative_path)).expect(\"written native fixture source\");\n        let sha256 = Sha256::digest(&bytes)\n            .iter()\n            .map(|byte| format!(\"{byte:02x}\"))\n            .collect::<String>();\n        eprintln!(\n            \"PLAN_GENERATED_PROJECT_SOURCE {}\",\n            serde_json::json!({\n                \"checkpoint\": checkpoint, \"response_profile\": format!(\"{response:?}\"),\n                \"invalid\": invalid, \"relative_path\": relative_path,\n                \"byte_length\": bytes.len(), \"sha256\": sha256,\n                \"scope\": \"reached-written-source-only; absent stages unobserved\"\n            })\n        );\n    }\n}\n", ""),
        ]),
        _ => return current.to_owned(),
    };
    assert_eq!(
        current, compiled,
        "complete current input before main bf8 bridge"
    );
    assert_eq!(current.len(), length);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        sha
    );
    let mut before = current.to_owned();
    for &(at, after, prior) in edits {
        assert_eq!(&before[at..at + after.len()], after);
        before.replace_range(at..at + after.len(), prior);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
        ),
        prior_sha
    );
    before
}

pub(crate) fn project(source: &str, current: &str) -> String {
    let outer = super::final_ready_inputs::current344_input(source, current);
    let current = outer.as_str();
    let incoming = bf8_current_input(source, current);
    let current = incoming.as_str();
    // Bind every complete raw input before the original historical bridge.
    let stage = match source {
        "src/cleanup_plan.rs" => Some(("144f255bb216245a2ef49c197b60659e2b725c45c0ca05209ebe16feb3e34920", include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/0e19e80ed9cc80eafe015ff4e11688d6aa24b80755e6afbe1ed727f5d2a63621.txt"))),
        "src/hir/ids.rs" => Some(("5e56cda454fa3f14f61e2db5f410eea13ac7795d0887e680348eefdc64e80e64", include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/186e98de002cd77f1e546a34423a6ca4792d8fbedc8ff7decc8d8a6a3e03b306.txt"))),
        "tests/native/string_settlement/codec_allocation/string_vectors.rs" => Some(("8fc18b5920eb5f4a5c5d43c886f327b32e6f0ead6570d1640e43db0e5f0cfddb", include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/8fc18b5920eb5f4a5c5d43c886f327b32e6f0ead6570d1640e43db0e5f0cfddb.txt"))),
        "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs" => Some(("01b3e408874930f2aad015d20cf8317fc1507183f92aae740d11985b99d223e1", include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/01b3e408874930f2aad015d20cf8317fc1507183f92aae740d11985b99d223e1.txt"))),
        "src/hir/function_value/funded.rs" => Some(("633299f2369b23030fddc8feaae09a97da2622bc831f9f766d52b618baea2019", include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/633299f2369b23030fddc8feaae09a97da2622bc831f9f766d52b618baea2019.txt"))),
        "tests/native/string_settlement/codec_allocation.rs" => Some(("82b7368428178f592d1de52dd17b175b75b5dfd7ca1543dca9bacbdcb5dc7200", include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/82b7368428178f592d1de52dd17b175b75b5dfd7ca1543dca9bacbdcb5dc7200.txt"))),
        "src/cleanup_plan/replay/sequence_prefix/main24/before_wrapper_tests.rs" => Some(("dbe4a9b878c0fa842f803b4ed8d2bc6e3849c72def30608fa1dd6c76a111b4f8", include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/dbe4a9b878c0fa842f803b4ed8d2bc6e3849c72def30608fa1dd6c76a111b4f8.txt"))),
        "src/cleanup_plan/replay/sequence_prefix/main_c11/before_wrapper_tests.rs" => Some(("dbe4a9b878c0fa842f803b4ed8d2bc6e3849c72def30608fa1dd6c76a111b4f8", include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/dbe4a9b878c0fa842f803b4ed8d2bc6e3849c72def30608fa1dd6c76a111b4f8.txt"))),
        _ => None,
    };
    let current = if let Some((sha256, before)) = stage {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            sha256,
            "complete current344 raw input: {source}"
        );
        before
    } else {
        current
    };
    assert_eq!(DATA.len(), DATA_LENGTH, "fixed bridge metadata length");
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(DATA.as_bytes())),
        DATA_SHA256
    );
    let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
    let rows = rows.as_array().unwrap();
    let matching: Vec<_> = rows.iter().filter(|row| row["source"] == source).collect();
    assert!(matching.len() <= 1, "unique direct-reader bridge");
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    assert_digest(current, &row["current_sha256"]);
    let mut previous = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert!(!new.is_empty(), "nonempty exact preimage");
        assert_eq!(
            previous.matches(new).count(),
            1,
            "complete unique bridge preimage"
        );
        previous = previous.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_digest(&previous, &row["original_sha256"]);
    previous
}

#[cfg(test)]
pub(crate) fn refuses_drift(source: &str, current: &str) {
    let mut changed = current.to_owned();
    changed.push(' ');
    assert!(std::panic::catch_unwind(|| project(source, &changed)).is_err());
    assert!(std::panic::catch_unwind(|| project(source, "")).is_err());
    let previous = project(source, current);
    assert_ne!(previous, current);
    assert!(std::panic::catch_unwind(|| project(source, &previous)).is_err());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_direct_reader_preimages_refuse_stale_truncated_and_historical_inputs() {
        super::super::final_ready_inputs::controls();
        let sources = [
            (
                "src/cleanup_plan.rs",
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan.rs")),
            ),
            (
                "src/hir/ids.rs",
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/ids.rs")),
            ),
            (
                "tests/native/string_settlement/codec_allocation/string_vectors.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/native/string_settlement/codec_allocation/string_vectors.rs"
                )),
            ),
            (
                "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs"
                )),
            ),
            (
                "src/hir/function_value/funded.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/hir/function_value/funded.rs"
                )),
            ),
            (
                "tests/native/string_settlement/codec_allocation.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/native/string_settlement/codec_allocation.rs"
                )),
            ),
            (
                "src/cleanup_plan/replay/sequence_prefix/main24/before_wrapper_tests.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/sequence_prefix/main24/before_wrapper_tests.rs"
                )),
            ),
            (
                "src/cleanup_plan/replay/factored/canonical_shared.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/factored/canonical_shared.rs"
                )),
            ),
            (
                "src/cleanup_plan/replay/sequence_prefix/main_c11/before_wrapper_tests.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/sequence_prefix/main_c11/before_wrapper_tests.rs"
                )),
            ),
        ];
        for (source, current) in sources {
            refuses_drift(source, current);
        }
        assert_eq!(project("unlisted", "unchanged"), "unchanged");
    }
}

// Historical sequence-minimum ownership checks predate Clippy's equivalent
// collapsed condition. Authenticate the entire actual function before undoing
// the reviewed finite shapes; no other code or assertion is normalized.
pub(crate) fn sequence_minimum_input(current: &str) -> String {
    let bound = super::opt858_sequence_input::sequence(current);
    let current = bound.as_str();
    const DATA: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/sequence-minimum-clippy-input-20261016/input.json"
    ));
    assert_eq!(DATA.len(), 1435);
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(DATA.as_bytes())),
        [
            184, 172, 143, 215, 75, 49, 164, 137, 21, 16, 110, 92, 235, 212, 12, 153, 16, 216, 108,
            147, 30, 30, 205, 242, 148, 65, 59, 136, 247, 217, 13, 112
        ]
    );
    let row: serde_json::Value = serde_json::from_str(DATA).unwrap();
    assert_digest(current, &row["current_sha256"]);
    let mut historical = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(historical.matches(new).count(), 1);
        historical = historical.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_digest(&historical, &row["original_sha256"]);
    historical
}

#[cfg(test)]
mod sequence_minimum_input_tests {
    use super::*;
    #[test]
    fn exact_complete_sequence_input_refuses_drift_and_historical_bytes() {
        let root = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay.rs"
        ));
        let current = format!(
            "fn sequence_skeleton_paths({}",
            root.split_once("fn sequence_skeleton_paths(")
                .unwrap()
                .1
                .split_once("\nuse path_summary::has_active_paths;")
                .unwrap()
                .0
        );
        let historical = sequence_minimum_input(&current);
        assert_ne!(historical, current);
        for wrong in [String::new(), format!("{current} "), historical] {
            assert!(std::panic::catch_unwind(|| sequence_minimum_input(&wrong)).is_err());
        }
    }
}

// Keep the original push-order source predicate on the real current owner as
// well as the independent historical Main44 namespace fixture.
#[cfg(test)]
mod live_push_source_tests {
    #[test]
    fn actual_skeleton_pushes_keep_work_before_materialization_and_growth() {
        let work = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/skeleton_work.rs"
        ));
        let original_push = "        self.charge(1, phase)?;\n        note_skeleton_materialization();\n        self.budget.reserve_vec(self.function, paths, 1, phase)?;\n        paths.push(path);";
        assert_eq!(work.matches(original_push).count(), 2);
    }
}
