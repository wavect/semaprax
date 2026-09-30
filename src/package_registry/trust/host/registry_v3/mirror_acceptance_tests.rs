//! R19 / issue #304 composition acceptance: drives the composed mirror path
//! (real loopback network fetch -> signed Trust-v3 proof -> held Registry-v3
//! commit -> live artifact read, and separately the signed resolver-cache and
//! Resolver-v2/Lock-v3 replay) against a genuine local TCP loopback listener.
//!
//! This module adds no new production machinery. Every checked property
//! (signature/threshold, freshness/rollback, publisher namespace binding,
//! yank, exact manifest/artifact binding, lock/artifact CAS pairing, held
//! fail-closed commit) is already enforced by `trust::registry_v3`,
//! `trust::host::registry_v3::{online, online_cache, read, store}` and is
//! separately unit-tested there with an in-process `MirrorTransport` fake.
//! What is new here is end-to-end evidence that those guarantees still hold
//! when the signed bytes actually cross a real socket: a genuine TCP
//! `TcpListener` on `127.0.0.1`, a genuine blocking `TcpStream` client, real
//! `ECONNREFUSED` and a real elapsed-time read timeout, not scripted errors.
//!
//! `LoopbackMirrorTransport` is a test-only, plain-HTTP-over-loopback client.
//! It is NOT a TLS client and must never be confused with the crate's sole
//! production transport, `NativeHttpsMirrorTransport` (which only trusts
//! public WebPKI roots and cannot be pointed at a self-signed loopback peer).
//! The `https://` origin text here only satisfies `MirrorOrigin::parse`'s
//! literal scheme check, exactly as the in-process `ScriptedMirror` fakes
//! elsewhere in this tree already do. All signing keys below are the same
//! test-only fixture keys used throughout `trust::registry_v3::tests`.

use super::*;
use crate::package_registry::mirror_transport::{
    MirrorError, MirrorGet, MirrorNetworkAuthority, MirrorObject, MirrorObjectKind, MirrorOrigin,
    MirrorResponse, MirrorTransport,
};
use crate::package_registry::trust::registry_v3::tests::host_fixture_until;
use crate::package_registry::trust::registry_v3::{
    MirrorMetadataPaths, MirrorPublisherPath, MAX_MIRROR_OFFLINE_SECONDS,
};
use crate::package_resolver_v2 as resolver;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Precomputed digests and owned response bytes for the six fixed mirror
/// paths shared by every scenario below. Fields are mutated in place by
/// individual tests to model a swapped, tampered or reassigned mirror.
struct MirrorMaterial {
    metadata_digests: [String; 4],
    artifact_digests: [String; 2],
    routes: Vec<(&'static str, Vec<u8>)>,
}

fn material(
    timestamp: &str,
    snapshot: &str,
    publishers: &[String; 2],
    app_bytes: &[u8],
    lib_bytes: &[u8],
) -> MirrorMaterial {
    MirrorMaterial {
        metadata_digests: [
            hash(timestamp.as_bytes()),
            hash(snapshot.as_bytes()),
            hash(publishers[0].as_bytes()),
            hash(publishers[1].as_bytes()),
        ],
        artifact_digests: [hash(app_bytes), hash(lib_bytes)],
        routes: vec![
            ("/metadata/timestamp.json", timestamp.as_bytes().to_vec()),
            ("/metadata/snapshot.json", snapshot.as_bytes().to_vec()),
            (
                "/metadata/publisher-app.json",
                publishers[0].as_bytes().to_vec(),
            ),
            (
                "/metadata/publisher-lib.json",
                publishers[1].as_bytes().to_vec(),
            ),
            ("/artifacts/app-root.wasm", app_bytes.to_vec()),
            ("/artifacts/lib-leaf.wasm", lib_bytes.to_vec()),
        ],
    }
}

fn object<'a>(
    kind: MirrorObjectKind,
    path: &'a str,
    digest: &'a str,
    max_bytes: usize,
) -> MirrorObject<'a> {
    MirrorObject {
        kind,
        path,
        digest,
        max_bytes,
    }
}

