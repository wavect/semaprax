use super::*;
use ed25519_dalek::{Signer, SigningKey};
use std::sync::OnceLock;

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub(super) fn host_v3_migration_fixture() -> (
    String,
    String,
    String,
    [String; 2],
    String,
    ManifestBoundEntry,
) {
    let mut root = root_value(1, 1, 4, 2);
    root["roles"][3]["name"] = json!("publisher-app");
    for seed in [6, 7] {
        let key = key(seed);
        root["keys"]
            .as_array_mut()
            .unwrap()
            .push(json!({"keyid":keyid(&key),"public":encoded(&key.verifying_key().to_bytes())}));
    }
    root["roles"].as_array_mut().unwrap().push(json!({"name":"publisher-lib","namespace":"lib.","keyids":[keyid(&key(6)),keyid(&key(7))],"threshold":2}));
    let root_bytes = wire(&root);
    let root = InstalledRoot::from_independently_installed_bytes(&root_bytes).unwrap();
    let entry = admitted().0.clone();
    let registry = super::super::registry_v2::build_snapshot(std::slice::from_ref(&entry))
        .unwrap()
        .envelope()
        .to_owned();
    let publishers = [
        metadata(
            &root,
            "publisher-app",
            1,
            500,
            json!({"targets":[{"package":entry.publication().package,"version":entry.publication().version,"manifest":blob(entry.artifact_manifest_bytes().as_bytes())}]}),
            &[key(4), key(5)],
        ),
        metadata(
            &root,
            "publisher-lib",
            1,
            500,
            json!({"targets":[]}),
            &[key(6), key(7)],
        ),
    ];
    let snapshot = metadata(
        &root,
        "snapshot",
        1,
        400,
        json!({"registry":blob(registry.as_bytes()),"publishers":[
        {"role":"publisher-app","metadata":metadata_ref(&publishers[0])},{"role":"publisher-lib","metadata":metadata_ref(&publishers[1])}]}),
        &[key(3)],
    );
    let timestamp = metadata(
        &root,
        "timestamp",
        1,
        300,
        json!({"snapshot":metadata_ref(&snapshot)}),
        &[key(2)],
    );
    (root_bytes, timestamp, snapshot, publishers, registry, entry)
}

