//! No-clobber publication and best-effort rollback for a standalone legacy Web package.
use super::{inspect_package_output_parent, is_plain_directory, is_plain_regular_file};
use crate::diagnostic::Diagnostic;
use same_file::Handle;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub(super) fn publish(
    output: &Path,
    artifacts: &[(&str, &[u8], &'static str, &str)],
) -> Result<(), Diagnostic> {
    let parent = inspect_package_output_parent(output, "legacy Web output parent")?;
    let parent_identity = Handle::from_path(parent).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot identify legacy Web output parent: {error}"),
        )
    })?;
    fs::create_dir(output).map_err(|error| {
        let code = if error.kind() == io::ErrorKind::AlreadyExists {
            "SPX-I307"
        } else {
            "SPX-I301"
        };
        Diagnostic::io(
            code,
            format!(
                "cannot create fresh web output {}: {error}",
                output.display()
            ),
        )
    })?;
    let output_identity = Handle::from_path(output).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot identify fresh legacy Web output: {error}"),
        )
    })?;
    let owns_destination = || {
        matches_directory(parent, &parent_identity) && matches_directory(output, &output_identity)
    };

    let mut created = Vec::with_capacity(artifacts.len());
    let result = (|| {
        for (name, bytes, code, label) in artifacts {
            if !owns_destination() {
                return Err(Diagnostic::io(
                    "SPX-I301",
                    "legacy Web output changed during publication",
                ));
            }
            let write_error =
                |error| Diagnostic::io(code, format!("cannot write {label}: {error}"));
            let path = output.join(name);
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(write_error)?;
            let identity = Handle::from_file(file).map_err(write_error)?;
            let written = identity.as_file().write_all(bytes);
            created.push((path, identity));
            written.map_err(write_error)?;
        }
        if !owns_destination() {
            return Err(Diagnostic::io(
                "SPX-I301",
                "legacy Web output changed during publication",
            ));
        }
        Ok(())
    })();

    if result.is_err() && owns_destination() {
        for (path, identity) in created.into_iter().rev() {
            let same_file = fs::symlink_metadata(&path)
                .ok()
                .is_some_and(|metadata| is_plain_regular_file(&metadata))
                && Handle::from_path(&path).ok().as_ref() == Some(&identity);
            drop(identity);
            if same_file {
                let _ = fs::remove_file(path);
            }
        }
        drop(output_identity);
        let _ = fs::remove_dir(output);
    }
    result
}

fn matches_directory(path: &Path, identity: &Handle) -> bool {
    fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| is_plain_directory(&metadata))
        && Handle::from_path(path).ok().as_ref() == Some(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_later_write_cleans_owned_files_and_same_destination_is_retryable() {
        let root =
            std::env::temp_dir().join(format!("semaprax-web-write-retry-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let output = root.join("out");
        let wasm = (
            "app.wasm",
            b"wasm bytes".as_slice(),
            "SPX-I302",
            "Wasm module",
        );
        let refused = (
            "missing/semaprax.js",
            b"js".as_slice(),
            "SPX-I303",
            "browser runtime",
        );

        let error = publish(&output, &[wasm, refused]).unwrap_err();
        assert_eq!(error.code, "SPX-I303");
        assert_eq!(
            fs::symlink_metadata(&output).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );

        publish(&output, &[wasm]).unwrap();
        assert_eq!(fs::read(output.join("app.wasm")).unwrap(), b"wasm bytes");
        fs::remove_dir_all(root).unwrap();
    }
}
