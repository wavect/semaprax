//! LAW-17 installed proof and candidate/reduction gates.
use super::*;

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn proved_add_zero_identity_yields_only_a_revalidated_candidate() {
    use semaprax::compute_profile::cpu_reference::{
        ComputeCapability, CpuReferenceSession, KernelShape, ReductionDomain, ReductionSchedule,
        Scalar, ScalarKind,
    };
    use semaprax::project::ProjectCandidate;
    let project = native_project("law-add-zero-rewrite", "n + 0 == n");
    let app = project.root.join("src/app.spx");
    let original = std::fs::read_to_string(&app).unwrap();
    let raw = original
        .replacen("seventeen(0)", "let n = 40; n + 0", 1)
        .replacen(
            "@id(\"fresh.main\")",
            "@id(\"fresh.combine\") fn combine(acc: i64, item: i64) -> i64 { acc + item }\n@id(\"fresh.guarded\") fn guarded() -> i64 { let n = 9223372036854775807 + 1; n + 0 }\n@id(\"fresh.main\")",
            1,
        );
    assert_ne!(original, raw);
    let changed = semaprax::format::canonical(&semaprax::parse(&raw, &app).unwrap());
    std::fs::write(&app, &changed).unwrap();
    let revision = project.revision();
    let laws = LawSet::derive(
        &revision,
        "native-proof-v1",
        revision.law_modules().to_vec(),
    )
    .unwrap();
    let tool = provisioned(&project, ToolKind::Z3);
    let proof = prove_scalar_law(&revision, &laws, "fresh.law.identity", &tool).unwrap();
    let mut session = CpuReferenceSession::open(ComputeCapability::cpu_reference_all());
    let fold = session
        .load_kernel(
            revision.entry_program(),
            "fresh.combine",
            KernelShape::SequentialFold,
        )
        .unwrap();
    let input = session.alloc(ScalarKind::I64, 3).unwrap();
    let output = session.alloc(ScalarKind::I64, 1).unwrap();
    session
        .upload(input, 0, &[Scalar::I64(1), Scalar::I64(2), Scalar::I64(3)])
        .unwrap();
    for (schedule, requires_commutativity) in [
        (ReductionSchedule::Regroup, false),
        (ReductionSchedule::Reorder, true),
    ] {
        let report = session
            .checked_add_reduction_eligibility(
                &revision,
                &fold,
                input,
                output,
                &laws,
                "fresh.law.identity",
                &proof,
                ReductionDomain {
                    minimum: 0,
                    maximum: 100,
                    maximum_elements: 3,
                },
                schedule,
            )
            .unwrap();
        let report: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(report["eligible"], true, "{report}");
        assert_eq!(report["commutativity_required"], requires_commutativity);
        assert_eq!(report["parallel_execution_occurred"], false);
        assert_eq!(report["transformation_applied"], false);
        assert_eq!(report["operation_id"], "fresh.combine");
    }
    let unsafe_bound = session
        .checked_add_reduction_eligibility(
            &revision,
            &fold,
            input,
            output,
            &laws,
            "fresh.law.identity",
            &proof,
            ReductionDomain {
                minimum: 0,
                maximum: i64::MAX,
                maximum_elements: 3,
            },
            ReductionSchedule::Regroup,
        )
        .unwrap();
    let unsafe_bound: serde_json::Value = serde_json::from_str(&unsafe_bound).unwrap();
    assert_eq!(unsafe_bound["eligible"], false);
    assert_eq!(
        unsafe_bound["reason"],
        "some_grouping_may_overflow_checked_i64"
    );
    for values in [[i64::MIN, i64::MAX, 1], [i64::MAX, 1, -1]] {
        session.upload(input, 0, &values.map(Scalar::I64)).unwrap();
        let report = session
            .checked_add_reduction_eligibility(
                &revision,
                &fold,
                input,
                output,
                &laws,
                "fresh.law.identity",
                &proof,
                ReductionDomain {
                    minimum: 0,
                    maximum: i64::MAX,
                    maximum_elements: 3,
                },
                ReductionSchedule::Reorder,
            )
            .unwrap();
        let report: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(report["eligible"], false);
    }
    let alias = session
        .checked_add_reduction_eligibility(
            &revision,
            &fold,
            input,
            input,
            &laws,
            "fresh.law.identity",
            &proof,
            ReductionDomain {
                minimum: 0,
                maximum: 100,
                maximum_elements: 3,
            },
            ReductionSchedule::Regroup,
        )
        .unwrap();
    let alias: serde_json::Value = serde_json::from_str(&alias).unwrap();
    assert_eq!(alias["eligible"], false);
    assert_eq!(
        alias["reason"],
        "buffer_alias_or_type_outside_admitted_fold"
    );
    let candidate = ProjectCandidate::open(revision.clone(), revision.project_revision()).unwrap();
    let catalogue: serde_json::Value =
        serde_json::from_str(&candidate.expression_catalog("fresh.main").unwrap()).unwrap();
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == "src/app.spx")
        .unwrap()
        .source();
    let selected = catalogue["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            let span = &entry["source_span"];
            source.get(
                span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize,
            ) == Some("n + 0")
        })
        .unwrap();
    let expression_id = selected["expression_id"].as_str().unwrap();
    let rewritten = candidate
        .propose_checked_i64_add_zero(
            candidate.candidate_digest(),
            "fresh.main",
            expression_id,
            &laws,
            "fresh.law.identity",
            &proof,
        )
        .unwrap();
    assert!(rewritten
        .revision()
        .sources()
        .iter()
        .any(|source| source.path() == "src/app.spx"
            && source.source().contains("let n = 40")
            && source.source().matches("n + 0").count() == 1));
    let guarded_catalogue: serde_json::Value =
        serde_json::from_str(&candidate.expression_catalog("fresh.guarded").unwrap()).unwrap();
    let guarded_id = guarded_catalogue["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            let span = &entry["source_span"];
            source.get(
                span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize,
            ) == Some("n + 0")
        })
        .unwrap()["expression_id"]
        .as_str()
        .unwrap();
    let guarded_rewrite = candidate
        .propose_checked_i64_add_zero(
            candidate.candidate_digest(),
            "fresh.guarded",
            guarded_id,
            &laws,
            "fresh.law.identity",
            &proof,
        )
        .unwrap();
    for (name, variant) in [
        ("before", revision.entry_program()),
        ("rewritten", rewritten.revision().entry_program()),
        ("guarded", guarded_rewrite.revision().entry_program()),
    ] {
        let bytes = semaprax::wasm::emit_resolved_module(variant).unwrap();
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        std::fs::write(project.root.join(format!("{name}.wasm")), bytes).unwrap();
    }
    let script = r#"
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const lo = -(1n << 63n), hi = (1n << 63n) - 1n;
const checked = n => { if (n < lo || n > hi) throw new RangeError('checked i64 overflow'); return n; };
const fail = () => { throw Error('unexpected host import'); };
const env = { spx_add: (a,b) => checked(a+b), spx_sub: fail, spx_mul: fail,
  spx_div: fail, spx_rem: fail, spx_neg: fail, spx_contract_fail: fail };
