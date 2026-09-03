//! Source-layout tripwires for the private Linux doctor provisioner.
//!
//! These textual checks detect unreviewed layout and authority-surface changes;
//! they are not executable evidence that the provisioner enforces its contract.

use std::fs;
use std::path::{Path, PathBuf};

const SPEC: &str = "docs/DOCTOR-PRODUCTION-PROVISIONER-V1.md";
const ROOT: &str =
    "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner.rs";
const ADMISSION: &str =
    "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner/admission.rs";
const CAPSULE: &str =
    "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner/capsule.rs";
const LINUX: &str =
    "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner/linux.rs";
const RUNTIME_MODULE_DIRECTORY: &str =
    "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner";
const RUNTIME_MODULES: [&str; 8] = [
    ROOT,
    ADMISSION,
    CAPSULE,
    "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner/cgroup.rs",
    LINUX,
    "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner/linux/capture.rs",
    "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner/linux/child.rs",
    "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner/linux/lifetime.rs",
];

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

fn require(source: &str, owner: &str, required: &[&str]) -> Result<(), String> {
    for needle in required {
        if !source.contains(needle) {
            return Err(format!("{owner} lost `{needle}`"));
        }
    }
    Ok(())
}

fn forbid(source: &str, owner: &str, forbidden: &[&str]) -> Result<(), String> {
    for needle in forbidden {
        if source.contains(needle) {
            return Err(format!("{owner} gained forbidden `{needle}`"));
        }
    }
    Ok(())
}

fn collect_rust_modules(repository: &Path, directory: &Path, modules: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read module directory {}: {error}", directory.display()))
    {
        let entry = entry.unwrap_or_else(|error| panic!("read module entry: {error}"));
        let file_type = entry
            .file_type()
            .unwrap_or_else(|error| panic!("inspect {}: {error}", entry.path().display()));
        if file_type.is_dir() {
            collect_rust_modules(repository, &entry.path(), modules);
        } else if file_type.is_symlink() {
            panic!(
                "provisioner source-layout tripwire rejects symlink {}",
                entry.path().display()
            );
        } else if file_type.is_file() && entry.path().extension().is_some_and(|value| value == "rs")
        {
            modules.push(
                entry
                    .path()
                    .strip_prefix(repository)
                    .unwrap_or_else(|error| {
                        panic!("relativize {}: {error}", entry.path().display())
                    })
                    .to_owned(),
            );
        }
    }
}

fn require_exact_runtime_module_inventory(repository: &Path) {
    let mut actual = vec![PathBuf::from(ROOT)];
    collect_rust_modules(
        repository,
        &repository.join(RUNTIME_MODULE_DIRECTORY),
        &mut actual,
    );
    actual.sort();
    let mut expected = RUNTIME_MODULES.map(PathBuf::from).to_vec();
    expected.sort();
    assert_eq!(
        actual, expected,
        "provisioner source-layout tripwire: review and join every new Rust module"
    );
}

#[derive(Clone)]
struct Sources {
    root: String,
    admission: String,
    capsule: String,
    cgroup: String,
    linux: String,
    linux_capture: String,
    linux_child: String,
    linux_lifetime: String,
    binary: String,
    cargo: String,
    known_ordinary_cli_surfaces: String,
    specification: String,
    summary: String,
    architecture: String,
    quality: String,
}

