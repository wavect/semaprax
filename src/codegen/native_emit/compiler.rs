use std::fs::File;
use std::io::{self, Write as _};
use std::path::Path;
use std::process::Command;

use crate::diagnostic::Diagnostic;

use super::native_scratch;

pub(in crate::codegen) fn write_and_compile_c(
    c_source: &str,
    output: &Path,
) -> Result<(), Diagnostic> {
    write_and_compile_c_with_mode(c_source, output, false)
}

pub(in crate::codegen) fn write_and_compile_c_with_mode(
    c_source: &str,
    output: &Path,
    native_command: bool,
) -> Result<(), Diagnostic> {
    write_and_compile_c_with_runner(c_source, output, native_command, Command::output)
}

pub(in crate::codegen) fn write_compile_and_publish_c(
    c_source: &str,
    output: &mut File,
    native_command: bool,
) -> Result<(), Diagnostic> {
    let leaf = format!("program{}", std::env::consts::EXE_SUFFIX);
    let mut artifact = native_scratch::Scratch::create(&leaf, None).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot create native artifact scratch: {error}"),
        )
    })?;
    write_and_compile_c_with_mode(c_source, artifact.path(), native_command)?;
    artifact.seal().map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot authenticate compiled native artifact: {error}"),
        )
    })?;
    let mut compiled = File::open(artifact.path()).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot open compiled native artifact: {error}"),
        )
    })?;
    let metadata = compiled.metadata().map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot inspect compiled native artifact: {error}"),
        )
    })?;
    io::copy(&mut compiled, output).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot publish native artifact bytes: {error}"),
        )
    })?;
    output
        .set_permissions(metadata.permissions())
        .map_err(|error| {
            Diagnostic::io(
                "SPX-I301",
                format!("cannot publish native artifact permissions: {error}"),
            )
        })?;
    output.flush().map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot flush native artifact publication: {error}"),
        )
    })?;
    let _ = artifact.cleanup();
    Ok(())
}

pub(super) fn write_and_compile_c_with_runner(
    c_source: &str,
    output: &Path,
    native_command: bool,
    run: impl FnOnce(&mut Command) -> std::io::Result<std::process::Output>,
) -> Result<(), Diagnostic> {
    let mut scratch = native_scratch::Scratch::create("source.c", Some(c_source.as_bytes()))
        .map_err(|error| {
            Diagnostic::io(
                "SPX-I101",
                format!("cannot create temporary C source: {error}"),
            )
        })?;
    scratch.seal().map_err(|error| {
        Diagnostic::io(
            "SPX-I101",
            format!("cannot authenticate temporary C source: {error}"),
        )
    })?;
    let mut compiler = Command::new("clang");
    compiler.args([
        "-std=c11",
        "-O2",
        "-Wall",
        "-Wextra",
        "-Werror",
        // Source-level self-comparisons are legal and meaningful (notably for
        // floating-point NaN tests). Generated locals preserve that spelling,
        // so this warning is not a backend-quality failure.
        "-Wno-tautological-compare",
    ]);
    #[cfg(all(windows, target_env = "gnu"))]
    if native_command {
        compiler.arg("-municode");
    }
    #[cfg(not(all(windows, target_env = "gnu")))]
    let _ = native_command;
    compiler.arg(scratch.path()).arg("-o").arg(output);
    let result = run(&mut compiler).map_err(|error| {
        Diagnostic::io(
            "SPX-B101",
            format!("failed to start clang; install a C11 toolchain: {error}"),
        )
    })?;
    if !result.status.success() {
        return Err(Diagnostic::io(
            "SPX-B102",
            format!(
                "native backend failed:\n{}",
                String::from_utf8_lossy(&result.stderr)
            ),
        ));
    }
    // A failed or uncertain compiler run retains all source scratch. Successful
    // cleanup remains best-effort and cannot replace the compiler outcome.
    let _ = scratch.cleanup();
    Ok(())
}
