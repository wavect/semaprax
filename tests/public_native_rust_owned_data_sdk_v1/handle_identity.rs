//! Physical regression probes for the private, single-linked-provider issuer.
//! The probes share the production translation unit; no test hook is exported.

use std::process::Command;

use super::{artifact, run, Fixture, SOURCE};

const IDENTITIES: &str = include_str!("handle_identity.c");
const CONTENTION: &str = include_str!("handle_contention.c");
const THREADS: &str = include_str!("handle_threads.c");

fn execute_probe(label: &str, provider: &str, probe: &str, threaded: bool) {
    let fixture = Fixture::new(label);
    for optimization in ["-O0", "-O2"] {
        let c = fixture.0.join(format!("{label}-{optimization}.c"));
        let executable = fixture.0.join(format!("{label}-{optimization}"));
        std::fs::write(&c, format!("{provider}\n{probe}")).unwrap();
        let mut compile = Command::new("clang");
        compile.args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"]);
        if threaded && !cfg!(windows) {
            compile.arg("-pthread");
        }
        // No -latomic: the production assertion requires native 64-bit atomics.
        run(
            compile.arg(&c).arg("-o").arg(&executable),
            "compile owned handle identity probe",
        );
        run(
            &mut Command::new(executable),
            "run owned handle identity probe",
        );
    }
    #[cfg(target_os = "linux")]
    {
        let c = fixture.0.join(format!("{label}-sanitized.c"));
        let executable = fixture.0.join(format!("{label}-sanitized"));
        std::fs::write(&c, format!("{provider}\n{probe}")).unwrap();
        let mut compile = Command::new("clang");
        compile.args([
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-fsanitize=address,undefined",
            "-fno-sanitize-recover=all",
        ]);
        if threaded {
            compile.arg("-pthread");
        }
        run(
            compile.arg(&c).arg("-o").arg(&executable),
            "compile sanitized owned handle identity probe",
        );
        run(
            &mut Command::new(executable),
            "run sanitized owned handle identity probe",
        );
    }
}

#[test]
fn live_contexts_reincarnation_and_all_4096_slots_keep_exact_handle_authority() {
    execute_probe(
        "handle-identity",
        artifact(SOURCE).source(),
        IDENTITIES,
        false,
    );
}

#[test]
fn exhausted_and_contended_serial_reservations_never_publish_or_retry() {
    let provider = artifact(SOURCE);
    let load = "atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed)";
    assert_eq!(provider.source().matches(load).count(), 1);
    // Insert only a test-translation-unit interleaving: another successful
    // reservation happens after load but before the actual strong CAS. The
    // production comparison/exhaustion/owner-publication code is unchanged.
    let interleaved = format!(
        "#include <stdint.h>\nstatic uint64_t spx_test_serial_snapshot(void);\n{}",
        provider
            .source()
            .replacen(load, "spx_test_serial_snapshot()", 1)
    );
    execute_probe("handle-contention", &interleaved, CONTENTION, false);
}

#[test]
fn distinct_thread_confined_contexts_share_one_nonreusing_issuer() {
    execute_probe("handle-threads", artifact(SOURCE).source(), THREADS, true);
}

#[test]
fn owned_issuer_is_private_atomic_and_uses_the_complete_slot_width() {
    let provider = artifact(SOURCE);
    let source = provider.source();
    assert!(source.contains("static _Atomic(uint64_t) spx_owned_data_next_serial_v1"));
    assert!(source.contains("__atomic_always_lock_free(sizeof(uint64_t), 0)"));
    assert!(source.contains("handle & UINT64_C(0x1fff)"));
    assert!(source.contains("handle >> UINT32_C(13)"));
    assert_eq!(
        source
            .matches("atomic_compare_exchange_strong_explicit")
            .count(),
        1
    );
    assert!(!source.contains("atomic_compare_exchange_weak"));
    assert!(!source.contains("spx_test_serial_snapshot"));
    assert!(!source.contains("SPX_OWNED_DATA_EXPORT bool spx_owned_data_reserve_serial_v1"));
}
