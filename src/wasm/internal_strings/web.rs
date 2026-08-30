//! Source snapshot and trusted-parent publication for the explicit Web route.
mod render;
#[cfg(test)]
mod tests;

use super::{emit_module, InternalStringOptions};
use crate::diagnostic::Diagnostic;
use std::path::Path;

const SOURCE_LIMIT: usize = 16 * 1024 * 1024;
const DESCRIPTOR_LIMIT: usize = 1024 * 1024;
const PACKAGE_LIMIT: usize = 32 * 1024 * 1024;

/// Build a fresh fixed-inventory package from one bounded verified source.
/// The caller must exclusively control the existing output parent throughout
/// publication. This is not atomic publication or a source lock.
pub fn build_web_from_source(
    source: &Path,
    output: &Path,
    export_ids: &[String],
) -> Result<(), Vec<Diagnostic>> {
    build(source, output, export_ids, || {})
}

fn build(
    source: &Path,
    output: &Path,
    export_ids: &[String],
    before_recheck: impl FnOnce(),
) -> Result<(), Vec<Diagnostic>> {
    let canonical = crate::patch::canonical_source_path(source)?;
    let snapshot =
        crate::patch::read_source_snapshot_bounded(&canonical, SOURCE_LIMIT, "SPX-W111")?;
    let program = crate::parse(snapshot.source(), source).map_err(|error| vec![error])?;
    let diagnostics = crate::verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = crate::graph::revision(&program);
    let module = emit_module(&program, export_ids, InternalStringOptions::default())
        .map_err(|error| vec![error])?;
    bounded(module.descriptor().len(), DESCRIPTOR_LIMIT, "descriptor")
        .map_err(|error| vec![error])?;
    let artifacts = render::artifacts(&program.module, snapshot.source(), &revision, &module)
        .map_err(|error| vec![error])?;
    package_size(artifacts.iter().map(|(_, bytes)| bytes.len())).map_err(|error| vec![error])?;
    before_recheck();
    crate::patch::validate_source_unchanged_bounded(
        &canonical,
        source,
        &snapshot,
        &revision,
        SOURCE_LIMIT,
    )?;
    let borrowed = artifacts
        .iter()
        .map(|(path, bytes)| (*path, bytes.as_slice()))
        .collect::<Vec<_>>();
    super::super::publish_scalar_package(output, &borrowed).map_err(|error| vec![error])
}

fn bounded(size: usize, limit: usize, label: &str) -> Result<(), Diagnostic> {
    if size > limit {
        Err(super::error(format!(
            "internal String Web {label} exceeds {limit} bytes"
        )))
    } else {
        Ok(())
    }
}

fn package_size(lengths: impl IntoIterator<Item = usize>) -> Result<(), Diagnostic> {
    let total = lengths
        .into_iter()
        .try_fold(0usize, |sum, size| sum.checked_add(size))
        .ok_or_else(|| super::error("internal String Web package size overflow"))?;
    bounded(total, PACKAGE_LIMIT, "package")
}
