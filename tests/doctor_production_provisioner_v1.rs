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
const CAPSULE_CORE: &str = "crates/semaprax-doctor-capsule/src/lib.rs";
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
    capsule_core: String,
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
            capsule_core: read(repository, CAPSULE_CORE),
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
                read(repository, "src/cli_driver/options.rs"),
                read(repository, "src/cli_driver/options/tests.rs"),
                read(repository, "src/cli_driver/report_options.rs"),
                read(repository, "src/cli_driver/report_options/tests.rs"),
                read(repository, "src/cli_driver/source_execution.rs"),
                read(repository, "src/cli_driver/project_scaffold_options.rs"),
                read(repository, "src/cli_driver/supply_chain.rs"),
                read(repository, "src/cli_driver/persistence_dispatch.rs"),
                read(repository, "src/cli/project_runtime.rs"),
                read(repository, "src/cli/help.rs"),
                read(repository, "src/doctor.rs"),
                read(repository, "src/doctor/offline_profile.rs"),
                read(
                    repository,
                    "crates/semaprax-toolchain/src/settled_report.rs",
                ),
                read(repository, "src/doctor/version_token.rs"),
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
            &self.capsule_core,
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
        "release-anchor capsule wrapper",
        &[
            "semaprax_doctor_capsule::{parse_public_key, parse_signed, Capsule}",
            "option_env!(\"SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX\")",
            "parse_signed(bytes, &key).map_err(map_error)",
        ],
    )?;
    require(
        &sources.capsule_core,
        "signed capsule admission",
        &[
            "const MAX_CAPSULE_BYTES: usize = 341;",
            "const MAX_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;",
            "key.verify_strict(body, &Signature::from_bytes(&signature_bytes))",
            "if roles != expected_roles",
            ".any(|artifact| artifact.length == 0 || artifact.length > MAX_ARTIFACT_BYTES)",
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
        &[
            "libc::MS_REC | libc::MS_PRIVATE",
            "libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC",
            "size=65536,nr_inodes=1,mode=0500",
            "libc::SYS_pivot_root",
            "libc::MNT_DETACH",
            "libc::MS_REMOUNT",
            "libc::MS_RDONLY",
            "libc::TMPFS_MAGIC",
            "Some(1..=65_536)",
            "filesystem.f_flags & 15 != 15",
            "empty_directory(c\"/\")",
            "libc::SYS_execveat",
            "libc::AT_EMPTY_PATH",
        ],
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
            "Status: implemented bounded profile; **HOSTED GREEN** under the",
            "Missing, malformed, or",
            "noncanonical key material makes the production entry unavailable",
            "namespace, cgroup, sealing, or kernel prerequisites fail rather than skip.",
            "does not make an ordinary `semaprax doctor --profile` selector authoritative",
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

fn workflow_lines(workflow: &str) -> Result<Vec<(usize, &str)>, String> {
    workflow
        .lines()
        .filter_map(|line| {
            if line.contains('\t') {
                return Some(Err("workflow uses a tab-indented line".to_owned()));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            Some(Ok((
                line.len() - line.trim_start().len(),
                line.trim_start(),
            )))
        })
        .collect()
}

fn section_end(lines: &[(usize, &str)], start: usize, indentation: usize) -> usize {
    lines[start + 1..]
        .iter()
        .position(|(line_indentation, _)| *line_indentation <= indentation)
        .map_or(lines.len(), |offset| start + 1 + offset)
}

fn direct_children(lines: &[(usize, &str)], start: usize, indentation: usize) -> Vec<String> {
    let end = section_end(lines, start, indentation);
    lines[start + 1..end]
        .iter()
        .filter(|(line_indentation, _)| *line_indentation == indentation + 2)
        .map(|(_, value)| (*value).to_owned())
        .collect()
}

fn required_section<'a>(
    lines: &'a [(usize, &'a str)],
    key: &str,
) -> Result<(usize, Vec<String>), String> {
    let matches: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, (indentation, value))| *indentation == 0 && *value == key)
        .map(|(index, _)| index)
        .collect();
    match matches.as_slice() {
        [start] => Ok((*start, direct_children(lines, *start, 0))),
        [] => Err(format!("workflow is missing top-level `{key}`")),
        _ => Err(format!("workflow repeats top-level `{key}`")),
    }
}

