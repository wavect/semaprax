//! Physical evidence authored for the real v10 length-header provider.
//! Allocation observation is confined to a separately compiled C fixture;
//! ordinary emit_c and a successful opaque-context close are not substitutes.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    derive_public_api_descriptor, replay_public_api_descriptor, PublicApiSubject,
    PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = include_str!("native_owned_utf8_settlement_v1/source.spx");
const FACT: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const SELECTED: &[&str] = &[
    "s.before",
    "s.body",
    "s.branch",
    "s.callee",
    "s.clone",
    "s.condition",
    "s.empty",
    "s.late",
    "s.local",
    "s.mixed",
    "s.nested",
    "s.pressure",
    "s.text",
];

fn provider() -> String {
    let program =
        semaprax::hir::resolve(&semaprax::check(SOURCE, "native-string-settlement.spx").unwrap())
            .unwrap();
    let selected = SELECTED
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    let subject = PublicApiSubject {
        project_schema: PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
        project_revision: FACT,
        workspace_revision: FACT,
        project_graph_digest: FACT,
    };
    let descriptor = derive_public_api_descriptor(&program, &selected, subject).unwrap();
    let bytes = descriptor.canonical_bytes();
    let digest = descriptor.digest();
    assert_eq!(
        replay_public_api_descriptor(&program, &selected, subject, &bytes, &digest).unwrap(),
        descriptor
    );
    for rejected in [
        "private.clone",
        "private.match",
        "private.post",
        "private.intrinsic",
        "private.equality",
        "private.loop-condition",
        "private.loop-body",
    ] {
        assert_eq!(
            derive_public_api_descriptor(&program, &[rejected.to_owned()], subject)
                .unwrap_err()
                .code,
            "SPX-J113"
        );
    }
    let emit = || {
        semaprax::codegen::emit_project_v10_native_owned_utf8_provider(
            &program, &selected, subject, &bytes, &digest,
        )
        .unwrap()
    };
    let artifact = emit();
    assert_eq!(artifact, emit());
    assert_eq!(artifact.descriptor(), bytes);
    assert_eq!(artifact.descriptor_digest(), digest);
    assert!(artifact.source().contains("struct spx_string_v10"));
    assert!(artifact.source().contains("spx_string_length_v10(result)"));
    artifact.source().to_owned()
}

fn run(sanitized: bool) {
    let provider = provider();
    let root = std::env::temp_dir().join(format!(
        "semaprax-native-string-settlement-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).unwrap();
    let source = root.join("probe.c");
    let mut private_symbols = String::new();
    for (name, id) in [
        ("CLONE", "private.clone"),
        ("MATCH", "private.match"),
        ("POST", "private.post"),
        ("INTRINSIC", "private.intrinsic"),
        ("EQUALITY", "private.equality"),
        ("LOOP_CONDITION", "private.loop-condition"),
        ("LOOP_BODY", "private.loop-body"),
    ] {
        let hex = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        private_symbols.push_str(&format!("#define FIXTURE_PRIVATE_{name} spx_decl_{hex}\n"));
    }
    fs::write(
        &source,
        format!(
            "{}\n{}\n{provider}\n{private_symbols}\n{}",
            include_str!("support/native_fixture_stdio.c"),
            include_str!("native_owned_utf8_settlement_v1/allocations.c"),
            include_str!("native_owned_utf8_settlement_v1/probe.c"),
        ),
    )
    .unwrap();
    let compiler = if sanitized {
        let configured = PathBuf::from(
            std::env::var_os("SEMAPRAX_STRING_SANITIZER_CLANG")
                .expect("selected sanitizer gate requires SEMAPRAX_STRING_SANITIZER_CLANG"),
        );
        assert!(configured.is_absolute() && configured.is_file());
        configured
    } else {
        std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from)
    };
    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let mut command = Command::new(&compiler);
        command.current_dir(&root);
        command.args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"]);
        if sanitized {
            command.args([
                "-fsanitize=address,undefined",
                "-fno-sanitize-recover=all",
                "-fno-omit-frame-pointer",
            ]);
        }
        let built = command
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Clang is required for native String physical evidence");
        assert!(
            built.status.success(),
            "{}: {}",
            root.display(),
            String::from_utf8_lossy(&built.stderr)
        );
        let mut command = Command::new(&executable);
        command.current_dir(&root);
        if sanitized {
            // Exact counters prove leaks even where the installed ASan runtime
            // has no LeakSanitizer. ASan/UBSan supply distinct memory/UB checks.
            command.env("ASAN_OPTIONS", "halt_on_error=1");
            command.env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}: stdout={} stderr={}",
            root.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"native-owned-utf8-settled\n");
        assert!(output.stderr.is_empty());
    }
    // Failed fixtures remain intact. Successful cleanup is exact, nonrecursive.
    // MSVC linkers may emit export/debug sidecars for exported provider symbols.
    let mut permitted = vec!["probe.c".to_owned()];
    for optimization in ["-O0", "-O2"] {
        permitted.push(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        if cfg!(windows) {
            for extension in ["lib", "exp", "pdb", "ilk"] {
                permitted.push(format!("probe{optimization}.{extension}"));
            }
        }
    }
    let entries = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    assert!(entries.len() <= permitted.len());
    for entry in &entries {
        assert!(permitted.contains(&entry.file_name().into_string().unwrap()));
        let metadata = fs::symlink_metadata(entry.path()).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            assert_eq!(metadata.file_attributes() & 0x400, 0);
        }
    }
    for entry in entries {
        fs::remove_file(entry.path()).unwrap();
    }
    fs::remove_dir(root).unwrap();
}

#[test]
fn real_v10_provider_settles_every_initialized_string_at_o0_and_o2() {
    run(false);
}

#[test]
#[ignore = "requires explicitly provisioned Clang ASan/UBSan runtime"]
fn provisioned_native_string_provider_asan_ubsan() {
    run(true);
}
