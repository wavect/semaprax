use super::*;
use crate::indexed_binding::SelectedPackage;
use semaprax_rust_api_index::RustApiIndex;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = include_str!("../../../../examples/ri13-m1-regex-url/project/src/app.spx");
const TESTS: &str = include_str!("../../../../examples/ri13-m1-regex-url/project/src/tests.spx");
const MANIFEST: &str = include_str!("../../../../examples/ri13-m1-regex-url/project/semaprax.toml");
const REGEX_INDEX: &[u8] =
    include_bytes!("../../../semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json");
const URL_INDEX: &[u8] =
    include_bytes!("../../../semaprax-rust-api-index/fixtures/url-2.5.8-index-envelope.json");
const REGEX_LOCK: &[u8] =
    include_bytes!("../../../semaprax-toolchain/src/fixtures/ri06-regex-1.13.1.Cargo.lock");
const URL_LOCK: &[u8] =
    include_bytes!("../../../semaprax-toolchain/src/fixtures/ri06-url-2.5.8.Cargo.lock");
const RI13_SOURCE: &str =
    include_str!("../../../../examples/ri13-combined-app/unified-project/src/app.spx");
const RI13_TESTS: &str =
    include_str!("../../../../examples/ri13-combined-app/unified-project/src/tests.spx");
const RI13_MANIFEST: &str =
    include_str!("../../../../examples/ri13-combined-app/unified-project/semaprax.toml");

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn one_held_regex_url_project_authenticates_four_imports_two_exports_and_both_locks() {
    let parsed = semaprax::parse(SOURCE, Path::new("src/app.spx")).unwrap();
    assert_eq!(semaprax::format::canonical(&parsed), SOURCE);
    let parsed_tests = semaprax::parse(TESTS, Path::new("src/tests.spx")).unwrap();
    assert_eq!(semaprax::format::canonical(&parsed_tests), TESTS);
    let root = Temp(
        fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "ri13-m1-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
    );
    fs::create_dir_all(root.0.join("src")).unwrap();
    fs::write(root.0.join("src/app.spx"), SOURCE).unwrap();
    fs::write(root.0.join("src/tests.spx"), TESTS).unwrap();
    fs::write(root.0.join("semaprax.toml"), MANIFEST).unwrap();
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
            source: SOURCE,
            import_id,
            index_bytes: regex_json.as_bytes(),
            package: regex_package,
        });
    let url = ["url.new", "url.view"].map(|import_id| IndexedProjectUrlRegistrySelection {
        source_path: "src/app.spx",
        source: SOURCE,
        import_id,
        index_bytes: url_json.as_bytes(),
        package: url_package,
    });
    let manifest_path = root.0.join("semaprax.toml");
    let built = prepare_indexed_regex_url_project_packages(
        &manifest_path,
        &regex,
        &url,
        "regex.run",
        "url.run",
        REGEX_LOCK,
        URL_LOCK,
    )
    .unwrap();
    assert_eq!(built.regex.project_subject_digest(), built.subject_digest);
    assert_eq!(built.url.project_subject_digest(), built.subject_digest);
    assert_eq!(built.regex.cargo_lock(), REGEX_LOCK);
    assert_eq!(built.url.cargo_lock(), URL_LOCK);
    assert!(std::str::from_utf8(built.regex.lib_rs())
        .unwrap()
        .contains("pub fn run()"));
    assert!(std::str::from_utf8(built.url.lib_rs())
        .unwrap()
        .contains("pub fn run()"));
    assert_ne!(built.regex.c_source(), built.url.c_source());
    assert!(std::str::from_utf8(built.url.lib_rs())
        .unwrap()
        .contains("spx_url_result_owner_context_new"));
    assert!(!std::str::from_utf8(built.url.lib_rs())
        .unwrap()
        .contains("spx_result_owner_context_new"));
    assert!(std::str::from_utf8(built.url.c_source())
        .unwrap()
        .contains("spx_url_result_owner_new_utf8"));
    let wrong_lock = prepare_indexed_regex_url_project_packages(
        &manifest_path,
        &regex,
        &url,
        "regex.run",
        "url.run",
        URL_LOCK,
        URL_LOCK,
    )
    .unwrap_err();
    assert!(wrong_lock[0].message.contains("pinned lock"));
    let wrong_export = prepare_indexed_regex_url_project_packages(
        &manifest_path,
        &regex,
        &url,
        "url.run",
        "regex.run",
        REGEX_LOCK,
        URL_LOCK,
    )
    .unwrap_err();
    assert!(
        wrong_export[0].message.contains("Regex Project")
            || wrong_export[0].message.contains("Url Project")
    );
    let stale_source = SOURCE.replace("example.invalid", "other.invalid");
    assert_ne!(SOURCE, stale_source);
    fs::write(root.0.join("src/app.spx"), stale_source).unwrap();
    let stale = prepare_indexed_regex_url_project_packages(
        &manifest_path,
        &regex,
        &url,
        "regex.run",
        "url.run",
        REGEX_LOCK,
        URL_LOCK,
    )
    .unwrap_err();
    assert_eq!(stale[0].code, "SPX-B142");
}

