use semaprax_native_rust_interop::{
    indexed_binding::SelectedPackage, prepare_indexed_regex_url_project_packages,
    IndexedProjectRegexRegistrySelection, IndexedProjectUrlRegistrySelection,
};
use semaprax_rust_api_index::RustApiIndex;
use std::{fs, path::Path};

const REGEX_INDEX: &[u8] = include_bytes!(
    "../../../../crates/semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json"
);
const URL_INDEX: &[u8] = include_bytes!(
    "../../../../crates/semaprax-rust-api-index/fixtures/url-2.5.8-index-envelope.json"
);
const REGEX_LOCK: &[u8] = include_bytes!(
    "../../../../crates/semaprax-toolchain/src/fixtures/ri06-regex-1.13.1.Cargo.lock"
);
const URL_LOCK: &[u8] =
    include_bytes!("../../../../crates/semaprax-toolchain/src/fixtures/ri06-url-2.5.8.Cargo.lock");

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let project = root.join("project");
    let source = fs::read_to_string(project.join("src/app.spx")).expect("saved source");
    let regex_index = RustApiIndex::admit_extractor_output(REGEX_INDEX).unwrap();
    let regex_json = regex_index.canonical_json();
    let url_index = RustApiIndex::admit_extractor_output(URL_INDEX).unwrap();
    let url_json = url_index.canonical_json();
    let regex_package = SelectedPackage {
        cargo_alias: "regex_alias",
        name: "regex",
        version: "1.13.1",
        source_sha256: regex_index.package().source_sha256.as_str(),
        target: regex_index.target(),
        feature_digest: regex_index.feature_digest(),
        stable_rustc_version: regex_index.stable_rustc_version(),
    };
    let url_package = SelectedPackage {
        cargo_alias: "url_alias",
        name: "url",
        version: "2.5.8",
        source_sha256: url_index.package().source_sha256.as_str(),
        target: url_index.target(),
        feature_digest: url_index.feature_digest(),
        stable_rustc_version: url_index.stable_rustc_version(),
    };
    let regex =
        ["regex.new", "regex.match"].map(|import_id| IndexedProjectRegexRegistrySelection {
            source_path: "src/app.spx",
            source: &source,
            import_id,
            index_bytes: regex_json.as_bytes(),
            package: regex_package,
        });
    let url = ["url.new", "url.view"].map(|import_id| IndexedProjectUrlRegistrySelection {
        source_path: "src/app.spx",
        source: &source,
        import_id,
        index_bytes: url_json.as_bytes(),
        package: url_package,
    });
    let prepared = prepare_indexed_regex_url_project_packages(
        &project.join("semaprax.toml"),
        &regex,
        &url,
        "regex.run",
        "url.run",
        REGEX_LOCK,
        URL_LOCK,
    )
    .expect("held four-import Project");
    let regex = root.join("generated/regex");
    let url = root.join("generated/url");
    fs::create_dir_all(regex.join("src")).unwrap();
    fs::create_dir_all(url.join("src")).unwrap();
    for (name, bytes) in [
        ("Cargo.toml", prepared.regex.cargo_toml()),
        ("Cargo.lock", prepared.regex.cargo_lock()),
        ("src/lib.rs", prepared.regex.lib_rs()),
        ("src/regex_project.c", prepared.regex.c_source()),
        ("src/regex_project.h", prepared.regex.header()),
        ("binding-plan.json", prepared.regex.binding_plan()),
        ("descriptor.json", prepared.regex.descriptor()),
    ] {
        fs::write(regex.join(name), bytes).unwrap();
    }
    for (name, bytes) in [
        ("Cargo.toml", prepared.url.cargo_toml()),
        ("Cargo.lock", prepared.url.cargo_lock()),
        ("src/lib.rs", prepared.url.lib_rs()),
        ("src/url_project.c", prepared.url.c_source()),
        ("src/url_project.h", prepared.url.header()),
        ("binding-plan.json", prepared.url.binding_plan()),
        ("descriptor.json", prepared.url.descriptor()),
    ] {
        fs::write(url.join(name), bytes).unwrap();
    }
    println!("ri13-m1-prepared:{}", prepared.subject_digest);
}
