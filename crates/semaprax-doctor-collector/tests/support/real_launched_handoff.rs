//! Opt-in real distributions through the actual production launcher and both
//! downstream processes. The trusted provisioner supplies the closed bundle,
//! immutable startup images/loader closure and independent expected details.
//! No observed version/report is used to manufacture its own passing oracle.
use super::{launch, launched_handoff, report};
use semaprax_native_rust_interop_platform_sys::{
    create_doctor_offline_input, DoctorOfflineBundle, DoctorOfflineInput, DoctorOfflineTarget,
};
use std::fs::File;
use std::io::Read;

const BUNDLE_LIMIT: usize = 512 * 1024 * 1024;
const DETAIL_LIMIT: usize = 8192;

fn expected_detail(variable: &str) -> String {
    let detail = std::env::var(variable).expect(variable);
    assert!(
        !detail.is_empty() && detail.len() <= DETAIL_LIMIT,
        "{variable} must contain 1..={DETAIL_LIMIT} UTF-8 bytes"
    );
    assert_eq!(
        detail.trim(),
        detail,
        "{variable} must already be normalized"
    );
    assert!(
        !detail.chars().any(char::is_control),
        "{variable} must contain no control characters"
    );
    detail
}

fn supplied_bundle() -> Vec<u8> {
    let path = launch::provisioned_path("SEMAPRAX_DOCTOR_REAL_BUNDLE");
    let file = File::open(path).unwrap();
    let metadata = file.metadata().unwrap();
    assert!(metadata.is_file());
    assert!(metadata.len() > 0 && metadata.len() <= BUNDLE_LIMIT as u64);
    let mut bytes = Vec::new();
    file.take(BUNDLE_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len() as u64, metadata.len());
    assert!(!bytes.is_empty() && bytes.len() <= BUNDLE_LIMIT);
    // Quiescence and real-distribution provenance are provisioner facts. This
    // bounded read neither authenticates provenance nor assembles loader files.
    bytes
}

#[test]
#[ignore = "requires real all-role bundle, independent expected details and fully provisioned current-head launcher/worker/collector"]
fn production_launcher_reports_all_roles_from_provisioned_real_distributions() {
    launched_handoff::context();
    let selector = std::env::var("SEMAPRAX_DOCTOR_REAL_SELECTOR").expect("provision real selector");
    assert!(!selector.is_empty() && selector.len() <= 64 && selector.is_ascii());
    let clang = expected_detail("SEMAPRAX_DOCTOR_EXPECTED_CLANG_DETAIL");
    let node = expected_detail("SEMAPRAX_DOCTOR_EXPECTED_NODE_DETAIL");
    let rust = expected_detail("SEMAPRAX_DOCTOR_EXPECTED_RUST_DETAIL");
    // Clang's supplied detail includes the exact absolute in-root path and
    // parenthesized first version line. Node/Rust supply their normalized first
    // lines. All must satisfy current production version policy; no downgrades.
    let tools = [
        ("clang", "ok", clang.as_str()),
        ("node", "ok", node.as_str()),
        ("rust", "ok", rust.as_str()),
    ];
    let bytes = supplied_bundle();
    let (bundle_file, snapshot) = create_doctor_offline_input(&bytes, bytes.len()).unwrap();
    assert_eq!(snapshot.bytes(), bytes);
    // This sole existing parser enforces the selector grammar, native host,
    // closed inventory and ELF contract. Encoding All requires all three roles.
    let bundle = DoctorOfflineBundle::parse(snapshot, &selector).unwrap();
    let request = bundle
        .encode_worker_request(DoctorOfflineTarget::All, [0x37; 32])
        .unwrap();
    drop(bundle);
    let (request_file, snapshot) = create_doctor_offline_input(&request, request.len()).unwrap();
    assert_eq!(snapshot.bytes(), request);
    drop(snapshot);
    // Fixed test nonce binds this invocation's bytes, not freshness/provenance.
    // Production-created executable files remain the actual transferred images.
    let worker = launched_handoff::prepared_executable(&launched_handoff::installed_image(
        "SEMAPRAX_DOCTOR_WORKER",
    ));
    let collector = launched_handoff::prepared_executable(&launched_handoff::installed_image(
        "SEMAPRAX_DOCTOR_COLLECTOR",
    ));
    report::require_for_selector(
        launched_handoff::run(&request_file, &bundle_file, &worker, &collector),
        &selector,
        "all",
        &tools,
        0,
    );
    // The launcher receives duplicates, not ownership of these originals.
    // Require exact retained transport bytes after the complete live handoff.
    assert_eq!(
        DoctorOfflineInput::acquire(&bundle_file, bytes.len())
            .unwrap()
            .bytes(),
        bytes
    );
    assert_eq!(
        DoctorOfflineInput::acquire(&request_file, request.len())
            .unwrap()
            .bytes(),
        request
    );
}