impl Sources {
    fn checked_in(repository: &Path) -> Self {
        require_exact_runtime_module_inventory(repository);
        Self {
            root: read(repository, ROOT),
            admission: read(repository, ADMISSION),
            capsule: read(repository, CAPSULE),
            cgroup: read(repository, RUNTIME_MODULES[3]),
            linux: read(repository, LINUX),
            linux_capture: read(repository, RUNTIME_MODULES[5]),
            linux_child: read(repository, RUNTIME_MODULES[6]),
            linux_lifetime: read(repository, RUNTIME_MODULES[7]),
            binary: read(
                repository,
                "crates/semaprax-native-rust-interop-platform-sys/src/bin/doctor_provisioner.rs",
            ),
            cargo: read(
                repository,
                "crates/semaprax-native-rust-interop-platform-sys/Cargo.toml",
            ),
            // This is deliberately a tripwire over the currently registered
            // ordinary doctor/CLI surfaces, not a repository-wide call-graph
            // proof that no future command module can activate the provisioner.
            known_ordinary_cli_surfaces: [
                read(repository, "src/cli_driver.rs"),
                read(repository, "src/cli/project_runtime.rs"),
                read(repository, "src/cli/help.rs"),
                read(repository, "crates/semaprax-toolchain/src/doctor.rs"),
                read(
                    repository,
                    "crates/semaprax-toolchain/src/doctor/offline_profile.rs",
                ),
                read(
                    repository,
                    "crates/semaprax-toolchain/src/doctor/settled_report.rs",
                ),
                read(
                    repository,
                    "crates/semaprax-toolchain/src/doctor/version_token.rs",
                ),
            ]
            .join("\n"),
            specification: read(repository, SPEC),
            summary: read(repository, "docs/SUMMARY.md"),
            architecture: read(repository, "docs/ARCHITECTURE.md"),
            quality: read(repository, "docs/QUALITY-GATES.md"),
        }
    }

    fn joined_runtime(&self) -> String {
        [
            &self.root,
            &self.admission,
            &self.capsule,
            &self.cgroup,
            &self.linux,
            &self.linux_capture,
            &self.linux_child,
            &self.linux_lifetime,
        ]
        .map(String::as_str)
        .join("\n")
    }
}

fn provisioner_source_tripwires(sources: &Sources) -> Result<(), String> {
    require(
        &sources.cargo,
        "sys Cargo manifest",
        &[
            "name = \"semaprax-doctor-provisioner\"",
            "path = \"src/bin/doctor_provisioner.rs\"",
        ],
    )?;
    require(
        &sources.binary,
        "dedicated provisioner binary",
        &[
            "Dedicated signed offline-doctor provisioner entry; no argument surface.",
            "fn main()",
            "provisioned_doctor_provisioner_entry()",
        ],
    )?;
    forbid(
        &sources.binary,
        "dedicated provisioner binary",
        &["std::env", "args()", "args_os()", "Command::new", "PATH"],
    )?;

    require(
        &sources.root,
        "provisioner entry",
        &[
            "pub unsafe fn provisioned_doctor_provisioner_entry() -> !",
            "target_os = \"linux\"",
            "target_pointer_width = \"64\"",
            "target_endian = \"little\"",
            "target_arch = \"x86_64\"",
            "target_arch = \"aarch64\"",
            "linux::entry();",
            "std::process::exit(125)",
        ],
    )?;
    require(
        &sources.admission,
        "immutable admission",
        &[
            "const CAPSULE_FD: i32 = 3;",
            "const REQUEST_FD: i32 = 4;",
            "const BUNDLE_FD: i32 = 5;",
            "const LAUNCHER_FD: i32 = 6;",
            "const WORKER_FD: i32 = 7;",
            "const COLLECTOR_FD: i32 = 8;",
            "const CGROUP_FD: i32 = 9;",
            "const PROC_FD: i32 = 10;",
            "for fd in 0..=PROC_FD",
            "require_exact_descriptor_inventory()?",
            "libc::CGROUP2_SUPER_MAGIC",
            "libc::PROC_SUPER_MAGIC",
            "capsule::parse_with_release_anchor",
            "validate_image(LAUNCHER_FD, capsule.launcher())?",
            "validate_image(WORKER_FD, capsule.worker())?",
            "validate_image(COLLECTOR_FD, capsule.collector())?",
        ],
    )?;
    require(
        &sources.capsule,
        "signed capsule admission",
        &[
            "const MAX_CAPSULE_BYTES: usize = 341;",
            "const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;",
            "option_env!(\"SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX\")",
            "key.verify_strict(body, &Signature::from_bytes(&signature_bytes))",
            "if roles != expected_roles",
            "if artifact.length == 0 || artifact.length > MAX_ARTIFACT_BYTES",
            "if cursor != body.len()",
        ],
    )?;

    let runtime = sources.joined_runtime();
    forbid(
        &runtime,
        "production provisioner runtime",
        &[
            "std::process::Command",
            "Command::new",
            "std::env::var(",
            "std::env::var_os(",
            "std::env::current_dir",
            "SEMAPRAX_DOCTOR_PROVISIONER",
            "which::",
            "execvp(",
            "execvpe(",
            "\"/bin/",
            "\"/usr/bin/",
        ],
    )?;
    require(
        &sources.linux,
        "isolated clone source-layout tripwire",
        &[
            "const CLONE_CLEAR_SIGHAND: u64 = 1 << 32;",
            "| CLONE_CLEAR_SIGHAND",
            "| CLONE_INTO_CGROUP",
            "| libc::CLONE_NEWUSER as u64",
            "| libc::CLONE_NEWNS as u64",
            "| libc::CLONE_NEWNET as u64",
        ],
    )?;
    require(
        &sources.linux_child,
        "held launcher child",
        &["libc::SYS_execveat", "libc::AT_EMPTY_PATH"],
    )?;

    forbid(
        &sources.known_ordinary_cli_surfaces,
        "known ordinary CLI source-layout tripwire",
        &[
            "semaprax-doctor-provisioner",
            "provisioned_doctor_provisioner_entry",
            "SEMAPRAX_DOCTOR_PROVISIONER",
        ],
    )?;
    require(
        &sources.specification,
        "provisioner specification",
        &[
            "Status: private Linux implementation contract",
            "Missing, malformed, or",
            "noncanonical key material makes the production entry unavailable",
            "namespace, cgroup, sealing, or kernel prerequisites fail rather than skip.",
            "ordinary CLI activation remain unrun and unpromoted",
        ],
    )?;
    for (owner, source) in [
        ("documentation catalog", &sources.summary),
        ("architecture", &sources.architecture),
        ("quality gates", &sources.quality),
    ] {
        if !source.contains("DOCTOR-PRODUCTION-PROVISIONER-V1.md") {
            return Err(format!("{owner} does not reference {SPEC}"));
        }
    }
    Ok(())
}

