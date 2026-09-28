//! Scalar package Agent facts remain descriptive, not a runtime/owned ABI.
#[path = "../workspace/agent_execution_fixture.rs"]
mod fixture;
use semaprax::package_lock_v2::{self, Coordinate};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolver::{self, Requirement, ResolutionInput, ResolutionOptions};
use semaprax::package_semantic_graph::PackageSemanticGraph;
use semaprax::package_source_capsule::{self, PackageSource, SourceCapsuleOptions};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Package {
    root: PathBuf,
    sources: Vec<PackageSource>,
    input: ResolutionInput,
    resolution: ResolutionOptions,
    evidence: String,
    options: SourceCapsuleOptions,
}
impl Package {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-agent-package-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let interface =
            semaprax::format::canonical(&semaprax::check(source, "interface.spx").unwrap());
        let source = source.replace("module fixture.app;", "module fixture.app; use function @id(\"fixture.lib.support\") from fixture.lib as support;")
            .replace("{observe(41)}", "{observe(41) + support()}");
        let source = semaprax::format::canonical(&semaprax::parse(&source, "fixture.spx").unwrap());
        let path = root.join("interface.spx");
        std::fs::write(&path, &interface).unwrap();
        let report =
            package_report_v2::generate(&path, &PackageReportV2Options::default()).unwrap();
        let coordinate = Coordinate {
            package: "fixture.app".into(),
            version: "1.0.0".into(),
        };
        let library_coordinate = Coordinate {
            package: "fixture.lib".into(),
            version: "1.0.0".into(),
        };
        let library_source = semaprax::format::canonical(
            &semaprax::check(
                "module fixture.lib; @id(\"fixture.lib.support\") fn main()->i64 {0}",
                "library.spx",
            )
            .unwrap(),
        );
        let library_path = root.join("library.spx");
        std::fs::write(&library_path, &library_source).unwrap();
        let library_report =
            package_report_v2::generate(&library_path, &PackageReportV2Options::default()).unwrap();
        let library_subject =
            package_lock_v2::create_subject(&library_coordinate, &library_report, &[], &[])
                .unwrap();
        let subject =
            package_lock_v2::create_subject(&coordinate, &report, &[library_coordinate], &[])
                .unwrap();
        let input = ResolutionInput {
            requirements: vec![Requirement {
                package: "fixture.app".into(),
                range: "=1.0.0".into(),
            }],
            subjects: vec![subject, library_subject],
            target: "wasm32".into(),
            allowed_capabilities: vec![],
        };
        let resolution = ResolutionOptions::default();
        let evidence = package_resolver::generate(&input, &resolution).unwrap();
        let options = SourceCapsuleOptions::new("fixture.app".into(), 32 * 1024 * 1024).unwrap();
        Self {
            root,
            sources: vec![
                PackageSource {
                    package: "fixture.app".into(),
                    report,
                    source,
                },
                PackageSource {
                    package: "fixture.lib".into(),
                    report: library_report,
                    source: library_source,
                },
            ],
            input,
            resolution,
            evidence,
            options,
        }
    }
    fn capsule(&self) -> Result<String, Vec<semaprax::diagnostic::Diagnostic>> {
        package_source_capsule::generate(
            &self.sources,
            &self.evidence,
            &self.input,
            &self.resolution,
            &self.options,
        )
    }
}
impl Drop for Package {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn scalar_agent_package_v4_qualifies_checked_rows_over_all_dynamic_bases() {
    for version in 1..=3 {
        let source = fixture::source(true, true);
        let source = if version == 1 {
            source
        } else {
            fixture::protocol(&source, version == 3)
        };
        let package = Package::new(&source);
        let capsule = package.capsule().unwrap();
        package_source_capsule::verify(
            &capsule,
            &package.sources,
            &package.evidence,
            &package.input,
            &package.resolution,
            &package.options,
        )
        .unwrap();
        let graph = PackageSemanticGraph::derive(
            &capsule,
            &package.sources,
            &package.evidence,
            &package.input,
            &package.resolution,
            &package.options,
        )
        .unwrap();
        let graph: Value = serde_json::from_str(graph.to_json()).unwrap();
        assert_eq!(graph["schema"], "semaprax.package-semantic-graph.v4");
        let section = &graph["agent_execution"];
        assert_eq!(
            section["base_schema"],
            format!("semaprax.package-semantic-graph.v{version}")
        );
        assert_eq!(section["authority"], "none");
        let rows = section["agents"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["package"], "fixture.app");
        assert_eq!(rows[0]["version"], "1.0.0");
        assert_eq!(rows[0]["agent"], "fixture.agent");
        assert_eq!(rows[0]["operations"].as_array().unwrap().len(), 4);
        assert_eq!(rows[0]["model_wait"]["helper_id"], "fixture.wait");
        assert!(rows[0]["operations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["origin"] == "embedded"));
        if version >= 2 {
            assert!(graph.get("session_protocols").is_some());
        }
        if version == 3 {
            assert!(graph.get("session_protocol_follows").is_some());
        }
    }
}

#[test]
fn nominal_owned_lifecycle_is_still_refused_by_scalar_package_profile() {
    let scalar = fixture::source(true, true);
    let package = Package::new(&scalar);
    package.capsule().unwrap();
    let admitted = &package.sources[0].source;
    let owner = "\n@id(\"fixture.state\") record State { @id(\"fixture.state.buffer\") buffer:Bytes, }\n@id(\"fixture.owned.identity\") fn retained(value:own State)->State {value}\n";
    let owned = format!("{admitted}{owner}");
    // Resolve a stand-alone copy to demonstrate genuine compiler-owned Bytes
    // cleanup; the package copy retains its exact admitted import edge.
    let local = format!("{scalar}{owner}");
    let checked = semaprax::check(&local, "owned-fixture.spx").unwrap();
    let resolved = semaprax::hir::resolve(&checked).unwrap();
    assert!(resolved
        .types
        .iter()
        .any(|ty| ty.id.as_str() == "fixture.state"));
    assert!(resolved
        .functions
        .iter()
        .any(|function| function.id.as_str() == "fixture.owned.identity"));
    let retained = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "fixture.owned.identity")
        .unwrap();
    assert_eq!(
        retained.params[0].ownership,
        semaprax::hir::OwnershipMode::Own
    );
    assert!(!retained.cleanup_plan.slots.is_empty());
    // Keep the authenticated scalar report; this source extension is profile
    // refused before it can become a package owned-State/runtime ABI.
    let mut sources = package.sources.clone();
    sources[0].source =
        semaprax::format::canonical(&semaprax::parse(&owned, "package-owned.spx").unwrap());
    let error = package_source_capsule::generate(
        &sources,
        &package.evidence,
        &package.input,
        &package.resolution,
        &package.options,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-PS504");
}
