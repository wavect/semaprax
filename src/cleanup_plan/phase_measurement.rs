//! Opt-in compiler-phase measurements, never allocation or runtime-cost receipts.
use super::build::helper_index;
use super::*;
use sha2::{Digest, Sha256};
use std::time::Instant;
const RESERVATION_LIMIT: usize = 67_108_864;
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn plans(program: &crate::hir::ResolvedProgram) -> Vec<Vec<u8>> {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .map(|function| crate::cache_codec::encode(&function.cleanup_plan).unwrap())
        .collect()
}
fn constructed_bytes(plans: &[CleanupPlan]) -> Vec<Vec<u8>> {
    plans
        .iter()
        .map(|plan| crate::cache_codec::encode(plan).unwrap())
        .collect()
}
fn verify_pair(program: &crate::hir::ResolvedProgram) {
    crate::hir::validate(program).unwrap();
    let expected = plans(program);
    for indexed in [false, true] {
        let (built, overflowed, _) =
            crate::bounded_output::with_limit_usage(RESERVATION_LIMIT, || {
                build::phase_measurement::construct(program, indexed)
            });
        assert!(!overflowed);
        assert_eq!(constructed_bytes(&built.unwrap()), expected);
        let (replayed, overflowed, reserved) = replay::phase_measurement::replay(program, indexed);
        assert!(
            !overflowed,
            "replay indexed={indexed} exhausted scratch after {reserved} bytes: {replayed:?}"
        );
        replayed.unwrap();
    }
    assert_eq!(
        constructed_bytes(&helper_index::build_program_plans(program).unwrap()),
        expected
    );
    validate::owned::validate_program_unindexed_reference(program).unwrap();
    validate_program(program).unwrap();
}
#[test]
fn paired_routes_preserve_complete_program_plans_and_independent_replay() {
    let source = include_str!("../../tests/owned_data/byte_helper_renewal.spx");
    let ast = crate::check(source, "paired-proof.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    verify_pair(&program);
}
fn emit(row: serde_json::Value) {
    println!("OPT738_JSON={row}");
}
#[test]
#[ignore = "opt-in paired phase measurement; run the bound driver after qualification"]
fn generated_codec_construction_and_replay_measurement() {
    let root = std::path::PathBuf::from(
        std::env::var_os("SEMAPRAX_OPT738_PROJECT").expect("driver supplies private fixture root"),
    );
    let warmup: usize = std::env::var("SEMAPRAX_OPT738_WARMUP")
        .unwrap()
        .parse()
        .unwrap();
    let samples: usize = std::env::var("SEMAPRAX_OPT738_SAMPLES")
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=100).contains(&warmup) && (1..=1000).contains(&samples));
    let manifest = root.join("semaprax.toml");
    let profile = crate::project::JsonCodecProfile::CollectionResponse {
        max_string_bytes: 64,
    };
    let generated = crate::project::with_authenticated_project(&manifest, |snapshot| {
        let revision = snapshot.retain_revision();
        let source = crate::project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "work.report",
            profile,
        )?;
        crate::project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "work.report",
            &source,
            profile,
        )?;
        Ok(source)
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), &generated).unwrap();
    let app = include_str!("phase_measurement/app.spx");
    std::fs::write(root.join("src/app.spx"), app).unwrap();
    crate::project::with_authenticated_project(&manifest, |snapshot| {
        let program = snapshot.entry_program();
        verify_pair(program);
        assert!(!program.function_instances.is_empty());
        let index = crate::loop_calls::byte_writer::ModuleIndex::new(program).unwrap();
        let helper_functions = program.functions.iter().filter(|function| index.has_sites(function)).count();
        assert!(helper_functions > 0, "workload must actually exercise helper sites");
        drop(index);
        emit(serde_json::json!({"record":"context","schema":"semaprax.opt738.phase-measurement.v1","generated_source_sha256":sha(generated.as_bytes()),"app_source_sha256":sha(app.as_bytes()),"functions":program.functions.len(),"instances":program.function_instances.len(),"helper_functions":helper_functions,"reservation_limit":RESERVATION_LIMIT,"replay_reservation_limit":validate::scratch_ledger::MAX_REPLAY_SCRATCH_BYTES,"warmup":warmup,"samples":samples}));
        let expected = plans(program);
        for iteration in 0..warmup+samples {
            for phase in ["construction", "replay", "validation"] {
                for indexed in if iteration % 2 == 0 {[false,true]} else {[true,false]} {
                    let start = Instant::now();
                    if phase == "construction" {
                        let (output, overflowed, reserved) = crate::bounded_output::with_limit_usage(RESERVATION_LIMIT, || build::phase_measurement::construct(program,indexed));
                        let elapsed_ns=start.elapsed().as_nanos();
                        assert!(!overflowed);let output=output.unwrap();
                        // Serialization and assertions are outside the measured interval.
                        assert_eq!(constructed_bytes(&output),expected);
                        emit(serde_json::json!({"record":"sample","phase":phase,"indexed":indexed,"iteration":iteration,"warmup":iteration<warmup,"elapsed_ns":elapsed_ns,"reservation_ledger_bytes":reserved,"replay_work_units":null,"status":"success"}));
                    } else if phase == "validation" {
                        let before = crate::bounded_output::detached::detached_debits();
                        let (output, overflowed, reserved) = crate::bounded_output::with_limit_usage(RESERVATION_LIMIT, || {
                            if indexed {validate_program(program)} else {validate::owned::validate_program_unindexed_reference(program)}
                        });
                        let elapsed_ns=start.elapsed().as_nanos();
                        let replay_reserved = crate::bounded_output::detached::detached_debits().checked_sub(before).unwrap();
                        assert!(!overflowed);output.unwrap();
                        emit(serde_json::json!({"record":"sample","phase":phase,"indexed":indexed,"iteration":iteration,"warmup":iteration<warmup,"elapsed_ns":elapsed_ns,"reservation_ledger_bytes":reserved,"detached_replay_ledger_bytes":replay_reserved,"replay_work_units":null,"status":"success"}));
                    } else {
                        let (output, overflowed, reserved) = replay::phase_measurement::replay(program,indexed);
                        let elapsed_ns=start.elapsed().as_nanos();
                        assert!(!overflowed, "replay indexed={indexed} exhausted scratch after {reserved} bytes: {output:?}");
                        let (work,skeleton)=output.unwrap();
                        emit(serde_json::json!({"record":"sample","phase":phase,"indexed":indexed,"iteration":iteration,"warmup":iteration<warmup,"elapsed_ns":elapsed_ns,"reservation_ledger_bytes":reserved,"replay_work_units":work,"skeleton_work_units":skeleton,"status":"success"}));
                    }
                }
            }
        }
        Ok(())
    }).unwrap();
}
