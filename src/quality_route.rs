//! Fail-closed, repository-aware local quality routing.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const SCHEMA: &str = "semaprax.quality-route.v2";
type PathRow = (String, &'static str, &'static str);

/// Build the canonical gate plan for a repository.
///
/// `changed` discovers committed base-to-HEAD, staged, unstaged, untracked,
/// deleted, and rename-side paths from Git. If explicit paths are supplied
/// they must equal that discovered set.
pub fn plan(
    repository: &Path,
    requested: &str,
    explicit_paths: &[String],
) -> Result<String, String> {
    plan_with_base(repository, requested, explicit_paths, None)
}

/// Build a plan with an exact committed comparison base.
///
/// This entry exists for deterministic tests and CI callers that already own
/// a reviewed base object ID. Ordinary callers should set
/// `SEMAPRAX_QUALITY_BASE`, configure `SEMAPRAX_QUALITY_TARGET_REF`, or set the
/// repository's `refs/remotes/origin/HEAD` symbolic default branch.
pub fn plan_with_base(
    repository: &Path,
    requested: &str,
    explicit_paths: &[String],
    explicit_base: Option<&str>,
) -> Result<String, String> {
    if !matches!(requested, "quick" | "changed" | "full") {
        return Err("quality profile must be quick, changed, or full".to_owned());
    }
    if requested != "changed" && !explicit_paths.is_empty() {
        return Err(format!(
            "quality profile `{requested}` does not accept path arguments"
        ));
    }
    let root = repository
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize quality repository: {error}"))?;
    let git_root = git(&root, &["rev-parse", "--show-toplevel"])?;
    let git_root = utf8(&git_root.stdout, "Git repository root")?.trim_end();
    let git_root = Path::new(git_root)
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize Git repository root: {error}"))?;
    if root != git_root {
        return Err("quality repository must be the exact Git worktree root".to_owned());
    }

    let (effective, reason, base, paths) = match requested {
        "quick" => (
            "quick",
            "explicit-advisory-preflight",
            "not-applicable".to_owned(),
            Vec::new(),
        ),
        "full" => (
            "full",
            "explicit-full",
            "not-applicable".to_owned(),
            Vec::new(),
        ),
        "changed" => {
            let base = resolve_base(&root, explicit_base)?;
            let (effective, reason, paths) = changed_plan(&root, &base, explicit_paths)?;
            (effective, reason, base, paths)
        }
        _ => unreachable!(),
    };
    let mut output = format!(
        "schema\t{SCHEMA}\nrequested\t{requested}\neffective\t{effective}\nreason\t{reason}\nbase\t{base}\n"
    );
    for (path, invariant, evidence) in paths {
        output.push_str(&format!("path\t{path}\t{invariant}\t{evidence}\n"));
    }
    for gate in gates(effective) {
        output.push_str(&format!("gate\t{gate}\n"));
    }
    output.push_str("end\tquality-plan\n");
    Ok(output)
}

fn changed_plan(
    root: &Path,
    base: &str,
    explicit_paths: &[String],
) -> Result<(&'static str, &'static str, Vec<PathRow>), String> {
    let discovered = discover_changes(root, base)?;
    let explicit = explicit_paths
        .iter()
        .map(|path| {
            validate_alias(path)?;
            Ok(path.clone())
        })
        .collect::<Result<BTreeSet<_>, String>>()?;
    if !explicit_paths.is_empty() && explicit.len() != explicit_paths.len() {
        return Err("explicit changed paths contain duplicates".to_owned());
    }
    if !explicit.is_empty() && explicit != discovered {
        let missing = discovered
            .difference(&explicit)
            .cloned()
            .collect::<Vec<_>>();
        let extra = explicit
            .difference(&discovered)
            .cloned()
            .collect::<Vec<_>>();
        return Err(format!(
            "explicit changed paths do not equal Git state (missing: {}; extra: {})",
            display_paths(&missing),
            display_paths(&extra)
        ));
    }
    if discovered.is_empty() {
        return Ok(("full", "changed-worktree-is-empty", Vec::new()));
    }

    let mut rows = Vec::with_capacity(discovered.len());
    let mut narrow = true;
    for path in discovered {
        validate_worktree_path(root, base, &path)?;
        let (invariant, evidence, mapped) = mapping(&path);
        narrow &= mapped;
        rows.push((path, invariant, evidence));
    }
    if narrow {
        Ok(("changed", "complete-git-state-has-narrow-mappings", rows))
    } else {
        Ok(("full", "git-state-includes-wide-or-unmapped-path", rows))
    }
}

