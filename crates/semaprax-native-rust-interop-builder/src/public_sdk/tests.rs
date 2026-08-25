//! Boundary, failure-injection, and determinism tests for the SDK authority.

use super::authority::build_native_rust_sdk_inner;
use super::descriptor::{canonical_spec, parse_descriptor};
use super::package::{
    render_package_sources, render_sdk_manifest, verify_sdk_manifest, SdkManifestInputs,
    SdkManifestSubject,
};
use super::*;

const MINIMAL_SOURCE: &str = r#"module sdk.path_fixture;

@id("sdk.value")
fn value() -> i64 { 1 }

@id("sdk.main")
fn main() -> i64 { 0 }
"#;

const BOUNDARY_SOURCE: &str = r#"module interop.fixture;

permit { host.math }

@id("host.math")
interface HostMath permits { host.math } {
    @id("host.add")
    import rust fn host_add(left: i64, right: i64) -> i64
        effects { host.math }
        failure status "host.math.v1";
}

@id("interop.add")
fn add(left: i64, right: i64) -> i64 uses { host.math } {
    host_add(left, right) + right
}

@id("interop.main")
fn main() -> i64 { 0 }
"#;

fn minimal_options() -> NativeRustSdkOptions {
    NativeRustSdkOptions {
        exports: vec!["sdk.value".into()],
        imports: Vec::new(),
        capabilities: Vec::new(),
    }
}

fn boundary_options() -> NativeRustSdkOptions {
    NativeRustSdkOptions {
        exports: vec!["interop.add".into()],
        imports: vec!["host.add".into()],
        capabilities: vec!["host.math".into()],
    }
}

fn required_env_is_one(name: &str) -> bool {
    std::env::var_os(name).as_deref() == Some(OsStr::new("1"))
}

fn required_windows_public_sdk_build() -> bool {
    required_env_is_one("SEMAPRAX_REQUIRE_WINDOWS_REAL_ARCHIVE")
}

fn required_public_sdk_build() -> bool {
    required_windows_public_sdk_build() || required_env_is_one("SEMAPRAX_REQUIRE_PUBLIC_SDK_BUILD")
}

fn missing_public_sdk_tools() -> Vec<&'static str> {
    let windows_required = cfg!(windows) && required_public_sdk_build();
    let required_tools: &[&str] = if windows_required {
        &[
            "RUSTC",
            "CLANG",
            "SEMAPRAX_ARCHIVER",
            "SEMAPRAX_VCTOOLS",
            "SEMAPRAX_LINKER",
        ]
    } else {
        &["RUSTC", "CLANG", "SEMAPRAX_ARCHIVER"]
    };
    required_tools
        .iter()
        .copied()
        .filter(|name| std::env::var_os(name).is_none())
        .collect()
}

fn bounded_remaining_sdk_names(root: &Path) -> Vec<String> {
    const MAX_NAMES: usize = 16;
    const MAX_NAME_BYTES: usize = 160;
    const OWNED_PREFIXES: [&[u8]; 2] = [
        b".semaprax-native-rust-sdk-",
        b".semaprax-native-rust-interop-",
    ];

    let Ok(entries) = std::fs::read_dir(root) else {
        return vec!["<read-dir-error>".to_owned()];
    };
    let mut names = Vec::new();
    let mut truncated = false;
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let name = entry.file_name();
        let bytes = name.as_encoded_bytes();
        if bytes != b"generated"
            && !OWNED_PREFIXES
                .iter()
                .any(|prefix| bytes.starts_with(prefix))
        {
            continue;
        }
        if names.len() == MAX_NAMES {
            truncated = true;
            break;
        }
        let mut bounded = String::with_capacity(bytes.len().min(MAX_NAME_BYTES));
        for byte in bytes.iter().copied().take(MAX_NAME_BYTES) {
            bounded.push(if byte.is_ascii_graphic() {
                char::from(byte)
            } else {
                '?'
            });
        }
        if bytes.len() > MAX_NAME_BYTES {
            bounded.push_str("...");
        }
        names.push(bounded);
    }
    names.sort();
    if truncated {
        names[MAX_NAMES - 1] = "<truncated>".to_owned();
    }
    names
}

