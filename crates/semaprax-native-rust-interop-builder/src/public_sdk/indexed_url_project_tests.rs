//! Physical checked-body Url Project and returned-view lifetime controls.
use super::*;
fn canonical(source: &str, path: &str) -> String {
    semaprax::format::canonical(&semaprax::parse(source, Path::new(path)).unwrap())
}
// The committed real-registry index is extracted for aarch64-apple-darwin.
// The aarch64 macOS CI job runs this physical owner-bound view gate.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn indexed_real_url_project_executes_receiver_tied_view_and_cleanup() {
    let cargo = std::env::var("CARGO").expect("Cargo supplies its executable");
    assert!(Path::new(&cargo).is_absolute());
    let index_bytes =
        include_bytes!("../../../semaprax-rust-api-index/fixtures/url-2.5.8-index-envelope.json");
    let index = RustApiIndex::admit_extractor_output(index_bytes).unwrap();
    let index_bytes = index.canonical_json().as_bytes();
    let index = RustApiIndex::replay(index_bytes).unwrap();
    let source = canonical(
        r#"module url.fixture;
@id("url.resource") resource Url { @id("url.resource.drop") drop import "url.drop"; }
@id("url.host") interface Host permits { } {
 @id("url.drop") import fn drop_url(url: own Url) -> unit effects { } failure infallible consumes url always;
 @id("url.new") import rust selected fn url_new from "url_alias::Url::parse" effects { } failure infallible;
 @id("url.view") import rust selected fn url_view from "url_alias::Url::as_str" effects { } failure infallible;
}
@id("url.run") fn run() -> i64 {
 let input = "https://example.invalid/path";
 let created = url_new(input);
 match borrow created {
  Result::Ok { value: owner } => {
   let view = url_view(owner);
   let bytes = str_as_bytes(view);
   if byte_len(bytes) == 28usize { 41 } else { 7 }
  },
  Result::Err { error: error } => 9,
 }
}
@id("url.main") fn main() -> i64 { 0 }
"#,
        "src/app.spx",
    );
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-indexed-real-url-project-{}",
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
            "module url.tests; @id(\"url.tests.main\") fn main() -> i64 { 0 }",
            "src/tests.spx",
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"url\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"url.fixture\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"url.tests\"]\n\n[exports]\nweb = [\"url.run\"]\n\n[rust-dependencies]\nurl = [\"=2.5.8\"]\n",
    )
    .unwrap();
    let package = SelectedPackage {
        cargo_alias: "url_alias",
        name: "url",
        version: "2.5.8",
        source_sha256: index.package().source_sha256.as_str(),
        target: index.target(),
        feature_digest: index.feature_digest(),
        stable_rustc_version: index.stable_rustc_version(),
    };
    let selected = ["url.new", "url.view"].map(|import_id| IndexedProjectUrlRegistrySelection {
        source_path: "src/app.spx",
        source: &source,
        import_id,
        index_bytes,
        package,
    });
    let test_source = std::fs::read_to_string(root.join("src/tests.spx")).unwrap();
    let manifest = semaprax::project::ProjectManifest::parse(
        &std::fs::read_to_string(root.join("semaprax.toml")).unwrap(),
    )
    .unwrap();
    let bindings = [
        (
            "url.new",
            "url_alias::Url::parse",
            "url::Url::parse",
            "none",
        ),
        (
            "url.view",
            "url_alias::Url::as_str",
            "url::Url::as_str",
            "shared",
        ),
    ]
    .map(
        |(id, path, index_path, receiver)| semaprax::project::ProjectIndexedRustImport {
            source_path: "src/app.spx".into(),
            source_sha256: raw_digest(source.as_bytes()),
            import_id: id.into(),
            rust_path: path.into(),
            signature: index
                .select_closed_url_method(index_path)
                .unwrap()
                .signature
                .clone(),
            index_digest: index.digest().into(),
            receiver: receiver.into(),
        },
    );
    let mut cache = semaprax::project::ProjectFrontendCache::new_with_semantic_cache();
    let inputs = [
        semaprax::project::ProjectFrontendSource::new("src/app.spx", &source).unwrap(),
        semaprax::project::ProjectFrontendSource::new("src/tests.spx", &test_source).unwrap(),
    ];
    let built = cache
        .build_indexed_rust(&manifest, &inputs, &bindings)
        .unwrap();
    let graph: serde_json::Value = serde_json::from_str(built.revision().semantic_graph()).unwrap();
    assert_eq!(graph["schema"], "semaprax.project-semantic-graph.v7");
    let view = graph["indexed_rust_imports"]["imports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|import| import["id"] == "url.view")
        .unwrap();
    assert_eq!(
        view["borrowed_from"],
        serde_json::json!({"parameter":0,"resource":"url.resource"})
    );
    assert_eq!(canonical(&source, "src/app.spx"), source);
    let lock = include_bytes!("../../../semaprax-toolchain/src/fixtures/ri06-url-2.5.8.Cargo.lock");
    let plan =
        prepare_indexed_url_project_package(&root.join("semaprax.toml"), &selected, lock).unwrap();
    assert_eq!(plan.cargo_lock(), lock);
    assert!(std::str::from_utf8(plan.binding_plan())
        .unwrap()
        .contains("url_alias::Url::as_str"));
    let sdk = root.join("sdk");
    std::fs::create_dir_all(sdk.join("src")).unwrap();
    for (name, bytes) in [
        ("Cargo.toml", plan.cargo_toml()),
        ("Cargo.lock", plan.cargo_lock()),
        ("src/lib.rs", plan.lib_rs()),
        ("src/url_project.c", plan.c_source()),
        ("src/url_project.h", plan.header()),
        ("binding-plan.json", plan.binding_plan()),
        ("descriptor.json", plan.descriptor()),
    ] {
        std::fs::write(sdk.join(name), bytes).unwrap();
    }
    let mut generated = std::str::from_utf8(plan.lib_rs()).unwrap().to_owned();
    generated.push_str(include_str!("url_project_controls.rs.txt"));
    generated.push_str(include_str!("url_project_callback_controls.rs.txt"));
    generated.push_str(include_str!("url_project_carrier_corpus.rs.txt"));
    std::fs::write(sdk.join("src/lib.rs"), &generated).unwrap();
    std::fs::write(
        sdk.join("src/main.rs"),
        r#"fn main() {
    ri06_url_owner::assert_view_controls();
    ri06_url_owner::assert_exclusive_callback_controls();
    ri06_url_owner::assert_carrier_mutation_corpus();
    assert_eq!(ri06_url_owner::run(), Ok(41));
    assert!(ri06_url_owner::projected_borrow_matches_target());
    assert_eq!(ri06_url_owner::adapter_copy_count(), 0);
    assert_eq!(ri06_url_owner::adapter_copied_bytes(), 0);
    assert_eq!(ri06_url_owner::string_constructions(), 1);
    assert_eq!(ri06_url_owner::live_string_count(), 0);
    assert_eq!(ri06_url_owner::live_owner_count(), 0);
    assert_eq!(ri06_url_owner::live_view_count(), 0);
    let batch = ri06_url_owner::run_batch(32).unwrap();
    assert_eq!(batch.operations, 32);
    assert_eq!(batch.checksum, 1312);
    assert_eq!(batch.borrowed_input_bytes, 896);
    assert_eq!(batch.adapter_copy_events, 0);
    assert_eq!(batch.adapter_copied_bytes, 0);
    assert_eq!(batch.live_owner_count, 0);
    assert_eq!(batch.live_view_count, 0);
    assert_eq!(batch.live_string_count, 0);
    assert_eq!(ri06_url_owner::run_batch(0), Err(4));
    assert_eq!(ri06_url_owner::run_batch(4097), Err(4));
}
"#,
    )
    .unwrap();
    // The C relay makes the exclusive Rust loan cross an actual foreign
    // callback boundary before the Rust callback attempts same-owner re-entry.
    let relay = r#"
