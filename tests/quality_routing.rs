use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::quality_route;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

#[test]
fn profiles_are_deterministic_and_broad_dispatch_files_force_full() {
    let repository = Repository::new();
    assert_plan(&repository, "quick", "effective\tquick\n");
    assert_plan(&repository, "full", "effective\tfull\n");

    repository.write("src/main.rs", "fn main() { println!(\"changed\"); }\n");
    let plan = repository.changed_plan(&[]).unwrap();
    assert!(plan.contains("effective\tfull\n"));
    assert!(plan.contains("path\tsrc/main.rs\tbroad-compiler-or-graph-dispatch\tfull-workspace\n"));
    assert!(plan.contains("gate\ttest-workspace\n"));
}

#[test]
fn changed_reconciles_every_git_channel_and_rejects_incomplete_lists() {
    let repository = Repository::new();
    repository.write("docs/staged.md", "staged change\n");
    repository.git(&["add", "docs/staged.md"]);
    repository.write("README.md", "unstaged change\n");
    repository.write("docs/untracked.md", "untracked\n");
    fs::remove_file(repository.directory.join("docs/deleted.md")).unwrap();
    repository.git(&["mv", "docs/renamed-old.md", "docs/renamed-new.md"]);

    let plan = repository.changed_plan(&[]).unwrap();
    assert!(plan.contains("effective\tchanged\n"));
    for path in [
        "README.md",
        "docs/deleted.md",
        "docs/renamed-new.md",
        "docs/renamed-old.md",
        "docs/staged.md",
        "docs/untracked.md",
    ] {
        assert!(plan.contains(&format!("path\t{path}\t")), "missing {path}");
    }
    assert_eq!(plan, repository.changed_plan(&[]).unwrap());

    let error = repository
        .changed_plan(&["docs/staged.md".to_owned()])
        .unwrap_err();
    assert!(error.contains("do not equal Git state"));
}

