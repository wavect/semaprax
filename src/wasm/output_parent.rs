//! Shared `SPX-I301` wording for rejected or missing output parents.
//!
//! Native single-file outputs, Web packages and explicit Project outputs
//! enforce the same parent policy: the inspected directory must already be a
//! real non-reparse directory. These helpers only describe a rejection; they
//! never relax the policy, follow a link for publication, or create a path.
//!
//! Message and help text are printed verbatim by `Diagnostic`'s human
//! rendering, so every path interpolated here goes through [`shown`], which
//! escapes control characters the same way the rendered `Diagnostic.path`
//! is escaped.
//!
//! The module is declared under `wasm`, the library owner of Web package
//! publication, rather than under `diagnostic`: `src/diagnostic.rs` is a
//! byte-authenticated source owner (`src/diagnostic/writer/tests.rs`), and the
//! single-file native and Project routes in `src/cli/build.rs` reach the same
//! helpers through `semaprax::wasm::output_parent`.

use std::fs;
use std::io;
use std::path::Path;

use crate::diagnostic::Diagnostic;

/// Describe an inspected output parent that is not a real non-reparse
/// directory.
///
/// `subject` names the checked directory role, for example
/// `"single-file native output parent"`. `output` is the requested output
/// path; when `directory` is a link that resolves to a directory containing
/// it, the help suggests the equivalent output path below the resolved
/// directory. Resolution is read-only and happens only on this rejection
/// path.
pub fn rejected_directory(
    subject: &str,
    directory: &Path,
    output: &Path,
    metadata: &fs::Metadata,
) -> Diagnostic {
    let is_link = metadata.file_type().is_symlink();
    let kind = if is_link {
        link_kind()
    } else if !metadata.is_dir() {
        "is not a directory"
    } else {
        "is a reparse point"
    };
    let help = if is_link {
        link_help(directory, output)
    } else {
        "choose an output whose parent is an existing real directory".to_owned()
    };
    Diagnostic::io(
        "SPX-I301",
        format!(
            "{subject} {} {kind}; it must be a real non-reparse directory",
            shown(directory)
        ),
    )
    .with_help(help)
}

/// Recovery hint for an output parent that could not be inspected.
///
/// Only a missing directory has a hint: create it first. Other inspection
/// failures keep their message without help.
pub fn missing_directory_help(directory: &Path, error: &io::Error) -> Option<String> {
    (error.kind() == io::ErrorKind::NotFound).then(|| {
        format!(
            "create the directory {} first, then rerun the build",
            shown(directory)
        )
    })
}

/// Render a path for interpolation into diagnostic message or help text.
///
/// Control characters (newline, escape, delete and the rest) are replaced by
/// their `char::escape_default` spelling, so a hostile file name cannot
/// forge diagnostic lines or emit terminal control sequences.
pub fn shown(path: &Path) -> String {
    let mut rendered = String::new();
    for character in path.display().to_string().chars() {
        if character.is_control() {
            rendered.extend(character.escape_default());
        } else {
            rendered.push(character);
        }
    }
    rendered
}

fn link_help(directory: &Path, output: &Path) -> String {
    const FALLBACK: &str = "choose an output whose parent is a real directory";
    match fs::canonicalize(directory) {
        Ok(resolved) if resolved.is_dir() => match output.strip_prefix(directory) {
            Ok(rest) => format!(
                "{} resolves to {}; use the resolved output path {}, or {FALLBACK}",
                shown(directory),
                shown(&resolved),
                shown(&resolved.join(rest))
            ),
            Err(_) => format!(
                "{} resolves to {}; {FALLBACK}",
                shown(directory),
                shown(&resolved)
            ),
        },
        Ok(resolved) => format!(
            "{} resolves to {}, which is not a directory; {FALLBACK}",
            shown(directory),
            shown(&resolved)
        ),
        Err(error) => format!(
            "cannot resolve the link {}: {error}; {FALLBACK}",
            shown(directory)
        ),
    }
}

#[cfg(windows)]
fn link_kind() -> &'static str {
    "is a symbolic link or junction"
}