fn standard_metadata_objects(m: &MirrorMaterial) -> [MirrorObject<'_>; 4] {
    [
        object(
            MirrorObjectKind::Metadata,
            "/metadata/timestamp.json",
            &m.metadata_digests[0],
            m.routes[0].1.len(),
        ),
        object(
            MirrorObjectKind::Metadata,
            "/metadata/snapshot.json",
            &m.metadata_digests[1],
            m.routes[1].1.len(),
        ),
        object(
            MirrorObjectKind::Metadata,
            "/metadata/publisher-app.json",
            &m.metadata_digests[2],
            m.routes[2].1.len(),
        ),
        object(
            MirrorObjectKind::Metadata,
            "/metadata/publisher-lib.json",
            &m.metadata_digests[3],
            m.routes[3].1.len(),
        ),
    ]
}

fn standard_artifacts(m: &MirrorMaterial) -> [MirrorArtifact<'_>; 2] {
    [
        MirrorArtifact {
            object: object(
                MirrorObjectKind::Artifact,
                "/artifacts/app-root.wasm",
                &m.artifact_digests[0],
                m.routes[4].1.len(),
            ),
            package: "app.root",
            version: "1.0.0",
            path: "module.wasm",
        },
        MirrorArtifact {
            object: object(
                MirrorObjectKind::Artifact,
                "/artifacts/lib-leaf.wasm",
                &m.artifact_digests[1],
                m.routes[5].1.len(),
            ),
            package: "lib.leaf",
            version: "1.0.0",
            path: "module.wasm",
        },
    ]
}

fn standard_publisher_paths() -> [MirrorPublisherPath<'static>; 2] {
    [
        MirrorPublisherPath {
            role: "publisher-app",
            path: "/metadata/publisher-app.json",
        },
        MirrorPublisherPath {
            role: "publisher-lib",
            path: "/metadata/publisher-lib.json",
        },
    ]
}

/// Flips one signature nibble without recomputing any signature: the
/// corrupted bytes still decode, but no installed key produced them.
fn corrupt_signature(metadata: &str) -> String {
    let mut bytes = metadata.as_bytes().to_vec();
    let offset = bytes
        .windows(7)
        .position(|window| window == b"\"sig\":\"")
        .expect("signed metadata carries a sig field")
        + 7;
    bytes[offset] = if bytes[offset] == b'a' { b'b' } else { b'a' };
    String::from_utf8(bytes).expect("flipping one hex nibble keeps valid UTF-8")
}

fn install(f: &Fixture) -> (Temp, HeldTrustStore) {
    let temp = Temp::new();
    let store = HeldTrustStore::install(&temp.0, &f.root, &f.pin, 90).unwrap();
    (temp, store)
}

/// A real, bounded, single-purpose loopback HTTP server: `connections`
/// sequential TCP connections, each answered from `routes` by exact request
/// path or `404`, then the listener is dropped and the worker thread ends.
fn spawn_route_server(
    routes: Vec<(&'static str, Vec<u8>)>,
    connections: usize,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("listener address").port();
    let origin = format!("https://127.0.0.1:{port}/");
    let worker = std::thread::spawn(move || {
        for _ in 0..connections {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(_) => break,
            };
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            let mut data = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        data.extend_from_slice(&buffer[..n]);
                        if data.windows(4).any(|window| window == b"\r\n\r\n") {
                            break;
                        }
                    }
                }
            }
            let request_text = String::from_utf8_lossy(&data);
            let path = request_text
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("")
                .to_owned();
            match routes.iter().find(|(route, _)| *route == path) {
                Some((_, body)) => {
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(body);
                }
                None => {
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
            }
            let _ = stream.flush();
        }
    });
    (origin, worker)
}

/// A free loopback port with nothing listening on it: a real `ECONNREFUSED`.
fn refused_origin() -> String {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("listener address").port();
    drop(listener);
    format!("https://127.0.0.1:{port}/")
}

/// Accepts exactly one connection, then sleeps past any reasonable client
/// read timeout without ever writing a response.
fn spawn_stalling_server(delay: Duration) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("listener address").port();
    let origin = format!("https://127.0.0.1:{port}/");
    let worker = std::thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            std::thread::sleep(delay);
            drop(stream);
        }
    });
    (origin, worker)
}