async function loaded(file) {
  return (await WebAssembly.instantiate(readFileSync(file), {env})).instance.exports;
}
const [before, rewritten, guarded] = await Promise.all(process.argv.slice(1).map(loaded));
assert.equal(before.semaprax_main(), 40n);
assert.equal(rewritten.semaprax_main(), 40n);
function failure(exports) {
  try { exports.semaprax_guarded(); }
  catch (error) { return [error.constructor.name, error.message]; }
  assert.fail('guarded call returned');
}
assert.deepEqual(failure(before), failure(guarded));
"#;
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", script])
        .arg(project.root.join("before.wasm"))
        .arg(project.root.join("rewritten.wasm"))
        .arg(project.root.join("guarded.wasm"))
        .output()
        .expect("Node is required for the LAW-17 emitted-Wasm comparison");
    assert!(
        output.status.success(),
        "Node stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(&app).unwrap(), changed);
    assert!(candidate
        .propose_checked_i64_add_zero(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "fresh.main",
            expression_id,
            &laws,
            "fresh.law.identity",
            &proof,
        )
        .is_err());
    let other = native_project("law-add-zero-stale", "n + 0 == n");
    let other_revision = other.revision();
    let other_laws = LawSet::derive(
        &other_revision,
        "native-proof-v1",
        other_revision.law_modules().to_vec(),
    )
    .unwrap();
    assert!(candidate
        .propose_checked_i64_add_zero(
            candidate.candidate_digest(),
            "fresh.main",
            expression_id,
            &other_laws,
            "fresh.law.identity",
            &proof,
        )
        .is_err());
}
