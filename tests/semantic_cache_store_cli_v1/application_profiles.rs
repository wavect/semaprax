//! Real persisted-cache witnesses for the exact application profile fixtures.
use super::{value, Fixture};
use semaprax::project;
use serde_json::Value;

#[path = "../project/standard_library/application_json/views/identifier_fixture.rs"]
mod identifier_fixture;

const IDENTIFIER_MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "application-json"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/schema.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = []

[dependencies]
std.data.json.digits = "=0.1.0"
std.data.json.query = "=0.1.0"
std.data.json.scan = "=0.1.0"
std.data.json.token = "=0.1.0"
std.data.json.write = "=0.1.0"
"#;

fn write(fixture: &Fixture, path: &str, source: &str) {
    let parsed = semaprax::parse(source, path).unwrap();
    std::fs::write(
        fixture.root.join(path),
        semaprax::format::canonical(&parsed),
    )
    .unwrap();
}
fn same_product(cold: &Value, warm: &Value) {
    assert_eq!(cold["project_revision"], warm["project_revision"]);
    assert_eq!(cold["image_revision"], warm["image_revision"]);
    for phase in [
        "full_source_verification",
        "full_HIR_validation",
        "full_cross_file_checks",
        "full_link_and_profile_admission",
    ] {
        assert_eq!(warm["frontend_work"]["work"][phase], true, "{phase}");
    }
}
fn unchanged(fixture: &Fixture, digest: &str) {
    let cold = value(fixture.cold_open());
    let restored = value(fixture.load(digest));
    assert_eq!(restored["work"]["modules_resolved"], 0);
    let closure_count = cold["frontend_work"]["work"]["modules_resolved"]
        .as_u64()
        .unwrap();
    assert!(
        closure_count > 3,
        "actual bundled closure accompanies the three authored files"
    );
    assert_eq!(restored["work"]["checked_HIR_reused"], closure_count);
    let warm = value(fixture.warm_open(digest));
    same_product(&cold, &warm);
    assert_eq!(warm["frontend_work"]["work"]["modules_resolved"], 0);
    assert_eq!(
        warm["frontend_work"]["work"]["checked_HIR_reused"], closure_count,
        "every exact current authored and bundled module is reused"
    );
    assert_eq!(warm["frontend_work"]["manifest_context_reset"], false);
    assert!(
        warm["frontend_work"]["work"]["checked_HIR_reused"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(warm["frontend_work"]["invalidated_sources"]
        .as_array()
        .unwrap()
        .is_empty());
}
fn changed(fixture: &Fixture, digest: &str, path: &str) {
    let warm = value(fixture.warm_open(digest));
    same_product(&value(fixture.cold_open()), &warm);
    assert!(warm["frontend_work"]["invalidated_sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p.as_str() == Some(path)));
    assert!(
        warm["frontend_work"]["work"]["modules_resolved"]
            .as_u64()
            .unwrap()
            > 0
    );
}
fn execute(fixture: &Fixture, expected: i64) {
    project::with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        assert_eq!(
            snapshot
                .execute_entry(&project::ProjectExecutionOptions::default())?
                .outcome(),
            &project::ProjectExecutionOutcome::Returned(expected)
        );
        Ok(())
    })
    .unwrap();
}
fn envelope_hostile(fixture: &Fixture, digest: &str) {
    // The real authenticated store must reject altered schema/header and payload
    // before either can become a restored checked module.
    let hex = digest
        .strip_prefix("sha256:")
        .expect("store digest algorithm");
    assert_eq!(hex.len(), 64);
    assert!(hex
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    let path = fixture.store.join(format!("{hex}.bin"));
    let original = std::fs::read(&path).unwrap();
    assert!(original.len() > 16);
    for index in [0, original.len() - 1] {
        let mut forged = original.clone();
        forged[index] ^= 1;
        std::fs::write(&path, forged).unwrap();
        assert!(
            !fixture.load(digest).status.success(),
            "envelope byte {index}"
        );
        std::fs::write(&path, &original).unwrap();
    }
    assert!(!fixture.load(&"0".repeat(64)).status.success());
    assert!(fixture.load(digest).status.success());
}

#[test]
fn request_views_collision_source_persisted_cache_replays_exact_source_and_rejects_drift() {
    let fixture = Fixture::new();
    std::fs::write(fixture.root.join("semaprax.toml"), IDENTIFIER_MANIFEST).unwrap();
    write(&fixture, "src/schema.spx", identifier_fixture::SCHEMA);
    write(
        &fixture,
        "src/app.spx",
        "module consumer.app; @id(\"consumer.main\") fn main()->i64 {0}",
    );
    write(
        &fixture,
        "src/tests.spx",
        "module consumer.tests; @id(\"consumer.tests.main\") fn main()->i64 {0}",
    );
    let generated =
        project::with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            let generated = project::derive_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "application.request",
                project::JsonCodecProfile::RequestViews,
            )?;
            project::verify_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "application.request",
                &generated,
                project::JsonCodecProfile::RequestViews,
            )?;
            Ok(generated)
        })
        .unwrap();
    write(&fixture, "src/schema.spx", &generated);
    write(&fixture, "src/app.spx", &identifier_fixture::app());
    execute(&fixture, 728); // Exact existing collision/escaped/domain/priority oracle.
    fixture.initialize();
    let receipt = fixture.persist();
    let digest = receipt["entry_digest"].as_str().unwrap();
    unchanged(&fixture, digest);
    envelope_hostile(&fixture, digest);
    let app = std::fs::read_to_string(fixture.root.join("src/app.spx")).unwrap();
    let revised = app.replace("728", "729");
    assert_ne!(revised, app);
    write(&fixture, "src/app.spx", &revised);
    changed(&fixture, digest, "src/app.spx");
    execute(&fixture, 729);
    write(&fixture, "src/app.spx", &app);
    let provider = std::fs::read_to_string(fixture.root.join("src/schema.spx")).unwrap();
    let forged = provider.replace(
        "application.patient.json.identifier-fingerprint",
        "application.patient.json.identifier-fingerprint.foreign",
    );
    assert_ne!(forged, provider);
    write(&fixture, "src/schema.spx", &forged);
    assert!(!fixture.warm_open(digest).status.success());
    assert!(!fixture.cold_open().status.success());
    write(&fixture, "src/schema.spx", &provider);
    unchanged(&fixture, digest);
    execute(&fixture, 728);
}

