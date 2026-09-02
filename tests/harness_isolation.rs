//! Fixture isolation within a merged test harness.
//!
//! Integration tests here build a temporary fixture root from a literal prefix,
//! the process id, and a per-module serial counter that starts at zero:
//!
//! ```ignore
//! format!("{}{}-{}", "spx-draft-archive-v5-", std::process::id(), SERIAL.fetch_add(1, ..))
//! ```
//!
//! That was unconditionally safe while every file was its own test binary,
//! because each had its own process id. Merging files into a shared harness to
//! stop linking the compiler once per binary removes that guarantee: the
//! modules now share a pid, and each module's counter still starts at zero. Two
//! modules in one harness that share a prefix therefore derive the *same* first
//! fixture path and interfere — intermittently, and only when both run, which is
//! the worst way to find out.
//!
//! The merges rely on every prefix in a harness being distinct. This checks it,
//! so a module added later cannot quietly reintroduce the collision.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Fixture prefixes a module names.
///
/// The prefix is nearly always a format string — `"spx-artifact-delta-{}-{}"` —
/// so the part that actually distinguishes one module's fixture root from
/// another's is the literal text before the first substitution.
fn prefixes(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (index, _) in source.match_indices("\"spx-") {
        let rest = &source[index + 1..];
        let Some(end) = rest.find('"') else { continue };
        let literal = &rest[..end];
        if literal.contains(' ') {
            continue;
        }
        let stem = literal.split('{').next().unwrap_or(literal);
        if stem.len() > "spx-".len() {
            found.push(stem.to_owned());
        }
    }
    found.sort();
    found.dedup();
    found
}

#[test]
fn modules_of_a_harness_do_not_share_a_fixture_prefix() {
    let tests = repository_root().join("tests");
    let mut collisions = Vec::new();

    let Ok(entries) = fs::read_dir(&tests) else {
        panic!("tests directory is readable");
    };
    let mut harnesses: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    harnesses.sort();

    for harness in harnesses {
        // Only a directory paired with a `tests/<name>.rs` harness shares a process.
        if !harness.with_extension("rs").is_file() {
            continue;
        }
        let Ok(modules) = fs::read_dir(&harness) else {
            continue;
        };
        let mut owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut paths: Vec<PathBuf> = modules
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        paths.sort();
        for module in paths {
            let Ok(source) = fs::read_to_string(&module) else {
                continue;
            };
            let name = module
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            for prefix in prefixes(&source) {
                owners.entry(prefix).or_default().push(name.clone());
            }
        }
        let harness_name = harness
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        for (prefix, mut modules) in owners {
            modules.dedup();
            if modules.len() > 1 {
                collisions.push(format!(
                    "  {harness_name}: {:?} is used by {}",
                    prefix,
                    modules.join(", ")
                ));
            }
        }
    }

    assert!(
        collisions.is_empty(),
        "these modules share a harness, and therefore a process id, while deriving their \
         temporary fixture root from the same prefix. Their first fixture paths collide and the \
         tests will interfere intermittently. Give each module a distinct prefix:\n{}",
        collisions.join("\n")
    );
}