/// Test-only genuine-socket `MirrorTransport`. See the module doc comment.
struct LoopbackMirrorTransport;
impl MirrorTransport for LoopbackMirrorTransport {
    fn get(&mut self, request: MirrorGet<'_>) -> std::result::Result<MirrorResponse, MirrorError> {
        let url = reqwest::Url::parse(request.url()).map_err(|_| MirrorError::TransportFailed)?;
        let host = url.host_str().ok_or(MirrorError::TransportFailed)?;
        let port = url.port().ok_or(MirrorError::TransportFailed)?;
        let address: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|_| MirrorError::TransportFailed)?;
        let mut stream = TcpStream::connect_timeout(&address, request.timeout())
            .map_err(|_| MirrorError::TransportFailed)?;
        stream
            .set_read_timeout(Some(request.timeout()))
            .map_err(|_| MirrorError::TransportFailed)?;
        let head = format!(
            "GET {} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n",
            url.path()
        );
        stream
            .write_all(head.as_bytes())
            .map_err(|_| MirrorError::TransportFailed)?;
        let mut buffer = Vec::new();
        stream
            .take((request.max_bytes() as u64).saturating_add(65_536))
            .read_to_end(&mut buffer)
            .map_err(|_| MirrorError::TransportFailed)?;
        let split = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or(MirrorError::TransportFailed)?;
        let head_text =
            std::str::from_utf8(&buffer[..split]).map_err(|_| MirrorError::TransportFailed)?;
        let status = head_text
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .ok_or(MirrorError::TransportFailed)?;
        Ok(MirrorResponse {
            status,
            final_url: request.url().to_owned(),
            body: buffer[split + 4..].to_vec(),
        })
    }
}