// Test-only signed bytes and independently replayed admission for the physical
// host regressions. No signing constructor is exposed by production modules.
#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub(super) fn host_fixture(
    yanked: bool,
) -> (String, String, String, String, String, ManifestBoundEntry) {
    let fixture = if yanked {
        Fixture::with_entry(
            super::super::registry_v2::tests::real_admitted_fixture_with_status(
                super::super::PublicationStatus::Yanked {
                    reason: "host regression yank".to_owned(),
                },
            )
            .0,
        )
    } else {
        Fixture::new()
    };
    (
        wire(&root_value(1, 1, 4, 2)),
        fixture.timestamp,
        fixture.snapshot,
        fixture.publisher,
        fixture.registry,
        fixture.entry,
    )
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub(super) fn host_artifact_fixture() -> (
    String,
    String,
    String,
    String,
    String,
    ManifestBoundEntry,
    Vec<u8>,
) {
    let fixture = Fixture::new();
    (
        wire(&root_value(1, 1, 4, 2)),
        fixture.timestamp,
        fixture.snapshot,
        fixture.publisher,
        fixture.registry,
        fixture.entry,
        admitted().1.module_wasm.clone(),
    )
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub(super) fn host_rotation_fixture() -> (String, String, String, String, String, ManifestBoundEntry)
{
    let mut fixture = Fixture::new();
    let next = root_value(2, 10, 20, 2);
    let rotation = sign(
        next.clone(),
        &[key(1), key(10)],
        b"semaprax.registry-trust-root.v1\0",
    );
    fixture.root = InstalledRoot::from_independently_installed_bytes(&wire(&next)).unwrap();
    let mut publisher: Value = serde_json::from_str(&fixture.publisher).unwrap();
    publisher["signed"]["root_digest"] = json!(fixture.root.digest);
    publisher["signed"]["version"] = json!(2);
    fixture.publisher = sign(
        publisher["signed"].clone(),
        &[key(20), key(21)],
        SIGN_DOMAIN,
    );
    fixture.refresh(2, 2);
    (
        rotation,
        fixture.timestamp,
        fixture.snapshot,
        fixture.publisher,
        fixture.registry,
        fixture.entry,
    )
}

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}
fn encoded(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn keyid(key: &SigningKey) -> String {
    hash(&key.verifying_key().to_bytes())
}
fn blob(bytes: &[u8]) -> Value {
    json!({"length":bytes.len(), "sha256":hash(bytes)})
}
fn metadata_ref(bytes: &str) -> Value {
    let value: Value = serde_json::from_str(bytes).unwrap();
    json!({"version":value["signed"]["version"], "file":blob(bytes.as_bytes())})
}
fn sign(signed: Value, keys: &[SigningKey], domain: &[u8]) -> String {
    let mut message = domain.to_vec();
    message.extend_from_slice(wire(&signed).as_bytes());
    let signatures = keys
        .iter()
        .map(|key| json!({"keyid":keyid(key),"sig":encoded(&key.sign(&message).to_bytes())}))
        .collect::<Vec<_>>();
    wire(&json!({"signed":signed,"signatures":signatures}))
}
fn root_value(version: u64, root_seed: u8, publisher_seed: u8, threshold: usize) -> Value {
    let roles = [
        ("root", "", vec![key(root_seed)]),
        ("timestamp", "", vec![key(2)]),
        ("snapshot", "", vec![key(3)]),
        (
            "publisher",
            "app.",
            vec![key(publisher_seed), key(publisher_seed + 1)],
        ),
    ];
    let keys = roles
        .iter()
        .flat_map(|(_, _, keys)| keys.iter())
        .map(|key| json!({"keyid":keyid(key),"public":encoded(&key.verifying_key().to_bytes())}))
        .collect::<Vec<_>>();
    let roles = roles.iter().map(|(name, namespace, keys)| json!({"name":name,"namespace":namespace,"keyids":keys.iter().map(keyid).collect::<Vec<_>>(),"threshold":if *name == "publisher" {threshold} else {1}})).collect::<Vec<_>>();
    json!({"schema":ROOT_SCHEMA,"registry":"test.registry","version":version,"expires":1000,"keys":keys,"roles":roles})
}
fn metadata(
    root: &InstalledRoot,
    role: &str,
    version: u64,
    expires: u64,
    payload: Value,
    keys: &[SigningKey],
) -> String {
    sign(
        json!({"schema":METADATA_SCHEMA,"registry":root.registry,"root_digest":root.digest,"role":role,"version":version,"expires":expires,"payload":payload}),
        keys,
        SIGN_DOMAIN,
    )
}
fn admitted() -> &'static (
    ManifestBoundEntry,
    crate::package_build_v2::LinkedOfflinePackageBuild,
) {
    static FIXTURE: OnceLock<(
        ManifestBoundEntry,
        crate::package_build_v2::LinkedOfflinePackageBuild,
    )> = OnceLock::new();
    FIXTURE.get_or_init(super::super::registry_v2::tests::real_admitted_fixture)
}
struct Fixture {
    root: InstalledRoot,
    entry: ManifestBoundEntry,
    registry: String,
    publisher: String,
    snapshot: String,
    timestamp: String,
}
impl Fixture {
    fn new() -> Self {
        Self::with_entry(admitted().0.clone())
    }
    fn with_entry(entry: ManifestBoundEntry) -> Self {
        let root =
            InstalledRoot::from_independently_installed_bytes(&wire(&root_value(1, 1, 4, 2)))
                .unwrap();
        let registry = super::super::registry_v2::build_snapshot(std::slice::from_ref(&entry))
            .unwrap()
            .envelope()
            .to_owned();
        let publisher = metadata(
            &root,
            "publisher",
            1,
            500,
            json!({"targets":[{"package":entry.publication().package,"version":entry.publication().version,"manifest":blob(entry.artifact_manifest_bytes().as_bytes())}]}),
            &[key(4), key(5)],
        );
        let mut fixture = Self {
            root,
            entry,
            registry,
            publisher,
            snapshot: String::new(),
            timestamp: String::new(),
        };
        fixture.refresh(1, 1);
        fixture
    }
    fn refresh(&mut self, snapshot_version: u64, timestamp_version: u64) {
        self.snapshot = metadata(
            &self.root,
            "snapshot",
            snapshot_version,
            400,
            json!({"registry":blob(self.registry.as_bytes()),"publishers":[{"role":"publisher","metadata":metadata_ref(&self.publisher)}]}),
            &[key(3)],
        );
        self.timestamp = metadata(
            &self.root,
            "timestamp",
            timestamp_version,
            300,
            json!({"snapshot":metadata_ref(&self.snapshot)}),
            &[key(2)],
        );
    }
    fn verify(&self, checkpoint: &Checkpoint, now: u64) -> Result<RegistryUpdateCandidate> {
        verify_update(
            &self.root,
            checkpoint,
            now,
            &UpdateInputs {
                timestamp: &self.timestamp,
                snapshot: &self.snapshot,
                publishers: &[("publisher", &self.publisher)],
                registry_snapshot: &self.registry,
                admitted_entries: std::slice::from_ref(&self.entry),
            },
        )
    }
    fn initial(&self) -> Checkpoint {
        Checkpoint::initial(&self.root)
    }
    fn publisher_edit(&mut self, edit: impl FnOnce(&mut Value), keys: &[SigningKey]) {
        let mut value: Value = serde_json::from_str(&self.publisher).unwrap();
        edit(&mut value["signed"]);
        self.publisher = sign(value["signed"].clone(), keys, SIGN_DOMAIN);
        self.refresh(1, 1);
    }
}
fn refused<T>(result: Result<T>, code: &str) {
    assert_eq!(result.err().expect("must refuse").code, code);
}

#[test]
fn real_admission_signed_chain_artifact_and_checkpoint_roundtrip() {
    let fixture = Fixture::new();
    let initial = fixture.initial();
    let candidate = fixture.verify(&initial, 100).unwrap();
    assert_eq!(
        candidate.prior_checkpoint_digest(),
        hash(initial.canonical_bytes().as_bytes())
    );
    candidate
        .check_artifact(
            "app.main",
            "1.0.0",
            "module.wasm",
            &admitted().1.module_wasm,
        )
        .unwrap();
    refused(
        candidate.check_artifact("app.main", "1.0.0", "module.wasm", b"tampered"),
        "SPX-PKR624",
    );
    refused(
        candidate.check_artifact(
            "app.main",
            "1.0.0",
            "../module.wasm",
            &admitted().1.module_wasm,
        ),
        "SPX-PKR624",
    );
    let stored_bytes = candidate.checkpoint().canonical_bytes();
    let restored = Checkpoint::from_trusted_store_bytes(&stored_bytes).unwrap();
    let next = fixture.verify(&restored, 100).unwrap();
    assert_eq!(
        next.prior_checkpoint_digest(),
        hash(stored_bytes.as_bytes())
    );
    let mut unsorted: Value = serde_json::from_str(&stored_bytes).unwrap();
    unsorted["roles"].as_array_mut().unwrap().reverse();
    assert_ne!(wire(&unsorted), stored_bytes);
    refused(
        Checkpoint::from_trusted_store_bytes(&wire(&unsorted)),
        "SPX-PKR621",
    );
    assert_eq!(
        candidate.checkpoint().canonical_bytes(),
        next.checkpoint().canonical_bytes()
    );
    assert_eq!(
        candidate.registry_snapshot_digest(),
        super::super::registry_v2::decode_snapshot(&fixture.registry)
            .unwrap()
            .digest()
    );
}

#[test]
fn threshold_duplicate_unknown_wrong_and_tampered_signatures_refuse() {
    for keys in [
        vec![key(4)],
        vec![key(4), key(4)],
        vec![key(4), key(6)],
        vec![key(2), key(3)],
    ] {
        let mut fixture = Fixture::new();
        fixture.publisher_edit(|_| {}, &keys);
        refused(fixture.verify(&fixture.initial(), 100), "SPX-PKR622");
    }
    let mut fixture = Fixture::new();
    let mut value: Value = serde_json::from_str(&fixture.publisher).unwrap();
    value["signed"]["version"] = json!(2);
    fixture.publisher = wire(&value);
    fixture.refresh(1, 1);
    refused(fixture.verify(&fixture.initial(), 100), "SPX-PKR622");
}

#[test]
fn namespace_and_exact_manifest_inventory_cannot_be_substituted() {
    let mut fixture = Fixture::new();
    fixture.publisher_edit(
        |v| v["payload"]["targets"][0]["package"] = json!("other.main"),
        &[key(4), key(5)],
    );
    refused(fixture.verify(&fixture.initial(), 100), "SPX-PKR622");
    for mutate in 0..3 {
        let mut fixture = Fixture::new();
        fixture.publisher_edit(
            |v| match mutate {
                0 => v["payload"]["targets"][0]["manifest"]["sha256"] = json!(hash(b"foreign")),
                1 => v["payload"]["targets"][0]["version"] = json!("2.0.0"),
                _ => v["payload"]["targets"] = json!([]),
            },
            &[key(4), key(5)],
        );
        refused(fixture.verify(&fixture.initial(), 100), "SPX-PKR624");
    }
}

#[test]
fn timestamp_snapshot_and_publisher_expiry_and_time_rollback_fail_closed() {
    let fixture = Fixture::new();
    refused(fixture.verify(&fixture.initial(), 300), "SPX-PKR623");
    let candidate = fixture.verify(&fixture.initial(), 100).unwrap();
    refused(fixture.verify(candidate.checkpoint(), 99), "SPX-PKR623");
    for role in ["timestamp", "snapshot", "publisher"] {
        let mut fixture = Fixture::new();
        let (bytes, keys) = match role {
            "timestamp" => (&mut fixture.timestamp, vec![key(2)]),
            "snapshot" => (&mut fixture.snapshot, vec![key(3)]),
            _ => (&mut fixture.publisher, vec![key(4), key(5)]),
        };
        let mut value: Value = serde_json::from_str(bytes).unwrap();
        value["signed"]["expires"] = json!(100);
        *bytes = sign(value["signed"].clone(), &keys, SIGN_DOMAIN);
        if role == "publisher" {
            fixture.refresh(1, 1);
        }
        if role == "snapshot" {
            fixture.timestamp = metadata(
                &fixture.root,
                "timestamp",
                1,
                300,
                json!({"snapshot":metadata_ref(&fixture.snapshot)}),
                &[key(2)],
            );
        }
        refused(fixture.verify(&fixture.initial(), 100), "SPX-PKR623");
    }
}

#[test]
fn rollback_same_version_equivocation_and_mix_and_match_refuse() {
    let mut fixture = Fixture::new();
    fixture.publisher_edit(|v| v["version"] = json!(2), &[key(4), key(5)]);
    fixture.refresh(2, 2);
    let candidate = fixture.verify(&fixture.initial(), 100).unwrap();
    let prior = Fixture::new();
    refused(prior.verify(candidate.checkpoint(), 100), "SPX-PKR623");
    fixture.publisher_edit(|v| v["expires"] = json!(501), &[key(4), key(5)]);
    fixture.refresh(3, 3);
    refused(fixture.verify(candidate.checkpoint(), 100), "SPX-PKR623");
    let mut spliced = Fixture::new();
    spliced.publisher_edit(|v| v["version"] = json!(2), &[key(4), key(5)]);
    spliced.timestamp = prior.timestamp;
    refused(spliced.verify(&spliced.initial(), 100), "SPX-PKR624");
}

#[test]
fn root_rotation_requires_old_and_new_thresholds_and_revokes_old_publisher() {
    let fixture = Fixture::new();
    let next = root_value(2, 10, 20, 2);
    let domain = b"semaprax.registry-trust-root.v1\0";
    for keys in [vec![key(1)], vec![key(10)], vec![key(2), key(10)]] {
        refused(
            verify_root_rotation(&fixture.root, &sign(next.clone(), &keys, domain), 100),
            "SPX-PKR622",
        );
    }
    let rotated = verify_root_rotation(
        &fixture.root,
        &sign(next.clone(), &[key(1), key(10)], domain),
        100,
    )
    .unwrap();
    assert_eq!(rotated.previous_root_digest(), fixture.root.digest());
    refused(
        verify_root_rotation(
            &fixture.root,
            &sign(root_value(3, 10, 20, 2), &[key(1), key(10)], domain),
            100,
        ),
        "SPX-PKR623",
    );
    let mut replacement = fixture;
    replacement.root = rotated.root;
    // Rebind all metadata to the new anchor; only the revoked publisher keys
    // remain old, so refusal cannot be attributed to a cross-root digest.
    let mut publisher: Value = serde_json::from_str(&replacement.publisher).unwrap();
    publisher["signed"]["root_digest"] = json!(replacement.root.digest());
    replacement.publisher = sign(publisher["signed"].clone(), &[key(4), key(5)], SIGN_DOMAIN);
    replacement.refresh(2, 2);
    refused(
        replacement.verify(&replacement.initial(), 100),
        "SPX-PKR622",
    );
}

#[test]
fn root_policy_rejects_reused_keys_overlapping_namespaces_and_bad_threshold() {
    for mutation in 0..4 {
        let mut root = root_value(1, 1, 4, 2);
        match mutation {
            0 => root["roles"][1]["keyids"] = root["roles"][0]["keyids"].clone(),
            1 => root["roles"][3]["namespace"] = json!("std."),
            2 => root["roles"][3]["threshold"] = json!(3),
            _ => {
                root["keys"].as_array_mut().unwrap().push(json!({"keyid":keyid(&key(6)),"public":encoded(&key(6).verifying_key().to_bytes())}));
                root["roles"].as_array_mut().unwrap().push(json!({"name":"other","namespace":"app.deep.","keyids":[keyid(&key(6))],"threshold":1}));
            }
        }
        refused(
            InstalledRoot::from_independently_installed_bytes(&wire(&root)),
            "SPX-PKR621",
        );
    }
}

#[test]
fn decoded_registry_without_independent_admission_and_noncanonical_wire_refuse() {
    let fixture = Fixture::new();
    refused(
        verify_update(
            &fixture.root,
            &fixture.initial(),
            100,
            &UpdateInputs {
                timestamp: &fixture.timestamp,
                snapshot: &fixture.snapshot,
                publishers: &[("publisher", &fixture.publisher)],
                registry_snapshot: &fixture.registry,
                admitted_entries: &[],
            },
        ),
        "SPX-PKR620",
    );
    refused(
        InstalledRoot::from_independently_installed_bytes(&format!(
            " {}",
            wire(&root_value(1, 1, 4, 2))
        )),
        "SPX-PKR621",
    );
    refused(
        InstalledRoot::from_independently_installed_bytes(&"x".repeat(MAX_BYTES + 1)),
        "SPX-PKR621",
    );
}

#[test]
fn authenticated_yank_blocks_artifact_and_subject_and_old_snapshot_cannot_resurrect() {
    let active = Fixture::new();
    let prior = active.verify(&active.initial(), 100).unwrap();
    let (entry, _) = super::super::registry_v2::tests::real_admitted_fixture_with_status(
        super::super::PublicationStatus::Yanked {
            reason: "withdrawn".to_owned(),
        },
    );
    let mut yanked = Fixture::with_entry(entry);
    yanked.refresh(2, 2);
    let candidate = yanked.verify(prior.checkpoint(), 101).unwrap();
    refused(
        candidate.check_artifact(
            "app.main",
            "1.0.0",
            "module.wasm",
            &admitted().1.module_wasm,
        ),
        "SPX-PKR625",
    );
    refused(
        candidate.check_lock(
            "unused: subject refusal precedes lock replay",
            &[yanked.entry.publication().subject_bytes.clone()],
        ),
        "SPX-PKR625",
    );
    refused(active.verify(candidate.checkpoint(), 102), "SPX-PKR623");
}