fn parse_aarch64_tracking_workflow(workflow: &str) -> Result<(), String> {
    let lines = workflow_lines(workflow)?;
    let top_level: Vec<_> = lines
        .iter()
        .filter(|(indentation, _)| *indentation == 0)
        .map(|(_, value)| *value)
        .collect();
    let expected_top_level = [
        "name: Doctor AArch64 Linux lifecycle tracking",
        "on:",
        "permissions:",
        "concurrency:",
        "env:",
        "jobs:",
    ];
    if top_level != expected_top_level {
        return Err(format!(
            "workflow top-level shape changed: expected {expected_top_level:?}, got {top_level:?}"
        ));
    }

    let (_, triggers) = required_section(&lines, "on:")?;
    let (_, permissions) = required_section(&lines, "permissions:")?;
    if triggers != ["workflow_dispatch:"] {
        return Err(format!(
            "workflow triggers must be only workflow_dispatch, got {triggers:?}"
        ));
    }
    if permissions != ["contents: read"] {
        return Err(format!(
            "workflow permissions must be only contents: read, got {permissions:?}"
        ));
    }

    let (_, concurrency) = required_section(&lines, "concurrency:")?;
    if concurrency
        != [
            "group: doctor-provisioned-linux-aarch64",
            "cancel-in-progress: false",
        ]
    {
        return Err(format!(
            "workflow concurrency shape changed: {concurrency:?}"
        ));
    }
    let (_, environment) = required_section(&lines, "env:")?;
    if environment
        != [
            "CARGO_INCREMENTAL: \"0\"",
            "CARGO_PROFILE_DEV_DEBUG: \"0\"",
            "CARGO_PROFILE_TEST_DEBUG: \"0\"",
            "CARGO_TERM_COLOR: never",
            "PYTHONDONTWRITEBYTECODE: \"1\"",
        ]
    {
        return Err(format!(
            "workflow environment shape changed: {environment:?}"
        ));
    }

    let (jobs_start, jobs) = required_section(&lines, "jobs:")?;
    if jobs != ["doctor-provisioned-linux-aarch64:"] {
        return Err(format!(
            "workflow must define exactly one named job, got {jobs:?}"
        ));
    }
    let job_start = jobs_start + 1;
    let job_fields = direct_children(&lines, job_start, 2);
    if job_fields
        != [
            "name: Linux AArch64 offline doctor lifecycle (partial tracking)",
            "runs-on: ubuntu-24.04-arm",
            "timeout-minutes: 180",
            "steps:",
        ]
    {
        return Err(format!("workflow job shape changed: {job_fields:?}"));
    }
    let job_end = section_end(&lines, job_start, 2);
    let step_starts: Vec<_> = lines[job_start + 1..job_end]
        .iter()
        .enumerate()
        .filter(|(_, (indentation, value))| *indentation == 6 && value.starts_with("- "))
        .map(|(offset, _)| job_start + 1 + offset)
        .collect();
    let steps: Vec<_> = step_starts
        .iter()
        .map(|start| lines[*start].1.to_owned())
        .collect();
    let expected_step_headers = [
        "- uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7",
        "- uses: dtolnay/rust-toolchain@02cb101ec7c40f2c49e1d9714d64511d8e1b74de # master",
        "- name: Require a native Linux AArch64 runner",
        "- name: Acquire the locked dependency closure before offline execution",
        "- name: Require the source bytes the tracking result binds",
        "- name: Run the AArch64 partial lifecycle tracking probe",
        "- name: Preserve the explicit non-promotion boundary",
    ];
    if steps != expected_step_headers {
        return Err(format!(
            "workflow step headers changed: expected {expected_step_headers:?}, got {steps:?}"
        ));
    }
    let actual_step_bodies: Vec<Vec<String>> = step_starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = step_starts.get(index + 1).copied().unwrap_or(job_end);
            lines[*start..end]
                .iter()
                .map(|(indentation, value)| format!("{indentation}:{value}"))
                .collect()
        })
        .collect();
    let expected_step_bodies: Vec<Vec<String>> = vec![
        vec![
            "6:- uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7",
            "8:with:",
            "10:fetch-depth: 1",
            "10:filter: blob:none",
            "10:fetch-tags: false",
            "10:lfs: false",
        ],
        vec![
            "6:- uses: dtolnay/rust-toolchain@02cb101ec7c40f2c49e1d9714d64511d8e1b74de # master",
            "8:with:",
            "10:toolchain: 1.97.1",
        ],
        vec![
            "6:- name: Require a native Linux AArch64 runner",
            "8:run: |",
            "10:set -euo pipefail",
            "10:test \"$(uname -s)\" = Linux",
            "10:case \"$(uname -m)\" in",
            "12:aarch64|arm64) ;;",
            "12:*) echo \"expected native AArch64 runner, got $(uname -m)\" >&2; exit 1 ;;",
            "10:esac",
        ],
        vec![
            "6:- name: Acquire the locked dependency closure before offline execution",
            "8:run: cargo fetch --locked",
        ],
        vec![
            "6:- name: Require the source bytes the tracking result binds",
            "8:run: |",
            "10:set -euo pipefail",
            "10:test -z \"$(git status --porcelain)\"",
        ],
        vec![
            "6:- name: Run the AArch64 partial lifecycle tracking probe",
            "8:env:",
            "10:CARGO_NET_OFFLINE: \"true\"",
            "10:CARGO_TARGET_DIR: ${{ runner.temp }}/semaprax-doctor-aarch64-target",
            "8:run: bash scripts/doctor-provisioned-linux-aarch64-local-lifecycle.sh",
        ],
        vec![
            "6:- name: Preserve the explicit non-promotion boundary",
            "8:run: |",
            "10:set -euo pipefail",
            "10:printf '%s\\n' \\",
            "12:'This dispatch is a 24-case AArch64 tracking probe, not the full 26-case signed-release gate.' \\",
            "12:'A result does not resolve the two real-distribution exclusions or promote WP-05.'",
        ],
    ]
    .into_iter()
    .map(|step| step.into_iter().map(str::to_owned).collect())
    .collect();
    if actual_step_bodies != expected_step_bodies {
        return Err("workflow step keys or contents changed".to_owned());
    }
    if workflow.contains("continue-on-error:") || workflow.contains("if: ${{") {
        return Err("workflow permits a conditional or masked result".to_owned());
    }

    Ok(())
}

