use std::path::{Path, PathBuf};

use semaprax::project::{
    derive_public_api_descriptor, PublicApiSubject, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};
use semaprax_native_rust_interop::build_native_rust_owned_data_sdk;

const SOURCE: &str = include_str!("../owned_data.spx");
const REVISION: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const SELECTED: [&str; 3] = [
    "frame.payload",
    "frame.payload-maybe",
    "frame.payload-result",
];

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| panic!("expected one absolute output directory"));
    let checked = semaprax::check(SOURCE, Path::new("owned_data.spx")).unwrap();
    let program = semaprax::hir::resolve(&checked).unwrap();
    let selected = SELECTED.map(str::to_owned);
    let subject = PublicApiSubject {
        project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: REVISION,
        workspace_revision: REVISION,
        project_graph_digest: REVISION,
    };
    let descriptor = derive_public_api_descriptor(&program, &selected, subject).unwrap();
    build_native_rust_owned_data_sdk(
        &program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
        &output,
    )
    .unwrap();
}
