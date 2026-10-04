use semaprax::project::with_authenticated_project;
use semaprax_native_rust_interop::{
    indexed_binding::SelectedPackage, prepare_indexed_regex_url_project_packages,
    prepare_native_rust_serde_iterator_callbacks, IndexedProjectRegexRegistrySelection,
    IndexedProjectUrlRegistrySelection,
};
use semaprax_rust_api_index::RustApiIndex;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

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

const LINUX_X86_64_TARGET: &str = "x86_64-unknown-linux-gnu";

fn selected_index(name: &str, built_in: &[u8]) -> Vec<u8> {
    let Some(directory) = env::var_os("RI13_RUST_API_INDEX_DIR") else {
        return built_in.to_vec();
    };
    let directory = PathBuf::from(directory);
    assert!(
        directory.is_absolute(),
        "RI13 Rust API index directory must be absolute"
    );
    let path = directory.join(name);
    let metadata = fs::symlink_metadata(&path).expect("selected RI13 Rust API index metadata");
    assert!(
        metadata.file_type().is_file(),
        "selected RI13 Rust API index must be a regular file"
    );
    fs::read(path).expect("selected RI13 Rust API index")
}

fn admit_linux_index(name: &str, built_in: &[u8]) -> RustApiIndex {
    let index = RustApiIndex::admit_extractor_output(&selected_index(name, built_in))
        .expect("admitted selected RI13 Rust API index");
    if env::var_os("RI13_RUST_API_INDEX_DIR").is_some() {
        assert_eq!(
            index.target(),
            LINUX_X86_64_TARGET,
            "selected RI13 Rust API index target"
        );
    }
    index
}

fn embed_generated_package(destination: &Path) {
    let manifest = destination.join("Cargo.toml");
    let source = fs::read_to_string(&manifest).expect("generated package manifest");
    assert_eq!(source.matches("\n[workspace]\n").count(), 1);
    // The generated crates are standalone by default. In this combined app,
    // they are path dependencies within the consumer's single workspace.
    fs::write(manifest, source.replacen("\n[workspace]\n", "\n", 1))
        .expect("embedded package manifest");
}

fn require_fragment(source: &str, fragment: &str, subject: &str) {
    assert!(
        source.contains(fragment),
        "linked RI-13 candidate {subject} is missing {fragment:?}"
    );
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let examples = root.parent().unwrap().parent().unwrap();
    // M1, M2, and M3 retain their independently admitted source profiles.
    // Combining their declarations into the Future Project was never valid:
    // dependency tables are scalar-package-only, while M3 selects the Future
    // profile. This linker exercises the generated outputs together without
    // widening either profile's manifest authority.
    let m1_project = examples.join("ri13-m1-regex-url/project");
    let m1_source_path = m1_project.join("src/app.spx");
    let m1_source = fs::read_to_string(&m1_source_path).expect("saved M1 Project source");
    let m2_project = examples.join("ri13-m2-record-iterator/project");
    let m2_source_path = m2_project.join("app.spx");
    let m3_project = examples.join("ri13-m3-local-http/project");
    // This is the authored one-Project candidate. It intentionally cannot be
    // admitted today: `source-local-future.v1` rejects M1's dependency table.
    // Keep it bound here so the successful generated linkage and the exact
    // failed-unification boundary advance together.
    let unified = root.parent().unwrap().join("unified-project");
    let unified_manifest = fs::read_to_string(unified.join("semaprax.toml"))
        .expect("authored unified Project candidate manifest");
    let unified_source = fs::read_to_string(unified.join("src/app.spx"))
        .expect("authored unified Project candidate source");
    require_fragment(
        &unified_manifest,
        "profile = \"source-local-future.v1\"",
        "manifest",
    );
    require_fragment(&unified_manifest, "[rust-dependencies]", "manifest");
    for identity in [
        "regex.run",
        "url.run",
        "ri13.event",
        "callback.factory",
        "callback.advance",
        "ri13.m3.score",
    ] {
        require_fragment(&unified_source, &format!("@id(\"{identity}\")"), "source");
    }
    let regex_index = admit_linux_index("regex-1.13.1-index-envelope.json", REGEX_INDEX);
    let url_index = admit_linux_index("url-2.5.8-index-envelope.json", URL_INDEX);
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
            source: &m1_source,
            import_id,
            index_bytes: regex_index.canonical_json().as_bytes(),
            package: regex_package,
        });
    let url = ["url.new", "url.view"].map(|import_id| IndexedProjectUrlRegistrySelection {
        source_path: "src/app.spx",
        source: &m1_source,
        import_id,
        index_bytes: url_index.canonical_json().as_bytes(),
        package: url_package,
    });
    let m1 = prepare_indexed_regex_url_project_packages(
        &m1_project.join("semaprax.toml"),
        &regex,
        &url,
        "regex.run",
        "url.run",
        REGEX_LOCK,
        URL_LOCK,
    )
    .expect("one held M1 Project selection");
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
    embed_generated_package(&destination);
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
    embed_generated_package(&destination);
    let m2 = with_authenticated_project(&m2_project.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let source = snapshot
            .sources()
            .iter()
            .find(|source| source.path() == "app.spx")
            .expect("authenticated M2 manifest requires app.spx");
        prepare_native_rust_serde_iterator_callbacks(
            source.source(),
            &m2_source_path,
            "ri13.event",
            "callback.factory",
            "callback.advance",
        )
    })
    .expect("authenticated M2 Project source selection");
    let m2_dir = root.join("generated/m2");
    fs::create_dir_all(&m2_dir).unwrap();
    fs::write(m2_dir.join("module.c"), &m2.callback.c_source).unwrap();
    fs::write(
        m2_dir.join("semaprax_native_rust_interop_ffi.rs"),
        &m2.callback.ffi_rust,
    )
    .unwrap();
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
    let (m3, m3_revision) =
        with_authenticated_project(&m3_project.join("semaprax.toml"), |snapshot| {
            snapshot.check()?;
            let revision = snapshot.retain_revision();
            Ok((
                snapshot.render_source_local_future_rust_module()?,
                revision.project_revision().to_owned(),
            ))
        })
        .expect("held M3 Future Project selection");
    fs::write(root.join("generated/m3.rs"), m3).unwrap();
    fs::write(
        root.join("generated/linked-subject.json"),
        format!(
            concat!(
                "{\n",
                "  \"schema\": \"semaprax.ri13.linked-subject.v1\",\n",
                "  \"m1_project_subject\": {:?},\n",
                "  \"m2_source_revision\": {:?},\n",
                "  \"m3_project_revision\": {:?},\n",
                "  \"candidate\": \"unified-project/semaprax.toml\"\n",
                "}\n"
            ),
            m1.subject_digest, m2.source_revision, m3_revision,
        ),
    )
    .expect("linked subject binding");
    println!(
        "ri13-linked-prepared:{}:{}",
        m1.subject_digest, m2.source_revision
    );
}
