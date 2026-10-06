use super::*;
use crate::package_registry::mirror_transport::{
    MirrorBytes, MirrorError, MirrorGet, MirrorNetworkAuthority, MirrorObject, MirrorObjectKind,
    MirrorOrigin, MirrorResponse, MirrorTransport,
};
use ed25519_dalek::{Signer, SigningKey};
use std::collections::VecDeque;
use std::sync::OnceLock;
use std::time::Duration;

pub(in crate::package_registry::trust) type Admission = (
    registry::RegistrySnapshotV3,
    Vec<String>,
    String,
    Vec<u8>,
    Vec<u8>,
);

// Test-only producer replay and signing. No production constructor is exposed.
#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub(in crate::package_registry::trust) fn host_fixture(
    yanked: bool,
    version: u64,
) -> (String, String, String, [String; 2], &'static Admission) {
    let mut f = Fixture::new(yanked);
    f.publish(version, 4);
    f.refresh(version, version);
    (
        wire(&root_value(1, 1, 4)),
        f.timestamp,
        f.snapshot,
        f.publishers,
        f.admitted,
    )
}
/// Long-lived test-only signing fixture for offline-age controls. The caller
/// chooses an expiry beyond the seven-day boundary so a refusal pins the
/// bridge's local policy rather than ordinary metadata expiry.
#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub(in crate::package_registry::trust) fn host_fixture_until(
    yanked: bool,
    version: u64,
    expires: u64,
) -> (String, String, String, [String; 2], &'static Admission) {
    let mut f = Fixture::new_until(yanked, expires);
    f.publish(version, 4);
    f.refresh(version, version);
    (
        wire(&root_value_until(1, 1, 4, expires)),
        f.timestamp,
        f.snapshot,
        f.publishers,
        f.admitted,
    )
}
#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub(in crate::package_registry::trust) fn host_rotation_fixture(
) -> (String, String, String, [String; 2], &'static Admission) {
    let mut f = Fixture::new(false);
    let root = root_value(2, 10, 20);
    let rotation = sign(root.clone(), &[1, 10], b"semaprax.registry-trust-root.v1\0");
    f.root = InstalledRoot::from_independently_installed_bytes(&wire(&root)).unwrap();
    f.publish(2, 20);
    f.refresh(2, 2);
    (rotation, f.timestamp, f.snapshot, f.publishers, f.admitted)
}
fn admission(yanked: bool) -> &'static Admission {
    static ACTIVE: OnceLock<Admission> = OnceLock::new();
    static YANKED: OnceLock<Admission> = OnceLock::new();
    (if yanked { &YANKED } else { &ACTIVE }).get_or_init(|| registry::tests::trust_fixture(yanked))
}
// All private keys and signing constructors are test-only.
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
    json!({"length":bytes.len(),"sha256":hash(bytes)})
}
fn metadata_ref(bytes: &str) -> Value {
    let value: Value = serde_json::from_str(bytes).unwrap();
    json!({"version":value["signed"]["version"],"file":blob(bytes.as_bytes())})
}
fn sign(signed: Value, seeds: &[u8], domain: &[u8]) -> String {
    let mut message = domain.to_vec();
    message.extend_from_slice(wire(&signed).as_bytes());
    let signatures = seeds
        .iter()
        .map(|seed| {
            let key = key(*seed);
            json!({"keyid":keyid(&key),"sig":encoded(&key.sign(&message).to_bytes())})
        })
        .collect::<Vec<_>>();
    wire(&json!({"signed":signed,"signatures":signatures}))
}
fn root_value(version: u64, root_seed: u8, publisher_seed: u8) -> Value {
    root_value_until(version, root_seed, publisher_seed, 1000)
}
fn root_value_until(version: u64, root_seed: u8, publisher_seed: u8, expires: u64) -> Value {
    let roles = [
        ("root", "", vec![root_seed]),
        ("timestamp", "", vec![2]),
        ("snapshot", "", vec![3]),
        (
            "publisher-app",
            "app.",
            vec![publisher_seed, publisher_seed + 1],
        ),
        (
            "publisher-lib",
            "lib.",
            vec![publisher_seed + 2, publisher_seed + 3],
        ),
    ];
    let keys = roles
        .iter()
        .flat_map(|(_, _, seeds)| seeds.iter())
        .map(|seed| {
            let key = key(*seed);
            json!({"keyid":keyid(&key),"public":encoded(&key.verifying_key().to_bytes())})
        })
        .collect::<Vec<_>>();
    let roles = roles.iter().map(|(name,namespace,seeds)|json!({"name":name,"namespace":namespace,
        "keyids":seeds.iter().map(|seed|keyid(&key(*seed))).collect::<Vec<_>>(),"threshold":seeds.len()})).collect::<Vec<_>>();
    json!({"schema":ROOT_SCHEMA,"registry":"test.registry","version":version,"expires":expires,"keys":keys,"roles":roles})
}
struct Fixture {
    root: InstalledRoot,
    admitted: &'static Admission,
    publishers: [String; 2],
    snapshot: String,
    timestamp: String,
    expires: u64,
}
impl Fixture {
    fn new(yanked: bool) -> Self {
        Self::new_until(yanked, 1000)
    }
    fn new_until(yanked: bool, expires: u64) -> Self {
        let root = InstalledRoot::from_independently_installed_bytes(&wire(&root_value_until(
            1, 1, 4, expires,
        )))
        .unwrap();
        let mut f = Self {
            root,
            admitted: admission(yanked),
            publishers: Default::default(),
            snapshot: String::new(),
            timestamp: String::new(),
            expires,
        };
        f.publish(1, 4);
        f.refresh(1, 1);
        f
    }
    fn metadata(
        &self,
        role: &str,
        version: u64,
        expires: u64,
        payload: Value,
        seeds: &[u8],
    ) -> String {
        sign(
            json!({"schema":METADATA_SCHEMA_V2,"registry":self.root.registry,"root_digest":self.root.digest,
            "role":role,"version":version,"expires":expires,"payload":payload}),
            seeds,
            DOMAIN,
        )
    }
    fn publish(&mut self, version: u64, seed: u8) {
        for (i, namespace) in ["app.", "lib."].iter().enumerate() {
            let targets=self.admitted.0.entries().iter().filter(|e|e.publication().package.starts_with(namespace)).map(|entry| {
                json!({"package":entry.publication().package,"version":entry.publication().version,
                    "manifest":{"schema":manifest_schema(entry).unwrap(),"digest":entry.artifact_manifest_digest(),
                        "file":blob(entry.artifact_manifest_bytes().as_bytes())}})
            }).collect::<Vec<_>>();
            self.publishers[i] = self.metadata(
                ["publisher-app", "publisher-lib"][i],
                version,
                self.expires,
                json!({"targets":targets}),
                &[seed + i as u8 * 2, seed + i as u8 * 2 + 1],
            );
        }
    }
    fn refresh(&mut self, snapshot_version: u64, timestamp_version: u64) {
        self.snapshot = self.metadata(
            "snapshot",
            snapshot_version,
            self.expires,
            json!({"registry":{"schema":registry::SNAPSHOT_SCHEMA,
            "file":blob(self.admitted.0.envelope().as_bytes())},"publishers":[
            {"role":"publisher-app","metadata":metadata_ref(&self.publishers[0])},
            {"role":"publisher-lib","metadata":metadata_ref(&self.publishers[1])}]}),
            &[3],
        );
        self.refresh_timestamp(timestamp_version);
    }
    fn refresh_timestamp(&mut self, version: u64) {
        self.timestamp = self.metadata(
            "timestamp",
            version,
            self.expires,
            json!({"snapshot":metadata_ref(&self.snapshot)}),
            &[2],
        );
    }
    fn verify(
        &self,
        checkpoint: &RegistryCheckpoint,
        now: u64,
    ) -> Result<RegistryUpdateCandidate<'_>> {
        verify_update(
            &self.root,
            checkpoint,
            now,
            &UpdateInputs {
                timestamp: &self.timestamp,
                snapshot: &self.snapshot,
                publishers: &[
                    ("publisher-app", &self.publishers[0]),
                    ("publisher-lib", &self.publishers[1]),
                ],
                registry: &self.admitted.0,
            },
        )
    }
    fn initial(&self) -> RegistryCheckpoint {
        RegistryCheckpoint::initial(&self.root)
    }
}
fn rewrite(bytes: &str, seeds: &[u8], edit: impl FnOnce(&mut Value)) -> String {
    let mut value: Value = serde_json::from_str(bytes).unwrap();
    edit(&mut value["signed"]);
    sign(value["signed"].clone(), seeds, DOMAIN)
}
fn refused<T>(result: Result<T>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(error) => assert_eq!(error.code, code),
    }
}

