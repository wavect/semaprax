//! Module size budget.
//!
//! Oversized modules are the main obstacle to reading this compiler, for
//! humans and for software agents alike: a reader pays for the whole file to
//! reach one item, and every edit competes with unrelated code in the same
//! blast radius.
//!
//! This gate keeps that pressure from returning. A Rust source file may not
//! exceed `LIMIT` lines unless it is recorded in the budget beside this test,
//! and a recorded file may not grow past the size it was recorded at. Files
//! are expected to leave the budget over time; an entry that no longer
//! exceeds `LIMIT` fails the gate so the ledger shrinks with the code.
//!
//! To regenerate after a legitimate split:
//!     cargo test --locked -p semaprax --test module_size -- --ignored regenerate

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Lines above which a Rust source file must carry a budget entry.
const LIMIT: usize = 1500;

const BUDGET: &str = include_str!("module-size-budget.tsv");

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Every `.rs` file that belongs to the workspace's own sources.
fn tracked_sources(root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![root.join("src"), root.join("tests")];
    if let Ok(crates) = fs::read_dir(root.join("crates")) {
        let mut entries: Vec<_> = crates.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for entry in entries {
            roots.push(entry.join("src"));
            roots.push(entry.join("tests"));
        }
    }

    let mut found = Vec::new();
    for base in roots {
        collect(&base, &mut found);
    }
    found.sort();
    found
}

fn collect(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

fn line_count(path: &Path) -> usize {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .lines()
        .count()
}

fn parse_budget() -> BTreeMap<String, usize> {
    let mut budget = BTreeMap::new();
    for (index, line) in BUDGET.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (path, max) = line
            .split_once('\t')
            .unwrap_or_else(|| panic!("budget line {} is not <path>\\t<max>: {line:?}", index + 1));
        let max = max.parse::<usize>().unwrap_or_else(|_| {
            panic!("budget line {} has a non-numeric max: {line:?}", index + 1)
        });
        assert!(
            max > LIMIT,
            "budget line {} records {path} at {max} lines, which is within the {LIMIT} line \
             limit; delete the entry instead",
            index + 1
        );
        assert!(
            budget.insert(path.to_owned(), max).is_none(),
            "budget lists {path} twice"
        );
    }
    budget
}

fn measure(root: &Path) -> BTreeMap<String, usize> {
    tracked_sources(root)
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(root)
                .expect("source path is inside the repository")
                .to_string_lossy()
                .replace('\\', "/");
            let count = line_count(&path);
            (relative, count)
        })
        .collect()
}

#[test]
fn no_module_exceeds_its_recorded_size() {
    let root = repository_root();
    let budget = parse_budget();
    let measured = measure(&root);

    let mut over_limit = Vec::new();
    let mut grown = Vec::new();
    for (path, count) in &measured {
        match budget.get(path) {
            None if *count > LIMIT => over_limit.push(format!("  {path}\t{count}")),
            Some(max) if count > max => {
                grown.push(format!("  {path}\t{count} (recorded at {max})"));
            }
            _ => {}
        }
    }

    let stale: Vec<_> = budget
        .keys()
        .filter(|path| match measured.get(*path) {
            Some(count) => *count <= LIMIT,
            None => true,
        })
        .map(|path| format!("  {path}"))
        .collect();

    let mut failures = String::new();
    if !over_limit.is_empty() {
        failures.push_str(&format!(
            "these files exceed the {LIMIT} line limit and are not budgeted; split them, or \
             record them in tests/module-size-budget.tsv:\n{}\n",
            over_limit.join("\n")
        ));
    }
    if !grown.is_empty() {
        failures.push_str(&format!(
            "these budgeted files grew past their recorded size; move the new code into a \
             submodule rather than raising the budget:\n{}\n",
            grown.join("\n")
        ));
    }
    if !stale.is_empty() {
        failures.push_str(&format!(
            "these budget entries are obsolete because the files no longer exceed {LIMIT} lines \
             (or no longer exist); delete the entries:\n{}\n",
            stale.join("\n")
        ));
    }

    assert!(failures.is_empty(), "{failures}");
}

/// Rewrites the budget from the working tree. Run deliberately, never to make
/// a failing gate pass without splitting the module that grew.
#[test]
#[ignore = "regenerates a checked-in file; run deliberately"]
fn regenerate() {
    let root = repository_root();
    let measured = measure(&root);
    let mut out = String::from(
        "# Rust source files permitted to exceed the module size limit, with the line count each\n\
         # was recorded at. Entries may shrink and disappear; they may not grow. Regenerate with\n\
         #     cargo test --locked -p semaprax --test module_size -- --ignored regenerate\n",
    );
    for (path, count) in measured.iter().filter(|(_, count)| **count > LIMIT) {
        out.push_str(&format!("{path}\t{count}\n"));
    }
    fs::write(root.join("tests/module-size-budget.tsv"), out).expect("write budget");
}
