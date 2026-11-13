//! Build each real application's existing generator route; never edit its logic.
use crate::hir::ResolvedProgram;
use crate::project::{self, JsonCodecPlanEntry, JsonCodecProfile};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn copy_directory(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &target.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}
fn digest(root: &Path) -> String {
    let mut paths = vec![root.join("semaprax.toml")];
    paths.extend(
        std::fs::read_dir(root.join("src"))
            .unwrap()
            .map(|e| e.unwrap().path()),
    );
    paths.sort_unstable();
    let mut hash = Sha256::new();
    for path in paths {
        assert!(
            path.is_file(),
            "application source directory must stay flat"
        );
        let name = path
            .strip_prefix(root)
            .unwrap()
            .to_str()
            .unwrap()
            .as_bytes();
        let bytes = std::fs::read(&path).unwrap();
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    format!("{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}
fn generate(root: &Path, prefix: &str) -> Result<(), Vec<crate::diagnostic::Diagnostic>> {
    let entries = [
        JsonCodecPlanEntry {
            source_path: "src/request.spx".to_owned(),
            record_id: format!("{prefix}.request"),
            profile: if prefix == "catalog" {
                JsonCodecProfile::StreamUtf8OwnedRequest {
                    max_string_bytes: 16,
                }
            } else {
                JsonCodecProfile::StreamOwnedRequest
            },
        },
        JsonCodecPlanEntry {
            source_path: "src/response.spx".to_owned(),
            record_id: format!("{prefix}.report"),
            profile: JsonCodecProfile::CollectionResponse {
                max_string_bytes: 16,
            },
        },
    ];
    let bundle = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let bundle = project::derive_json_codec_plan(&revision, &entries)?;
        project::verify_json_codec_plan(&revision, &entries, &bundle)?;
        Ok(bundle)
    })?;
    for entry in &bundle.entries {
        let candidate = bundle
            .candidate_sources
            .iter()
            .find(|s| s.path == entry.source_path)
            .unwrap();
        std::fs::write(root.join(&candidate.path), &candidate.source).unwrap();
    }
    // The actual command body used by each owning corpus, without a source
    // workaround, weaker profile, or altered constants.
    std::fs::copy(root.join("src/app.command.spx"), root.join("src/app.spx")).unwrap();
    Ok(())
}
pub(super) fn run(report: impl Fn(&str, &str, &ResolvedProgram)) {
    assert_eq!(crate::workspace_graph::MAX_BUILDER_BYTES, 67_108_864);
    assert_eq!(
        project::ProjectExecutionOptions::default().max_steps,
        1_000_000
    );
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (name, prefix) in [
        ("catalog-scoped-record-successor", Some("catalog")),
        ("shiftsim-typed-record-successor", Some("shiftsim")),
        ("catalog-normalizer-project", None),
    ] {
        let original = repository.join("examples").join(name);
        let original_digest = digest(&original);
        let scratch = Scratch(std::env::temp_dir().join(format!(
            "semaprax-replay-measure-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        copy_directory(&original.join("src"), &scratch.0.join("src"));
        std::fs::copy(
            original.join("semaprax.toml"),
            scratch.0.join("semaprax.toml"),
        )
        .unwrap();
        let (result, preparation_wall_ns) =
            crate::workspace_graph::forecast_observation::observe(name, || {
                let preparation = Instant::now();
                let generated = prefix.map_or(Ok(()), |p| generate(&scratch.0, p));
                let result = generated.and_then(|()| {
                    project::with_authenticated_project(
                        &scratch.0.join("semaprax.toml"),
                        |snapshot| {
                            // Full ordinary project authentication precedes measurement.
                            crate::hir::validate(snapshot.public_api_program())
                                .map_err(|e| vec![e])?;
                            Ok(snapshot.retain_revision())
                        },
                    )
                });
                (
                    result,
                    u64::try_from(preparation.elapsed().as_nanos()).unwrap(),
                )
            });
        let effective_digest = digest(&scratch.0);
        eprintln!(
            "{}",
            serde_json::json!({
                "schema":"semaprax.replay-constructor-preparation.v1","application":name,
                "original_source_sha256":original_digest,"effective_source_sha256":effective_digest,
                "preparation_wall_ns":preparation_wall_ns,
                "preparation_scope":"unchanged generator derive/replay, full project and HIR validation; file copy and component serialization excluded",
                "admission":match &result {Ok(_)=>"accepted",Err(_)=>"refused"},
                "diagnostics":result.as_ref().err().map(|errors| errors.iter().map(|e|
                    format!("{}: {}",e.code,e.message)).collect::<Vec<_>>()),
                "application_execution":"not performed",
                "builder_limit":crate::workspace_graph::MAX_BUILDER_BYTES,
                "ordinary_execution_fuel":project::ProjectExecutionOptions::default().max_steps,
                "receiving_route":"fresh scratch with_authenticated_project; no retained frontend supplied",
                "forecast_interpretation":"completed module components are not global execution peaks; incomplete final passes report null totals",
                "fixed_context":"source/snapshot and observer initialization excluded from paired constructor samples"
            })
        );
        assert_eq!(
            digest(&original),
            original_digest,
            "measurement must not rewrite original application"
        );
        if let Ok(revision) = result {
            report(name, &effective_digest, revision.public_api_program());
        }
    }
}