struct ScriptedMirror {
    replies: VecDeque<MirrorResponse>,
}
impl MirrorTransport for ScriptedMirror {
    fn get(&mut self, request: MirrorGet<'_>) -> std::result::Result<MirrorResponse, MirrorError> {
        assert!(request
            .url()
            .starts_with("https://mirror.example.test/metadata/"));
        self.replies.pop_front().ok_or(MirrorError::TransportFailed)
    }
}

fn mirror_bytes(rows: [(&str, &str); 4]) -> Vec<MirrorBytes> {
    let digests = rows.map(|(_, bytes)| hash(bytes.as_bytes()));
    let objects = rows
        .iter()
        .zip(&digests)
        .map(|((path, bytes), digest)| MirrorObject {
            kind: MirrorObjectKind::Metadata,
            path,
            digest,
            max_bytes: bytes.len(),
        })
        .collect::<Vec<_>>();
    let replies = rows
        .iter()
        .map(|(path, bytes)| MirrorResponse {
            status: 200,
            final_url: format!("https://mirror.example.test{path}"),
            body: bytes.as_bytes().to_vec(),
        })
        .collect();
    let authority = MirrorNetworkAuthority::new(
        MirrorOrigin::parse("https://mirror.example.test/").unwrap(),
        Duration::from_secs(1),
    )
    .unwrap();
    authority
        .acquire(
            &mut ScriptedMirror { replies },
            &crate::package_registry::mirror_transport::MirrorRequest { objects: &objects },
        )
        .unwrap()
}

