use semaprax::project::with_authenticated_project;
use semaprax_native_rust_interop::{
    indexed_binding::SelectedPackage, prepare_indexed_regex_url_project_packages,
    prepare_native_rust_serde_iterator_callbacks, IndexedProjectRegexRegistrySelection,
    IndexedProjectUrlRegistrySelection,
};
use semaprax_rust_api_index::RustApiIndex;
use std::{fs, path::Path};

const REGEX_INDEX: &[u8] = include_bytes!(
    "../../../crates/semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json"
);
const URL_INDEX: &[u8] = include_bytes!(
    "../../../crates/semaprax-rust-api-index/fixtures/url-2.5.8-index-envelope.json"
);
const REGEX_LOCK: &[u8] =
    include_bytes!("../../../crates/semaprax-toolchain/src/fixtures/ri06-regex-1.13.1.Cargo.lock");
const URL_LOCK: &[u8] =
    include_bytes!("../../../crates/semaprax-toolchain/src/fixtures/ri06-url-2.5.8.Cargo.lock");

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = root.parent().unwrap().join("project");
    let source_path = project.join("src/app.spx");
    let source = fs::read_to_string(&source_path).expect("combined Project source");
    let regex_index = RustApiIndex::admit_extractor_output(REGEX_INDEX).unwrap();
    let url_index = RustApiIndex::admit_extractor_output(URL_INDEX).unwrap();
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
            index_bytes: regex_index.canonical_json().as_bytes(),
            package: regex_package,
        });
    let url = ["url.new", "url.view"].map(|import_id| IndexedProjectUrlRegistrySelection {
        source_path: "src/app.spx",
        source: &source,
        import_id,
        index_bytes: url_index.canonical_json().as_bytes(),
        package: url_package,
    });
    let m1 = prepare_indexed_regex_url_project_packages(
        &project.join("semaprax.toml"),
        &regex,
        &url,
        "regex.run",
        "url.run",
        REGEX_LOCK,
        URL_LOCK,
    )
    .expect("one held combined Project M1 selection");
    let destination = root.join("generated/regex");
    fs::create_dir_all(destination.join("src")).unwrap();
    for (path, bytes) in [
        ("Cargo.toml", m1.regex.cargo_toml()),
        ("Cargo.lock", m1.regex.cargo_lock()),
        ("src/lib.rs", m1.regex.lib_rs()),
        ("src/regex_project.c", m1.regex.c_source()),
        ("src/regex_project.h", m1.regex.header()),
        ("binding-plan.json", m1.regex.binding_plan()),
        ("descriptor.json", m1.regex.descriptor()),
    ] {
        fs::write(destination.join(path), bytes).unwrap();
    }
    let destination = root.join("generated/url");
    fs::create_dir_all(destination.join("src")).unwrap();
    for (path, bytes) in [
        ("Cargo.toml", m1.url.cargo_toml()),
        ("Cargo.lock", m1.url.cargo_lock()),
        ("src/lib.rs", m1.url.lib_rs()),
        ("src/url_project.c", m1.url.c_source()),
        ("src/url_project.h", m1.url.header()),
        ("binding-plan.json", m1.url.binding_plan()),
        ("descriptor.json", m1.url.descriptor()),
    ] {
        fs::write(destination.join(path), bytes).unwrap();
    }
    let m2 = prepare_native_rust_serde_iterator_callbacks(
        &source,
        &source_path,
        "ri13.event",
        "callback.factory",
        "callback.advance",
    )
    .expect("combined Project M2 selection");
    let m2_dir = root.join("generated/m2");
    fs::create_dir_all(&m2_dir).unwrap();
    fs::write(m2_dir.join("module.c"), &m2.callback.c_source).unwrap();
    fs::write(
        m2_dir.join("semaprax_native_rust_interop.h"),
        &m2.callback.header,
    )
    .unwrap();
    fs::write(
        m2_dir.join("module.rs"),
        format!(
            "{}\n{}\n{}\n",
            m2.callback.safe_rust, m2.callback.adapter_rust, m2.record.rust_source
        ),
    )
    .unwrap();
    let m3 = with_authenticated_project(&project.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        snapshot.render_source_local_future_rust_module()
    })
    .expect("combined Project M3 selection");
    fs::write(root.join("generated/m3.rs"), m3).unwrap();
    println!(
        "ri13-linked-prepared:{}:{}",
        m1.subject_digest, m2.source_revision
    );
}