#include <stdint.h>
typedef struct { uint64_t context, generation, slot; } relay_owner;
typedef int32_t (*relay_callback)(uint64_t, relay_owner, void *);
int32_t ri06_url_callback_relay(uint64_t context, relay_owner owner, void *state, relay_callback callback) {
    return callback(context, owner, state);
}
"#;
    std::fs::write(
        sdk.join("src/url_project.c"),
        format!("{}{}", std::str::from_utf8(plan.c_source()).unwrap(), relay),
    )
    .unwrap();
    let object = sdk.join("url_project.o");
    let clang = std::env::var("CLANG").expect("explicit CLANG for native Url Project");
    let compiled = Command::new(&clang)
        .args(["-std=c11", "-O0", "-c"])
        .arg(sdk.join("src/url_project.c"))
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
    url_safety::run_native_sanitizers(&cargo, &clang, &sdk, &target, &object);
    // Cross-crate rustc controls authenticate the generated HRTB boundary.
    let main_source = std::fs::read_to_string(sdk.join("src/main.rs")).unwrap();
    for (name, body, expected) in [
        ("exclusive reference escape", "let escaped=ri06_url_owner::with_exclusive_url(c,o,|value|value).unwrap();drop(escaped);", "lifetime may not live long enough"),
        ("stored exclusive escape", "let mut saved=None;ri06_url_owner::with_exclusive_url(c,o,|value|saved=Some(value)).unwrap();drop(saved);", "E0521"),
        ("view then mutation", "ri06_url_owner::with_exclusive_url(c,o,|value|{let view=value.as_str();value.set_path(\"changed\");view.len()}).unwrap();", "E0502"),
        ("async escape", "let _future=ri06_url_owner::with_exclusive_url(c,o,|value|async move {value.as_str().len()}).unwrap();", "lifetime may not live long enough"),
    ] {
        std::fs::write(sdk.join("src/main.rs"), format!("fn refuse(c:u64,o:ri06_url_owner::SpxOwner){{{body}}}fn main(){{}}\n")).unwrap();
        let negative = Command::new(&cargo).args(["check", "--locked", "--offline", "--quiet"])
            .current_dir(&sdk).env("CARGO_TARGET_DIR", &target)
            .env("RUSTFLAGS", format!("-C link-arg={}", object.display()))
            .env("CARGO_INCREMENTAL", "0").env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_BUILD_JOBS", "1").output().unwrap();
        let errors = String::from_utf8_lossy(&negative.stderr);
        assert!(!negative.status.success() && errors.contains(expected), "{name}: {errors}");
    }
    std::fs::write(sdk.join("src/main.rs"), main_source).unwrap();
    // These deliberate guard removals must fail the unchanged consumer.
    for needle in [
        "if slot.view.is_some() { return Err(6); }",
        "if owner.context != context { return Err(3); }",
        "if slot.exclusive { return Err(6); }",
    ] {
        assert!(generated.contains(needle));
        let mutant = generated.replacen(needle, "", 1);
        std::fs::write(sdk.join("src/lib.rs"), mutant).unwrap();
        let failed = Command::new(&cargo)
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
            !failed.status.success(),
            "guard removal unexpectedly passed: {needle}"
        );
        assert!(
            String::from_utf8_lossy(&failed.stderr).contains("panicked"),
            "control must fail at runtime, not compilation: {}",
            String::from_utf8_lossy(&failed.stderr)
        );
    }
    std::fs::write(sdk.join("src/lib.rs"), &generated).unwrap();
    // A canonical authored-body change must alter executable output; the SDK
    // cannot satisfy this gate by running a source-independent Rust facade.
    let flipped = source.replace("{ 41 }", "{ 42 }");
    assert_ne!(flipped, source);
    std::fs::write(root.join("src/app.spx"), &flipped).unwrap();
    let flipped_selection =
        ["url.new", "url.view"].map(|import_id| IndexedProjectUrlRegistrySelection {
            source_path: "src/app.spx",
            source: &flipped,
            import_id,
            index_bytes,
            package,
        });
    let changed =
        prepare_indexed_url_project_package(&root.join("semaprax.toml"), &flipped_selection, lock)
            .unwrap();
    assert_ne!(changed.c_source(), plan.c_source());
    std::fs::write(sdk.join("src/url_project.c"), changed.c_source()).unwrap();
    let compiled = Command::new(&clang)
        .args(["-std=c11", "-O2", "-c"])
        .arg(sdk.join("src/url_project.c"))
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
    std::fs::write(sdk.join("src/main.rs"), "fn main(){assert_eq!(ri06_url_owner::run(),Ok(42));assert_eq!(ri06_url_owner::live_owner_count(),0);}").unwrap();
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
    let domain_source = source.replace("\"https://example.invalid/path\"", "\":invalid\"");
    assert_ne!(domain_source, source);
    std::fs::write(root.join("src/app.spx"), &domain_source).unwrap();
    let domain_selection =
        ["url.new", "url.view"].map(|import_id| IndexedProjectUrlRegistrySelection {
            source_path: "src/app.spx",
            source: &domain_source,
            import_id,
            index_bytes,
            package,
        });
    let domain =
        prepare_indexed_url_project_package(&root.join("semaprax.toml"), &domain_selection, lock)
            .unwrap();
    std::fs::write(sdk.join("src/url_project.c"), domain.c_source()).unwrap();
    let compiled = Command::new(&clang)
        .args(["-std=c11", "-O2", "-c"])
        .arg(sdk.join("src/url_project.c"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::fs::write(sdk.join("src/main.rs"), "fn main(){assert_eq!(ri06_url_owner::run(),Ok(9));assert_eq!(ri06_url_owner::live_owner_count(),0);}").unwrap();
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
    std::fs::write(sdk.join("src/url_project.c"), lines).unwrap();
    let compiled = Command::new(&clang)
        .args(["-std=c11", "-O2", "-c"])
        .arg(sdk.join("src/url_project.c"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::fs::write(sdk.join("src/main.rs"), "fn main(){assert_eq!(ri06_url_owner::run(),Ok(42));assert_eq!(ri06_url_owner::live_owner_count(),0);} // finalizer control").unwrap();
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
    for (changed, code) in [
        (
            source.replace("if byte_len(bytes) == 28usize { 41 } else { 7 }", "view"),
            "SPX-T258",
        ),
        (
            source.replace("url_view(owner)", "url_view(url_new(input))"),
            "SPX-B107",
        ),
    ] {
        assert_ne!(changed, source);
        std::fs::write(root.join("src/app.spx"), &changed).unwrap();
        let selected_changed =
            ["url.new", "url.view"].map(|import_id| IndexedProjectUrlRegistrySelection {
                source_path: "src/app.spx",
                source: &changed,
                import_id,
                index_bytes,
                package,
            });
        let errors = prepare_indexed_url_project_package(
            &root.join("semaprax.toml"),
            &selected_changed,
            lock,
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == code),
            "expected {code}: {errors:?}"
        );
    }
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
    assert!(prepare_indexed_url_project_package(
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
        prepare_indexed_url_project_package(&root.join("semaprax.toml"), &selected, lock,).is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(target).unwrap();
}

#[path = "url_loan_tests.rs"]
mod url_loan;

#[path = "url_safety_tests.rs"]
mod url_safety;

#[path = "url_miri_tests.rs"]
mod url_miri;