fn long_lived_mirror_fixture() -> Fixture {
    let mut f = Fixture::new(false);
    let expires = 100 + MAX_MIRROR_OFFLINE_SECONDS + 100;
    let mut root = root_value(1, 1, 4);
    root["expires"] = json!(expires);
    f.root = InstalledRoot::from_independently_installed_bytes(&wire(&root)).unwrap();
    f.publish(1, 4);
    f.refresh(1, 1);
    f.publishers[0] = rewrite(&f.publishers[0], &[4, 5], |value| {
        value["expires"] = json!(expires)
    });
    f.publishers[1] = rewrite(&f.publishers[1], &[6, 7], |value| {
        value["expires"] = json!(expires)
    });
    f.snapshot = f.metadata(
        "snapshot",
        1,
        expires,
        json!({"registry":{"schema":registry::SNAPSHOT_SCHEMA,
        "file":blob(f.admitted.0.envelope().as_bytes())},"publishers":[
        {"role":"publisher-app","metadata":metadata_ref(&f.publishers[0])},
        {"role":"publisher-lib","metadata":metadata_ref(&f.publishers[1])}]}),
        &[3],
    );
    f.timestamp = f.metadata(
        "timestamp",
        1,
        expires,
        json!({"snapshot":metadata_ref(&f.snapshot)}),
        &[2],
    );
    f
}

