use semaprax::project::{
    derive_project_v8_promotion_receipt, parse_project_v8_promotion_receipt,
    replay_project_v8_promotion_receipt, ProjectV8PromotionArtifact,
    ProjectV8PromotionGateObservation, ProjectV8PromotionGateOutcome, ProjectV8PromotionSubject,
    MAX_PROJECT_V8_PROMOTION_RECEIPT_BYTES, PROJECT_V8_PROMOTION_RECEIPT_SCHEMA,
};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const GATES: [(&str, &str, &str); 15] = [
    (
        "manifest-descriptor-compatibility",
        "linux-x86_64",
        "rust-1.88",
    ),
    (
        "manifest-descriptor-compatibility",
        "macos-aarch64",
        "rust-1.88",
    ),
    (
        "manifest-descriptor-compatibility",
        "windows-x86_64",
        "rust-1.88",
    ),
    (
        "npm-installed-consumer",
        "linux-x86_64",
        "node-22-typescript-5.8.3",
    ),
    (
        "npm-installed-consumer",
        "macos-aarch64",
        "node-22-typescript-5.8.3",
    ),
    (
        "npm-installed-consumer",
        "windows-x86_64",
        "node-22-typescript-5.8.3",
    ),
    ("rust-sdk-consumer", "linux-x86_64", "rust-1.88"),
    ("rust-sdk-consumer", "macos-aarch64", "rust-1.88"),
    ("rust-sdk-consumer", "windows-x86_64", "rust-1.88"),
    (
        "backend-equivalence-o0-o2",
        "linux-x86_64",
        "clang-native-core-wasm",
    ),
    (
        "browser-consumer",
        "linux-x86_64",
        "playwright-1.55-chromium",
    ),
    (
        "browser-consumer",
        "linux-x86_64",
        "playwright-1.55-firefox",
    ),
    ("browser-consumer", "linux-x86_64", "playwright-1.55-webkit"),
    ("asan-ubsan", "linux-x86_64", "clang-address-undefined"),
    (
        "hostile-carrier-settlement",
        "linux-x86_64",
        "rust-1.88-clang",
    ),
];

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn subject(label: &str, first: char) -> ProjectV8PromotionSubject {
    let offset = first.to_digit(16).unwrap();
    let hex = |add| char::from_digit((offset + add) % 16, 16).unwrap();
    ProjectV8PromotionSubject {
        label: label.to_owned(),
        project_revision: digest(hex(0)),
        workspace_revision: digest(hex(1)),
        project_graph_digest: digest(hex(2)),
        descriptor_digest: digest(hex(3)),
        npm_carrier_digest: digest(hex(4)),
        rust_package_digest: digest(hex(5)),
        export_stable_ids: [
            "frame.payload",
            "frame.payload-maybe",
            "frame.payload-result",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    }
}

fn inputs() -> (
    ProjectV8PromotionSubject,
    ProjectV8PromotionSubject,
    Vec<ProjectV8PromotionArtifact>,
    Vec<ProjectV8PromotionGateObservation>,
) {
    let baseline = subject("baseline", '1');
    let renamed = subject("display-rename", '8');
    let artifacts = [
        ("baseline.descriptor", baseline.descriptor_digest.clone()),
        ("baseline.npm-carrier", baseline.npm_carrier_digest.clone()),
        (
            "baseline.rust-package",
            baseline.rust_package_digest.clone(),
        ),
        (
            "display-rename.descriptor",
            renamed.descriptor_digest.clone(),
        ),
        (
            "display-rename.npm-carrier",
            renamed.npm_carrier_digest.clone(),
        ),
        (
            "display-rename.rust-package",
            renamed.rust_package_digest.clone(),
        ),
        ("browser-toolchain-lock", digest('e')),
        ("compatibility-kat-inventory", digest('f')),
    ]
    .into_iter()
    .map(|(artifact_id, digest)| ProjectV8PromotionArtifact {
        artifact_id: artifact_id.to_owned(),
        digest,
    })
    .collect();
    let observations = GATES
        .into_iter()
        .map(
            |(gate_id, platform_id, tool_profile_id)| ProjectV8PromotionGateObservation {
                gate_id: gate_id.to_owned(),
                platform_id: platform_id.to_owned(),
                tool_profile_id: tool_profile_id.to_owned(),
                project_schema: "semaprax.project.v8".to_owned(),
                profile: "owned-data-api.v1".to_owned(),
                commit: COMMIT.to_owned(),
                outcome: ProjectV8PromotionGateOutcome::Passed,
            },
        )
        .collect();
    (baseline, renamed, artifacts, observations)
}

fn derive() -> semaprax::project::ProjectV8PromotionReceipt {
    let (baseline, renamed, artifacts, observations) = inputs();
    derive_project_v8_promotion_receipt(COMMIT, baseline, renamed, artifacts, observations).unwrap()
}

fn assert_invalid<T>(result: Result<T, Vec<semaprax::diagnostic::Diagnostic>>, message: &str) {
    let error = result.err().expect("hostile receipt was admitted");
    assert_eq!(error[0].code, "SPX-J119");
    assert_eq!(error[0].message, message);
}

#[test]
fn complete_observations_are_canonical_domain_separated_and_exactly_replayed() {
    let receipt = derive();
    assert!(receipt.canonical_json().starts_with(&format!(
        "{{\"schema\":\"{PROJECT_V8_PROMOTION_RECEIPT_SCHEMA}\",\"commit\":\"{COMMIT}\""
    )));
    assert!(receipt.canonical_json().ends_with("}\n"));
    assert_eq!(receipt.gates(), 15);
    assert_eq!(receipt.commit(), COMMIT);
    assert!(receipt.digest().starts_with("sha256:"));
    assert_ne!(receipt.digest(), digest('0'));
    assert_eq!(
        parse_project_v8_promotion_receipt(receipt.canonical_json()).unwrap(),
        receipt
    );
    let (baseline, renamed, artifacts, observations) = inputs();
    assert_eq!(
        replay_project_v8_promotion_receipt(
            receipt.canonical_json(),
            COMMIT,
            baseline,
            renamed,
            artifacts,
            observations,
        )
        .unwrap(),
        receipt
    );
    for forbidden in ["authority", "supported", "published", "hosted_success"] {
        assert!(
            !receipt
                .canonical_json()
                .contains(&format!("\"{forbidden}\":")),
            "receipt exposes forbidden claim field {forbidden}"
        );
    }
}

#[test]
fn gate_inventory_is_closed_ordered_and_pass_only() {
    for mutation in 0..7 {
        let (baseline, renamed, artifacts, mut observations) = inputs();
        match mutation {
            0 => {
                observations.pop();
            }
            1 => observations.swap(0, 1),
            2 => observations[0] = observations[1].clone(),
            3 => observations[0].outcome = ProjectV8PromotionGateOutcome::Skipped,
            4 => observations[0].outcome = ProjectV8PromotionGateOutcome::Masked,
            5 => observations[0].outcome = ProjectV8PromotionGateOutcome::Cancelled,
            6 => observations[0].outcome = ProjectV8PromotionGateOutcome::Failed,
            _ => unreachable!(),
        }
        assert_invalid(
            derive_project_v8_promotion_receipt(COMMIT, baseline, renamed, artifacts, observations),
            if mutation == 0 {
                "Project v8 promotion gate inventory is incomplete"
            } else {
                "Project v8 promotion gate observation is invalid"
            },
        );
    }
}

#[test]
fn foreign_heads_profiles_tools_and_subjects_fail_closed() {
    for mutation in 0..7 {
        let (mut baseline, renamed, artifacts, mut observations) = inputs();
        let mut commit = COMMIT.to_owned();
        match mutation {
            0 => commit = "A123456789abcdef0123456789abcdef01234567".to_owned(),
            1 => observations[0].commit = "1123456789abcdef0123456789abcdef01234567".to_owned(),
            2 => observations[0].project_schema = "semaprax.project.v9".to_owned(),
            3 => observations[0].profile = "flat-owned-record-api.v1".to_owned(),
            4 => observations[0].tool_profile_id = "rust-current".to_owned(),
            5 => baseline.export_stable_ids.swap(0, 1),
            6 => baseline.project_revision = renamed.project_revision.clone(),
            _ => unreachable!(),
        }
        assert!(derive_project_v8_promotion_receipt(
            &commit,
            baseline,
            renamed,
            artifacts,
            observations,
        )
        .is_err());
    }
}

#[test]
fn artifacts_are_exact_and_reminted_inputs_do_not_replay() {
    for mutation in 0..4 {
        let (baseline, renamed, mut artifacts, observations) = inputs();
        match mutation {
            0 => {
                artifacts.pop();
            }
            1 => artifacts.swap(0, 1),
            2 => artifacts[0] = artifacts[1].clone(),
            3 => artifacts[0].digest = digest('0'),
            _ => unreachable!(),
        }
        assert!(derive_project_v8_promotion_receipt(
            COMMIT,
            baseline,
            renamed,
            artifacts,
            observations,
        )
        .is_err());
    }

    let receipt = derive();
    let (baseline, renamed, mut artifacts, observations) = inputs();
    artifacts[6].digest = digest('0');
    assert_invalid(
        replay_project_v8_promotion_receipt(
            receipt.canonical_json(),
            COMMIT,
            baseline,
            renamed,
            artifacts,
            observations,
        ),
        "Project v8 promotion receipt disagrees with explicit gate observations",
    );
}

#[test]
fn parser_rejects_noncanonical_surplus_duplicate_deep_and_oversized_bytes() {
    let receipt = derive();
    let canonical = receipt.canonical_json();
    let prefix =
        format!("{{\"schema\":\"{PROJECT_V8_PROMOTION_RECEIPT_SCHEMA}\",\"commit\":\"{COMMIT}\"");
    let reordered_prefix =
        format!("{{\"commit\":\"{COMMIT}\",\"schema\":\"{PROJECT_V8_PROMOTION_RECEIPT_SCHEMA}\"");
    let reordered = canonical.replacen(&prefix, &reordered_prefix, 1);
    assert!(parse_project_v8_promotion_receipt(&reordered).is_err());
    let surplus = canonical.replacen("{\"schema\":", "{\"surplus\":false,\"schema\":", 1);
    assert!(parse_project_v8_promotion_receipt(&surplus).is_err());
    let duplicate = canonical.replacen(
        "{\"schema\":",
        &format!("{{\"schema\":\"{PROJECT_V8_PROMOTION_RECEIPT_SCHEMA}\",\"schema\":"),
        1,
    );
    assert!(parse_project_v8_promotion_receipt(&duplicate).is_err());
    let deep = format!("{}0{}\n", "[".repeat(129), "]".repeat(129));
    assert!(parse_project_v8_promotion_receipt(&deep).is_err());
    let oversized = format!("{}\n", " ".repeat(MAX_PROJECT_V8_PROMOTION_RECEIPT_BYTES));
    assert!(parse_project_v8_promotion_receipt(&oversized).is_err());
    assert!(parse_project_v8_promotion_receipt(canonical.trim_end()).is_err());
}