#[test]
fn production_provisioner_source_layout_tripwires_are_present() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    provisioner_source_tripwires(&Sources::checked_in(repository))
        .unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn source_layout_tripwire_rejects_representative_widening_and_role_swap() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checked_in = Sources::checked_in(repository);
    provisioner_source_tripwires(&checked_in).expect("checked-in source-layout tripwires");

    let mut mutations = Vec::new();
    mutations.push((
        "descriptor inventory",
        checked_in
            .admission
            .replacen("const PROC_FD: i32 = 10;", "const PROC_FD: i32 = 11;", 1),
        "admission",
    ));
    mutations.push((
        "launcher/worker role swap",
        checked_in.admission.replacen(
            "validate_image(LAUNCHER_FD, capsule.launcher())?",
            "validate_image(LAUNCHER_FD, capsule.worker())?",
            1,
        ),
        "admission",
    ));
    mutations.push((
        "runtime PATH fallback",
        format!(
            "{}\nstd::process::Command::new(\"git\");",
            checked_in.linux_child
        ),
        "linux_child",
    ));
    mutations.push((
        "ordinary CLI activation",
        format!(
            "{}\nprovisioned_doctor_provisioner_entry();",
            checked_in.known_ordinary_cli_surfaces
        ),
        "known_ordinary_cli_surfaces",
    ));
    mutations.push((
        "unsupported-host continuation",
        checked_in
            .root
            .replacen("std::process::exit(125)", "loop {}", 1),
        "root",
    ));
    mutations.push((
        "unsigned capsule parser",
        checked_in.capsule.replacen(
            "key.verify_strict(body, &Signature::from_bytes(&signature_bytes))",
            "Ok(())",
            1,
        ),
        "capsule",
    ));

    for (name, mutation, field) in mutations {
        let mut hostile = checked_in.clone();
        match field {
            "root" => hostile.root = mutation,
            "admission" => hostile.admission = mutation,
            "capsule" => hostile.capsule = mutation,
            "linux" => hostile.linux = mutation,
            "linux_child" => hostile.linux_child = mutation,
            "known_ordinary_cli_surfaces" => hostile.known_ordinary_cli_surfaces = mutation,
            _ => panic!("unknown hostile source field {field}"),
        }
        assert!(
            provisioner_source_tripwires(&hostile).is_err(),
            "hostile {name} mutation escaped the source-layout tripwire"
        );
    }
}
