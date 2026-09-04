//! The hint for a `use` whose target module no listed source declares.
//!
//! The Workspace Semantic Graph reports `SPX-G172` "target module is missing
//! or equals the caller module" for an import it cannot resolve. Inside a
//! project the common cause is a new `.spx` file that was never added to
//! `sources` in `semaprax.toml`, and the graph cannot say so: it sees only the
//! listed files. This pass runs after a failed project build, re-reads the
//! importing file the diagnostic names, recovers the imported module from the
//! `use` at the diagnostic's span, and looks for an unlisted `.spx` file under
//! the project's source directories that declares that module. It only ever
//! adds `help` text; codes, messages, spans, and ordering are unchanged.
//!
//! Reading the project's own source directories for a hint grants nothing: the
//! scan is bounded, happens only after the build has already failed, and its
//! result is advisory text.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;

const UNRESOLVED_USE_CODE: &str = "SPX-G172";
const UNRESOLVED_USE_MESSAGE: &str = "target module is missing or equals the caller module";
const MAX_HINT_FILE_BYTES: u64 = 1024 * 1024;
const MAX_SCANNED_FILES: usize = 512;

pub(super) fn hint_unlisted_module(
    errors: Vec<Diagnostic>,
    root: &Path,
    declared_sources: &[String],
) -> Vec<Diagnostic> {
    errors
        .into_iter()
        .map(|diagnostic| {
            if diagnostic.code != UNRESOLVED_USE_CODE
                || diagnostic.message != UNRESOLVED_USE_MESSAGE
                || diagnostic.help.is_some()
            {
                return diagnostic;
            }
            match unresolved_use_help(&diagnostic, root, declared_sources) {
                Some(help) => diagnostic.with_help(help),
                None => diagnostic,
            }
        })
        .collect()
}

fn unresolved_use_help(
    diagnostic: &Diagnostic,
    root: &Path,
    declared_sources: &[String],
) -> Option<String> {
    let importer = diagnostic.path.as_deref()?;
    let span = diagnostic.span?;
    let source = read_bounded(&root.join(importer))?;
    let program = crate::parse(&source, Path::new(importer)).ok()?;
    let module_use = program
        .module_uses
        .iter()
        .find(|module_use| module_use.span == span)?;
    let target = module_use.target_module.as_str();
    if target == program.module {
        return Some(format!(
            "module `{target}` imports from itself; import from the module that declares the target"
        ));
    }
    let declaring = unlisted_declaring_file(root, declared_sources, target);
    Some(match declaring {
        Some(file) => format!(
            "`{file}` declares module `{target}` but is not listed under `sources` in semaprax.toml; add it there"
        ),
        None => format!(
            "no file listed under `sources` in semaprax.toml declares module `{target}`; \
             declare it in a listed `.spx` file or add that file to `sources`"
        ),
    })
}

/// The first unlisted `.spx` file, in path order, under a directory that holds a
/// listed source and whose module header declares `target`.
fn unlisted_declaring_file(
    root: &Path,
    declared_sources: &[String],
    target: &str,
) -> Option<String> {
    let listed = declared_sources
        .iter()
        .map(|source| normalize(Path::new(source)))
        .collect::<BTreeSet<_>>();
    let directories = declared_sources
        .iter()
        .map(|source| {
            Path::new(source)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default()
        })
        .collect::<BTreeSet<PathBuf>>();
    let mut scanned = 0usize;
    for directory in directories {
        let Ok(entries) = std::fs::read_dir(root.join(&directory)) else {
            continue;
        };
        let mut candidates = entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .filter(|name| Path::new(name).extension().is_some_and(|ext| ext == "spx"))
            .collect::<Vec<_>>();
        candidates.sort();
        for name in candidates {
            scanned += 1;
            if scanned > MAX_SCANNED_FILES {
                return None;
            }
            let relative = normalize(&directory.join(&name));
            if listed.contains(&relative) {
                continue;
            }
            let Some(source) = read_bounded(&root.join(&directory).join(&name)) else {
                continue;
            };
            if declared_module(&source) == Some(target) {
                return Some(relative);
            }
        }
    }
    None
}