#[test]
fn hostile_aliases_and_nonexistent_extras_fail_closed() {
    let repository = Repository::new();
    repository.write("docs/staged.md", "changed\n");
    let mut hostile = [
        "../docs/staged.md",
        "./docs/staged.md",
        "/docs/staged.md",
        "docs//staged.md",
        "docs/",
        "docs\\..\\src\\main.rs",
        "docs/STAGED.md",
        "docs/control\u{7f}.md",
        "docs/trailing.",
        "docs/trailing ",
        "docs/file:stream",
        "docs/nonexistent.md",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for character in ['<', '>', '"', '|', '?', '*'] {
        hostile.push(format!("docs/bad{character}.md"));
    }
    for device in windows_reserved_devices() {
        hostile.push(format!("docs/{device}.txt"));
    }
    for hostile in hostile {
        assert!(
            repository
                .changed_plan(std::slice::from_ref(&hostile))
                .is_err(),
            "hostile alias was accepted: {hostile:?}"
        );
    }
}

fn windows_reserved_devices() -> Vec<String> {
    let mut devices = ["CON", "con", "PRN", "AUX", "NUL"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for prefix in ["COM", "LPT"] {
        for digit in 1..=9 {
            devices.push(format!("{prefix}{digit}"));
        }
    }
    devices.extend(
        ["COM¹", "com²", "CoM³", "LPT¹", "lpt²", "LpT³"]
            .into_iter()
            .map(str::to_owned),
    );
    devices
}

#[cfg(unix)]
#[test]
fn changed_rejects_symlink_paths_even_when_the_target_stays_inside_the_repo() {
    use std::os::unix::fs::symlink;

    let repository = Repository::new();
    symlink(
        repository.directory.join("docs/staged.md"),
        repository.directory.join("docs/link.md"),
    )
    .unwrap();
    let error = repository.changed_plan(&[]).unwrap_err();
    assert!(error.contains("traverses a symlink"));
}

#[test]
fn committed_wide_change_and_dirty_documentation_route_full_from_exact_base() {
    let repository = Repository::new();
    let base = repository.baseline.clone();
    repository.write(
        "src/main.rs",
        "fn main() { println!(\"committed wide\"); }\n",
    );
    fs::remove_file(repository.directory.join("docs/deleted.md")).unwrap();
    repository.git(&["add", "-A"]);
    repository.commit("wide change");
    repository.write("docs/staged.md", "dirty documentation\n");

    let plan =
        quality_route::plan_with_base(&repository.directory, "changed", &[], Some(&base)).unwrap();
    assert!(plan.contains(&format!("base\t{base}\n")));
    assert!(plan.contains("effective\tfull\n"));
    assert!(plan.contains("path\tsrc/main.rs\tbroad-compiler-or-graph-dispatch"));
    assert!(plan.contains("path\tdocs/deleted.md\tdocumentation-truth"));
    assert!(plan.contains("path\tdocs/staged.md\tdocumentation-truth"));
}

#[test]
fn changed_requires_a_canonical_ancestor_base() {
    let repository = Repository::new();
    repository.write("docs/staged.md", "dirty\n");
    for base in ["HEAD", "ABCDEF", "0000000000000000000000000000000000000000"] {
        assert!(
            quality_route::plan_with_base(&repository.directory, "changed", &[], Some(base),)
                .is_err()
        );
    }
}

#[test]
fn changed_does_not_use_a_current_branch_upstream_that_equals_head() {
    let repository = Repository::new();
    repository.write("src/main.rs", "fn main() { println!(\"pushed\"); }\n");
    repository.git(&["add", "src/main.rs"]);
    repository.commit("pushed feature");
    let branch = git_output(&repository.directory, &["branch", "--show-current"]);
    repository.git(&["config", &format!("branch.{branch}.remote"), "."]);
    repository.git(&[
        "config",
        &format!("branch.{branch}.merge"),
        &format!("refs/heads/{branch}"),
    ]);
    assert_eq!(
        git_output(&repository.directory, &["rev-parse", "@{upstream}"]),
        git_output(&repository.directory, &["rev-parse", "HEAD"])
    );

    let output = quality_cli(&repository)
        .env_remove("SEMAPRAX_QUALITY_BASE")
        .env_remove("SEMAPRAX_QUALITY_TARGET_REF")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("configured origin/HEAD"));
}

#[test]
fn changed_rejects_multiple_merge_bases_from_the_default_target_branch() {
    let repository = Repository::new();
    let root_commit = repository.baseline.clone();
    let tree = git_output(&repository.directory, &["rev-parse", "HEAD^{tree}"]);
    let a1 = commit_tree(&repository.directory, &tree, &[&root_commit], "a1");
    let b1 = commit_tree(&repository.directory, &tree, &[&root_commit], "b1");
    let a2 = commit_tree(&repository.directory, &tree, &[&a1, &b1], "a2");
    let b2 = commit_tree(&repository.directory, &tree, &[&b1, &a1], "b2");
    repository.git(&["update-ref", "HEAD", &a2]);
    repository.git(&["update-ref", "refs/remotes/origin/main", &b2]);
    repository.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);

    let output = quality_cli(&repository)
        .env_remove("SEMAPRAX_QUALITY_BASE")
        .env_remove("SEMAPRAX_QUALITY_TARGET_REF")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("exactly one merge base"));
}

#[test]
fn changed_rejects_a_target_branch_without_a_merge_base() {
    let repository = Repository::new();
    let tree = git_output(&repository.directory, &["rev-parse", "HEAD^{tree}"]);
    let unrelated = commit_tree(&repository.directory, &tree, &[], "unrelated root");
    repository.git(&["update-ref", "refs/remotes/origin/main", &unrelated]);
    repository.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);

    let output = quality_cli(&repository)
        .env_remove("SEMAPRAX_QUALITY_BASE")
        .env_remove("SEMAPRAX_QUALITY_TARGET_REF")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn changed_rejects_non_utf8_base_environment_without_fallback() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let repository = Repository::new();
    let output = quality_cli(&repository)
        .env("SEMAPRAX_QUALITY_BASE", OsString::from_vec(vec![0xff]))
        .env_remove("SEMAPRAX_QUALITY_TARGET_REF")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be valid UTF-8"));
}