#[test]
fn acquired_metadata_replays_signed_snapshot_publishers_and_offline_policy() {
    let f = long_lived_mirror_fixture();
    let downloaded = mirror_bytes([
        ("/metadata/timestamp.json", &f.timestamp),
        ("/metadata/snapshot.json", &f.snapshot),
        ("/metadata/publisher-app.json", &f.publishers[0]),
        ("/metadata/publisher-lib.json", &f.publishers[1]),
    ]);
    let publishers = [
        MirrorPublisherPath {
            role: "publisher-app",
            path: "/metadata/publisher-app.json",
        },
        MirrorPublisherPath {
            role: "publisher-lib",
            path: "/metadata/publisher-lib.json",
        },
    ];
    let paths = MirrorMetadataPaths {
        timestamp_path: "/metadata/timestamp.json",
        snapshot_path: "/metadata/snapshot.json",
        publishers: &publishers,
        registry: &f.admitted.0,
    };
    let initial = MirrorCheckpoint::initial(&f.root);
    let candidate = verify_mirror_update(&f.root, &initial, 100, &paths, &downloaded).unwrap();
    candidate.check_lock(&f.admitted.2, &f.admitted.1).unwrap();

    let mut tampered = f.publishers[0].clone();
    tampered = tampered.replacen("\"version\":1", "\"version\":2", 1);
    let tampered_downloaded = mirror_bytes([
        ("/metadata/timestamp.json", &f.timestamp),
        ("/metadata/snapshot.json", &f.snapshot),
        ("/metadata/publisher-app.json", &tampered),
        ("/metadata/publisher-lib.json", &f.publishers[1]),
    ]);
    refused(
        verify_mirror_update(&f.root, &initial, 100, &paths, &tampered_downloaded),
        "SPX-PKR622",
    );
    let replayed =
        verify_mirror_update(&f.root, candidate.checkpoint(), 200, &paths, &downloaded).unwrap();
    assert_eq!(
        replayed
            .checkpoint()
            .registry_checkpoint()
            .previous
            .observed_time,
        200,
        "unchanged mirror bytes retain ordinary Trust-v2 trusted-time high-water"
    );
    refused(
        verify_mirror_update(&f.root, replayed.checkpoint(), 150, &paths, &downloaded),
        "SPX-PKR623",
    );
    let replayed = verify_mirror_update(
        &f.root,
        replayed.checkpoint(),
        100 + MAX_MIRROR_OFFLINE_SECONDS - 1,
        &paths,
        &downloaded,
    )
    .unwrap();
    refused(
        verify_mirror_update(
            &f.root,
            replayed.checkpoint(),
            100 + MAX_MIRROR_OFFLINE_SECONDS + 1,
            &paths,
            &downloaded,
        ),
        "SPX-PKR623",
    );
}

#[test]
fn admitted_root_leaf_lock_and_artifacts_bind_exact_checkpoint() {
    let f = Fixture::new(false);
    let initial = f.initial();
    let candidate = f.verify(&initial, 100).unwrap();
    assert_eq!(
        candidate.prior_checkpoint_digest(),
        hash(initial.canonical_bytes().as_bytes())
    );
    assert_eq!(candidate.registry_snapshot_digest(), f.admitted.0.digest());
    candidate.check_lock(&f.admitted.2, &f.admitted.1).unwrap();
    for (package, bytes) in [("app.root", &f.admitted.3), ("lib.leaf", &f.admitted.4)] {
        candidate
            .check_artifact(package, "1.0.0", "module.wasm", bytes)
            .unwrap();
        refused(
            candidate.check_artifact(package, "1.0.0", "module.wasm", b"tamper"),
            "SPX-PKR624",
        );
        refused(
            candidate.check_artifact(package, "1.0.0", "other.wasm", bytes),
            "SPX-PKR624",
        );
    }
    let bytes = candidate.checkpoint().canonical_bytes();
    let stored = RegistryCheckpoint::from_trusted_store_bytes(&bytes).unwrap();
    let repeat = f.verify(&stored, 101).unwrap();
    assert_eq!(repeat.prior_checkpoint_digest(), hash(bytes.as_bytes()));
    assert!(candidate
        .check_lock(&f.admitted.2, &f.admitted.1[..1])
        .is_err());
}