#[test]
fn omitted_private_layout_source_persisted_cache_reconstructs_provider_layout_and_names() {
    const CALLER: &str = include_str!(
        "../../src/workspace_graph/expected_projection/private_layout/function_caller.spx"
    );
    const PROVIDER: &str = include_str!(
        "../../src/workspace_graph/expected_projection/private_layout/function_provider.spx"
    );
    let fixture = Fixture::new();
    let manifest = IDENTIFIER_MANIFEST
        .replace("consumer.app", "function_layout.app")
        .replace("consumer.tests", "function_layout.tests")
        .replace("src/schema.spx", "src/provider.spx");
    std::fs::write(fixture.root.join("semaprax.toml"), manifest).unwrap();
    write(&fixture, "src/app.spx", CALLER);
    write(&fixture, "src/provider.spx", PROVIDER);
    write(
        &fixture,
        "src/tests.spx",
        "module function_layout.tests; @id(\"layout.tests.main\") fn main()->i64 {0}",
    );
    assert!(!CALLER.contains("use type")); // Exact omitted private descendants.
    execute(&fixture, 42);
    fixture.initialize();
    let receipt = fixture.persist();
    let digest = receipt["entry_digest"].as_str().unwrap();
    unchanged(&fixture, digest);
    envelope_hostile(&fixture, digest);
    let provider = std::fs::read_to_string(fixture.root.join("src/provider.spx")).unwrap();
    let changed_layout = provider.replace("@id(\"config.text\")", "@id(\"config.text.revised\")");
    assert_ne!(changed_layout, provider);
    write(&fixture, "src/provider.spx", &changed_layout);
    changed(&fixture, digest, "src/provider.spx");
    execute(&fixture, 42);
    let foreign = provider.replace("@id(\"make\")", "@id(\"make.foreign\")");
    assert_ne!(foreign, provider);
    write(&fixture, "src/provider.spx", &foreign);
    assert!(!fixture.warm_open(digest).status.success());
    assert!(!fixture.cold_open().status.success());
    write(&fixture, "src/provider.spx", &provider);
    let authored_name =
        format!("{CALLER}\n@id(\"bad.layout\") fn forbidden(value:own Report)->i64 {{0}}");
    write(&fixture, "src/app.spx", &authored_name);
    assert!(!fixture.warm_open(digest).status.success());
    assert!(!fixture.cold_open().status.success());
    write(&fixture, "src/app.spx", CALLER);
    unchanged(&fixture, digest);
    execute(&fixture, 42);
}

