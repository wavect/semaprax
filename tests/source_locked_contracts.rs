//! Source-locked contract coverage.
//!
//! Several gates in this repository assert over the *text* of a module rather
//! than its behaviour: they read a `.rs` file with `include_str!` or a path
//! read, then require substrings, forbid others, count occurrences, or slice a
//! region between two markers. Those gates are the only check on some
//! authority and hostile-input boundaries.
//!
//! Splitting a module silently narrows every such gate. The text simply moves
//! to a sibling file, the assertions still pass against the smaller root, and
//! the coverage is gone with nothing failing. That is the exact failure this
//! gate exists to prevent, and it has already happened twice: relocating the
//! inline tests of `native_callable_provider_v3` and splitting
//! `economic_agent.rs` both left a scan reading a fraction of what it had.
//!
//! The rule enforced here: when a contract reads a module root that has
//! submodules, it must also read those submodules. A reader satisfies that by
//! naming each submodule — by path (`include_str!("hir/resolve_expr.rs")`) or
//! by stem in a helper's name list (`read_module(root, &["observability"])`).
//! A reader that cannot be checked textually, because it joins the directory at
//! runtime, records the reason in `ACKNOWLEDGED`.
//!
//! Not every reader covers every submodule today, and some never did — a few
//! bind a root that has had submodules far longer than this gate has existed.
//! Demanding repository-wide coverage at once would say nothing about whether
//! coverage is being *lost*, which is the failure that matters. So the existing
//! shortfall is frozen in `tests/source-locked-coverage.tsv` and this gate fails
//! when it grows: when a split moves text out from under a contract, that
//! contract's uncovered count rises and the gate names it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Readers this gate cannot verify textually, with the reason. Adding a line
/// is a claim that the contract's coverage is sound for one of two reasons: it
/// joins the submodules at runtime, or the submodules carry nothing the
/// contract is responsible for.
const COVERAGE: &str = include_str!("source-locked-coverage.tsv");

const ACKNOWLEDGED: &[(&str, &str, &str)] = &[
    (
        "tests/hir_module_boundaries.rs",
        "src/hir.rs",
        "binds the root's own macro/`mod` ordering, not resolver bodies",
    ),
    (
        "tests/public_native_rust_sdk_ci_contract.rs",
        "tests/frame_payload_product_v1.rs",
        "joined at runtime by read_consumer, which walks the consumer directory",
    ),
    (
        "tests/public_native_rust_sdk_ci_contract.rs",
        "tests/public_native_rust_owned_data_sdk_v1.rs",
        "joined at runtime by read_consumer, which walks the consumer directory",
    ),
    (
        "tests/public_native_rust_sdk_v1.rs",
        "crates/semaprax-native-rust-interop-builder/src/implementation.rs",
        "positive existence checks for the public facade; every required name \
         resolves in the root or public_sdk/, and the private stage submodules \
         carry none of them",
    ),
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            rust_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

/// Repository-relative `.rs` paths a file names in a string literal.
fn referenced_sources(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for (index, _) in text.match_indices('"') {
        let rest = &text[index + 1..];
        let Some(end) = rest.find('"') else { continue };
        let literal = &rest[..end];
        if !literal.ends_with(".rs") || literal.contains('\n') {
            continue;
        }
        // `#[path = "x.rs"]` names a module to compile, not text to assert over.
        let before = text[..index].trim_end();
        if before.ends_with("#[path =") || before.ends_with("#[path=") {
            continue;
        }
        // normalise `../../src/x.rs` and `crates/y/src/x.rs` to a repo path
        let trimmed = literal.trim_start_matches("./");
        let cleaned = trimmed.rsplit("../").next().unwrap_or(trimmed);
        found.insert(cleaned.to_owned());
    }
    found
}

/// The submodule files a module root owns, if it has a directory.
/// A submodule, in both spellings a contract may use to address it.
struct Owned {
    from_repository: String,
    from_reader: Option<String>,
    /// The module name alone, as a helper's name list spells it.
    stem: String,
}

fn submodules(root: &Path, repository: &Path, reader_directory: &Path) -> Vec<Owned> {
    let Some(directory) = root.to_str().and_then(|p| p.strip_suffix(".rs")) else {
        return Vec::new();
    };
    let directory = Path::new(directory);
    if !directory.is_dir() {
        return Vec::new();
    }
    let Ok(text) = fs::read_to_string(root) else {
        return Vec::new();
    };
    let mut owned = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("mod ").or_else(|| {
            line.strip_prefix("pub mod ")
                .or_else(|| line.strip_prefix("pub(crate) mod "))
                .or_else(|| line.strip_prefix("pub(super) mod "))
        }) else {
            continue;
        };
        let Some(name) = rest.strip_suffix(';') else {
            continue;
        };
        let name = name.trim();
        // A module's own test submodule is not production text; a contract that
        // scans a module for authority or shape never covered it.
        if name == "tests" || name.ends_with("_tests") {
            continue;
        }
        let candidate = directory.join(format!("{name}.rs"));
        if candidate.is_file() {
            let from_repository = candidate
                .strip_prefix(repository)
                .unwrap_or(&candidate)
                .to_string_lossy()
                .replace('\\', "/");
            let from_reader = candidate
                .strip_prefix(reader_directory)
                .ok()
                .map(|path| path.to_string_lossy().replace('\\', "/"));
            owned.push(Owned {
                from_repository,
                from_reader,
                stem: name.to_owned(),
            });
        }
    }
    owned
}