#[test]
fn publisher_threshold_role_namespace_and_domain_refusals() {
    for mode in 0..6 {
        let mut f = Fixture::new(false);
        let value: Value = serde_json::from_str(&f.publishers[0]).unwrap();
        f.publishers[0] = match mode {
            0 => sign(value["signed"].clone(), &[4], DOMAIN),
            1 => sign(value["signed"].clone(), &[4, 4], DOMAIN),
            2 => sign(value["signed"].clone(), &[6, 7], DOMAIN),
            3 => sign(value["signed"].clone(), &[4, 5], SIGN_DOMAIN),
            4 => rewrite(&f.publishers[0], &[4, 5], |v| {
                v["payload"]["targets"][0]["package"] = json!("lib.leaf")
            }),
            _ => rewrite(&f.publishers[0], &[4, 5], |v| {
                v["schema"] = json!(METADATA_SCHEMA)
            }),
        };
        f.refresh(1, 1);
        refused(f.verify(&f.initial(), 100), "SPX-PKR622");
    }
}

#[test]
fn exact_manifest_schema_digest_bytes_and_complete_inventory() {
    for mode in 0..5 {
        let mut f = Fixture::new(false);
        f.publishers[1] = rewrite(&f.publishers[1], &[6, 7], |v| {
            let target = &mut v["payload"]["targets"][0];
            match mode {
                0 => target["manifest"]["schema"] = json!(artifact_manifest::SCHEMA),
                1 => target["manifest"]["digest"] = json!(hash(b"wrong")),
                2 => target["manifest"]["file"]["sha256"] = json!(hash(b"wrong")),
                3 => target["version"] = json!("2.0.0"),
                _ => v["payload"]["targets"] = json!([]),
            }
        });
        f.refresh(1, 1);
        refused(f.verify(&f.initial(), 100), "SPX-PKR624");
    }
}

#[test]
fn registry_timestamp_and_publisher_pins_refuse_mix_and_match() {
    for mode in 0..4 {
        let mut f = Fixture::new(false);
        match mode {
            0 => {
                f.snapshot = rewrite(&f.snapshot, &[3], |v| {
                    v["payload"]["registry"]["file"]["sha256"] = json!(hash(b"wrong"))
                });
                f.refresh_timestamp(1);
            }
            1 => {
                f.snapshot = rewrite(&f.snapshot, &[3], |v| {
                    v["payload"]["registry"]["schema"] =
                        json!("semaprax.package-registry-snapshot.v2")
                });
                f.refresh_timestamp(1);
            }
            2 => {
                f.publishers[0] = rewrite(&f.publishers[0], &[4, 5], |v| v["version"] = json!(2));
            }
            _ => {
                f.snapshot = rewrite(&f.snapshot, &[3], |v| v["version"] = json!(2));
            }
        }
        refused(f.verify(&f.initial(), 100), "SPX-PKR624");
    }
}

