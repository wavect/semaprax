//! Public validation entry; all effectful work is delegated to the private authority.

use super::authority::build_native_rust_sdk_inner;
use super::*;

/// Builds and publishes one fresh, current-host Native Rust SDK package.
pub fn build_native_rust_sdk(
    source: &str,
    source_path: &Path,
    options: NativeRustSdkOptions,
    output: &Path,
) -> Result<NativeRustSdkBundle, Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![sdk_error("Native Rust SDK source exceeds its bound")]);
    }
    let program = semaprax::check(source, source_path)?;
    build_native_rust_sdk_inner(&program, options, output)
        .map_err(PublicBuildError::into_diagnostics)
}
