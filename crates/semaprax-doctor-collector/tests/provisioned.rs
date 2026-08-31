//! Ignored physical fixtures; missing provisioning is failure, never a skip.
//! The harness is a TRUSTED provisioner outside both offline guarantees. Supply
//! immutable current-head worker/collector binaries, private mapped user/mount
//! namespaces, exclusive lifecycle/signal ownership and aggregate cgroup cleanup
//! exactly as DOCTOR-OFFLINE-{WORKER,COLLECTOR}-V1 require. The context environment
//! acknowledgement is not attestation. Surrogates additionally require explicit
//! executable-memfd support; there is no host-policy change or weaker fallback.
//! Select these ignored tests serially with `--test-threads=1`, with no other
//! descriptor mutators in the harness. This keeps launch reservations exclusive.
#![cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/launch.rs"]
mod launch;
#[path = "support/observe.rs"]
mod observe;

use fixture::{bundle, executable, reply, request, Ending, SELECTOR};
use observe::{run, Observation};
use std::time::{Duration, Instant};

fn healthy(observation: Observation) {
    assert_eq!(observation.status.code(), Some(0));
    assert!(observation.stderr.is_empty(), "{:?}", observation.stderr);
    // Exact canonical bytes and row order. Either explicitly provisioned build
    // profile is valid; do not infer it from this independently built test crate.
    let expected = |release: &str| {
        let arch = if fixture::architecture() == 1 {
            "x86_64"
        } else {
            "aarch64"
        };
        format!(concat!(
            "{{\"schema\":\"semaprax.doctor.v1\",\"target\":\"native\",\"checks\":[",
            "{{\"id\":\"semaprax\",\"required\":true,\"status\":\"ok\",\"detail\":\"0.2.0\"}},",
            "{{\"id\":\"os\",\"required\":true,\"status\":\"ok\",\"detail\":\"linux\"}},",
            "{{\"id\":\"arch\",\"required\":true,\"status\":\"ok\",\"detail\":\"{}\"}},",
            "{{\"id\":\"release\",\"required\":true,\"status\":\"ok\",\"detail\":\"{}\"}},",
            "{{\"id\":\"profile\",\"required\":true,\"status\":\"ok\",\"detail\":\"offline profile `{}`; checks describe this profile only\"}},",
            "{{\"id\":\"clang\",\"required\":true,\"status\":\"ok\",\"detail\":\"/bin/clang (clang version 1.0.0)\"}}]}}\n"
        ), arch, release, SELECTOR)
    };
    assert!(
        observation.stdout == expected("debug").as_bytes()
            || observation.stdout == expected("release").as_bytes(),
        "{:?}",
        observation.stdout
    );
}

fn rejected(observation: Observation) {
    assert_eq!(observation.status.code(), Some(126));
    assert!(
        observation.stdout.is_empty(),
        "failure published report bytes"
    );
    assert!(observation.stderr.is_empty(), "{:?}", observation.stderr);
}

fn calibrated_reply() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let bundle = bundle();
    let request = request(&bundle);
    let reply = reply(&request);
    healthy(run(
        &request,
        &bundle,
        Some(&executable(&reply, Ending::Exit(0))),
    ));
    (request, bundle, reply)
}

#[test]
#[ignore = "requires provisioned native Linux namespaces, current-head binaries and cgroup lifecycle"]
fn actual_worker_materializes_executes_and_settles_before_canonical_report() {
    let bundle = bundle();
    healthy(run(&request(&bundle), &bundle, None));
}

#[test]
#[ignore = "requires provisioned collector context and executable sealed-memfd support"]
fn literal_reply_surrogates_reject_cross_binding_and_malformed_frames() {
    let (request, bundle, good) = calibrated_reply();
    let mut variants = Vec::new();
    for offset in [0, 8, 40, 72, 73, 74, 75, 76, 77, 78, 79] {
        let mut bytes = good.clone();
        bytes[offset] ^= 0x80;
        variants.push(bytes);
    }
    variants.push(good[..good.len() - 1].to_vec());
    let mut trailing = good.clone();
    trailing.push(0);
    variants.push(trailing);
    for bytes in variants {
        rejected(run(
            &request,
            &bundle,
            Some(&executable(&bytes, Ending::Exit(0))),
        ));
    }
    // Replay a fully well-formed reply bound to a different otherwise valid
    // request, not merely a flipped hash in this invocation's frame.
    let mut other_request = request.clone();
    other_request[12] ^= 1;
    rejected(run(
        &request,
        &bundle,
        Some(&executable(&reply(&other_request), Ending::Exit(0))),
    ));
    healthy(run(
        &request,
        &bundle,
        Some(&executable(&good, Ending::Exit(0))),
    ));
}

#[test]
#[ignore = "requires provisioned collector context and executable sealed-memfd support"]
fn complete_literal_frame_followed_by_nonzero_exit_never_becomes_a_report() {
    let (request, bundle, frame) = calibrated_reply();
    rejected(run(
        &request,
        &bundle,
        Some(&executable(&frame, Ending::Exit(7))),
    ));
    healthy(run(
        &request,
        &bundle,
        Some(&executable(&frame, Ending::Exit(0))),
    ));
}

#[test]
#[ignore = "physical deadline gate: requires provisioned context and takes two real 60s deadlines"]
fn complete_frame_and_capture_eof_each_still_require_worker_exit() {
    let (request, bundle, frame) = calibrated_reply();
    for ending in [Ending::Spin, Ending::CloseAndSpin] {
        let started = Instant::now();
        rejected(run(&request, &bundle, Some(&executable(&frame, ending))));
        assert!(
            started.elapsed() >= Duration::from_secs(59),
            "did not observe the actual deadline"
        );
    }
    // The writer's identical prefix is calibrated, but this fixture does not
    // claim a separately observed physical 'last byte written' milestone.
    healthy(run(
        &request,
        &bundle,
        Some(&executable(&frame, Ending::Exit(0))),
    ));
}
