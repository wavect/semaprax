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
        name: "std.agent",
        path: "dependencies/std.agent/0.1.0/agent.spx",
        source: include_str!("../../std/agent/src/agent.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.auth",
        path: "dependencies/std.auth/0.1.0/auth.spx",
        source: include_str!("../../std/auth/src/auth.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.bytes",
        path: "dependencies/std.bytes/0.1.0/bytes.spx",
        source: include_str!("../../std/bytes/src/bytes.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.collections",
        path: "dependencies/std.collections/0.1.0/collections.spx",
        source: include_str!("../../std/collections/src/collections.spx"),
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
        name: "std.data.json.dec",
        path: "dependencies/std.data.json.dec/0.1.0/dec.spx",
        source: include_str!("../../std/data-json-dec/src/dec.spx"),
        dependencies: &["std.io"],
    },
    BundledPackage {
        name: "std.data.json.digits",
        path: "dependencies/std.data.json.digits/0.1.0/digits.spx",
        source: include_str!("../../std/data-json-digits/src/digits.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.data.json.doc",
        path: "dependencies/std.data.json.doc/0.1.0/doc.spx",
        source: include_str!("../../std/data-json-doc/src/doc.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.data.json.token",
        path: "dependencies/std.data.json.token/0.1.0/token.spx",
        source: include_str!("../../std/data-json-token/src/token.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.data.json.utf8",
        path: "dependencies/std.data.json.utf8/0.1.0/utf8.spx",
        source: include_str!("../../std/data-json-utf8/src/utf8.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.data.json.write",
        path: "dependencies/std.data.json.write/0.1.0/write.spx",
        source: include_str!("../../std/data-json-write/src/write.spx"),
        dependencies: &["std.io"],
    },
    BundledPackage {
        name: "std.data.toml",
        path: "dependencies/std.data.toml/0.1.0/toml.spx",
        source: include_str!("../../std/data-toml/src/toml.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.db",
        path: "dependencies/std.db/0.1.0/db.spx",
        source: include_str!("../../std/db/src/db.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.encoding",
        path: "dependencies/std.encoding/0.1.0/encoding.spx",
        source: include_str!("../../std/encoding/src/encoding.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.env",
        path: "dependencies/std.env/0.1.0/env.spx",
        source: include_str!("../../std/env/src/env.spx"),
        dependencies: &["std.format", "std.io"],
    },
    BundledPackage {
        name: "std.format",
        path: "dependencies/std.format/0.1.0/format.spx",
        source: include_str!("../../std/format/src/format.spx"),
        dependencies: &["std.io"],
    },
    BundledPackage {
        name: "std.fs",
        path: "dependencies/std.fs/0.1.0/fs.spx",
        source: include_str!("../../std/fs/src/fs.spx"),
        dependencies: &["std.io", "std.path.value"],
    },
    BundledPackage {
        name: "std.http",
        path: "dependencies/std.http/0.1.0/http.spx",
        source: include_str!("../../std/http/src/http.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.io",
        path: "dependencies/std.io/0.1.0/io.spx",
        source: include_str!("../../std/io/src/io.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.jobs",
        path: "dependencies/std.jobs/0.1.0/jobs.spx",
        source: include_str!("../../std/jobs/src/jobs.spx"),
        dependencies: &["std.bytes"],
    },
    BundledPackage {
        name: "std.log",
        path: "dependencies/std.log/0.1.0/log.spx",
        source: include_str!("../../std/log/src/log.spx"),
        dependencies: &["std.data.json.utf8", "std.data.json.write", "std.io"],
    },
    BundledPackage {
        name: "std.mem",
        path: "dependencies/std.mem/0.1.0/mem.spx",
        source: include_str!("../../std/mem/src/mem.spx"),
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
        name: "std.path.value",
        path: "dependencies/std.path.value/0.1.0/path.spx",
        source: include_str!("../../std/path-value/src/path.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.process",
        path: "dependencies/std.process/0.1.0/process.spx",
        source: include_str!("../../std/process/src/process.spx"),
        dependencies: &["std.io"],
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
        name: "std.test.bytes",
        path: "dependencies/std.test.bytes/0.1.0/bytes.spx",
        source: include_str!("../../std/test-bytes/src/bytes.spx"),
        dependencies: &["std.io", "std.test"],
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

#[cfg(test)]
mod tests {
    use super::*;

    // `std.auth`, `std.db`, `std.http`, and `std.jobs` shipped their pure
    // decision-procedure source under `std/` (issues #189-192) but were not
    // yet wired into this closed bundled-dependency registry, so no ordinary
    // consumer project could declare them in `[dependencies]` -- only their
    // own `std/<name>/semaprax.toml` (which lists the module's own file as a
    // `sources` entry, not a dependency) could check them. This regression
    // pins that they are now reachable the same way every other bundled
    // package is.
    #[test]
    fn issue_189_192_packages_are_bundled() {
        for name in ["std.auth", "std.db", "std.http", "std.jobs"] {
            assert!(is_bundled(name), "`{name}` is not a bundled package");
        }
    }

    #[test]
    fn issue_189_192_packages_resolve_their_declared_source_file() {
        for (name, path_suffix) in [
            ("std.auth", "auth.spx"),
            ("std.db", "db.spx"),
            ("std.http", "http.spx"),
            ("std.jobs", "jobs.spx"),
        ] {
            let bundled = package(name).unwrap_or_else(|| panic!("`{name}` is not bundled"));
            assert!(
                bundled.path.ends_with(path_suffix),
                "`{name}` path `{}` does not end with `{path_suffix}`",
                bundled.path
            );
            assert!(
                !bundled.source.is_empty(),
                "`{name}` embedded source is empty"
            );
        }
        // `std.jobs` itself depends on `std.bytes` (`std/jobs/src/jobs.spx`
        // imports `std.bytes.equals`), which must already be bundled.
        assert_eq!(package("std.jobs").unwrap().dependencies, &["std.bytes"]);
        assert!(is_bundled("std.bytes"));
    }

    #[test]
    fn an_unlisted_dependency_is_still_rejected() {
        assert!(!is_bundled("std.nonexistent"));
        assert!(package("std.nonexistent").is_none());
    }
}
