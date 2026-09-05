//! Closed, compiler-bundled standard-library dependency resolution.
//!
//! Ordinary packages still require the explicit offline resolver/cache path.
//! This module admits only the immutable `std.*` sources compiled into this
//! binary, checks the manifest range against their exact version, expands the
//! small transitive closure, and grants no filesystem or network authority.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::package_range::{self, Version};
use crate::semantic_workspace::SemanticWorkspaceSource;

use super::ProjectManifest;

const VERSION: Version = Version(0, 1, 0);

struct BundledPackage {
    name: &'static str,
    path: &'static str,
    source: &'static str,
    dependencies: &'static [&'static str],
}

const PACKAGES: &[BundledPackage] = &[
    BundledPackage {
        name: "std.bytes",
        path: "dependencies/std.bytes/0.1.0/bytes.spx",
        source: include_str!("../../std/bytes/src/bytes.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.core",
        path: "dependencies/std.core/0.1.0/core.spx",
        source: include_str!("../../std/core/src/core.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.data.csv",
        path: "dependencies/std.data.csv/0.1.0/csv.spx",
        source: include_str!("../../std/data-csv/src/csv.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.data.json",
        path: "dependencies/std.data.json/0.1.0/json.spx",
        source: include_str!("../../std/data-json/src/json.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.data.toml",
        path: "dependencies/std.data.toml/0.1.0/toml.spx",
        source: include_str!("../../std/data-toml/src/toml.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.encoding",
        path: "dependencies/std.encoding/0.1.0/encoding.spx",
        source: include_str!("../../std/encoding/src/encoding.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.num",
        path: "dependencies/std.num/0.1.0/num.spx",
        source: include_str!("../../std/num/src/num.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.num.overflow",
        path: "dependencies/std.num.overflow/0.1.0/overflow.spx",
        source: include_str!("../../std/num-overflow/src/overflow.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.path",
        path: "dependencies/std.path/0.1.0/path.spx",
        source: include_str!("../../std/path/src/path.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.random",
        path: "dependencies/std.random/0.1.0/random.spx",
        source: include_str!("../../std/random/src/random.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.test",
        path: "dependencies/std.test/0.1.0/test.spx",
        source: include_str!("../../std/test/src/test.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.text",
        path: "dependencies/std.text/0.1.0/text.spx",
        source: include_str!("../../std/text/src/text.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.time",
        path: "dependencies/std.time/0.1.0/time.spx",
        source: include_str!("../../std/time/src/time.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.url",
        path: "dependencies/std.url/0.1.0/url.spx",
        source: include_str!("../../std/url/src/url.spx"),
        dependencies: &["std.encoding"],
    },
];

pub(super) fn extend_sources(
    manifest: &ProjectManifest,
    sources: &mut Vec<SemanticWorkspaceSource>,
) -> Result<(), Vec<Diagnostic>> {
    let mut selected = BTreeSet::new();
    let mut pending = Vec::new();
    for dependency in manifest.dependencies() {
        let Some(package) = package(dependency.name()) else {
            if manifest
                .dependency_sources()
                .iter()
                .any(|source| source.name() == dependency.name())
            {
                continue;
            }
            return Err(unresolved(format!(
                "dependency `{}` is not a compiler-bundled standard-library package",
                dependency.name()
            )));
        };
        let range = package_range::parse_range(dependency.range(), range_error)
            .map_err(|error| vec![error])?;
        if !range.contains(VERSION) {
            return Err(unresolved(format!(
                "dependency `{}` range `{}` does not admit bundled version 0.1.0",
                dependency.name(),
                dependency.range()
            )));
        }
        pending.push(package.name);
    }
    while let Some(name) = pending.pop() {
        if !selected.insert(name) {
            continue;
        }
        let package = package(name).expect("bundled transitive dependency is closed");
        pending.extend(package.dependencies.iter().copied());
    }
    for name in selected {
        let package = package(name).expect("selected bundled dependency exists");
        sources.push(SemanticWorkspaceSource {
            path: package.path.to_owned(),
            source: package.source.to_owned(),
        });
    }
    Ok(())
}

fn package(name: &str) -> Option<&'static BundledPackage> {
    PACKAGES.iter().find(|package| package.name == name)
}

pub(super) fn is_bundled(name: &str) -> bool {
    package(name).is_some()
}

fn unresolved(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J121", message).with_help(
        "use a bundled `std.*` dependency at version 0.1.0, or list an ordinary package's complete exact Subject-v3 closure under `[dependency-sources]`",
    )]
}

fn range_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-J121", format!("standard-library dependency {message}"))
}
