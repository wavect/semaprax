//! Production preparation meets independent literal wire and physical handoff.
//! Encoding is only bytes: admission still requires actual sealed acquisition,
//! native parsing, and the separately provisioned worker/collector lifecycle.
use super::{fixture, launch, observe, report};
use semaprax_native_rust_interop_platform_sys::{
    encode_doctor_offline_bundle, DoctorOfflineArchitecture, DoctorOfflineBundle,
    DoctorOfflineBundleEntry, DoctorOfflineBundleError, DoctorOfflineBundleRoles,
    DoctorOfflineInput, DoctorOfflineTarget,
};

#[test]
#[ignore = "requires provisioned current-head worker/collector and real sealed native input handoff"]
fn prepared_native_and_all_role_handoffs_preserve_literal_wire_and_reject_transport_drift() {
    assert_eq!(
        std::env::var("SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT").as_deref(),
        Ok("private-mapped-user-mount-clean-worker-cgroup-v1")
    );
    let architecture = if fixture::architecture() == 1 {
        DoctorOfflineArchitecture::LinuxX86_64
    } else {
        DoctorOfflineArchitecture::LinuxAarch64
    };
    let images = [
        fixture::executable(fixture::VERSION, fixture::Ending::Exit(0)),
        fixture::executable(b"v22.0.0\n", fixture::Ending::Exit(0)),
        fixture::executable(b"rustc 1.88.0\n", fixture::Ending::Exit(0)),
    ];
    let entries = [
        DoctorOfflineBundleEntry {
            path: "bin/clang",
            bytes: &images[0],
            executable: true,
        },
        DoctorOfflineBundleEntry {
            path: "bin/node",
            bytes: &images[1],
            executable: true,
        },
        DoctorOfflineBundleEntry {
            path: "bin/rustc",
            bytes: &images[2],
            executable: true,
        },
    ];
    for all in [false, true] {
        // These existing literal helpers remain independent of both production
        // encoders. Never rewrite them to call the code under test.
        let literal = if all {
            fixture::all_bundle(fixture::Ending::Exit(0))
        } else {
            fixture::bundle()
        };
        let roles = DoctorOfflineBundleRoles {
            clang: Some(0),
            node: all.then_some(1),
            rustc: all.then_some(2),
        };
        let selected = &entries[..if all { 3 } else { 1 }];
        assert_eq!(
            encode_doctor_offline_bundle(
                architecture,
                fixture::SELECTOR,
                selected,
                roles,
                literal.len() - 1
            )
            .err(),
            Some(DoctorOfflineBundleError::Limit)
        );
        let encoded = encode_doctor_offline_bundle(
            architecture,
            fixture::SELECTOR,
            selected,
            roles,
            literal.len(),
        )
        .unwrap();
        assert_eq!(encoded, literal);
        let file = launch::sealed(&encoded, false);
        let snapshot = DoctorOfflineInput::acquire(&file, encoded.len()).unwrap();
        let bundle = DoctorOfflineBundle::parse(snapshot, fixture::SELECTOR).unwrap();
        drop(file); // Request encoding uses retained admitted bytes, not a path.
        let (target, target_byte, target_name) = if all {
            (DoctorOfflineTarget::All, 3, "all")
        } else {
            (DoctorOfflineTarget::Native, 1, "native")
        };
        assert_eq!(
            bundle.encode_worker_request(target, [0; 32]).err(),
            Some(DoctorOfflineBundleError::Invalid)
        );
        if !all {
            assert_eq!(
                bundle
                    .encode_worker_request(DoctorOfflineTarget::All, [0x37; 32])
                    .err(),
                Some(DoctorOfflineBundleError::Invalid)
            );
        }
        let request = bundle.encode_worker_request(target, [0x37; 32]).unwrap();
        // Full framing/nonce/role equality and hashing of independently literal
        // bundle bytes. This is not a self round-trip through the same encoder.
        assert_eq!(request, fixture::request_target(&literal, target_byte));
        let tools = [
            ("clang", "ok", "/bin/clang (clang version 1.0.0)"),
            ("node", "ok", "v22.0.0"),
            ("rust", "ok", "rustc 1.88.0"),
        ];
        let tools = &tools[..if all { 3 } else { 1 }];
        report::require(
            observe::run(&request, &encoded, None),
            target_name,
            tools,
            0,
        );

        // Prepared output cannot bypass storage or live re-admission: drifted
        // transport bytes are sealed as-is, not repaired or rehashed for use.
        let mut wrong_digest = request.clone();
        wrong_digest[52] ^= 1;
        super::rejected(observe::run(&wrong_digest, &encoded, None));
        let mut drifted_bundle = encoded.clone();
        *drifted_bundle.last_mut().unwrap() ^= 1;
        super::rejected(observe::run(&request, &drifted_bundle, None));
        report::require(
            observe::run(&request, &encoded, None),
            target_name,
            tools,
            0,
        );
    }
}
