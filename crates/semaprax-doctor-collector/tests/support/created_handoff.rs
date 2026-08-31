//! Production-created sealed files cross the actual worker/collector handoff.
//! The literal helpers stay independent; neither launch nor this fixture
//! reconstructs or reseals the production-created transport files.
use super::{fixture, observe, report};
use semaprax_native_rust_interop_platform_sys::{
    create_doctor_offline_input, encode_doctor_offline_bundle, DoctorOfflineArchitecture,
    DoctorOfflineBundle, DoctorOfflineBundleEntry, DoctorOfflineBundleRoles, DoctorOfflineInput,
    DoctorOfflineTarget,
};

#[test]
#[ignore = "requires provisioned current-head worker/collector and strict non-executable sealed-memfd creation"]
fn production_created_native_and_all_files_reach_worker_and_reject_digest_drift() {
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
        let literal_bundle = if all {
            fixture::all_bundle(fixture::Ending::Exit(0))
        } else {
            fixture::bundle()
        };
        let encoded = encode_doctor_offline_bundle(
            architecture,
            fixture::SELECTOR,
            &entries[..if all { 3 } else { 1 }],
            DoctorOfflineBundleRoles {
                clang: Some(0),
                node: all.then_some(1),
                rustc: all.then_some(2),
            },
            literal_bundle.len(),
        )
        .unwrap();
        assert_eq!(encoded, literal_bundle);

        // No capability fallback: an unavailable strict creation prerequisite
        // fails this selected physical gate. The returned File is the one sent
        // to both processes; the returned snapshot supplies request derivation.
        let (bundle_file, snapshot) = create_doctor_offline_input(&encoded, encoded.len()).unwrap();
        assert_eq!(snapshot.bytes(), literal_bundle);
        assert_eq!(
            DoctorOfflineInput::acquire(&bundle_file, encoded.len())
                .unwrap()
                .bytes(),
            literal_bundle
        );
        let bundle = DoctorOfflineBundle::parse(snapshot, fixture::SELECTOR).unwrap();
        let (target, tag, name) = if all {
            (DoctorOfflineTarget::All, 3, "all")
        } else {
            (DoctorOfflineTarget::Native, 1, "native")
        };
        let request = bundle.encode_worker_request(target, [0x37; 32]).unwrap();
        let literal_request = fixture::request_target(&literal_bundle, tag);
        assert_eq!(request, literal_request);
        let (request_file, request_snapshot) =
            create_doctor_offline_input(&request, request.len()).unwrap();
        assert_eq!(request_snapshot.bytes(), literal_request);
        assert_eq!(
            DoctorOfflineInput::acquire(&request_file, request.len())
                .unwrap()
                .bytes(),
            literal_request
        );
        drop(request_snapshot);
        drop(bundle);
        let tools = [
            ("clang", "ok", "/bin/clang (clang version 1.0.0)"),
            ("node", "ok", "v22.0.0"),
            ("rust", "ok", "rustc 1.88.0"),
        ];
        let tools = &tools[..if all { 3 } else { 1 }];
        report::require(
            observe::run_prepared(&request_file, &bundle_file),
            name,
            tools,
            0,
        );

        // Strict immutable creation does not authenticate a request's meaning.
        // Seal a digest mutation as-is, with no rehash or repair; the actual
        // worker/collector must reject it before any report is published.
        let mut wrong_digest = request.clone();
        wrong_digest[52] ^= 1;
        let (wrong_file, wrong_snapshot) =
            create_doctor_offline_input(&wrong_digest, wrong_digest.len()).unwrap();
        assert_eq!(wrong_snapshot.bytes(), wrong_digest);
        drop(wrong_snapshot);
        super::rejected(observe::run_prepared(&wrong_file, &bundle_file));
        report::require(
            observe::run_prepared(&request_file, &bundle_file),
            name,
            tools,
            0,
        );
        // Launch owns only duplicates. Original descriptors still authenticate
        // the exact literal bytes after successful and rejected handoffs.
        assert_eq!(
            DoctorOfflineInput::acquire(&bundle_file, encoded.len())
                .unwrap()
                .bytes(),
            literal_bundle
        );
        assert_eq!(
            DoctorOfflineInput::acquire(&request_file, request.len())
                .unwrap()
                .bytes(),
            literal_request
        );
        assert_eq!(
            DoctorOfflineInput::acquire(&wrong_file, wrong_digest.len())
                .unwrap()
                .bytes(),
            wrong_digest
        );
    }
}