#[cfg(not(windows))]
fn link_kind() -> &'static str {
    "is a symbolic link"
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "semaprax-output-parent-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("create scratch root");
            Self(root)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn reject(directory: &Path, output: &Path) -> Diagnostic {
        let metadata = fs::symlink_metadata(directory).expect("inspect fixture");
        rejected_directory("test output parent", directory, output, &metadata)
    }

    #[test]
    fn link_to_directory_suggests_resolved_output() {
        let scratch = Scratch::new("dir-link");
        let real = scratch.0.join("real");
        let link = scratch.0.join("link");
        fs::create_dir(&real).expect("create real directory");
        symlink(&real, &link).expect("create link");
        let resolved = fs::canonicalize(&link).expect("resolve link");

        let diagnostic = reject(&link, &link.join("artifact"));
        assert_eq!(
            diagnostic.message,
            format!(
                "test output parent {} is a symbolic link; it must be a real non-reparse directory",
                link.display()
            )
        );
        assert_eq!(
            diagnostic.help.as_deref(),
            Some(
                format!(
                    "{} resolves to {}; use the resolved output path {}, or choose an output whose parent is a real directory",
                    link.display(),
                    resolved.display(),
                    resolved.join("artifact").display()
                )
                .as_str()
            )
        );
        assert!(diagnostic.path.is_none());
    }

    #[test]
    fn link_to_directory_with_unrelated_output_names_only_resolution() {
        let scratch = Scratch::new("unrelated");
        let real = scratch.0.join("real");
        let link = scratch.0.join("link");
        fs::create_dir(&real).expect("create real directory");
        symlink(&real, &link).expect("create link");
        let resolved = fs::canonicalize(&link).expect("resolve link");

        let diagnostic = reject(&link, Path::new("relative/artifact"));
        assert_eq!(
            diagnostic.help.as_deref(),
            Some(
                format!(
                    "{} resolves to {}; choose an output whose parent is a real directory",
                    link.display(),
                    resolved.display()
                )
                .as_str()
            )
        );
    }

    #[test]
    fn link_to_file_reports_non_directory_target() {
        let scratch = Scratch::new("file-link");
        let file = scratch.0.join("file");
        let link = scratch.0.join("link");
        fs::write(&file, b"x").expect("create file");
        symlink(&file, &link).expect("create link");

        let diagnostic = reject(&link, &link.join("artifact"));
        assert!(diagnostic.message.contains("is a symbolic link"));
        let help = diagnostic.help.expect("link help");
        assert!(help.contains("which is not a directory"), "{help}");
    }

    #[test]
    fn dangling_link_reports_unresolvable_link() {
        let scratch = Scratch::new("dangling");
        let link = scratch.0.join("link");
        symlink(scratch.0.join("absent"), &link).expect("create link");

        let diagnostic = reject(&link, &link.join("artifact"));
        let help = diagnostic.help.expect("link help");
        assert!(
            help.starts_with(&format!("cannot resolve the link {}: ", link.display())),
            "{help}"
        );
        assert!(help.ends_with("; choose an output whose parent is a real directory"));
    }

    #[test]
    fn regular_file_parent_is_not_a_directory() {
        let scratch = Scratch::new("file-parent");
        let file = scratch.0.join("file");
        fs::write(&file, b"x").expect("create file");

        let diagnostic = reject(&file, &file.join("artifact"));
        assert_eq!(
            diagnostic.message,
            format!(
                "test output parent {} is not a directory; it must be a real non-reparse directory",
                file.display()
            )
        );
        assert_eq!(
            diagnostic.help.as_deref(),
            Some("choose an output whose parent is an existing real directory")
        );
    }

    #[test]
    fn hostile_link_and_output_names_render_without_control_characters() {
        let scratch = Scratch::new("hostile");
        let real = scratch.0.join("real");
        let link = scratch.0.join("link\n\u{1b}[31m");
        fs::create_dir(&real).expect("create real directory");
        symlink(&real, &link).expect("create link");
        let resolved = fs::canonicalize(&link).expect("resolve link");

        let diagnostic = reject(
            &link,
            &link.join("evil\n\u{1b}[31mFAKE error[SPX-X]: forged\u{7f}"),
        );
        let help = diagnostic.help.as_deref().expect("link help");
        for text in [&*diagnostic.message, help] {
            assert!(!text.chars().any(char::is_control), "{text:?}");
        }
        assert!(diagnostic
            .message
            .contains("link\\n\\u{1b}[31m is a symbolic link"));
        assert!(
            help.ends_with(&format!(
                "use the resolved output path {}/evil\\n\\u{{1b}}[31mFAKE error[SPX-X]: forged\\u{{7f}}, or choose an output whose parent is a real directory",
                resolved.display()
            )),
            "{help}"
        );

        // The only line break in the human rendering is the help separator.
        let rendered = diagnostic.to_string();
        assert_eq!(rendered.matches('\n').count(), 1, "{rendered:?}");
        assert!(!rendered.contains('\u{1b}'), "{rendered:?}");
        assert!(rendered.contains("\n  help: "), "{rendered:?}");

        let missing = scratch.0.join("absent\u{1b}]0;title\u{7}");
        assert_eq!(
            missing_directory_help(&missing, &io::Error::from(io::ErrorKind::NotFound)).as_deref(),
            Some(
                format!(
                    "create the directory {}/absent\\u{{1b}}]0;title\\u{{7}} first, then rerun the build",
                    scratch.0.display()
                )
                .as_str()
            )
        );
    }

    #[test]
    fn only_missing_directory_has_create_help() {
        let directory = Path::new("/absent/parent");
        assert_eq!(
            missing_directory_help(directory, &io::Error::from(io::ErrorKind::NotFound)).as_deref(),
            Some("create the directory /absent/parent first, then rerun the build")
        );
        assert_eq!(
            missing_directory_help(directory, &io::Error::from(io::ErrorKind::PermissionDenied)),
            None
        );
    }
}