#[test]
#[cfg(unix)]
fn quality_executor_requires_each_profiles_exact_ordered_gate_sequence() {
    let fixture = TempDirectory::new("quality-executor");
    fs::create_dir(fixture.path.join("scripts")).unwrap();
    fs::create_dir(fixture.path.join("bin")).unwrap();
    fs::copy(
        root().join("scripts/quality.sh"),
        fixture.path.join("scripts/quality.sh"),
    )
    .unwrap();
    fs::copy(
        root().join("scripts/quality-route.sh"),
        fixture.path.join("scripts/quality-route.sh"),
    )
    .unwrap();
    let cargo = fixture.path.join("bin/cargo");
    let git = fixture.path.join("bin/git");
    fs::write(
        &cargo,
        r#"#!/bin/sh
case "$*" in
  *"quality-plan quick"*)
    printf 'schema\tsemaprax.quality-route.v2\nrequested\tquick\neffective\tquick\nreason\ttest\nbase\tnot-applicable\n'
    case "${HOSTILE_PLAN:-normal}" in
      normal) printf 'gate\tdiff-check\ngate\tfmt-check\ngate\tcheck-workspace\ngate\ttest-advisory\n' ;;
      missing) printf 'gate\tdiff-check\ngate\tfmt-check\ngate\tcheck-workspace\n' ;;
      duplicate) printf 'gate\tdiff-check\ngate\tfmt-check\ngate\tfmt-check\ngate\ttest-advisory\n' ;;
      reordered) printf 'gate\tfmt-check\ngate\tdiff-check\ngate\tcheck-workspace\ngate\ttest-advisory\n' ;;
      wrong-profile) printf 'gate\tdiff-check\ngate\tfmt-check\ngate\tcheck-workspace\ngate\tclippy-workspace\n' ;;
      unknown) printf 'gate\tdiff-check\ngate\tfmt-check\ngate\tcheck-workspace\ngate\tunknown\n' ;;
    esac
    printf 'end\tquality-plan\n'
    ;;
  *"quality-plan changed"*)
    printf 'schema\tsemaprax.quality-route.v2\nrequested\tchanged\neffective\tchanged\nreason\ttest\nbase\t0000000000000000000000000000000000000000\ngate\tdiff-check\ngate\tfmt-check\ngate\tcheck-workspace\ngate\ttest-advisory\ngate\tclippy-package\ngate\ttest-agent-context\ngate\trustdoc-package\nend\tquality-plan\n'
    ;;
  *"quality-plan full"*)
    printf 'schema\tsemaprax.quality-route.v2\nrequested\tfull\neffective\tfull\nreason\ttest\nbase\tnot-applicable\ngate\tdiff-check\ngate\tfmt-check\ngate\tcheck-workspace\ngate\ttest-advisory\ngate\tclippy-workspace\ngate\ttest-workspace\ngate\tdoctest-workspace\ngate\trustdoc-workspace\ngate\tbuild-release\ngate\tpackage\ngate\texample-checks\ngate\texample-fmt\nend\tquality-plan\n'
    ;;
  *) printf 'cargo:%s\n' "$*" >>"$QUALITY_LOG" ;;