#[path = "application_profiles/statement_markers.rs"]
mod statement_markers;

#[test]
#[ignore = "explicit hosted wire audit after the owning persisted-cache shard fails"]
fn private_layout_cold_warm_image_wire_audit() {
    use semaprax::project::{
        ProjectFrontendCache, ProjectFrontendSource, ProjectManifest, ProjectSemanticImage,
    };
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;

    let fixture = Fixture::new();
    let manifest_text = IDENTIFIER_MANIFEST
        .replace("consumer.app", "function_layout.app")
        .replace("consumer.tests", "function_layout.tests")
        .replace("src/schema.spx", "src/provider.spx");
    std::fs::write(fixture.root.join("semaprax.toml"), &manifest_text).unwrap();
    write(
        &fixture,
        "src/app.spx",
        include_str!(
            "../../src/workspace_graph/expected_projection/private_layout/function_caller.spx"
        ),
    );
    write(
        &fixture,
        "src/provider.spx",
        include_str!(
            "../../src/workspace_graph/expected_projection/private_layout/function_provider.spx"
        ),
    );
    write(
        &fixture,
        "src/tests.spx",
        "module function_layout.tests; @id(\"layout.tests.main\") fn main()->i64 {0}",
    );
    let manifest = ProjectManifest::parse(&manifest_text).unwrap();
    let sources = manifest
        .sources()
        .iter()
        .map(|path| {
            ProjectFrontendSource::new(
                path,
                &std::fs::read_to_string(fixture.root.join(path)).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    let cold = cache.build(&manifest, &sources).unwrap();
    let warm = cache.build(&manifest, &sources).unwrap();
    semaprax::semantic_cache_store::initialize(&fixture.store).unwrap();
    let sealed = semaprax::semantic_cache_store::persist(&fixture.store, &cache).unwrap();
    let mut restored =
        semaprax::semantic_cache_store::load(&fixture.store, sealed.entry_digest()).unwrap();
    let restored = restored.build(&manifest, &sources).unwrap();
    let mut records = Vec::new();
    let mut images = Vec::new();
    for (label, build) in [("cold", &cold), ("warm", &warm), ("restored", &restored)] {
        assert_eq!(
            build.revision().project_revision(),
            cold.revision().project_revision()
        );
        let image = ProjectSemanticImage::derive(
            std::sync::Arc::clone(build.revision()),
            build.revision().project_revision(),
        )
        .unwrap();
        for (kind, wire, semantic_digest) in [
            (
                "graph",
                build.revision().semantic_graph(),
                build.revision().semantic_graph_digest(),
            ),
            ("image", image.to_json(), image.image_digest()),
        ] {
            let name = format!("{label}-{kind}.json");
            let bytes = wire.as_bytes();
            let chunks = bytes.chunks(2048);
            records.push(serde_json::json!({
                "name":name,"bytes":bytes.len(),"chunks":chunks.len(),
                "sha256":format!("{:x}",semaprax::digest_hex::LowerHex(Sha256::digest(bytes))),
                "semantic_digest":semantic_digest,
            }));
            for (index, chunk) in chunks.enumerate() {
                let mut hex = String::with_capacity(chunk.len() * 2);
                for byte in chunk {
                    write!(hex, "{byte:02x}").unwrap();
                }
                println!(
                    "SEMAPRAX_CACHE_WIRE_CHUNK_V1 {}",
                    serde_json::json!({
                        "name":name,"index":index,"hex":hex,
                    })
                );
            }
        }
        images.push(image);
    }
    println!(
        "SEMAPRAX_CACHE_WIRE_MANIFEST_V1 {}",
        serde_json::json!({
            "head":std::env::var("GITHUB_SHA").ok(),
            "fixture":"exact omitted-private-layout provider/caller",
            "owner":"public library frontend build and authenticated store in this test executable",
            "compiler_digest":sealed.compiler_digest(),
            "files":records,
            "scope":"diagnostic wire evidence only; no CLI or current-main success claim",
        })
    );
    assert_eq!(images[0].image_digest(), images[1].image_digest());
    assert_eq!(images[0].image_digest(), images[2].image_digest());
}