#[test]
fn expiry_and_trusted_clock_are_fail_closed_for_every_role() {
    for mode in 0..5 {
        let mut f = Fixture::new(false);
        match mode {
            0 => f.root.expires = 100,
            1 => f.timestamp = rewrite(&f.timestamp, &[2], |v| v["expires"] = json!(100)),
            2 => {
                f.snapshot = rewrite(&f.snapshot, &[3], |v| v["expires"] = json!(100));
                f.refresh_timestamp(1);
            }
            i => {
                let p = i - 3;
                f.publishers[p] = rewrite(
                    &f.publishers[p],
                    if p == 0 { &[4, 5] } else { &[6, 7] },
                    |v| v["expires"] = json!(100),
                );
                f.refresh(1, 1);
            }
        }
        refused(f.verify(&f.initial(), 100), "SPX-PKR623");
    }
    let f = Fixture::new(false);
    let checkpoint = f.verify(&f.initial(), 100).unwrap().checkpoint().clone();
    refused(f.verify(&checkpoint, 99), "SPX-PKR623");
}

#[test]
fn rollback_equivocation_and_cross_profile_high_water_are_refused() {
    for mode in 0..4 {
        let mut f = Fixture::new(false);
        f.publish(2, 4);
        f.refresh(2, 2);
        let checkpoint = f.verify(&f.initial(), 100).unwrap().checkpoint().clone();
        match mode {
            0 => f.refresh(1, 3),
            1 => {
                f.publish(1, 4);
                f.refresh(3, 3);
            }
            2 => {
                f.publishers[0] = rewrite(&f.publishers[0], &[4, 5], |v| v["expires"] = json!(501));
                f.refresh(3, 3);
            }
            _ => f.timestamp = rewrite(&f.timestamp, &[2], |v| v["expires"] = json!(301)),
        }
        refused(f.verify(&checkpoint, 100), "SPX-PKR623");
    }
    let f = Fixture::new(false);
    let mut checkpoint = f.initial();
    // A checkpoint stamped by a different signed profile is not reset on migration.
    checkpoint.previous.roles.insert(
        "snapshot".into(),
        Stamp {
            version: 1,
            digest: hash(b"wrong"),
        },
    );
    refused(f.verify(&checkpoint, 100), "SPX-PKR623");
}

#[test]
fn dual_threshold_root_rotation_revokes_old_publishers() {
    for rename in [true, false] {
        let mut f = Fixture::new(false);
        f.publish(5, 4);
        f.refresh(5, 5);
        let checkpoint = f.verify(&f.initial(), 100).unwrap().checkpoint().clone();
        let mut next = root_value(2, 10, 20);
        if rename {
            next["roles"][3]["name"] = json!("publisher-app-renamed");
        } else {
            next["roles"][3]["namespace"] = json!("lib.");
            next["roles"][4]["namespace"] = json!("app.");
        }
        let rotated = verify_root_rotation(
            &f.root,
            &sign(next, &[1, 10], b"semaprax.registry-trust-root.v1\0"),
            100,
        )
        .unwrap();
        f.root = rotated.root;
        f.publish(6, 20);
        let app_role = if rename {
            "publisher-app-renamed"
        } else {
            "publisher-app"
        };
        if rename {
            f.publishers[0] = rewrite(&f.publishers[0], &[20, 21], |v| {
                v["role"] = json!(app_role);
                v["version"] = json!(1);
            });
        } else {
            let app: Value = serde_json::from_str(&f.publishers[0]).unwrap();
            let lib: Value = serde_json::from_str(&f.publishers[1]).unwrap();
            f.publishers[0] = rewrite(&f.publishers[0], &[20, 21], |v| {
                v["payload"] = lib["signed"]["payload"].clone()
            });
            f.publishers[1] = rewrite(&f.publishers[1], &[22, 23], |v| {
                v["payload"] = app["signed"]["payload"].clone()
            });
        }
        f.refresh(6, 6);
        if rename {
            f.snapshot = rewrite(&f.snapshot, &[3], |v| {
                v["payload"]["publishers"][0]["role"] = json!(app_role)
            });
            f.refresh_timestamp(6);
        }
        let publishers = [
            (app_role, f.publishers[0].as_str()),
            ("publisher-lib", f.publishers[1].as_str()),
        ];
        let inputs = UpdateInputs {
            timestamp: &f.timestamp,
            snapshot: &f.snapshot,
            publishers: &publishers,
            registry: &f.admitted.0,
        };
        // This otherwise coherent signed metadata passes from a fresh authorized
        // root but must not resume the existing namespace high-water chain.
        verify_update(&f.root, &RegistryCheckpoint::initial(&f.root), 100, &inputs).unwrap();
        refused(
            verify_update(&f.root, &checkpoint, 100, &inputs),
            "SPX-PKR623",
        );
    }
    let mut f = Fixture::new(false);
    let checkpoint = f.verify(&f.initial(), 100).unwrap().checkpoint().clone();
    let next = root_value(2, 10, 20);
    refused(
        verify_root_rotation(
            &f.root,
            &sign(next.clone(), &[10], b"semaprax.registry-trust-root.v1\0"),
            100,
        ),
        "SPX-PKR622",
    );
    let rotation = verify_root_rotation(
        &f.root,
        &sign(next, &[1, 10], b"semaprax.registry-trust-root.v1\0"),
        100,
    )
    .unwrap();
    assert_eq!(rotation.previous_root_digest(), f.root.digest);
    f.root = rotation.root;
    f.publish(2, 20);
    f.refresh(2, 2);
    let candidate = f.verify(&checkpoint, 100).unwrap();
    candidate.check_lock(&f.admitted.2, &f.admitted.1).unwrap();
    let rotated = candidate.checkpoint().clone();
    f.publishers[0] = rewrite(&f.publishers[0], &[4, 5], |v| v["version"] = json!(3));
    f.refresh(3, 3);
    refused(f.verify(&rotated, 100), "SPX-PKR622");
    let old = Fixture::new(false);
    refused(old.verify(&rotated, 100), "SPX-PKR623");
}