esac
"#,
    )
    .unwrap();
    fs::write(
        &git,
        "#!/bin/sh\nprintf 'git:%s\\n' \"$*\" >>\"$QUALITY_LOG\"\n",
    )
    .unwrap();
    make_executable(&cargo);
    make_executable(&git);

    let log = fixture.path.join("quality.log");
    let path = format!(
        "{}:{}",
        fixture.path.join("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let success = Command::new("sh")
        .arg("scripts/quality.sh")
        .arg("quick")
        .current_dir(&fixture.path)
        .env("PATH", &path)
        .env("QUALITY_LOG", &log)
        .output()
        .unwrap();
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    let executed = fs::read_to_string(&log).unwrap();
    assert!(executed.contains("git:diff --check\n"));
    assert!(executed.contains("cargo:fmt --all --check\n"));
    assert!(executed.contains("cargo:check --locked --workspace --all-targets --all-features\n"));
    assert!(executed.contains("--test quality_routing"));
    assert!(String::from_utf8_lossy(&success.stderr).contains("==> diff-check"));

    fs::write(&log, "").unwrap();
    let help = Command::new("sh")
        .arg("scripts/quality.sh")
        .arg("--help")
        .current_dir(&fixture.path)
        .env("PATH", &path)
        .env("QUALITY_LOG", &log)
        .output()
        .unwrap();
    assert!(help.status.success());
    let help_stdout = String::from_utf8_lossy(&help.stdout);
    assert!(help_stdout.contains("Usage: scripts/quality.sh"));
    assert!(help_stdout.contains("--plan"));
    assert_eq!(fs::read_to_string(&log).unwrap(), "");

    let plan_only = Command::new("sh")
        .arg("scripts/quality.sh")
        .args(["quick", "--plan"])
        .current_dir(&fixture.path)
        .env("PATH", &path)
        .env("QUALITY_LOG", &log)
        .output()
        .unwrap();
    assert!(plan_only.status.success());
    assert!(String::from_utf8_lossy(&plan_only.stdout).contains("effective\tquick"));
    assert_eq!(fs::read_to_string(&log).unwrap(), "");

    for profile in ["changed", "full"] {
        fs::write(&log, "").unwrap();
        let output = Command::new("sh")
            .arg("scripts/quality.sh")
            .arg(profile)
            .current_dir(&fixture.path)
            .env("PATH", &path)
            .env("QUALITY_LOG", &log)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "profile {profile}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for hostile in [
        "missing",
        "duplicate",
        "reordered",
        "wrong-profile",
        "unknown",
    ] {
        fs::write(&log, "").unwrap();
        let rejected = Command::new("sh")
            .arg("scripts/quality.sh")
            .arg("quick")
            .current_dir(&fixture.path)
            .env("PATH", &path)
            .env("QUALITY_LOG", &log)
            .env("HOSTILE_PLAN", hostile)
            .output()
            .unwrap();
        assert_eq!(rejected.status.code(), Some(2), "hostile: {hostile}");
        assert_eq!(fs::read_to_string(&log).unwrap(), "", "hostile: {hostile}");
    }
}

fn assert_plan(repository: &Repository, profile: &str, expected: &str) {
    let plan = quality_route::plan(&repository.directory, profile, &[]).unwrap();
    assert!(plan.starts_with("schema\tsemaprax.quality-route.v2\n"));
    assert!(plan.contains(expected));
    assert_eq!(
        plan,
        quality_route::plan(&repository.directory, profile, &[]).unwrap()
    );
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

struct Repository {
    directory: PathBuf,
    baseline: String,
    _temporary: TempDirectory,
}

impl Repository {
    fn new() -> Self {
        let temporary = TempDirectory::new("quality-routing");
        let directory = temporary.path.clone();
        fs::create_dir(directory.join("docs")).unwrap();
        fs::create_dir(directory.join("src")).unwrap();
        fs::write(directory.join("README.md"), "baseline\n").unwrap();
        fs::write(directory.join("docs/staged.md"), "baseline\n").unwrap();
        fs::write(directory.join("docs/deleted.md"), "baseline\n").unwrap();
        fs::write(directory.join("docs/renamed-old.md"), "baseline\n").unwrap();
        fs::write(directory.join("src/main.rs"), "fn main() {}\n").unwrap();
        git_at(&directory, &["init", "-q"]);
        git_at(&directory, &["add", "."]);
        git_at(
            &directory,
            &[
                "-c",
                "user.name=SEMAPRAX Test",
                "-c",
                "user.email=test@invalid.example",
                "commit",
                "-q",
                "-m",
                "baseline",
            ],
        );
        let baseline = git_output(&directory, &["rev-parse", "HEAD"]);
        Self {
            directory,
            baseline,
            _temporary: temporary,
        }
    }

    fn write(&self, path: &str, contents: &str) {
        fs::write(self.directory.join(path), contents).unwrap();
    }

    fn git(&self, arguments: &[&str]) {
        git_at(&self.directory, arguments);
    }

    fn commit(&self, message: &str) {
        self.git(&[
            "-c",
            "user.name=SEMAPRAX Test",
            "-c",
            "user.email=test@invalid.example",
            "commit",
            "-q",
            "-m",
            message,
        ]);
    }

    fn changed_plan(&self, paths: &[String]) -> Result<String, String> {
        quality_route::plan_with_base(&self.directory, "changed", paths, Some(&self.baseline))
    }
}

fn git_at(directory: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_output(directory: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end()
        .to_owned()
}

fn commit_tree(directory: &Path, tree: &str, parents: &[&str], message: &str) -> String {
    let mut command = Command::new("git");
    command
        .args([
            "-c",
            "user.name=SEMAPRAX Test",
            "-c",
            "user.email=test@invalid.example",
            "commit-tree",
            tree,
        ])
        .current_dir(directory);
    for parent in parents {
        command.args(["-p", parent]);
    }
    let output = command.args(["-m", message]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end()
        .to_owned()
}

fn quality_cli(repository: &Repository) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
    command
        .args(["quality-plan", "changed"])
        .current_dir(&repository.directory);
    command
}

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new(label: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

mod first_contribution {
    //! Drift gate for the onboarding walkthrough.
    //!
    //! `docs/FIRST-CONTRIBUTION.md` is the one document that hands a newcomer
    //! literal profile names, gate identifiers, path classifications, and
    //! routing reasons to type and to read back out of a plan. Prose drifts
    //! silently: a renamed gate leaves the walkthrough telling a first
    //! contributor to run something that no longer exists, which is the exact
    //! failure onboarding documentation cannot afford.
    //!
    //! Every identifier the walkthrough cites is therefore bound here to the
    //! file that owns it: the executor `scripts/quality.sh` for profiles and
    //! gates, the router `src/quality_route.rs` for classifications, reasons,
    //! and the refusals the walkthrough quotes. The checks run in both the
    //! `test-advisory` and `test-agent-context` gates, which already select
    //! this target, so a documentation-only change set exercises them.

    use std::collections::BTreeSet;
    use std::fs;

    const WALKTHROUGH: &str = "docs/FIRST-CONTRIBUTION.md";

    /// Profiles the walkthrough tells a contributor to select.
    const PROFILES: [&str; 3] = ["quick", "changed", "full"];

    /// Gate identifiers the walkthrough lists for those profiles.
    const GATES: [&str; 15] = [
        "diff-check",
        "fmt-check",
        "check-workspace",
        "test-advisory",
        "clippy-package",
        "test-agent-context",
        "rustdoc-package",
        "clippy-workspace",
        "test-workspace",
        "doctest-workspace",
        "rustdoc-workspace",
        "build-release",
        "package",
        "example-checks",
        "example-fmt",
    ];

    /// Path classifications and routing reasons the walkthrough quotes. The
    /// router decides these; they never appear in the executor.
    const ROUTES: [&str; 7] = [
        "documentation-truth",
        "agent-context-economics",
        "broad-compiler-or-graph-dispatch",
        "unmapped-or-wide",
        "complete-git-state-has-narrow-mappings",
        "git-state-includes-wide-or-unmapped-path",
        "changed-worktree-is-empty",
    ];

    /// Refusals the walkthrough quotes verbatim so a newcomer recognizes them.
    const REFUSALS: [&str; 2] = [
        "changed quality routing requires SEMAPRAX_QUALITY_BASE, SEMAPRAX_QUALITY_TARGET_REF, or configured origin/HEAD",
        "must be an exact refs/remotes/ reference",
    ];

    #[test]
    fn every_documented_profile_and_gate_exists_in_the_executor() {
        let script = read("scripts/quality.sh");
        let offered = usage_profiles(&script);
        let labels = case_labels(&script);

        for profile in PROFILES {
            assert!(
                offered.contains(profile),
                "scripts/quality.sh no longer offers the `{profile}` profile that docs/FIRST-CONTRIBUTION.md documents"
            );
            assert!(
                script.contains(&format!("{profile}:0:")),
                "scripts/quality.sh no longer validates a gate sequence for the `{profile}` profile that docs/FIRST-CONTRIBUTION.md documents"
            );
        }

        for gate in GATES {
            assert!(
                labels.contains(gate),
                "scripts/quality.sh no longer dispatches the `{gate}` gate that docs/FIRST-CONTRIBUTION.md documents"
            );
            assert!(
                script.contains(&format!(":{gate}")),
                "scripts/quality.sh no longer admits `{gate}` in any validated gate sequence, though docs/FIRST-CONTRIBUTION.md documents it"
            );
        }

        assert!(
            script.contains("semaprax.quality-route.v2"),
            "scripts/quality.sh no longer validates the plan schema that docs/FIRST-CONTRIBUTION.md names"
        );
    }

    #[test]
    fn every_documented_classification_and_reason_exists_in_the_router() {
        let router = read("src/quality_route.rs");
        for route in ROUTES {
            assert!(
                router.contains(&format!("\"{route}\"")),
                "src/quality_route.rs no longer emits `{route}`, which docs/FIRST-CONTRIBUTION.md documents"
            );
        }
        assert!(
            router.contains("\"semaprax.quality-route.v2\""),
            "src/quality_route.rs no longer emits the plan schema that docs/FIRST-CONTRIBUTION.md names"
        );
    }

    #[test]
    fn the_walkthrough_quotes_the_routers_actual_refusals() {
        let router = read("src/quality_route.rs");
        let walkthrough = prose(&read(WALKTHROUGH));
        for refusal in REFUSALS {
            assert!(
                router.contains(refusal),
                "src/quality_route.rs no longer refuses with {refusal:?}, which docs/FIRST-CONTRIBUTION.md quotes"
            );
            assert!(
                walkthrough.contains(refusal),
                "docs/FIRST-CONTRIBUTION.md no longer quotes the refusal {refusal:?}"
            );
        }
    }

    #[test]
    fn the_walkthrough_states_the_owning_gates_module_size_facts() {
        let gate = read("tests/module_size.rs");
        let walkthrough = prose(&read(WALKTHROUGH));
        let regenerate =
            "cargo test --locked -p semaprax --test module_size -- --ignored regenerate";

        assert!(
            gate.contains("const LIMIT: usize = 1500;"),
            "tests/module_size.rs no longer enforces the 1500-line limit that docs/FIRST-CONTRIBUTION.md states"
        );
        assert!(
            walkthrough.contains("1500 lines"),
            "docs/FIRST-CONTRIBUTION.md no longer states the module size limit"
        );
        assert!(
            gate.contains(regenerate),
            "tests/module_size.rs no longer documents the regeneration command that docs/FIRST-CONTRIBUTION.md repeats"
        );
        assert!(
            walkthrough.contains(regenerate),
            "docs/FIRST-CONTRIBUTION.md no longer shows the budget regeneration command"
        );
    }

    #[test]
    fn the_walkthrough_cites_exactly_the_identifiers_this_gate_tracks() {
        let walkthrough = read(WALKTHROUGH);
        let tracked = GATES.into_iter().chain(ROUTES).collect::<BTreeSet<_>>();

        for token in walkthrough.split('`') {
            if !identifier_shaped(token) {
                continue;
            }
            assert!(
                tracked.contains(token),
                "docs/FIRST-CONTRIBUTION.md cites `{token}`, which is not a gate identifier or route classification this gate binds to its owner"
            );
        }

        for identifier in GATES.into_iter().chain(ROUTES).chain(PROFILES) {
            assert!(
                walkthrough.contains(&format!("`{identifier}`")),
                "docs/FIRST-CONTRIBUTION.md no longer cites `{identifier}`; drop it from this gate's lists as well"
            );
        }
    }

    fn read(path: &str) -> String {
        fs::read_to_string(super::root().join(path)).unwrap()
    }

    /// Profile names the executor offers in the `Profiles:` block of its own
    /// usage text.
    fn usage_profiles(script: &str) -> BTreeSet<&str> {
        script
            .lines()
            .skip_while(|line| line.trim() != "Profiles:")
            .skip(1)
            .take_while(|line| !line.trim().is_empty())
            .filter_map(|line| line.split_whitespace().next())
            .collect()
    }

    /// Labels of every `case` arm in the executor, which is where each gate it
    /// dispatches appears. Plan record kinds share the shape, so a caller pairs
    /// this with the validated gate sequences rather than trusting it alone.
    fn case_labels(script: &str) -> BTreeSet<&str> {
        script
            .lines()
            .map(str::trim_start)
            .filter_map(|line| line.split_once(')').map(|(label, _)| label))
            .filter(|label| identifier_like(label))
            .collect()
    }

    /// Whether a token could be a gate or route identifier: lowercase ASCII
    /// words joined by interior hyphens.
    fn identifier_shaped(token: &str) -> bool {
        token.contains('-') && identifier_like(token)
    }

    fn identifier_like(token: &str) -> bool {
        !token.is_empty()
            && !token.starts_with('-')
            && !token.ends_with('-')
            && token
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
    }

    /// Markdown wraps prose and marks code with backticks, so a quoted message
    /// reaches the page across several lines. Compare on the words alone.
    fn prose(document: &str) -> String {
        document
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace('`', "")
    }
}