#[test]
fn closed_ri13_profile_binds_indexed_m1_signatures_before_future_admission() {
    let root = Temp(
        fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "ri13-indexed-future-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
    );
    fs::create_dir_all(root.0.join("src")).unwrap();
    fs::write(root.0.join("src/app.spx"), RI13_SOURCE).unwrap();
    fs::write(root.0.join("src/tests.spx"), RI13_TESTS).unwrap();
    let manifest_path = root.0.join("semaprax.toml");
    fs::write(&manifest_path, RI13_MANIFEST).unwrap();

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
            source: RI13_SOURCE,
            import_id,
            index_bytes: regex_json.as_bytes(),
            package: regex_package,
        });
    let url = ["url.new", "url.view"].map(|import_id| IndexedProjectUrlRegistrySelection {
        source_path: "src/app.spx",
        source: RI13_SOURCE,
        import_id,
        index_bytes: url_json.as_bytes(),
        package: url_package,
    });

    with_authenticated_indexed_regex_url_project(&manifest_path, &regex, &url, |snapshot| {
        snapshot.check()?;
        assert_eq!(
            snapshot.retain_revision().manifest().project_profile(),
            semaprax::project::ProjectProfile::SourceLocalFutureIndexedRustV1
        );
        assert_eq!(
            snapshot.source_local_future_signature()?.function_id(),
            "ri13.m3.score"
        );
        Ok(())
    })
    .unwrap();

    let source_drift = RI13_SOURCE.replace("example.invalid", "other.invalid");
    fs::write(root.0.join("src/app.spx"), source_drift).unwrap();
    let refusal =
        with_authenticated_indexed_regex_url_project(&manifest_path, &regex, &url, |_| Ok(()))
            .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-B142");

    fs::write(root.0.join("src/app.spx"), RI13_SOURCE).unwrap();
    let untrusted_dependency = RI13_MANIFEST.replace("url = [\"=2.5.8\"]", "url = [\"=2.5.7\"]");
    fs::write(&manifest_path, untrusted_dependency).unwrap();
    let refusal =
        with_authenticated_indexed_regex_url_project(&manifest_path, &regex, &url, |_| Ok(()))
            .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-H006");

    let unsupported_profile =
        RI13_MANIFEST.replace("profile = \"source-local-future-indexed-rust.v1\"\n", "");
    fs::write(&manifest_path, unsupported_profile).unwrap();
    let refusal =
        with_authenticated_indexed_regex_url_project(&manifest_path, &regex, &url, |_| Ok(()))
            .unwrap_err();
    assert!(refusal[0]
        .message
        .contains("source-local-future-indexed-rust.v1"));
}