/// One standard successful (or lock/artifact-checked) mirror commit-and-read
/// round trip over a real loopback server, reused by the scenarios that do
/// not need to mutate the served bytes or the role/path mapping.
fn mirror_commit(
    f: &Fixture,
    transport: &mut LoopbackMirrorTransport,
    store: &mut HeldTrustStore,
) -> std::result::Result<MirrorFlowResult, MirrorFlowError> {
    let m = material(
        &f.timestamp,
        &f.snapshot,
        &f.publishers,
        &f.admitted.3,
        &f.admitted.4,
    );
    let metadata = standard_metadata_objects(&m);
    let artifacts = standard_artifacts(&m);
    let publisher_paths = standard_publisher_paths();
    let metadata_paths = MirrorMetadataPaths {
        timestamp_path: "/metadata/timestamp.json",
        snapshot_path: "/metadata/snapshot.json",
        publishers: &publisher_paths,
        registry: &f.admitted.0,
    };
    let (origin, worker) = spawn_route_server(m.routes.clone(), 6);
    let authority = MirrorNetworkAuthority::new(
        MirrorOrigin::parse(&origin).unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    let outcome = acquire_commit_and_read(
        &authority,
        transport,
        store,
        &MirrorFlowRequest {
            metadata_paths: &metadata_paths,
            metadata_objects: &metadata,
            artifacts: &artifacts,
            lock: &f.admitted.2,
            subjects: &f.admitted.1,
            read: MirrorArtifactSelection {
                package: "app.root",
                version: "1.0.0",
                path: "module.wasm",
            },
            update_time: 100,
            read_time: 100,
        },
    );
    worker.join().unwrap();
    outcome
}

/// One full mirror-fetch -> held-commit -> signed-cache -> Resolver-v2/
/// Lock-v3 replay round trip against its own fresh store, cache directory
/// and real loopback server, returning the reproducible evidence: the
/// replayed Lock-v3 text and the resolved `app.root` artifact bytes.
fn online_cache_run(f: &Fixture) -> (String, Vec<u8>) {
    let m = material(
        &f.timestamp,
        &f.snapshot,
        &f.publishers,
        &f.admitted.3,
        &f.admitted.4,
    );
    let metadata = standard_metadata_objects(&m);
    let artifacts = standard_artifacts(&m);
    let publisher_paths = standard_publisher_paths();
    let metadata_paths = MirrorMetadataPaths {
        timestamp_path: "/metadata/timestamp.json",
        snapshot_path: "/metadata/snapshot.json",
        publishers: &publisher_paths,
        registry: &f.admitted.0,
    };
    let store_temp = Temp::new();
    let cache_temp = Temp::new();
    let mut store = HeldTrustStore::install(&store_temp.0, &f.root, &f.pin, 90).unwrap();
    let (origin, worker) = spawn_route_server(m.routes.clone(), 6);
    let authority = MirrorNetworkAuthority::new(
        MirrorOrigin::parse(&origin).unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    let mut transport = LoopbackMirrorTransport;
    let result = acquire_commit_cache_and_resolve(
        &authority,
        &mut transport,
        &mut store,
        &MirrorCacheFlowRequest {
            mirror: MirrorFlowRequest {
                metadata_paths: &metadata_paths,
                metadata_objects: &metadata,
                artifacts: &artifacts,
                lock: &f.admitted.2,
                subjects: &f.admitted.1,
                read: MirrorArtifactSelection {
                    package: "app.root",
                    version: "1.0.0",
                    path: "module.wasm",
                },
                update_time: 100,
                read_time: 100,
            },
            cache_path: &cache_temp.0.join("cache"),
            cache_time: 100,
            resolution: MirrorResolutionTemplate {
                requirements: &[resolver::Requirement {
                    package: "app.root".into(),
                    range: "=1.0.0".into(),
                }],
                target: "wasm32",
                allowed_capabilities: &[],
                options: Default::default(),
            },
        },
    )
    .unwrap();
    worker.join().unwrap();
    (result.resolved.lock.clone(), result.artifact.into_bytes())
}

#[test]
fn composed_online_path_reproduces_clean_offline_lock_and_artifact_bytes() {
    let f = Fixture::new(false, 1);

    // Clean offline consumption: an ordinary signed commit with no network
    // participation at all.
    let offline_temp = Temp::new();
    let mut offline_store = HeldTrustStore::install(&offline_temp.0, &f.root, &f.pin, 90).unwrap();
    let publishers = f.publishers();
    let artifacts = f.artifacts();
    let offline_receipt = offline_store
        .commit_update(&f.update(&publishers, &artifacts))
        .unwrap();
    let offline_artifact = offline_store
        .read_artifact(&ArtifactRead {
            expected_generation_digest: &offline_receipt.generation_digest,
            lock: &f.admitted.2,
            registry: &f.admitted.0,
            package: "app.root",
            version: "1.0.0",
            path: "module.wasm",
            trusted_time: 100,
        })
        .unwrap();
    assert_eq!(offline_artifact.bytes(), f.admitted.3.as_slice());

    // Two independent online-as-authorized runs: separate held stores,
    // separate resolver-cache directories and separate real loopback mirror
    // servers, all serving the identical signed bytes.
    let (lock_a, bytes_a) = online_cache_run(&f);
    let (lock_b, bytes_b) = online_cache_run(&f);
    assert_eq!(
        lock_a, lock_b,
        "two independent online runs must reproduce the same Lock-v3"
    );
    assert_eq!(
        bytes_a, bytes_b,
        "two independent online runs must reproduce the same package bytes"
    );
    assert_eq!(
        lock_a, f.admitted.2,
        "online resolution must match clean offline consumption's lock"
    );
    assert_eq!(
        bytes_a.as_slice(),
        offline_artifact.bytes(),
        "online resolution must match clean offline consumption's package bytes"
    );
}

#[test]
fn composed_online_path_refuses_a_swapped_artifact() {
    let f = Fixture::new(false, 1);
    let (temp, mut store) = install(&f);
    let before = inventory(&temp);
    let mut m = material(
        &f.timestamp,
        &f.snapshot,
        &f.publishers,
        &f.admitted.3,
        &f.admitted.4,
    );
    // The declared digest for app.root's artifact is left correct, but the
    // mirror actually serves lib.leaf's real (differently purposed) bytes
    // under that path: a genuine substitution.
    m.routes[4].1 = f.admitted.4.clone();
    let metadata = standard_metadata_objects(&m);
    let artifacts = standard_artifacts(&m);
    let publisher_paths = standard_publisher_paths();
    let metadata_paths = MirrorMetadataPaths {
        timestamp_path: "/metadata/timestamp.json",
        snapshot_path: "/metadata/snapshot.json",
        publishers: &publisher_paths,
        registry: &f.admitted.0,
    };
    // The batch fails closed on the mismatched digest for the app-root
    // artifact (the fifth planned object); lib.leaf is never requested.
    let (origin, worker) = spawn_route_server(m.routes.clone(), 5);
    let authority = MirrorNetworkAuthority::new(
        MirrorOrigin::parse(&origin).unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    let mut transport = LoopbackMirrorTransport;
    let outcome = acquire_commit_and_read(
        &authority,
        &mut transport,
        &mut store,
        &MirrorFlowRequest {
            metadata_paths: &metadata_paths,
            metadata_objects: &metadata,
            artifacts: &artifacts,
            lock: &f.admitted.2,
            subjects: &f.admitted.1,
            read: MirrorArtifactSelection {
                package: "app.root",
                version: "1.0.0",
                path: "module.wasm",
            },
            update_time: 100,
            read_time: 100,
        },
    );
    worker.join().unwrap();
    match outcome {
        Err(MirrorFlowError::Acquisition(MirrorError::DigestMismatch)) => {}
        Ok(_) => panic!("swapped artifact was unexpectedly accepted"),
        Err(other) => panic!("swapped artifact produced the wrong outcome: {other:?}"),
    }
    assert_eq!(inventory(&temp), before);
}

#[test]
fn composed_online_path_refuses_tampered_metadata() {
    let f = Fixture::new(false, 1);
    let (temp, mut store) = install(&f);
    let before = inventory(&temp);
    let tampered_timestamp = corrupt_signature(&f.timestamp);
    let mut m = material(
        &tampered_timestamp,
        &f.snapshot,
        &f.publishers,
        &f.admitted.3,
        &f.admitted.4,
    );
    // Recomputing the digest against the tampered bytes lets the download
    // itself succeed; only the embedded signature can no longer verify,
    // because the attacker holds no installed key.
    m.metadata_digests[0] = hash(tampered_timestamp.as_bytes());
    m.routes[0].1 = tampered_timestamp.into_bytes();
    let metadata = standard_metadata_objects(&m);
    let artifacts = standard_artifacts(&m);
    let publisher_paths = standard_publisher_paths();
    let metadata_paths = MirrorMetadataPaths {
        timestamp_path: "/metadata/timestamp.json",
        snapshot_path: "/metadata/snapshot.json",
        publishers: &publisher_paths,
        registry: &f.admitted.0,
    };
    let (origin, worker) = spawn_route_server(m.routes.clone(), 6);
    let authority = MirrorNetworkAuthority::new(
        MirrorOrigin::parse(&origin).unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    let mut transport = LoopbackMirrorTransport;
    let outcome = acquire_commit_and_read(
        &authority,
        &mut transport,
        &mut store,
        &MirrorFlowRequest {
            metadata_paths: &metadata_paths,
            metadata_objects: &metadata,
            artifacts: &artifacts,
            lock: &f.admitted.2,
            subjects: &f.admitted.1,
            read: MirrorArtifactSelection {
                package: "app.root",
                version: "1.0.0",
                path: "module.wasm",
            },
            update_time: 100,
            read_time: 100,
        },
    );
    worker.join().unwrap();
    match outcome {
        Err(MirrorFlowError::Proof(error)) => assert_eq!(error.code, "SPX-PKR622"),
        Ok(_) => panic!("tampered metadata was unexpectedly accepted"),
        Err(other) => panic!("tampered metadata produced the wrong outcome: {other:?}"),
    }
    assert_eq!(inventory(&temp), before);
}

#[test]
fn composed_online_path_refuses_a_stale_timestamp_after_the_offline_window() {
    let long_expiry = 100 + MAX_MIRROR_OFFLINE_SECONDS + 100;
    let (root, timestamp, snapshot, publishers, admitted) =
        host_fixture_until(false, 1, long_expiry);
    let pin = hash(root.as_bytes());
    let temp = Temp::new();
    let mut store = HeldTrustStore::install(&temp.0, &root, &pin, 90).unwrap();
    let m = material(&timestamp, &snapshot, &publishers, &admitted.3, &admitted.4);
    let metadata = standard_metadata_objects(&m);
    let artifacts = standard_artifacts(&m);
    let publisher_paths = standard_publisher_paths();
    let metadata_paths = MirrorMetadataPaths {
        timestamp_path: "/metadata/timestamp.json",
        snapshot_path: "/metadata/snapshot.json",
        publishers: &publisher_paths,
        registry: &admitted.0,
    };
    let mut transport = LoopbackMirrorTransport;
    // A first real network mirror update succeeds and anchors the timestamp
    // observation at trusted time 100.
    let (origin, worker) = spawn_route_server(m.routes.clone(), 6);
    let authority = MirrorNetworkAuthority::new(
        MirrorOrigin::parse(&origin).unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    acquire_commit_and_read(
        &authority,
        &mut transport,
        &mut store,
        &MirrorFlowRequest {
            metadata_paths: &metadata_paths,
            metadata_objects: &metadata,
            artifacts: &artifacts,
            lock: &admitted.2,
            subjects: &admitted.1,
            read: MirrorArtifactSelection {
                package: "app.root",
                version: "1.0.0",
                path: "module.wasm",
            },
            update_time: 100,
            read_time: 100,
        },
    )
    .unwrap();
    worker.join().unwrap();
    let before = inventory(&temp);

    // The same unchanged signed bytes, replayed after the seven-day local
    // offline window has elapsed, must be refused as stale rather than
    // silently treated as still fresh.
    let stale_time = 100 + MAX_MIRROR_OFFLINE_SECONDS + 1;
    let (origin, worker) = spawn_route_server(m.routes.clone(), 6);
    let authority = MirrorNetworkAuthority::new(
        MirrorOrigin::parse(&origin).unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    let outcome = acquire_commit_and_read(
        &authority,
        &mut transport,
        &mut store,
        &MirrorFlowRequest {
            metadata_paths: &metadata_paths,
            metadata_objects: &metadata,
            artifacts: &artifacts,
            lock: &admitted.2,
            subjects: &admitted.1,
            read: MirrorArtifactSelection {
                package: "app.root",
                version: "1.0.0",
                path: "module.wasm",
            },
            update_time: stale_time,
            read_time: stale_time,
        },
    );
    worker.join().unwrap();
    match outcome {
        Err(MirrorFlowError::Proof(error)) => assert_eq!(error.code, "SPX-PKR623"),
        Ok(_) => panic!("stale replayed timestamp was unexpectedly accepted"),
        Err(other) => panic!("stale replayed timestamp produced the wrong outcome: {other:?}"),
    }
    assert_eq!(inventory(&temp), before);
}

#[test]
fn composed_online_path_refuses_metadata_version_rollback() {
    let high = Fixture::new(false, 2);
    let low = Fixture::new(false, 1);
    assert_eq!(
        high.root, low.root,
        "rollback fixtures must share one root and admitted registry"
    );
    let (temp, mut store) = install(&high);
    let mut transport = LoopbackMirrorTransport;
    mirror_commit(&high, &mut transport, &mut store).unwrap();
    let before = inventory(&temp);
    let outcome = mirror_commit(&low, &mut transport, &mut store);
    match outcome {
        Err(MirrorFlowError::Proof(error)) => assert_eq!(error.code, "SPX-PKR623"),
        Ok(_) => panic!("metadata version rollback was unexpectedly accepted"),
        Err(other) => panic!("metadata version rollback produced the wrong outcome: {other:?}"),
    }
    assert_eq!(inventory(&temp), before);
}

#[test]
fn composed_online_path_refuses_wrong_publisher_identity_reassignment() {
    let f = Fixture::new(false, 1);
    let (temp, mut store) = install(&f);
    let before = inventory(&temp);
    let m = material(
        &f.timestamp,
        &f.snapshot,
        &f.publishers,
        &f.admitted.3,
        &f.admitted.4,
    );
    let metadata = standard_metadata_objects(&m);
    let artifacts = standard_artifacts(&m);
    // The bytes served at each URL are genuine and unmodified; only the role
    // each publisher's own signed statement is trusted under gets reassigned
    // to the other publisher's namespace/URL.
    let swapped_paths = [
        MirrorPublisherPath {
            role: "publisher-app",
            path: "/metadata/publisher-lib.json",
        },
        MirrorPublisherPath {
            role: "publisher-lib",
            path: "/metadata/publisher-app.json",
        },
    ];
    let metadata_paths = MirrorMetadataPaths {
        timestamp_path: "/metadata/timestamp.json",
        snapshot_path: "/metadata/snapshot.json",
        publishers: &swapped_paths,
        registry: &f.admitted.0,
    };
    let (origin, worker) = spawn_route_server(m.routes.clone(), 6);
    let authority = MirrorNetworkAuthority::new(
        MirrorOrigin::parse(&origin).unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    let mut transport = LoopbackMirrorTransport;
    let outcome = acquire_commit_and_read(
        &authority,
        &mut transport,
        &mut store,
        &MirrorFlowRequest {
            metadata_paths: &metadata_paths,
            metadata_objects: &metadata,
            artifacts: &artifacts,
            lock: &f.admitted.2,
            subjects: &f.admitted.1,
            read: MirrorArtifactSelection {
                package: "app.root",
                version: "1.0.0",
                path: "module.wasm",
            },
            update_time: 100,
            read_time: 100,
        },
    );
    worker.join().unwrap();
    match outcome {
        Err(MirrorFlowError::Proof(error)) => assert_eq!(error.code, "SPX-PKR622"),
        Ok(_) => panic!("wrong publisher identity reassignment was unexpectedly accepted"),
        Err(other) => {
            panic!("wrong publisher identity reassignment produced the wrong outcome: {other:?}")
        }
    }
    assert_eq!(inventory(&temp), before);
}

#[test]
fn composed_online_path_refuses_a_revoked_yanked_package() {
    let f = Fixture::new(true, 1);
    let (temp, mut store) = install(&f);
    let before = inventory(&temp);
    let mut transport = LoopbackMirrorTransport;
    let outcome = mirror_commit(&f, &mut transport, &mut store);
    match outcome {
        Err(MirrorFlowError::Proof(error)) => assert_eq!(error.code, "SPX-PKR631"),
        Ok(_) => panic!("yanked package selection was unexpectedly accepted"),
        Err(other) => panic!("yanked package selection produced the wrong outcome: {other:?}"),
    }
    assert_eq!(inventory(&temp), before);
}

#[test]
fn composed_online_path_mirror_connection_refused_leaves_the_held_store_unchanged() {
    let f = Fixture::new(false, 1);
    let (temp, mut store) = install(&f);
    let before = inventory(&temp);
    let m = material(
        &f.timestamp,
        &f.snapshot,
        &f.publishers,
        &f.admitted.3,
        &f.admitted.4,
    );
    let metadata = standard_metadata_objects(&m);
    let artifacts = standard_artifacts(&m);
    let publisher_paths = standard_publisher_paths();
    let metadata_paths = MirrorMetadataPaths {
        timestamp_path: "/metadata/timestamp.json",
        snapshot_path: "/metadata/snapshot.json",
        publishers: &publisher_paths,
        registry: &f.admitted.0,
    };
    let origin = refused_origin();
    let authority = MirrorNetworkAuthority::new(
        MirrorOrigin::parse(&origin).unwrap(),
        Duration::from_millis(300),
    )
    .unwrap();
    let mut transport = LoopbackMirrorTransport;
    let outcome = acquire_commit_and_read(
        &authority,
        &mut transport,
        &mut store,
        &MirrorFlowRequest {
            metadata_paths: &metadata_paths,
            metadata_objects: &metadata,
            artifacts: &artifacts,
            lock: &f.admitted.2,
            subjects: &f.admitted.1,
            read: MirrorArtifactSelection {
                package: "app.root",
                version: "1.0.0",
                path: "module.wasm",
            },
            update_time: 100,
            read_time: 100,
        },
    );
    match outcome {
        Err(MirrorFlowError::Acquisition(MirrorError::TransportFailed)) => {}
        Ok(_) => panic!("connection-refused mirror was unexpectedly accepted"),
        Err(other) => panic!("connection-refused mirror produced the wrong outcome: {other:?}"),
    }
    assert_eq!(
        inventory(&temp),
        before,
        "a refused mirror connection must leave no partial ACTIVE effect"
    );
}

#[test]
fn composed_online_path_mirror_timeout_leaves_the_held_store_unchanged() {
    let f = Fixture::new(false, 1);
    let (temp, mut store) = install(&f);
    let before = inventory(&temp);
    let m = material(
        &f.timestamp,
        &f.snapshot,
        &f.publishers,
        &f.admitted.3,
        &f.admitted.4,
    );
    let metadata = standard_metadata_objects(&m);
    let artifacts = standard_artifacts(&m);
    let publisher_paths = standard_publisher_paths();
    let metadata_paths = MirrorMetadataPaths {
        timestamp_path: "/metadata/timestamp.json",
        snapshot_path: "/metadata/snapshot.json",
        publishers: &publisher_paths,
        registry: &f.admitted.0,
    };
    let timeout = Duration::from_millis(200);
    let (origin, worker) = spawn_stalling_server(Duration::from_millis(900));
    let authority =
        MirrorNetworkAuthority::new(MirrorOrigin::parse(&origin).unwrap(), timeout).unwrap();
    let mut transport = LoopbackMirrorTransport;
    let started = Instant::now();
    let outcome = acquire_commit_and_read(
        &authority,
        &mut transport,
        &mut store,
        &MirrorFlowRequest {
            metadata_paths: &metadata_paths,
            metadata_objects: &metadata,
            artifacts: &artifacts,
            lock: &f.admitted.2,
            subjects: &f.admitted.1,
            read: MirrorArtifactSelection {
                package: "app.root",
                version: "1.0.0",
                path: "module.wasm",
            },
            update_time: 100,
            read_time: 100,
        },
    );
    let elapsed = started.elapsed();
    worker.join().unwrap();
    match outcome {
        Err(MirrorFlowError::Acquisition(MirrorError::TransportFailed)) => {}
        Ok(_) => panic!("stalled mirror was unexpectedly accepted"),
        Err(other) => panic!("stalled mirror produced the wrong outcome: {other:?}"),
    }
    assert!(
        elapsed >= Duration::from_millis(150),
        "the read timeout must be a genuine elapsed wait, not an instant fake failure: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(900),
        "the client must not wait for the server's full stall: {elapsed:?}"
    );
    assert_eq!(
        inventory(&temp),
        before,
        "a timed-out mirror must leave no partial ACTIVE effect"
    );
}

#[test]
fn committed_generation_refuses_a_lock_paired_with_different_bytes() {
    let f = Fixture::new(false, 1);
    let temp = Temp::new();
    let mut store = HeldTrustStore::install(&temp.0, &f.root, &f.pin, 90).unwrap();
    let publishers = f.publishers();
    let artifacts = f.artifacts();
    let receipt = store
        .commit_update(&f.update(&publishers, &artifacts))
        .unwrap();
    // The exact committed pairing reads back correctly.
    let read = store
        .read_artifact(&ArtifactRead {
            expected_generation_digest: &receipt.generation_digest,
            lock: &f.admitted.2,
            registry: &f.admitted.0,
            package: "app.root",
            version: "1.0.0",
            path: "module.wasm",
            trusted_time: 100,
        })
        .unwrap();
    assert_eq!(read.bytes(), f.admitted.3.as_slice());
    // The same committed generation and artifact selection, paired with
    // different lock bytes, is refused: a lock cannot be swapped in against
    // artifacts it did not commit alongside.
    let mut different_lock = f.admitted.2.clone();
    different_lock.push('\n');
    code(
        store.read_artifact(&ArtifactRead {
            expected_generation_digest: &receipt.generation_digest,
            lock: &different_lock,
            registry: &f.admitted.0,
            package: "app.root",
            version: "1.0.0",
            path: "module.wasm",
            trusted_time: 100,
        }),
        "SPX-PKR626",
    );
}