#[test]
fn signed_yank_blocks_lock_and_artifact_but_not_snapshot_inspection() {
    let f = Fixture::new(true);
    let candidate = f.verify(&f.initial(), 100).unwrap();
    assert!(candidate.check_lock(&f.admitted.2, &f.admitted.1).is_err());
    refused(
        candidate.check_artifact("lib.leaf", "1.0.0", "module.wasm", &f.admitted.4),
        "SPX-PKR625",
    );
}

#[test]
fn checkpoint_protocol_floor_is_one_way_and_byte_exact() {
    let f = Fixture::new(false);
    let mut old = Checkpoint::initial(&f.root);
    old.roles.insert(
        "snapshot".into(),
        Stamp {
            version: 10,
            digest: hash(b"wrong"),
        },
    );
    let migrated = RegistryCheckpoint::migrate_from_v1(&old, &f.root).unwrap();
    assert_eq!(migrated.previous.canonical_bytes(), old.canonical_bytes());
    let other =
        InstalledRoot::from_independently_installed_bytes(&wire(&root_value(2, 10, 20))).unwrap();
    refused(
        RegistryCheckpoint::migrate_from_v1(&old, &other),
        "SPX-PKR623",
    );
    refused(f.verify(&migrated, 100), "SPX-PKR623");
    refused(
        Checkpoint::from_trusted_store_bytes(&migrated.canonical_bytes()),
        "SPX-PKR621",
    );
    refused(
        RegistryCheckpoint::from_trusted_store_bytes(&old.canonical_bytes()),
        "SPX-PKR621",
    );
    let candidate = f.verify(&f.initial(), 100).unwrap();
    let bytes = candidate.checkpoint().canonical_bytes();
    let mut outer: Value = serde_json::from_str(&bytes).unwrap();
    let mut inner: Value = serde_json::from_str(outer["checkpoint"].as_str().unwrap()).unwrap();
    inner["roles"].as_array_mut().unwrap().reverse();
    outer["checkpoint"] = json!(wire(&inner));
    refused(
        RegistryCheckpoint::from_trusted_store_bytes(&wire(&outer)),
        "SPX-PKR621",
    );
}