#[test]
fn stable_id_method_encoding_is_injective_for_the_public_grammar() {
    assert_eq!(
        encode_stable_id("calculator.add").unwrap(),
        "spx_calculator_dot_add"
    );
    assert_eq!(
        encode_stable_id("a_b-c.d").unwrap(),
        "spx_a_underscore_b_hyphen_c_dot_d"
    );
    let ids = ["a.b", "a_b", "a-b", "ab", "a0"];
    let encoded = ids
        .iter()
        .map(|id| encode_stable_id(id).unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(encoded.len(), ids.len());
}

#[test]
fn canonical_options_reject_duplicates_and_nonportable_ids() {
    assert!(canonical_values(vec!["a.b".into(), "a.b".into()], 32).is_err());
    assert!(canonical_values(vec!["Upper".into()], 32).is_err());
    assert_eq!(
        canonical_values(vec!["z".into(), "a".into()], 32).unwrap(),
        ["a", "z"]
    );
}

#[test]
fn generated_cargo_package_has_no_dependency_or_repository_escape() {
    let facts = DescriptorFacts {
        module: "calculator".into(),
        source_revision: "sha256:00".into(),
        target: target_triple().unwrap().into(),
        exports: Vec::new(),
        imports: Vec::new(),
    };
    let sources = render_package_sources(&facts, &[]);
    assert!(sources.cargo_toml.contains("publish = false"));
    assert!(!sources.cargo_toml.contains("dependencies"));
    assert!(!sources.cargo_toml.contains("path = \"../"));
    assert!(!sources.build_rs.contains("Command"));
    assert!(!sources.build_rs.contains("cargo:rustc-env"));
    assert!(sources.build_rs.contains("var_os(\"CARGO_MANIFEST_DIR\")"));
    assert!(sources.build_rs.contains("path.contains(['\\r','\\n'])"));
    assert!(!sources
        .build_rs
        .contains("cargo:rustc-link-search=native=native"));
    assert!(!sources.build_rs.contains("eprintln!"));
}

#[test]
fn hostile_output_paths_are_rejected_before_tool_configuration() {
    let newline = std::env::temp_dir().join("semaprax-sdk-hostile\noutput");
    let error = build_native_rust_sdk(
        MINIMAL_SOURCE,
        Path::new("path-fixture.spx"),
        minimal_options(),
        &newline,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I233");
    assert!(std::fs::symlink_metadata(&newline).is_err());

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt as _;
        let non_unicode = std::env::temp_dir().join(std::ffi::OsString::from_vec(vec![0xff]));
        let error = build_native_rust_sdk(
            MINIMAL_SOURCE,
            Path::new("path-fixture.spx"),
            minimal_options(),
            &non_unicode,
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I233");
        assert!(std::fs::symlink_metadata(&non_unicode).is_err());
    }
}

#[test]
fn effect_boundaries_fail_stop_and_preserve_sticky_status() {
    if std::env::var_os("RUSTC").is_none()
        || std::env::var_os("CLANG").is_none()
        || std::env::var_os("SEMAPRAX_ARCHIVER").is_none()
    {
        return;
    }
    let program = semaprax::check(BOUNDARY_SOURCE, Path::new("boundary-fixture.spx")).unwrap();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-sdk-boundaries-{}-{}",
            std::process::id(),
            STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
    std::fs::create_dir(&root).unwrap();

    let run = |point, name: &str| {
        TEST_BUILD_STATE.with(|state| {
            state.set(TestBuildState {
                point: Some(point),
                ..TestBuildState::default()
            });
        });
        let output = root.join(name);
        let result = build_native_rust_sdk_inner(&program, boundary_options(), &output);
        let state = TEST_BUILD_STATE.with(std::cell::Cell::get);
        TEST_BUILD_STATE.with(|slot| slot.set(TestBuildState::default()));
        (output, result, state)
    };

    let (output, error, state) = run(TestBuildPoint::BeforeArchive, "before-archive");
    let diagnostics = error.err().unwrap().into_diagnostics();
    assert_eq!(
        diagnostics[0].code, "SPX-B112",
        "{}",
        diagnostics[0].message
    );
    assert_eq!((state.archive_attempts, state.publish_calls), (0, 0));
    assert_eq!(state.last_stage, TestBuildLastStage::ArchiveStageCreated);
    assert!(!output.exists());

    let entries_before = std::fs::read_dir(&root).unwrap().count();
    let (output, error, state) = run(
        TestBuildPoint::ArchiveCreationCleanupUncertainty,
        "archive-creation-cleanup",
    );
    let diagnostics = error.err().unwrap().into_diagnostics();
    assert_eq!(diagnostics[0].code, "SPX-B112");
    assert_eq!(diagnostics[1].code, "SPX-I233");
    assert_eq!((state.archive_attempts, state.publish_calls), (0, 0));
    assert_eq!(state.last_stage, TestBuildLastStage::InnerPayloadVerified);
    assert!(!output.exists());
    assert!(std::fs::read_dir(&root).unwrap().count() >= entries_before + 2);

    let remaining_before = bounded_remaining_sdk_names(&root);
    let (output, error, state) = run(
        TestBuildPoint::ArchiveEffectUncertain,
        "archive-effect-uncertain",
    );
    let diagnostics = error.err().unwrap().into_diagnostics();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].code, "SPX-I233");
    assert_eq!(
        diagnostics[0].message,
        "Native Rust SDK publication failed at archive ExactArchive: Invalid"
    );
    assert_eq!(diagnostics[1].code, "SPX-I233");
    assert_eq!(
        diagnostics[1].message,
        "Native Rust SDK archive effect settlement is uncertain; preserved inert stage"
    );
    assert_eq!((state.archive_attempts, state.publish_calls), (1, 0));
    assert_eq!(state.last_stage, TestBuildLastStage::ArchiveStageCreated);
    assert!(!output.exists());
    let remaining_after = bounded_remaining_sdk_names(&root);
    let added = remaining_after
        .iter()
        .filter(|name| !remaining_before.contains(name))
        .collect::<Vec<_>>();
    // The exact inner and archive stages remain. No cleanup removed either
    // stage and no outer-stage creation added a third owned name.
    assert_eq!(added.len(), 2);

    let (output, error, state) = run(TestBuildPoint::ArchiveOutputMutation, "archive-mutation");
    let diagnostics = error.err().unwrap().into_diagnostics();
    assert_eq!(diagnostics[0].code, "SPX-I233");
    assert_eq!((state.archive_attempts, state.publish_calls), (1, 0));
    assert_eq!(state.last_stage, TestBuildLastStage::ArchiveStageCreated);
    assert!(!output.exists());
    assert!(std::fs::read_dir(&root).unwrap().any(|entry| {
        entry.ok().is_some_and(|entry| {
            std::fs::read(entry.path().join(if cfg!(windows) {
                "semaprax_native_rust_sdk.lib"
            } else {
                "libsemaprax_native_rust_sdk.a"
            }))
            .ok()
            .as_deref()
                == Some(b"foreign")
        })
    }));
    let (output, error, state) = run(TestBuildPoint::AfterFirstOuterWrite, "partial-outer");
    let diagnostics = error.err().unwrap().into_diagnostics();
    assert_eq!(diagnostics[0].code, "SPX-B112");
    assert_eq!((state.archive_attempts, state.publish_calls), (1, 0));
    assert_eq!(state.last_stage, TestBuildLastStage::OuterStageCreated);
    assert!(!output.exists());

    let settled_names_before = bounded_remaining_sdk_names(&root);
    let (output, error, state) = run(TestBuildPoint::BeforePublish, "before-publish");
    let diagnostics = error.err().unwrap().into_diagnostics();
    assert_eq!(diagnostics[0].code, "SPX-I233");
    assert_eq!((state.archive_attempts, state.publish_calls), (1, 1));
    assert_eq!(
        state.last_stage,
        TestBuildLastStage::OuterPublicationSettled
    );
    assert!(!output.exists());
    let settled_names_after = bounded_remaining_sdk_names(&root);
    let added_settled_names = settled_names_after
        .iter()
        .filter(|name| !settled_names_before.contains(name))
        .collect::<Vec<_>>();
    assert_eq!(added_settled_names.len(), 1);
    let settled_name = added_settled_names[0];
    assert!(settled_name.starts_with(".semaprax-native-rust-sdk-"));
    let settled_stage = root.join(settled_name);
    assert!(settled_stage.is_dir());
    assert_eq!(std::fs::read_dir(&settled_stage).unwrap().count(), 5);
    assert_eq!(
        std::fs::read_dir(settled_stage.join("src"))
            .unwrap()
            .count(),
        3
    );
    assert_eq!(
        std::fs::read_dir(settled_stage.join("native"))
            .unwrap()
            .count(),
        3
    );
    for relative in [
        "Cargo.toml",
        "build.rs",
        "semaprax.native-rust-sdk.json",
        "src/lib.rs",
        "src/semaprax_native_rust_interop.rs",
        "src/semaprax_native_rust_interop_ffi.rs",
        "native/descriptor.json",
        if cfg!(windows) {
            "native/semaprax_native_rust_sdk.lib"
        } else {
            "native/libsemaprax_native_rust_sdk.a"
        },
        "native/semaprax.native-rust-interop.json",
    ] {
        assert!(settled_stage.join(relative).is_file(), "missing {relative}");
    }

    let (output, error, state) = run(
        TestBuildPoint::ScratchCleanupUncertainty,
        "cleanup-uncertainty",
    );
    let diagnostics = error.err().unwrap().into_diagnostics();
    assert_eq!(diagnostics[0].code, "SPX-I233");
    assert_eq!((state.archive_attempts, state.publish_calls), (1, 0));
    assert_eq!(
        state.last_stage,
        TestBuildLastStage::OuterInventoryAuthenticated
    );
    assert!(!output.exists());
    assert!(std::fs::read_dir(&root).unwrap().any(|entry| {
        entry
            .ok()
            .is_some_and(|entry| entry.path().join("foreign").is_file())
    }));

    let (output, error, state) = run(
        TestBuildPoint::PostPivotAuthenticationFailure,
        "post-pivot-authentication",
    );
    let diagnostics = error.err().unwrap().into_diagnostics();
    assert_eq!(diagnostics[0].code, "SPX-I233");
    assert_eq!((state.archive_attempts, state.publish_calls), (1, 1));
    assert_eq!(state.last_stage, TestBuildLastStage::PublishReturned);
    assert!(output.is_dir());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn effectful_no_import_sdk_builds_the_exact_public_inventory() {
    let missing_tools = missing_public_sdk_tools();
    if !missing_tools.is_empty() {
        if required_public_sdk_build() {
            panic!(
                "required minimal public SDK build is missing configured tools: {missing_tools:?}"
            );
        }
        return;
    }
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-sdk-no-import-{}-{}",
            std::process::id(),
            STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
    std::fs::create_dir(&root).unwrap();
    let output = root.join("generated");
    TEST_BUILD_STATE.with(|state| state.set(TestBuildState::default()));
    let result = build_native_rust_sdk(
        MINIMAL_SOURCE,
        Path::new("no-import-fixture.spx"),
        minimal_options(),
        &output,
    );
    let snapshot = test_build_snapshot();
    let bundle = match result {
        Ok(bundle) => bundle,
        Err(diagnostics) => {
            let remaining_owned_names = bounded_remaining_sdk_names(&root);
            panic!(
                "minimal public SDK build failed: diagnostics={diagnostics:?}; last_stage={:?}; archive_attempts={}; publish_calls={}; remaining_owned_names={remaining_owned_names:?}",
                snapshot.last_stage, snapshot.archive_attempts, snapshot.publish_calls
            );
        }
    };
    assert_eq!(
        snapshot,
        TestBuildSnapshot {
            last_stage: TestBuildLastStage::PublishedAuthenticated,
            archive_attempts: 1,
            publish_calls: 1,
        }
    );
    assert_eq!(bundle.output_directory(), output);
    let root_entries = std::fs::read_dir(&output).unwrap().count();
    let src_entries = std::fs::read_dir(output.join("src")).unwrap().count();
    let native_entries = std::fs::read_dir(output.join("native")).unwrap().count();
    assert_eq!((root_entries, src_entries, native_entries), (5, 3, 3));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn public_intent_and_facade_replay_the_unchanged_private_artifacts() {
    const SOURCE: &str = r#"module sdk.fixture;

permit { host.math }

@id("host.math")
interface HostMath permits { host.math } {
    @id("host.add")
    import rust fn host_add(left: i64, right: i64) -> i64
        effects { host.math }
        failure status "host.math.v1";
}

@id("sdk.add")
fn add(left: i64, right: i64) -> i64 uses { host.math } {
    host_add(left, right)
}

@id("sdk.main")
fn main() -> i64 { 0 }
"#;
    let program = semaprax::check(SOURCE, Path::new("sdk-fixture.spx")).unwrap();
    let options = NativeRustSdkOptions {
        exports: vec!["sdk.add".into()],
        imports: vec!["host.add".into()],
        capabilities: vec!["host.math".into()],
    };
    let canonical = semaprax::format::canonical(&program);
    let revision = domain_digest(SOURCE_DOMAIN, canonical.as_bytes());
    let target = target_triple().unwrap();
    let spec = canonical_spec(&program.module, &revision, target, &options).unwrap();
    let prepared =
        crate::implementation::prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    assert_eq!(prepared.canonical_spec(), spec);
    let facts = parse_descriptor(
        prepared.descriptor().as_bytes(),
        &program.module,
        &revision,
        target,
        &options,
    )
    .unwrap();
    assert_eq!(facts.exports[0].public_method, "spx_sdk_dot_add");
    assert_eq!(facts.imports[0].public_method, "spx_host_dot_add");
    let sources = render_package_sources(&facts, &options.capabilities);
    assert!(!sources.lib_rs.starts_with("#![forbid(unsafe_code)]"));
    assert!(sources
        .lib_rs
        .contains("mod public_api{#![forbid(unsafe_code)]"));
    assert!(sources.lib_rs.contains("pub fn spx_sdk_dot_add"));
    assert!(sources.lib_rs.contains("fn spx_host_dot_add"));
    let inner_manifest = b"inner\n";
    let archive = b"archive";
    let manifest_inputs = SdkManifestInputs {
        facts: &facts,
        options: &options,
        descriptor: prepared.descriptor().as_bytes(),
        inner_manifest,
        sources: &sources,
        safe_inner: prepared.generated_rust().as_bytes(),
        ffi_inner: prepared.private_ffi_source().as_bytes(),
        archive,
    };
    let manifest = render_sdk_manifest(manifest_inputs, SdkManifestSubject::Source).unwrap();
    verify_sdk_manifest(
        manifest.as_bytes(),
        manifest_inputs,
        SdkManifestSubject::Source,
    )
    .unwrap();
    let manifest = manifest.into_bytes();
    let rejects = |bytes: &[u8]| {
        verify_sdk_manifest(bytes, manifest_inputs, SdkManifestSubject::Source).is_err()
    };
    for index in 0..manifest.len() {
        let mut substituted = manifest.clone();
        substituted[index] ^= 1;
        assert!(rejects(&substituted), "substitution {index}");

        let mut deleted = manifest.clone();
        deleted.remove(index);
        assert!(rejects(&deleted), "deletion {index}");
    }
    for index in 0..=manifest.len() {
        let mut inserted = manifest.clone();
        inserted.insert(index, b'x');
        assert!(rejects(&inserted), "insertion {index}");
    }
    for length in 0..manifest.len() {
        assert!(rejects(&manifest[..length]), "truncation {length}");
    }
}