fn discover_changes(root: &Path, base: &str) -> Result<BTreeSet<String>, String> {
    let mut paths = BTreeSet::new();
    let committed = format!("{base}...HEAD");
    for arguments in [
        vec![
            "diff",
            "--name-status",
            "-z",
            "--no-renames",
            &committed,
            "--",
        ],
        vec!["diff", "--name-status", "-z", "--no-renames", "--"],
        vec![
            "diff",
            "--cached",
            "--name-status",
            "-z",
            "--no-renames",
            "--",
        ],
    ] {
        let output = git(root, &arguments)?;
        let fields = nul_fields(&output.stdout, "Git diff")?;
        let mut pairs = fields.chunks_exact(2);
        for pair in &mut pairs {
            if !matches!(pair[0], "A" | "C" | "D" | "M" | "T" | "U" | "X" | "B") {
                return Err(format!(
                    "Git returned unsupported change status `{}`",
                    pair[0]
                ));
            }
            validate_alias(pair[1])?;
            paths.insert(pair[1].to_owned());
        }
        if !pairs.remainder().is_empty() {
            return Err("Git diff returned a malformed NUL record".to_owned());
        }
    }
    let output = git(
        root,
        &["ls-files", "--others", "--exclude-standard", "-z", "--"],
    )?;
    for path in nul_fields(&output.stdout, "Git untracked paths")? {
        validate_alias(path)?;
        paths.insert(path.to_owned());
    }
    Ok(paths)
}

fn validate_alias(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.contains('\\')
        || path.split('/').any(str::is_empty)
        || path.chars().any(char::is_control)
        || Path::new(path).is_absolute()
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "quality path `{path}` is not a canonical relative alias"
        ));
    }
    for component in Path::new(path).components() {
        let Component::Normal(component) = component else {
            return Err(format!(
                "quality path `{path}` is not a canonical relative alias"
            ));
        };
        let component = component
            .to_str()
            .ok_or_else(|| format!("quality path `{path}` is not UTF-8"))?;
        if !portable_windows_component(component) {
            return Err(format!(
                "quality path `{path}` is not portable across Windows filesystems"
            ));
        }
    }
    Ok(())
}

fn portable_windows_component(component: &str) -> bool {
    if component.ends_with('.')
        || component.ends_with(' ')
        || component.contains(':')
        || component
            .bytes()
            .any(|byte| matches!(byte, b'<' | b'>' | b'"' | b'|' | b'?' | b'*'))
    {
        return false;
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return false;
    }
    for prefix in ["COM", "LPT"] {
        let Some(suffix) = stem.strip_prefix(prefix) else {
            continue;
        };
        let ascii_device = suffix.len() == 1 && (b'1'..=b'9').contains(&suffix.as_bytes()[0]);
        if ascii_device || matches!(suffix, "¹" | "²" | "³") {
            return false;
        }
    }
    true
}

fn validate_worktree_path(root: &Path, base: &str, path: &str) -> Result<(), String> {
    validate_alias(path)?;
    let mut candidate = PathBuf::from(root);
    for component in Path::new(path).components() {
        let Component::Normal(component) = component else {
            return Err(format!("quality path `{path}` is not canonical"));
        };
        let exact_entry = match std::fs::read_dir(&candidate) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .any(|entry| entry.file_name() == component),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(format!("cannot inspect quality path `{path}`: {error}"));
            }
        };
        candidate.push(component);
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!("quality path `{path}` traverses a symlink"));
            }
            Ok(_) if !exact_entry => {
                return Err(format!(
                    "quality path `{path}` does not use exact directory-entry spelling"
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // A path discovered from Git may be a tracked deletion or the
                // old side of a rename. Its lexical alias remains authoritative.
                if tracked_in_index_head_or_base(root, base, path)? {
                    return Ok(());
                }
                return Err(format!(
                    "quality path `{path}` disappeared and is not a tracked deletion"
                ));
            }
            Err(error) => return Err(format!("cannot inspect quality path `{path}`: {error}")),
        }
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize quality path `{path}`: {error}"))?;
    if canonical.strip_prefix(root).is_err() {
        return Err(format!("quality path `{path}` escapes the worktree"));
    }
    Ok(())
}

fn resolve_base(root: &Path, explicit: Option<&str>) -> Result<String, String> {
    let configured = match explicit {
        Some(base) => Some(base.to_owned()),
        None => utf8_environment("SEMAPRAX_QUALITY_BASE")?,
    };
    let base = if let Some(base) = configured {
        validate_object_id(&base)?;
        base
    } else {
        let target_ref = match utf8_environment("SEMAPRAX_QUALITY_TARGET_REF")? {
            Some(target_ref) => target_ref,
            None => {
                let target = git(
                    root,
                    &["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"],
                )
                .map_err(|_| {
                    "changed quality routing requires SEMAPRAX_QUALITY_BASE, SEMAPRAX_QUALITY_TARGET_REF, or configured origin/HEAD"
                        .to_owned()
                })?;
                utf8(&target.stdout, "Git default remote branch")?
                    .trim_end()
                    .to_owned()
            }
        };
        validate_target_ref(root, &target_ref)?;
        let target = git(
            root,
            &["rev-parse", "--verify", &format!("{target_ref}^{{commit}}")],
        )?;
        let target = utf8(&target.stdout, "quality target commit")?.trim_end();
        validate_object_id(target)?;
        let merge_bases = git(root, &["merge-base", "--all", "HEAD", target])?;
        let merge_bases = utf8(&merge_bases.stdout, "Git merge bases")?
            .lines()
            .collect::<Vec<_>>();
        if merge_bases.len() != 1 {
            return Err(format!(
                "quality target must have exactly one merge base with HEAD, found {}",
                merge_bases.len()
            ));
        }
        validate_object_id(merge_bases[0])?;
        merge_bases[0].to_owned()
    };
    validate_object_id(&base)?;
    let verified = git(
        root,
        &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
    )?;
    let verified = utf8(&verified.stdout, "quality base")?.trim_end();
    if verified != base {
        return Err("quality base must be the exact canonical commit object ID".to_owned());
    }
    let ancestor = Command::new("git")
        .args(["merge-base", "--is-ancestor", &base, "HEAD"])
        .current_dir(root)
        .status()
        .map_err(|error| format!("cannot execute Git: {error}"))?;
    if !ancestor.success() {
        return Err("quality base must be an ancestor of HEAD".to_owned());
    }
    Ok(base)
}