/// The module a source file declares: its first line that is neither blank nor a
/// `//` comment must be `module <name>;`.
pub(super) fn declared_module(source: &str) -> Option<&str> {
    let header = source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("//"))?;
    let name = header.strip_prefix("module ")?.strip_suffix(';')?.trim();
    (!name.is_empty()
        && name.split('.').all(|segment| {
            !segment.is_empty() && segment.chars().all(|c| c.is_alphanumeric() || c == '_')
        }))
    .then_some(name)
}

fn normalize(path: &Path) -> String {
    path.components()
        .filter(|component| !matches!(component, std::path::Component::CurDir))
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn read_bounded(path: &Path) -> Option<String> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_HINT_FILE_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_header_scan_accepts_comments_and_rejects_other_shapes() {
        assert_eq!(declared_module("module app.util;\n"), Some("app.util"));
        assert_eq!(
            declared_module("// leading comment\n\n  module app.util;  \nfn x() {}\n"),
            Some("app.util")
        );
        assert_eq!(declared_module("modules app.util;\n"), None);
        assert_eq!(declared_module("module app..util;\n"), None);
        assert_eq!(declared_module("module ;\n"), None);
        assert_eq!(
            declared_module("fn main() -> i64 { 0 }\nmodule app;\n"),
            None
        );
        assert_eq!(declared_module(""), None);
    }

    #[test]
    fn only_the_unresolved_use_diagnostic_without_help_is_considered() {
        let root =
            std::env::temp_dir().join(format!("semaprax-source-hint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/app.spx"),
            "module app.main;\nuse function @id(\"app.util.double\") from app.util as double;\n\n@id(\"app.main.main\")\nfn main() -> i64\n{\n    double(21)\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/util.spx"),
            "module app.util;\n\n@id(\"app.util.double\")\nfn double(value: i64) -> i64\n{\n    value * 2\n}\n",
        )
        .unwrap();
        let program = crate::parse(
            &std::fs::read_to_string(root.join("src/app.spx")).unwrap(),
            Path::new("src/app.spx"),
        )
        .unwrap();
        let span = program.module_uses[0].span;
        let unresolved = Diagnostic::error(UNRESOLVED_USE_CODE, UNRESOLVED_USE_MESSAGE, span)
            .at_path("src/app.spx");
        let declared = vec!["src/app.spx".to_owned()];

        let hinted = hint_unlisted_module(
            vec![
                unresolved.clone(),
                Diagnostic::io(UNRESOLVED_USE_CODE, "another G172 message"),
                unresolved.clone().with_help("already explained"),
            ],
            &root,
            &declared,
        );
        assert_eq!(
            hinted[0].help.as_deref(),
            Some("`src/util.spx` declares module `app.util` but is not listed under `sources` in semaprax.toml; add it there")
        );
        assert_eq!(hinted[1].help, None);
        assert_eq!(hinted[2].help.as_deref(), Some("already explained"));

        std::fs::remove_file(root.join("src/util.spx")).unwrap();
        let hinted = hint_unlisted_module(vec![unresolved.clone()], &root, &declared);
        assert_eq!(
            hinted[0].help.as_deref(),
            Some("no file listed under `sources` in semaprax.toml declares module `app.util`; declare it in a listed `.spx` file or add that file to `sources`")
        );

        // A listed file that declares the module is not reported as unlisted:
        // the graph then owns the diagnosis and the generic hint stands.
        std::fs::write(root.join("src/util.spx"), "module app.util;\n").unwrap();
        let hinted = hint_unlisted_module(
            vec![unresolved],
            &root,
            &["src/app.spx".to_owned(), "src/util.spx".to_owned()],
        );
        assert!(hinted[0]
            .help
            .as_deref()
            .unwrap()
            .starts_with("no file listed under `sources`"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