fn aarch64_local_driver_boundary(local_driver: &str) -> Result<(), String> {
    fn bash_array(source: &str, name: &str) -> Result<Vec<String>, String> {
        let header = format!("readonly -a {name}=(");
        let (_, body) = source
            .split_once(&header)
            .ok_or_else(|| format!("AArch64 driver is missing `{header}`"))?;
        let mut values = Vec::new();
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == ")" {
                return Ok(values);
            }
            let value = line
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .ok_or_else(|| format!("AArch64 driver has a non-literal `{name}` item"))?;
            values.push(value.to_owned());
        }
        Err(format!("AArch64 driver leaves `{name}` unterminated"))
    }

    let expected_platform = [
        "doctor::offline_root::linux::tests::provisioned_close_uncertainty_is_fail_stop",
        "doctor::offline_root::linux::tests::provisioned_detached_root_bytes_modes_and_read_only",
        "doctor::offline_root::linux::tests::provisioned_metadata_mismatches_feed_actual_admission",
        "doctor::offline_root::linux::tests::provisioned_setup_and_exact_write_failures_return_no_root",
        "doctor::offline_root::linux::tests::provisioned_wrong_page_cost_stops_before_tree_writes",
        "doctor::offline_worker::tests::hostile::provisioned_capability_operations_and_process_creation_are_denied",
        "doctor::offline_worker::tests::hostile::provisioned_root_hides_real_outside_file_and_rejects_write_opens",
        "doctor::offline_worker::tests::hostile::provisioned_stdin_is_eof_and_nonstandard_descriptors_are_closed",
        "doctor::offline_worker::tests::lifecycle::post_exec_capabilities_and_supervisor_death_are_observed_externally",
        "doctor::offline_worker::tests::provisioned_materializer_exec_and_socket_denial",
        "doctor::offline_worker::tests::provisioned_missing_role_bad_hash_and_invalid_request_emit_no_frame",
        "doctor::offline_worker::tests::provisioned_overflow_and_timeout_publish_only_settled_failure",
    ];
    let expected_collector = [
        "actual_worker_materializes_executes_and_settles_before_canonical_report",
        "complete_frame_and_capture_eof_each_still_require_worker_exit",
        "complete_literal_frame_followed_by_nonzero_exit_never_becomes_a_report",
        "created_handoff::production_created_native_and_all_files_reach_worker_and_reject_digest_drift",
        "launched_handoff::production_launcher_rejects_both_image_defects_and_digest_drift",
        "launched_handoff::production_launcher_rejects_structural_collector_with_missing_loader",
        "launched_handoff::production_launcher_reports_native_and_all_from_literal_transport_files",
        "literal_reply_surrogates_reject_cross_binding_and_malformed_frames",
        "nonchild::nonchild_pidfd_rejects_without_killing_or_stopping_the_owned_sentinel",
        "physical_reports::all_three_roles_settle_and_tool_failure_is_an_ordinary_exit_one_report",
        "physical_reports::closed_report_sink_fails_after_collection_without_successful_delivery",
        "prepared_handoff::prepared_native_and_all_role_handoffs_preserve_literal_wire_and_reject_transport_drift",
    ];
    if bash_array(local_driver, "PLATFORM_LIFECYCLE_TESTS")? != expected_platform {
        return Err("AArch64 platform lifecycle selection changed".to_owned());
    }
    if bash_array(local_driver, "COLLECTOR_LIFECYCLE_TESTS")? != expected_collector {
        return Err("AArch64 collector lifecycle selection changed".to_owned());
    }
    require(
        local_driver,
        "AArch64 local lifecycle plan",
        &[
            "readonly TRACKING_FIXTURE_COUNT=24",
            "[ \"${#PLATFORM_LIFECYCLE_TESTS[@]}\" -eq 12 ]",
            "[ \"${#COLLECTOR_LIFECYCLE_TESTS[@]}\" -eq 12 ]",
            "--ignored --exact --test-threads=1",
            "\"${PLATFORM_LIFECYCLE_TESTS[@]}\"",
            "\"${COLLECTOR_LIFECYCLE_TESTS[@]}\"",
        ],
    )?;
    let exclusions = [
        "--skip doctor::offline_worker::tests::provisioned_real_clang_node_rust_distributions",
        "--skip real_launched_handoff::production_launcher_reports_all_roles_from_provisioned_real_distributions",
    ];
    let actual_exclusions: Vec<_> = local_driver
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("--skip "))
        .map(|line| {
            line.strip_suffix(" \\").ok_or_else(|| {
                "AArch64 exclusion selector must continue into its exact plan".to_owned()
            })
        })
        .collect::<Result<_, _>>()?;
    if actual_exclusions != exclusions {
        return Err(format!(
            "AArch64 partial driver exclusions changed: expected {exclusions:?}, got {actual_exclusions:?}"
        ));
    }
    if local_driver
        .matches("--ignored --exact --test-threads=1")
        .count()
        != 4
        || local_driver.contains("doctor::offline_worker doctor::offline_root")
    {
        return Err(
            "AArch64 driver no longer uses only the closed exact lifecycle plan".to_owned(),
        );
    }

    let command_prefix = "unshare --user --map-root-user --mount --net --ipc --uts -- \\\n\t\tcargo test --locked --offline";
    let commands: Vec<_> = local_driver.split(command_prefix).skip(1).collect();
    if commands.len() != 4 {
        return Err(format!(
            "expected exactly four AArch64 lifecycle test commands, got {}",
            commands.len()
        ));
    }
    for (index, command) in commands[..2].iter().enumerate() {
        // Only the cargo test invocation itself must be unmasked; the
        // subsequent shell probes for the real-distribution fixtures
        // legitimately use `|| fail` after the lifecycle suite.
        let cargo_invocation = command.split("\n\techo").next().unwrap_or(command);
        if cargo_invocation.contains("||") {
            return Err(format!(
                "selected AArch64 lifecycle command {index} is masked instead of fail-fast"
            ));
        }
    }
    for (command, fixture, refusal) in [
        (
            commands[2],
            "doctor::offline_worker::tests::provisioned_real_clang_node_rust_distributions",
            "fail \"platform real-distribution fixture unexpectedly passed without the contracted provisioned bundle\"",
        ),
        (
            commands[3],
            "real_launched_handoff::production_launcher_reports_all_roles_from_provisioned_real_distributions",
            "fail \"collector real-distribution fixture unexpectedly passed without the contracted provisioned bundle\"",
        ),
    ] {
        if command.contains("--skip ") || !command.contains(fixture) || !command.contains(refusal) {
            return Err(
                "an excluded AArch64 fixture is not a required failing precondition probe"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn aarch64_tracking_contract_tripwires(
    tracking: &str,
    workflow: &str,
    local_driver: &str,
    guard_policy: &str,
    guard_tests: &str,
    provisioner_admission: &str,
) -> Result<(), String> {
    require(
        tracking,
        "AArch64 tracking contract",
        &[
            "Status: **in scope, tracked, and unexecuted on the proposed hosted runner.**",
            "first implementation is native 64-bit little-endian Linux\nx86-64 **and AArch64**",
            "default-deny syscall table for each native ABI",
            "Commit [`734e67af`]",
            "**local Docker-VM evidence**, not GitHub-hosted evidence and not\nphysical-device evidence",
            "passed 24 of the 26 ignored lifecycle fixtures",
            "doctor::offline_worker::tests::provisioned_real_clang_node_rust_distributions",
            "real_launched_handoff::production_launcher_reports_all_roles_from_provisioned_real_distributions",
            "failed fast because that local run supplied neither the required real\nClang/Node/Rust bundle nor its selector",
            "GitHub-hosted `ubuntu-24.04-arm` runner",
            "exactly two named\n`--skip` exclusions",
            "Those exclusions define its twenty-four-case boundary.",
            "Each probe is required to fail with its exact missing-bundle or\nmissing-selector reason",
            "Every selected lifecycle command is otherwise unmasked and\nfail-fast",
            "does **not** call itself a full AArch64 gate",
            "not a production or WP-05 promotion",
            "needs an observed real-binary behaviour and a negative control",
        ],
    )?;
    parse_aarch64_tracking_workflow(workflow)?;
    require(
        local_driver,
        "AArch64 local lifecycle driver",
        &[
            "THIS IS NOT scripts/doctor-provisioned-linux-gate.py",
            "aarch64 | arm64",
            "This is a failure, not a skip.",
            "--skip doctor::offline_worker::tests::provisioned_real_clang_node_rust_distributions",
            "--skip real_launched_handoff::production_launcher_reports_all_roles_from_provisioned_real_distributions",
            "observed required missing-platform-bundle refusal",
            "observed required missing-collector-bundle refusal",
            "partial probe requires SEMAPRAX_DOCTOR_REAL_BUNDLE to be absent",
            "partial probe requires SEMAPRAX_DOCTOR_REAL_SELECTOR to be absent",
            "grep -Fq 'provision real bundle'",
            "grep -Fq 'provision real selector'",
        ],
    )?;
    aarch64_local_driver_boundary(local_driver)?;
    require(
        guard_policy,
        "AArch64 closed syscall policy",
        &[
            "const ARM_ARCH: u32 = 0xc000_00b7;",
            "const ARM_COMMON",
            "const ARM_MANDATORY_DENY",
            "const ARM_SAFE_ADDITIONS: &[u32] = &[];",
            "ARM_ARCH => (",
            "ARM_COMMON,",
            "ARM_MANDATORY_DENY,",
        ],
    )?;
    require(
        guard_tests,
        "AArch64 default-deny proof",
        &[
            "fn complete_syscall_selection_is_default_deny_on_both_native_abis()",
            "for arch in [X86_ARCH, ARM_ARCH]",
        ],
    )?;
    require(
        provisioner_admission,
        "AArch64 provisioner admission",
        &[
            "DoctorOfflineArchitecture::LinuxAarch64",
            "crate::doctor::DoctorOfflineArchitecture::LinuxAarch64 => 183u16",
        ],
    )?;
    Ok(())
}

#[test]
fn production_provisioner_source_layout_tripwires_are_present() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    provisioner_source_tripwires(&Sources::checked_in(repository))
        .unwrap_or_else(|error| panic!("{error}"));
}

/// `scripts/doctor-provisioned-linux-gate.py --self-test` needs no Linux host
/// or provisioning: it drives the gate's pure decision logic with synthetic
/// inputs and walks the checked-in source tree for `#[ignore]`d lifecycle
/// tests the gate's fixed selection has drifted from (see
/// `docs/DOCTOR-PROVISIONED-LINUX-GATE-V1.md`'s "Test selection" section).
/// Nothing before this test ran it anywhere except two workflows that an
/// ordinary run never selects (both
/// `.github/workflows/doctor-provisioned-linux.yml` and
/// `.github/workflows/doctor-hosted-runner-probe.yml` are `workflow_dispatch`
/// only): a genuine drift between the gate's selection and the tree could go
/// unnoticed for as long as nobody happens to dispatch either workflow by
/// hand. Running it here, in an ordinary workspace test target, makes every
/// `main` push and every manually dispatched CI run prove it instead.
#[test]
fn provisioned_linux_gate_self_test_passes_and_stays_nonvacuous() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = std::process::Command::new("python3")
        .args(["scripts/doctor-provisioned-linux-gate.py", "--self-test"])
        .current_dir(repository)
        .output()
        .unwrap_or_else(|error| panic!("spawn doctor-provisioned-linux-gate.py: {error}"));
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let summary = stdout
        .lines()
        .find(|line| line.starts_with("self-test: "))
        .unwrap_or_else(|| panic!("no self-test summary line in:\n{stdout}"));
    let (passed, total) = summary
        .trim_start_matches("self-test: ")
        .split_once('/')
        .and_then(|(passed, rest)| {
            let total = rest.split_whitespace().next()?;
            Some((passed.parse::<u32>().ok()?, total.parse::<u32>().ok()?))
        })
        .unwrap_or_else(|| panic!("unparseable self-test summary: {summary}"));
    assert!(total > 0, "self-test ran zero checks: {summary}");
    assert_eq!(
        passed, total,
        "self-test did not pass every check: {summary}"
    );
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
        "launcher inherited host root",
        checked_in
            .linux_child
            .replacen("libc::SYS_pivot_root", "libc::SYS_getpid", 1),
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
        checked_in.capsule_core.replacen(
            "key.verify_strict(body, &Signature::from_bytes(&signature_bytes))",
            "Ok(())",
            1,
        ),
        "capsule_core",
    ));

    for (name, mutation, field) in mutations {
        let mut hostile = checked_in.clone();
        match field {
            "root" => hostile.root = mutation,
            "admission" => hostile.admission = mutation,
            "capsule" => hostile.capsule = mutation,
            "capsule_core" => hostile.capsule_core = mutation,
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

#[test]
fn aarch64_linux_tracking_contract_is_separate_fail_closed_and_non_promotional() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tracking = read(repository, "docs/DOCTOR-PROVISIONED-LINUX-AARCH64-V1.md");
    let workflow = read(
        repository,
        ".github/workflows/doctor-provisioned-linux-aarch64.yml",
    );
    let local_driver = read(
        repository,
        "scripts/doctor-provisioned-linux-aarch64-local-lifecycle.sh",
    );
    let guard_policy = read(
        repository,
        "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_worker/guard.rs",
    );
    let guard_tests = read(
        repository,
        "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_worker/guard/tests.rs",
    );
    let provisioner_admission = read(
        repository,
        "crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner/admission.rs",
    );
    aarch64_tracking_contract_tripwires(
        &tracking,
        &workflow,
        &local_driver,
        &guard_policy,
        &guard_tests,
        &provisioner_admission,
    )
    .unwrap_or_else(|error| panic!("{error}"));

    let added_lifecycle_case = local_driver.replacen(
        "\t\"doctor::offline_worker::tests::provisioned_overflow_and_timeout_publish_only_settled_failure\"\n)",
        "\t\"doctor::offline_worker::tests::provisioned_overflow_and_timeout_publish_only_settled_failure\"\n\t\"doctor::offline_worker::tests::unexpected_new_ignored_case\"\n)",
        1,
    );
    assert!(
        aarch64_local_driver_boundary(&added_lifecycle_case).is_err(),
        "an added ignored lifecycle case must require an explicit reviewed 24-case-plan update"
    );
    for refusal in [
        "fail \"platform real-distribution fixture unexpectedly passed without the contracted provisioned bundle\"",
        "fail \"collector real-distribution fixture unexpectedly passed without the contracted provisioned bundle\"",
    ] {
        let fail_open = local_driver.replacen(refusal, "true", 1);
        assert_ne!(fail_open, local_driver, "missing refusal mutation anchor");
        assert!(
            aarch64_local_driver_boundary(&fail_open).is_err(),
            "an unexpectedly passing excluded fixture must fail the tracking route"
        );
    }
    let masked_lifecycle = local_driver.replacen(
        "\t\t\"${PLATFORM_LIFECYCLE_TESTS[@]}\"",
        "\t\t\"${PLATFORM_LIFECYCLE_TESTS[@]}\" || true",
        1,
    );
    assert_ne!(
        masked_lifecycle, local_driver,
        "missing lifecycle mutation anchor"
    );
    assert!(
        aarch64_tracking_contract_tripwires(
            &tracking,
            &workflow,
            &masked_lifecycle,
            &guard_policy,
            &guard_tests,
            &provisioner_admission,
        )
        .is_err(),
        "a masked selected lifecycle command must fail the tracking route"
    );

    for (name, hostile_tracking, hostile_workflow) in [
        (
            "hosted promotion",
            tracking.replacen(
                "unexecuted on the proposed hosted runner",
                "HOSTED GREEN",
                1,
            ),
            workflow.clone(),
        ),
        (
            "foreign runner",
            tracking.clone(),
            workflow.replacen("ubuntu-24.04-arm", "ubuntu-24.04", 1),
        ),
        (
            "push trigger",
            tracking.clone(),
            format!("{workflow}\npush:\n  branches: [main]\n"),
        ),
        (
            "scheduled trigger",
            tracking.clone(),
            workflow.replacen(
                "  workflow_dispatch:\n",
                "  workflow_dispatch:\n  schedule:\n    - cron: '* * * * *'\n",
                1,
            ),
        ),
        (
            "reusable workflow trigger",
            tracking.clone(),
            workflow.replacen(
                "  workflow_dispatch:\n",
                "  workflow_dispatch:\n  workflow_call:\n",
                1,
            ),
        ),
        (
            "repository dispatch trigger",
            tracking.clone(),
            workflow.replacen(
                "  workflow_dispatch:\n",
                "  workflow_dispatch:\n  repository_dispatch:\n",
                1,
            ),
        ),
        (
            "additional job",
            tracking.clone(),
            workflow.replacen(
                "jobs:\n",
                "jobs:\n  unexpected-second-job:\n    runs-on: ubuntu-24.04-arm\n    steps: []\n",
                1,
            ),
        ),
        (
            "additional step",
            tracking.clone(),
            format!("{workflow}\n      - name: Unexpected extra step\n        run: true\n"),
        ),
        (
            "write permission",
            tracking.clone(),
            workflow.replacen("contents: read", "contents: write", 1),
        ),
        (
            "weakened native-host assertion",
            tracking.clone(),
            workflow.replacen("test \"$(uname -s)\" = Linux", "true", 1),
        ),
        (
            "unlocked dependency fetch",
            tracking.clone(),
            workflow.replacen("run: cargo fetch --locked", "run: cargo fetch", 1),
        ),
        (
            "online lifecycle probe",
            tracking.clone(),
            workflow.replacen(
                "CARGO_NET_OFFLINE: \"true\"",
                "CARGO_NET_OFFLINE: \"false\"",
                1,
            ),
        ),
        (
            "substituted lifecycle driver",
            tracking.clone(),
            workflow.replacen(
                "run: bash scripts/doctor-provisioned-linux-aarch64-local-lifecycle.sh",
                "run: true",
                1,
            ),
        ),
        (
            "masked host failure",
            tracking.clone(),
            format!("{workflow}\ncontinue-on-error: true\n"),
        ),
    ] {
        assert!(
            aarch64_tracking_contract_tripwires(
                &hostile_tracking,
                &hostile_workflow,
                &local_driver,
                &guard_policy,
                &guard_tests,
                &provisioner_admission,
            )
            .is_err(),
            "hostile {name} mutation escaped the AArch64 tracking tripwire"
        );
    }
}
