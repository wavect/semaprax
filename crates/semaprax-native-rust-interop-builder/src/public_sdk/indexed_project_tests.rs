//! Held Project sources bind selected imports before linking and publication.

use super::*;
use crate::public_sdk::indexed_project::prepare_project_bindings;
use semaprax::project::{ProjectFrontendCache, ProjectFrontendSource, ProjectManifest};

const MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"indexed\"\nentry = \"interop.fixture\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"interop.add\"]\ntests = [\"interop.tests\"]\n";

fn canonical(source: &str, path: &str) -> String {
    semaprax::format::canonical(&semaprax::parse(source, Path::new(path)).unwrap())
}

#[test]
fn indexed_project_rebinds_graph_and_executes_authenticated_package() {
    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG");
    let actual_version = Command::new(&rustc).arg("--version").output().unwrap();
    let actual_version = std::str::from_utf8(&actual_version.stdout).unwrap().trim();
    let crate_source = b"pub fn add(left:i64,right:i64)->i64{left+right}\n";
    let index = index_for(crate_source, actual_version);
    let replay = RustApiIndex::replay(&index).unwrap();
    let package_digest = raw_digest(crate_source);
    let source = canonical(SOURCE, "src/app.spx");
    let tests = canonical(
        "module interop.tests; @id(\"interop.tests.main\") fn main() -> i64 { 0 }",
        "src/tests.spx",
    );
    let selected = IndexedProjectScalarSelection {
        source_path: "src/app.spx",
        source: &source,
        selection: IndexedScalarSelection {
            import_id: "host.add",
            index_bytes: &index,
            package_source_bytes: crate_source,
            package: SelectedPackage {
                cargo_alias: "fixture_math",
                name: "fixture_math",
                version: "0.0.1",
                source_sha256: &package_digest,
                target: target_triple().unwrap(),
                feature_digest: replay.feature_digest(),
                stable_rustc_version: actual_version,
            },
        },
    };
    let bindings = prepare_project_bindings(&[selected]).unwrap();
    let sources = [
        ProjectFrontendSource::new("src/app.spx", &source).unwrap(),
        ProjectFrontendSource::new("src/tests.spx", &tests).unwrap(),
    ];
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    assert!(
        cache.build(&manifest, &sources).is_err(),
        "ordinary Project loading must not infer index authority"
    );
    let first = cache
        .build_indexed_rust(&manifest, &sources, &bindings)
        .unwrap();
    let graph: Value = serde_json::from_str(first.revision().semantic_graph()).unwrap();
    assert_eq!(graph["schema"], "semaprax.project-semantic-graph.v5");
    let imports = &graph["indexed_rust_imports"]["imports"];
    assert_eq!(imports[0]["id"], "host.add");
    assert_eq!(imports[0]["rust_path"], "fixture_math::add");
    assert_eq!(imports[0]["selected_index_digest"], replay.digest());
    assert_eq!(imports[0]["effects"], serde_json::json!(["host.math"]));
    assert_eq!(imports[0]["failure_domain"], "host.math.v1");
    let first_graph = first.revision().semantic_graph().to_owned();
    let repeated = cache
        .build_indexed_rust(&manifest, &sources, &bindings)
        .unwrap();
    assert_eq!(repeated.revision().semantic_graph(), first_graph);
    let mut drifted = bindings.clone();
    drifted[0].index_digest =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
    let rebound = cache
        .build_indexed_rust(&manifest, &sources, &drifted)
        .unwrap();
    let work: Value = serde_json::from_str(rebound.to_json()).unwrap();
    assert_eq!(work["manifest_context_reset"], true);
    assert_ne!(rebound.revision().semantic_graph(), first_graph);
    drifted[0].signature = "fn add(left: bool, right: i64) -> i64".into();
    assert!(cache
        .build_indexed_rust(&manifest, &sources, &drifted)
        .is_err());
    let changed_sources = [
        ProjectFrontendSource::new("src/app.spx", &(source.clone() + "\n")).unwrap(),
        ProjectFrontendSource::new("src/tests.spx", &tests).unwrap(),
    ];
    assert_eq!(
        cache
            .build_indexed_rust(&manifest, &changed_sources, &bindings)
            .err()
            .unwrap()[0]
            .code,
        "SPX-B142"
    );

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-ri04-indexed-project-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
    std::fs::write(root.join("src/app.spx"), &source).unwrap();
    std::fs::write(root.join("src/tests.spx"), &tests).unwrap();
    let output = root.join("sdk");
    reset_build_observer();
    let bundle =
        build_indexed_project_native_rust_sdk(&root.join("semaprax.toml"), &[selected], &output)
            .unwrap_or_else(|error| {
                panic!(
                    "indexed Project failed: {error:?}, stage {:?}",
                    test_build_snapshot()
                )
            });
    assert_eq!(bundle.output_directory(), output);
    assert_eq!(run_published_sdk(&rustc, &clang, &root, &output), 0);
    assert_eq!(
        std::fs::read_to_string(root.join("src/app.spx")).unwrap(),
        source
    );

    // An independently rebuilt package with changed behavior must fail the
    // exact same native consumer assertion, not succeed on graph metadata.
    let flipped = b"pub fn add(left:i64,right:i64)->i64{left+right+1}\n";
    let flipped_index = index_for(flipped, actual_version);
    let flipped_digest = raw_digest(flipped);
    let flipped_selection = IndexedProjectScalarSelection {
        selection: IndexedScalarSelection {
            index_bytes: &flipped_index,
            package_source_bytes: flipped,
            package: SelectedPackage {
                source_sha256: &flipped_digest,
                ..selected.selection.package
            },
            ..selected.selection
        },
        ..selected
    };
    let flipped_output = root.join("flipped");
    reset_build_observer();
    build_indexed_project_native_rust_sdk(
        &root.join("semaprax.toml"),
        &[flipped_selection],
        &flipped_output,
    )
    .unwrap();
    assert_eq!(
        run_published_sdk(&rustc, &clang, &root, &flipped_output),
        12
    );
    std::fs::write(
        root.join("src/app.spx"),
        source.replace("+ right", "+ left"),
    )
    .unwrap();
    let refused_output = root.join("stale");
    let errors = build_indexed_project_native_rust_sdk(
        &root.join("semaprax.toml"),
        &[selected],
        &refused_output,
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "SPX-B142");
    assert!(!refused_output.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn indexed_real_regex_project_generates_and_executes_locked_offline_owner_loan() {
    let cargo = std::env::var("CARGO").expect("Cargo supplies its executable");
    assert!(Path::new(&cargo).is_absolute());
    let index_bytes = include_bytes!(
        "../../../semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json"
    );
    let index = RustApiIndex::admit_extractor_output(index_bytes).unwrap();
    let index_bytes = index.canonical_json().as_bytes();
    let source = canonical(
        r#"module regex.fixture;
@id("regex.resource") resource Regex { @id("regex.resource.drop") drop import "regex.drop"; }
@id("regex.host") interface Host permits { } {
 @id("regex.drop") import fn drop_regex(regex: own Regex) -> unit effects { } failure infallible consumes regex always;
 @id("regex.new") import rust selected fn regex_new from "regex_alias::Regex::new" effects { } failure infallible;
 @id("regex.match") import rust selected fn regex_match from "regex_alias::Regex::is_match" effects { } failure infallible;
}
@id("regex.run") fn run() -> i64 {
 let pattern = "example";
 let input = "https://example.invalid/path";
 let created = regex_new(pattern);
 match borrow created {
  Result::Ok { value: owner } => if regex_match(owner, input) { 41 } else { 7 },
  Result::Err { error: error } => 9,
 }
}
@id("regex.main") fn main() -> i64 { 0 }
"#,
        "src/app.spx",
    );
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-indexed-real-regex-project-{}",
            std::process::id()
        ));
    struct Cleanup {
        root: std::path::PathBuf,
        target: std::path::PathBuf,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
            let _ = std::fs::remove_dir_all(&self.target);
        }
    }
    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("target")
        .join(format!("ri06-registry-project-{}", std::process::id()));
    let _cleanup = Cleanup {
        root: root.clone(),
        target: target.clone(),
    };
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/app.spx"), &source).unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        canonical(
            "module regex.tests; @id(\"regex.tests.main\") fn main() -> i64 { 0 }",
            "src/tests.spx",
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"regex\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"regex.fixture\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"regex.tests\"]\n\n[exports]\nweb = [\"regex.run\"]\n\n[rust-dependencies]\nregex = [\"=1.13.1\"]\n",
    )
    .unwrap();
    let package = SelectedPackage {
        cargo_alias: "regex_alias",
        name: "regex",
        version: "1.13.1",
        source_sha256: index.package().source_sha256.as_str(),
        target: index.target(),
        feature_digest: index.feature_digest(),
        stable_rustc_version: index.stable_rustc_version(),
    };
    let selected =
        ["regex.new", "regex.match"].map(|import_id| IndexedProjectRegexRegistrySelection {
            source_path: "src/app.spx",
            source: &source,
            import_id,
            index_bytes,
            package,
        });
    let lock =
        include_bytes!("../../../semaprax-toolchain/src/fixtures/ri06-regex-1.13.1.Cargo.lock");
    let plan = prepare_indexed_regex_project_package(&root.join("semaprax.toml"), &selected, lock)
        .unwrap();
    assert_eq!(plan.cargo_lock(), lock);
    let sdk = root.join("sdk");
    std::fs::create_dir_all(sdk.join("src")).unwrap();
    for (name, bytes) in [
        ("Cargo.toml", plan.cargo_toml()),
        ("Cargo.lock", plan.cargo_lock()),
        ("src/lib.rs", plan.lib_rs()),
        ("src/regex_project.c", plan.c_source()),
        ("src/regex_project.h", plan.header()),
        ("binding-plan.json", plan.binding_plan()),
        ("descriptor.json", plan.descriptor()),
    ] {
        std::fs::write(sdk.join(name), bytes).unwrap();
    }
    std::fs::write(
        sdk.join("src/main.rs"),
        r#"fn main() {
    assert_eq!(ri06_regex_owner::run(), Ok(41));
    assert!(ri06_regex_owner::projected_borrow_matches_target());
    assert_eq!(ri06_regex_owner::spx_result_owner_adapter_copies(), 0);
    assert_eq!(ri06_regex_owner::string_constructions(), 2);
    assert_eq!(ri06_regex_owner::live_string_count(), 0);
    assert_eq!(ri06_regex_owner::live_owner_count(), 0);
}
"#,
    )
    .unwrap();
    let object = sdk.join("regex_project.o");
    let clang = std::env::var("CLANG").expect("explicit CLANG for native Regex Project");
    let compiled = Command::new(&clang)
        .args(["-std=c11", "-O0", "-c"])
        .arg(sdk.join("src/regex_project.c"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let run = Command::new(&cargo)
        .args(["run", "--locked", "--offline", "--quiet"])
        .current_dir(&sdk)
        .env("CARGO_TARGET_DIR", &target)
        .env("RUSTFLAGS", format!("-C link-arg={}", object.display()))
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_BUILD_JOBS", "1")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    // A canonical authored-body change must alter executable output; the SDK
    // cannot satisfy this gate by running a source-independent Rust facade.
    let flipped = source.replace("{ 41 }", "{ 42 }");
    assert_ne!(flipped, source);
    std::fs::write(root.join("src/app.spx"), &flipped).unwrap();
    let flipped_selection =
        ["regex.new", "regex.match"].map(|import_id| IndexedProjectRegexRegistrySelection {
            source_path: "src/app.spx",
            source: &flipped,
            import_id,
            index_bytes,
            package,
        });
    let changed = prepare_indexed_regex_project_package(
        &root.join("semaprax.toml"),
        &flipped_selection,
        lock,
    )
    .unwrap();
    assert_ne!(changed.c_source(), plan.c_source());
    std::fs::write(sdk.join("src/regex_project.c"), changed.c_source()).unwrap();
    let compiled = Command::new(&clang)
        .args(["-std=c11", "-O2", "-c"])
        .arg(sdk.join("src/regex_project.c"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::fs::write(sdk.join("src/lib.rs"), changed.lib_rs()).unwrap();
    std::fs::write(sdk.join("src/main.rs"), "fn main(){assert_eq!(ri06_regex_owner::run(),Ok(42));assert_eq!(ri06_regex_owner::live_owner_count(),0);}").unwrap();
    let changed_run = Command::new(&cargo)
        .args(["run", "--locked", "--offline", "--quiet"])
        .current_dir(&sdk)
        .env("CARGO_TARGET_DIR", &target)
        .env("RUSTFLAGS", format!("-C link-arg={}", object.display()))
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .output()
        .unwrap();
    assert!(
        changed_run.status.success(),
        "{}",
        String::from_utf8_lossy(&changed_run.stderr)
    );
    let domain_source = source.replace("\"example\"", "\"(\"");
    assert_ne!(domain_source, source);
    std::fs::write(root.join("src/app.spx"), &domain_source).unwrap();
    let domain_selection =
        ["regex.new", "regex.match"].map(|import_id| IndexedProjectRegexRegistrySelection {
            source_path: "src/app.spx",
            source: &domain_source,
            import_id,
            index_bytes,
            package,
        });
    let domain =
        prepare_indexed_regex_project_package(&root.join("semaprax.toml"), &domain_selection, lock)
            .unwrap();
    std::fs::write(sdk.join("src/regex_project.c"), domain.c_source()).unwrap();
    let compiled = Command::new(&clang)
        .args(["-std=c11", "-O2", "-c"])
        .arg(sdk.join("src/regex_project.c"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::fs::write(sdk.join("src/main.rs"), "fn main(){assert_eq!(ri06_regex_owner::run(),Ok(9));assert_eq!(ri06_regex_owner::live_owner_count(),0);}").unwrap();
    let domain_run = Command::new(&cargo)
        .args(["run", "--locked", "--offline", "--quiet"])
        .current_dir(&sdk)
        .env("CARGO_TARGET_DIR", &target)
        .env("RUSTFLAGS", format!("-C link-arg={}", object.display()))
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .output()
        .unwrap();
    assert!(
        domain_run.status.success(),
        "{}",
        String::from_utf8_lossy(&domain_run.stderr)
    );
    // Removing a canonical finalizer must fail the unchanged SDK consumer.
    let c = std::str::from_utf8(changed.c_source()).unwrap();
    let needle = "int32_t dropped=spx_result_owner_drop(context,f->owners[";
    assert!(c.contains(needle));
    let lines = c
        .lines()
        .map(|line| {
            if line.contains(needle) {
                let start = line.find("int32_t dropped=").unwrap();
                let end = start + line[start..].find(';').unwrap() + 1;
                format!("{}int32_t dropped=0;{}", &line[..start], &line[end..])
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(sdk.join("src/regex_project.c"), lines).unwrap();
    let compiled = Command::new(&clang)
        .args(["-std=c11", "-O2", "-c"])
        .arg(sdk.join("src/regex_project.c"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::fs::write(sdk.join("src/main.rs"), "fn main(){assert_eq!(ri06_regex_owner::run(),Ok(42));assert_eq!(ri06_regex_owner::live_owner_count(),0);} // finalizer control").unwrap();
    let mutant = Command::new(&cargo)
        .args(["run", "--locked", "--offline", "--quiet"])
        .current_dir(&sdk)
        .env("CARGO_TARGET_DIR", &target)
        .env("RUSTFLAGS", format!("-C link-arg={}", object.display()))
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .output()
        .unwrap();
    assert!(!mutant.status.success());
    assert!(String::from_utf8_lossy(&mutant.stderr).contains("Err(5)"));
    std::fs::write(root.join("src/app.spx"), &source).unwrap();
    let corrupt_lock = std::str::from_utf8(lock)
        .unwrap()
        .replace(
            index
                .package()
                .source_sha256
                .strip_prefix("sha256:")
                .unwrap(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .into_bytes();
    assert!(prepare_indexed_regex_project_package(
        &root.join("semaprax.toml"),
        &selected,
        &corrupt_lock,
    )
    .is_err());
    std::fs::write(
        root.join("src/app.spx"),
        source.replace("fn run()", "fn run_changed()"),
    )
    .unwrap();
    assert!(
        prepare_indexed_regex_project_package(&root.join("semaprax.toml"), &selected, lock,)
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(target).unwrap();
}

#[test]
fn guarded_indexed_project_sdk_checks_physical_return_before_semantic_publication() {
    use semaprax::native_rust_binding::foreign_law::{DeclaredForeignSummary, ForeignLawRequest};

    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG");
    for tool in [&rustc, &clang] {
        assert!(Path::new(tool).is_absolute());
    }
    let version = Command::new(&rustc).arg("--version").output().unwrap();
    assert!(version.status.success());
    let version = std::str::from_utf8(&version.stdout).unwrap().trim();
    let crate_source = b"pub fn add(left:i64,right:i64)->i64{left+right}\n";
    let index = index_for(crate_source, version);
    let replay = RustApiIndex::replay(&index).unwrap();
    let package_digest = raw_digest(crate_source);
    let source = canonical(
        &SOURCE.replace("host_add(left, right) + right", "host_add(left, right)"),
        "src/app.spx",
    );
    let tests = canonical(
        "module interop.tests; @id(\"interop.tests.main\") fn main() -> i64 { 0 }",
        "src/tests.spx",
    );
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!("semaprax-law09-physical-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
    std::fs::write(root.join("src/app.spx"), &source).unwrap();
    std::fs::write(root.join("src/tests.spx"), &tests).unwrap();
    let selected = IndexedProjectScalarSelection {
        source_path: "src/app.spx",
        source: &source,
        selection: IndexedScalarSelection {
            import_id: "host.add",
            index_bytes: &index,
            package_source_bytes: crate_source,
            package: SelectedPackage {
                cargo_alias: "fixture_math",
                name: "fixture_math",
                version: "0.0.1",
                source_sha256: &package_digest,
                target: target_triple().unwrap(),
                feature_digest: replay.feature_digest(),
                stable_rustc_version: version,
            },
        },
    };
    let declared = DeclaredForeignSummary {
        assumption_id: "law09.add.behavior".into(),
        proposition_digest: raw_digest(b"fixture add has no hidden behavior"),
        assumes_no_effects: true,
        assumes_no_callbacks: true,
        assumes_no_panics: true,
        assumes_no_shared_state: true,
        return_i64_range: Some((0, 50)),
    };
    let law = ForeignLawRequest {
        law_id: "law09.add.range".into(),
        permit_assumptions: true,
        require_theorem: false,
        require_no_effects: true,
        require_no_callbacks: true,
        require_no_panics: true,
        require_no_shared_state: true,
        require_return_guard: true,
    };
    let guard = GuardedForeignLawSelection {
        import_id: "host.add",
        declared: &declared,
        law: &law,
    };
    let output = root.join("sdk");
    reset_build_observer();
    let (bundle, frontier) = build_guarded_indexed_project_native_rust_sdk(
        &root.join("semaprax.toml"),
        &[selected],
        guard,
        &output,
    )
    .unwrap_or_else(|error| {
        panic!(
            "guarded SDK failed: {error:?}; stage {:?}",
            test_build_snapshot()
        )
    });
    let report: Value = serde_json::from_str(&frontier.public_view()).unwrap();
    assert_eq!(report["adapter_digest"], bundle.manifest_digest());
    assert_eq!(report["conditions"][0], "law09.add.behavior:no_effects");
    assert_eq!(report["foreign_internals_proved"], false);
    let generated = std::fs::read_to_string(output.join("src/lib.rs")).unwrap();
    assert!(generated.contains("SEMAPRAX_FOREIGN_RETURN_GUARD"));
    assert!(generated.contains("NonZeroU32::new(40909)"));
    let bindings = prepare_project_bindings(&[selected]).unwrap();
    let (evidence, revision) = semaprax::project::with_authenticated_indexed_rust_project(
        &root.join("semaprax.toml"),
        &bindings,
        |snapshot| {
            let revision = snapshot.retain_revision();
            let import = revision
                .entry_program()
                .interfaces
                .iter()
                .flat_map(|interface| &interface.imports)
                .find(|import| import.id.as_str() == "host.add")
                .unwrap();
            let plan = crate::indexed_binding::prepare_indexed_scalar_binding(
                import,
                &index,
                selected.selection.package,
                import.rust_path.as_deref().unwrap(),
            )
            .map_err(|error| vec![error])?;
            let caller = revision.foreign_caller_certificate(
                "interop.add",
                &plan,
                plan.target.as_str(),
                bundle.manifest_digest(),
                &declared,
                &law,
            )?;
            let forged = revision.foreign_caller_certificate(
                "interop.add",
                &plan,
                plan.target.as_str(),
                "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                &declared,
                &law,
            )?;
            assert_eq!(
                forged
                    .verify_published_guard(&revision, &output, bundle.manifest_digest())
                    .unwrap_err()[0]
                    .code,
                "SPX-FL310"
            );
            caller.verify_published_guard(&revision, &output, bundle.manifest_digest())?;
            let view: Value = serde_json::from_str(&caller.public_view()).unwrap();
            assert_eq!(view["source_route_proved"], true);
            assert_eq!(view["foreign_internals_proved"], false);
            assert_eq!(caller.conditions().len(), 4);
            let mut changed = declared.clone();
            changed.proposition_digest =
                "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into();
            let wrong_summary = revision.foreign_caller_certificate(
                "interop.add",
                &plan,
                plan.target.as_str(),
                bundle.manifest_digest(),
                &changed,
                &law,
            )?;
            assert_eq!(
                bundle
                    .bind_guarded_foreign_caller(&revision, wrong_summary)
                    .unwrap_err()[0]
                    .code,
                "SPX-FL311"
            );
            let evidence = bundle.bind_guarded_foreign_caller(&revision, caller)?;
            evidence.replay(&revision)?;
            assert_eq!(evidence.manifest_digest(), bundle.manifest_digest());
            assert_eq!(evidence.caller().conditions().len(), 4);
            Ok((evidence, revision))
        },
    )
    .unwrap();
    assert_eq!(
        run_published_sdk_with_consumer(
            &rustc,
            &clang,
            &root,
            &output,
            r#"fn main(){
let mut sdk=indexed_sdk::indexed_scalar_sdk(&["host.math"]).unwrap();
assert_eq!(sdk.spx_interop_dot_add(20,22),Ok(42));
match sdk.spx_interop_dot_add(1000,22){
 Err(indexed_sdk::NativeRustSdkCallError::Semantic{domain_id,code,class,retryable})
  if domain_id=="host.math.v1" && code.get()==40909
   && class==indexed_sdk::NativeRustSdkStatusClass::Import && !retryable=>{},
 other=>panic!("foreign return escaped or wrong status: {other:?}"),
}
}"#,
        ),
        0,
    );
    let wrong = GuardedForeignLawSelection {
        import_id: "host.other",
        ..guard
    };
    let absent = root.join("wrong");
    assert!(build_guarded_indexed_project_native_rust_sdk(
        &root.join("semaprax.toml"),
        &[selected],
        wrong,
        &absent,
    )
    .is_err());
    assert!(!absent.exists());
    std::fs::write(
        output.join("src/lib.rs"),
        generated.replace(
            "SEMAPRAX_FOREIGN_RETURN_GUARD",
            "SEMAPRAX_FOREIGN_RETURN_GUARD_DRIFT",
        ),
    )
    .unwrap();
    assert_eq!(evidence.replay(&revision).unwrap_err()[0].code, "SPX-FL310");
    std::fs::remove_dir_all(root).unwrap();
}
