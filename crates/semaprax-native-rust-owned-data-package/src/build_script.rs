//! Shared private Cargo output protocol for owned-data package families.
use super::HostTarget;

pub(crate) fn render(target: HostTarget, flat_record: bool) -> String {
    let archive = target.archive_name();
    let triple = target.triple();
    let mismatch = if flat_record {
        "generated SEMAPRAX flat-record SDK target mismatch"
    } else {
        "generated SEMAPRAX owned-data SDK target mismatch"
    };
    format!(
        "#![forbid(unsafe_code)]\nfn main(){{if std::env::var(\"TARGET\").unwrap_or_default()!={triple:?}{{panic!({mismatch:?})}}let root=std::env::var_os(\"CARGO_MANIFEST_DIR\").expect(\"Cargo must set CARGO_MANIFEST_DIR\");let native=std::path::PathBuf::from(root);let native=native.to_str().filter(|path|!path.contains(['\\r','\\n'])).expect(\"generated SDK package path must be Unicode without CR/LF\");println!(\"cargo:rerun-if-changed={archive}\");println!(\"cargo:rustc-link-search=native={{native}}\");println!(\"cargo:rustc-link-lib=static=semaprax_native_rust_owned_data_sdk\");}}\n"
    )
}

pub(crate) fn render_nested(target: HostTarget) -> String {
    let archive = target.archive_name();
    let triple = target.triple();
    format!(
        "#![forbid(unsafe_code)]\nfn main(){{if std::env::var(\"TARGET\").unwrap_or_default()!={triple:?}{{panic!(\"generated SEMAPRAX nested-record SDK target mismatch\")}}let root=std::env::var_os(\"CARGO_MANIFEST_DIR\").expect(\"Cargo must set CARGO_MANIFEST_DIR\");let native=std::path::PathBuf::from(root);let native=native.to_str().filter(|path|!path.contains(['\\r','\\n'])).expect(\"generated SDK package path must be Unicode without CR/LF\");println!(\"cargo:rerun-if-changed={archive}\");println!(\"cargo:rustc-link-search=native={{native}}\");println!(\"cargo:rustc-link-lib=static=semaprax_native_rust_owned_data_sdk\");}}\n"
    )
}

#[cfg(test)]
mod tests;