fn utf8_environment(name: &str) -> Result<Option<String>, String> {
    let Some(value) = std::env::var_os(name) else {
        return Ok(None);
    };
    value
        .into_string()
        .map(Some)
        .map_err(|_| format!("{name} must be valid UTF-8"))
}

fn validate_target_ref(root: &Path, target_ref: &str) -> Result<(), String> {
    if !target_ref.starts_with("refs/remotes/")
        || target_ref.chars().any(char::is_control)
        || target_ref.trim() != target_ref
    {
        return Err("quality target ref must be an exact refs/remotes/ reference".to_owned());
    }
    git(root, &["check-ref-format", target_ref])?;
    Ok(())
}

fn validate_object_id(base: &str) -> Result<(), String> {
    if !matches!(base.len(), 40 | 64)
        || !base
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("quality base must be a canonical lowercase full object ID".to_owned());
    }
    Ok(())
}

fn tracked_in_index_head_or_base(root: &Path, base: &str, path: &str) -> Result<bool, String> {
    let index = Command::new("git")
        .args(["ls-files", "--error-unmatch", "--", path])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot execute Git: {error}"))?;
    if index.status.success() {
        return Ok(true);
    }
    for revision in ["HEAD", base] {
        let object = format!("{revision}:{path}");
        let tracked = Command::new("git")
            .args(["cat-file", "-e", &object])
            .current_dir(root)
            .output()
            .map_err(|error| format!("cannot execute Git: {error}"))?;
        if tracked.status.success() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn mapping(path: &str) -> (&'static str, &'static str, bool) {
    if path == "README.md"
        || path == "CHANGELOG.md"
        || (path.starts_with("docs/") && path.ends_with(".md"))
    {
        return (
            "documentation-truth",
            "documentation,examples,rustdoc",
            true,
        );
    }
    if path == "src/agent_economics.rs"
        || path == "tests/agent_economics.rs"
        || path.starts_with("benchmarks/agent-context-v1/")
        || path.starts_with("tests/snapshots/agent_context_")
        || path.starts_with("tests/snapshots/agent_economics.")
    {
        return (
            "agent-context-economics",
            "agent_context,agent_economics,documentation,examples",
            true,
        );
    }
    if matches!(path, "src/main.rs" | "src/graph.rs") {
        return ("broad-compiler-or-graph-dispatch", "full-workspace", false);
    }
    ("unmapped-or-wide", "full-workspace", false)
}

fn gates(profile: &str) -> &'static [&'static str] {
    match profile {
        "quick" => &[
            "diff-check",
            "fmt-check",
            "check-workspace",
            "test-advisory",
        ],
        "changed" => &[
            "diff-check",
            "fmt-check",
            "check-workspace",
            "test-advisory",
            "clippy-package",
            "test-agent-context",
            "rustdoc-package",
        ],
        "full" => &[
            "diff-check",
            "fmt-check",
            "check-workspace",
            "test-advisory",
            "clippy-workspace",
            "test-workspace",
            "doctest-workspace",
            "rustdoc-workspace",
            "build-release",
            "package",
            "example-checks",
            "example-fmt",
        ],
        _ => unreachable!(),
    }
}

fn git(root: &Path, arguments: &[&str]) -> Result<Output, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot execute Git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Git command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim_end()
        ));
    }
    Ok(output)
}

fn nul_fields<'a>(bytes: &'a [u8], label: &str) -> Result<Vec<&'a str>, String> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if bytes.last() != Some(&0) {
        return Err(format!("{label} omitted its terminal NUL"));
    }
    bytes[..bytes.len() - 1]
        .split(|byte| *byte == 0)
        .map(|field| utf8(field, label))
        .collect()
}

fn utf8<'a>(bytes: &'a [u8], label: &str) -> Result<&'a str, String> {
    std::str::from_utf8(bytes).map_err(|_| format!("{label} contains a non-UTF-8 path"))
}

fn display_paths(paths: &[String]) -> String {
    if paths.is_empty() {
        "none".to_owned()
    } else {
        paths.join(",")
    }
}