#[test]
fn a_contract_that_reads_a_module_root_also_reads_its_submodules() {
    let repository = repository_root();
    let mut readers = Vec::new();
    rust_files(&repository.join("tests"), &mut readers);
    for entry in fs::read_dir(repository.join("crates"))
        .into_iter()
        .flatten()
    {
        let Ok(entry) = entry else { continue };
        rust_files(&entry.path().join("src"), &mut readers);
        rust_files(&entry.path().join("tests"), &mut readers);
    }

    let mut measured: BTreeMap<(String, String), usize> = BTreeMap::new();
    for reader in readers {
        let Ok(text) = fs::read_to_string(&reader) else {
            continue;
        };
        if !text.contains("include_str!") && !text.contains("read(") {
            continue;
        }
        let reader_relative = reader
            .strip_prefix(&repository)
            .unwrap_or(&reader)
            .to_string_lossy()
            .replace('\\', "/");
        // This gate names module paths in its own tables; it is not a contract.
        if reader_relative == "tests/source_locked_contracts.rs" {
            continue;
        }
        let reader_directory = reader.parent().unwrap_or(&repository).to_path_buf();
        let referenced = referenced_sources(&text);
        for candidate in &referenced {
            let absolute = [repository.join(candidate), reader_directory.join(candidate)]
                .into_iter()
                .find(|path| path.is_file());
            let Some(absolute) = absolute else { continue };
            let owned = submodules(&absolute, &repository, &reader_directory);
            if owned.is_empty() {
                continue;
            }
            if ACKNOWLEDGED
                .iter()
                .any(|(who, what, _)| *who == reader_relative && what == candidate)
            {
                continue;
            }
            let uncovered = owned
                .iter()
                .filter(|module| {
                    !referenced.contains(&module.from_repository)
                        && !module
                            .from_reader
                            .as_ref()
                            .is_some_and(|path| referenced.contains(path))
                        && !text.contains(&format!("\"{}\"", module.stem))
                })
                .count();
            if uncovered > 0 {
                measured.insert((reader_relative.clone(), candidate.clone()), uncovered);
            }
        }
    }

    let mut baseline: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (number, line) in COVERAGE.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut columns = line.split('\t');
        let (Some(reader), Some(root), Some(count)) =
            (columns.next(), columns.next(), columns.next())
        else {
            panic!(
                "coverage line {} is not <reader>\\t<root>\\t<uncovered>",
                number + 1
            );
        };
        let count = count
            .parse::<usize>()
            .unwrap_or_else(|_| panic!("coverage line {} has a non-numeric count", number + 1));
        baseline.insert((reader.to_owned(), root.to_owned()), count);
    }

    let mut grew = Vec::new();
    for ((reader, root), count) in &measured {
        match baseline.get(&(reader.clone(), root.clone())) {
            Some(recorded) if count <= recorded => {}
            Some(recorded) => grew.push(format!(
                "  {reader} now misses {count} submodules of {root}, was {recorded}"
            )),
            None => grew.push(format!(
                "  {reader} misses {count} submodules of {root}, previously covered all of them"
            )),
        }
    }
    let stale: Vec<_> = baseline
        .iter()
        .filter(|((reader, root), recorded)| {
            measured
                .get(&((*reader).clone(), (*root).clone()))
                .is_none_or(|count| count < *recorded)
        })
        .map(|((reader, root), recorded)| {
            let now = measured
                .get(&(reader.clone(), root.clone()))
                .copied()
                .unwrap_or(0);
            format!("  {reader} / {root}: recorded {recorded}, now {now}")
        })
        .collect();

    let mut failures = String::new();
    if !grew.is_empty() {
        failures.push_str(&format!(
            "these source-locked contracts cover less of the module they bind than they did. The \
             text they assert over has moved into submodules, so the assertions still pass while \
             checking less. Join the submodules into the string the contract reads — in original \
             source order when it slices regions:\n{}\n",
            grew.join("\n")
        ));
    }
    if !stale.is_empty() {
        failures.push_str(&format!(
            "these coverage entries improved; lower the recorded counts in \
             tests/source-locked-coverage.tsv so the ledger keeps shrinking:\n{}\n",
            stale.join("\n")
        ));
    }
    assert!(failures.is_empty(), "{failures}");
}
