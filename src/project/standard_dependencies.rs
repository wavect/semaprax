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

struct BundledSource {
    path: &'static str,
    source: &'static str,
}

const CSV_ADDITIONAL_SOURCES: &[BundledSource] = &[BundledSource {
    path: "dependencies/std.data.csv/0.1.0/decode.spx",
    source: include_str!("../../std/data-csv/src/decode.spx"),
}];

const PACKAGES: &[BundledPackage] = &[
    BundledPackage {
        name: "std.agent",
        path: "dependencies/std.agent/0.1.0/agent.spx",
        source: include_str!("../../std/agent/src/agent.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.async",
        path: "dependencies/std.async/0.1.0/async.spx",
        source: include_str!("../../std/async/src/async.spx"),
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
        dependencies: &["std.io"],
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
        name: "std.data.json.query",
        path: "dependencies/std.data.json.query/0.1.0/query.spx",
        source: include_str!("../../std/data-json-query/src/query.spx"),
        dependencies: &["std.data.json"],
    },
    BundledPackage {
        name: "std.data.json.scan",
        path: "dependencies/std.data.json.scan/0.1.0/scan.spx",
        source: include_str!("../../std/data-json-scan/src/scan.spx"),
        dependencies: &[
            "std.data.json.doc",
            "std.data.json.query",
            "std.data.json.utf8",
        ],
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
        name: "std.email",
        path: "dependencies/std.email/0.1.0/policy.spx",
        source: include_str!("../../std/email/src/policy.spx"),
        dependencies: &["std.log.redact"],
    },
    BundledPackage {
        name: "std.encoding",
        path: "dependencies/std.encoding/0.1.0/encoding.spx",
        source: include_str!("../../std/encoding/src/encoding.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.encoding.base64",
        path: "dependencies/std.encoding.base64/0.1.0/base64.spx",
        source: include_str!("../../std/encoding-base64/src/base64.spx"),
        dependencies: &["std.encoding", "std.io"],
    },
    BundledPackage {
        name: "std.env",
        path: "dependencies/std.env/0.1.0/env.spx",
        source: include_str!("../../std/env/src/env.spx"),
        dependencies: &["std.format", "std.io"],
    },
    BundledPackage {
        name: "std.env.policy",
        path: "dependencies/std.env.policy/0.1.0/policy.spx",
        source: include_str!("../../std/env-policy/src/policy.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.export.policy",
        path: "dependencies/std.export.policy/0.1.0/policy.spx",
        source: include_str!("../../std/export-policy/src/policy.spx"),
        dependencies: &["std.http"],
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
        dependencies: &["std.log.redact"],
    },
    BundledPackage {
        name: "std.int.decimal",
        path: "dependencies/std.int.decimal/0.1.0/decimal.spx",
        source: include_str!("../../std/int-decimal/src/decimal.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.io",
        path: "dependencies/std.io/0.1.0/io.spx",
        source: include_str!("../../std/io/src/io.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.io.lines",
        path: "dependencies/std.io.lines/0.1.0/lines.spx",
        source: include_str!("../../std/io-lines/src/lines.spx"),
        dependencies: &["std.io"],
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
        dependencies: &[
            "std.data.json.utf8",
            "std.data.json.write",
            "std.io",
            "std.log.redact",
        ],
    },
    BundledPackage {
        name: "std.log.redact",
        path: "dependencies/std.log.redact/0.1.0/redact.spx",
        source: include_str!("../../std/log-redact/src/redact.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.mem",
        path: "dependencies/std.mem/0.1.0/mem.spx",
        source: include_str!("../../std/mem/src/mem.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.metrics",
        path: "dependencies/std.metrics/0.1.0/metrics.spx",
        source: include_str!("../../std/metrics/src/metrics.spx"),
        dependencies: &["std.log.redact", "std.num.overflow"],
    },
    BundledPackage {
        name: "std.net",
        path: "dependencies/std.net/0.1.0/net.spx",
        source: include_str!("../../std/net/src/net.spx"),
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
        name: "std.path.normalize",
        path: "dependencies/std.path.normalize/0.1.0/normalize.spx",
        source: include_str!("../../std/path-normalize/src/normalize.spx"),
        dependencies: &["std.path.value"],
    },
    BundledPackage {
        name: "std.path.value",
        path: "dependencies/std.path.value/0.1.0/path.spx",
        source: include_str!("../../std/path-value/src/path.spx"),
        dependencies: &[],
    },
    BundledPackage {
        name: "std.pattern",
        path: "dependencies/std.pattern/0.1.0/pattern.spx",
        source: include_str!("../../std/pattern/src/pattern.spx"),
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
        name: "std.tracing",
        path: "dependencies/std.tracing/0.1.0/policy.spx",
        source: include_str!("../../std/tracing/src/policy.spx"),
        dependencies: &["std.encoding", "std.log.redact"],
    },
    BundledPackage {
        name: "std.webhook",
        path: "dependencies/std.webhook/0.1.0/policy.spx",
        source: include_str!("../../std/webhook/src/policy.spx"),
        dependencies: &["std.log.redact"],
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
        // Idempotent on purpose. The `selected` set above dedupes within one
        // call, but this function was pushing unconditionally, so a caller
        // that extended the same source vector twice produced the same
        // bundled path twice. The workspace path set refuses a duplicate, so
        // the symptom surfaced far away as
        // `SPX-G174: ... paths must be strictly sorted and unique` with no
        // indication of which path or why (issue #272). Guarding here fixes
        // it for every caller rather than for the one that was found.
        let additional = if package.name == "std.data.csv" {
            CSV_ADDITIONAL_SOURCES
        } else {
            &[]
        };
        for (path, source) in std::iter::once((package.path, package.source))
            .chain(additional.iter().map(|source| (source.path, source.source)))
        {
            if sources.iter().any(|source| source.path == path) {
                continue;
            }
            sources.push(SemanticWorkspaceSource {
                path: path.to_owned(),
                source: source.to_owned(),
            });
        }
    }
    Ok(())
}

fn package(name: &str) -> Option<&'static BundledPackage> {
    PACKAGES.iter().find(|package| package.name == name)
}

/// Widened from `pub(super)` to `pub(crate)` for issue #195: the package
/// registry (`crate::package_registry`) reuses this exact check to refuse
/// publishing into the `std.*` namespace this closed bundled registry
/// already owns, rather than duplicating an independent name list.
pub(crate) fn is_bundled(name: &str) -> bool {
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

    // `std.auth`, `std.db`, `std.http`, `std.jobs`, `std.metrics`,
    // `std.export.policy`, and `std.webhook` ship their pure decision-procedure source under `std/`
    // (issues #189-193) but were not
    // yet wired into this closed bundled-dependency registry, so no ordinary
    // consumer project could declare them in `[dependencies]` -- only their
    // own `std/<name>/semaprax.toml` (which lists the module's own file as a
    // `sources` entry, not a dependency) could check them. This regression
    // pins that they are now reachable the same way every other bundled
    // package is.
    /// Issue #272: `extend_sources` deduplicates within one call but pushed
    /// unconditionally, so extending the same vector twice produced the same
    /// bundled path twice. The workspace path set then refused the project
    /// with `SPX-G174`, naming neither the path nor the reason. This pins the
    /// idempotency rather than the caller that happened to double-extend.
    #[test]
    fn extending_the_same_sources_twice_adds_each_bundled_package_once() {
        let manifest = ProjectManifest::parse(
            "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"dup\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"dup.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"dup.tests\"]\n\n[exports]\nweb = [\"dup.app.ok\"]\n\n[dependencies]\nstd.auth = \"=0.1.0\"\nstd.jobs = \"=0.1.0\"\n",
        )
        .expect("the fixture manifest parses");

        let mut once = Vec::new();
        extend_sources(&manifest, &mut once).expect("first extension resolves");
        let after_one = once.len();
        assert!(
            after_one >= 3,
            "expected std.auth, std.jobs and the transitive std.bytes, got {after_one}"
        );

        extend_sources(&manifest, &mut once).expect("second extension resolves");
        assert_eq!(
            once.len(),
            after_one,
            "a second extension must add nothing: {:?}",
            once.iter().map(|s| s.path.clone()).collect::<Vec<_>>()
        );

        let mut paths = once.iter().map(|s| s.path.clone()).collect::<Vec<_>>();
        paths.sort();
        let unique = paths
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        assert_eq!(unique, paths.len(), "duplicate bundled path in {paths:?}");
    }

    #[test]
    fn issue_189_193_packages_are_bundled() {
        for name in [
            "std.auth",
            "std.db",
            "std.http",
            "std.jobs",
            "std.metrics",
            "std.export.policy",
            "std.webhook",
        ] {
            assert!(is_bundled(name), "`{name}` is not a bundled package");
        }
    }

    #[test]
    fn issue_189_193_packages_resolve_their_declared_source_file() {
        for (name, path_suffix) in [
            ("std.auth", "auth.spx"),
            ("std.db", "db.spx"),
            ("std.http", "http.spx"),
            ("std.jobs", "jobs.spx"),
            ("std.metrics", "metrics.spx"),
            ("std.export.policy", "policy.spx"),
            ("std.webhook", "policy.spx"),
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
        assert_eq!(
            package("std.http").unwrap().dependencies,
            &["std.log.redact"]
        );
        assert_eq!(
            package("std.metrics").unwrap().dependencies,
            &["std.log.redact", "std.num.overflow"]
        );
        assert_eq!(
            package("std.export.policy").unwrap().dependencies,
            &["std.http"]
        );
        assert_eq!(
            package("std.webhook").unwrap().dependencies,
            &["std.log.redact"]
        );
    }

    #[test]
    fn shipped_package_catalog_and_bundled_registry_are_identical() {
        let catalog: serde_json::Value =
            serde_json::from_str(include_str!("../../std/packages.json")).unwrap();
        let mut catalog_names = catalog["packages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|package| package["module"].as_str().unwrap())
            .collect::<Vec<_>>();
        let mut bundled_names = PACKAGES
            .iter()
            .map(|package| package.name)
            .collect::<Vec<_>>();
        catalog_names.sort_unstable();
        bundled_names.sort_unstable();
        assert!(catalog_names.windows(2).all(|pair| pair[0] != pair[1]));
        assert!(bundled_names.windows(2).all(|pair| pair[0] != pair[1]));
        assert_eq!(bundled_names, catalog_names);
    }

    #[test]
    fn issue_619_packages_are_bundled_with_their_manifest_dependencies() {
        for name in [
            "std.async",
            "std.email",
            "std.encoding.base64",
            "std.env.policy",
            "std.io.lines",
            "std.net",
            "std.path.normalize",
            "std.pattern",
        ] {
            assert!(is_bundled(name), "`{name}` is not a bundled package");
        }
        assert!(package("std.async").unwrap().dependencies.is_empty());
        assert_eq!(
            package("std.email").unwrap().dependencies,
            &["std.log.redact"]
        );
        assert_eq!(
            package("std.encoding.base64").unwrap().dependencies,
            &["std.encoding", "std.io"]
        );
        assert!(package("std.env.policy").unwrap().dependencies.is_empty());
        assert_eq!(package("std.io.lines").unwrap().dependencies, &["std.io"]);
        assert!(package("std.net").unwrap().dependencies.is_empty());
        assert_eq!(
            package("std.path.normalize").unwrap().dependencies,
            &["std.path.value"]
        );
        assert!(package("std.pattern").unwrap().dependencies.is_empty());
    }

    #[test]
    fn tracing_bundles_its_encoding_and_redaction_dependencies() {
        assert_eq!(
            package("std.tracing").unwrap().dependencies,
            &["std.encoding", "std.log.redact"]
        );
        assert!(is_bundled("std.encoding"));
        assert!(is_bundled("std.log.redact"));
    }

    #[test]
    fn an_unlisted_dependency_is_still_rejected() {
        assert!(!is_bundled("std.nonexistent"));
        assert!(package("std.nonexistent").is_none());
    }
}
