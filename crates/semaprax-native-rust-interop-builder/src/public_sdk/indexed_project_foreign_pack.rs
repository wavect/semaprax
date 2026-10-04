//! LAW-15 saved example inputs and physical mutation/repair controls. Kept in
//! the existing guarded Project SDK owning harness, never a second test binary.
use super::*;
use semaprax::native_rust_binding::foreign_law::{DeclaredForeignSummary, ForeignLawRequest};

pub(super) const APP: &str =
    include_str!("../../../../examples/law-packs/foreign-boundary/src/app.spx");
pub(super) const TESTS: &str =
    include_str!("../../../../examples/law-packs/foreign-boundary/src/tests.spx");
pub(super) const MANIFEST: &str =
    include_str!("../../../../examples/law-packs/foreign-boundary/semaprax.toml");
pub(super) const CORRECT: &[u8] =
    include_bytes!("../../../../examples/law-packs/foreign-boundary/rust/correct.rs");
const BAD: &[u8] =
    include_bytes!("../../../../examples/law-packs/foreign-boundary/rust/bad-return.rs");
const REPAIRED: &[u8] =
    include_bytes!("../../../../examples/law-packs/foreign-boundary/rust/repaired.rs");
const WEAK: &[u8] =
    include_bytes!("../../../../examples/law-packs/foreign-boundary/rust/weak-law.rs");
pub(super) const ASSUMPTIONS: &[u8] =
    include_bytes!("../../../../examples/law-packs/foreign-boundary/assumptions.json");
const EXPECTED: &[u8] =
    include_bytes!("../../../../examples/law-packs/foreign-boundary/report-expectations.json");

pub(super) fn review() -> (DeclaredForeignSummary, ForeignLawRequest) {
    let value: Value = serde_json::from_slice(ASSUMPTIONS).unwrap();
    assert_eq!(value["law_version"], 1);
    assert_eq!(value["semantics"], "checked-v1");
    assert_eq!(value["return_i64_range"], serde_json::json!([0, 50]));
    let declared = DeclaredForeignSummary {
        assumption_id: value["assumption_id"].as_str().unwrap().into(),
        // The version and every declared condition participate in identity.
        proposition_digest: raw_digest(ASSUMPTIONS),
        assumes_no_effects: value["assumes_no_effects"].as_bool().unwrap(),
        assumes_no_callbacks: value["assumes_no_callbacks"].as_bool().unwrap(),
        assumes_no_panics: value["assumes_no_panics"].as_bool().unwrap(),
        assumes_no_shared_state: value["assumes_no_shared_state"].as_bool().unwrap(),
        return_i64_range: Some((
            value["return_i64_range"][0].as_i64().unwrap(),
            value["return_i64_range"][1].as_i64().unwrap(),
        )),
    };
    let law = ForeignLawRequest {
        law_id: value["law_id"].as_str().unwrap().into(),
        permit_assumptions: true,
        require_theorem: false,
        require_no_effects: true,
        require_no_callbacks: true,
        require_no_panics: true,
        require_no_shared_state: true,
        require_return_guard: true,
    };
    (declared, law)
}
pub(super) fn verify_report(view: &Value) {
    let expected: Value = serde_json::from_slice(EXPECTED).unwrap();
    for (key, value) in expected.as_object().unwrap() {
        assert_eq!(&view[key], value, "saved report expectation {key}");
    }
    let observed: serde_json::Map<String, Value> = expected
        .as_object()
        .unwrap()
        .keys()
        .map(|key| (key.clone(), view[key].clone()))
        .collect();
    eprintln!(
        "LAW15 foreign-boundary conditional report: {}",
        Value::Object(observed)
    );
}
pub(super) fn changed_version_digest() -> String {
    let mut value: Value = serde_json::from_slice(ASSUMPTIONS).unwrap();
    value["law_version"] = 2.into();
    raw_digest(&serde_json::to_vec(&value).unwrap())
}

#[allow(
    clippy::too_many_arguments,
    reason = "exact tools and proof inputs are independent fixture controls"
)]
pub(super) fn mutation_and_repair(
    root: &Path,
    source: &str,
    rustc: &str,
    clang: &str,
    version: &str,
    declared: &DeclaredForeignSummary,
    law: &ForeignLawRequest,
    original: &ProjectNativeRustSdkBundle,
) {
    let fixed_assumptions = declared.clone();
    let fixed_law = law.clone();
    let mut last_summary = String::new();
    for (label, crate_source, expected) in [
        ("bad-return", BAD, None),
        ("repaired", REPAIRED, Some(42)),
        ("weak-law", WEAK, Some(0)),
    ] {
        let index = index_for(crate_source, version);
        let replay = RustApiIndex::replay(&index).unwrap();
        let digest = raw_digest(crate_source);
        let selected = IndexedProjectScalarSelection {
            source_path: "src/app.spx",
            source,
            selection: IndexedScalarSelection {
                import_id: "host.add",
                index_bytes: &index,
                package_source_bytes: crate_source,
                package: SelectedPackage {
                    cargo_alias: "fixture_math",
                    name: "fixture_math",
                    version: "0.0.1",
                    source_sha256: &digest,
                    target: target_triple().unwrap(),
                    feature_digest: replay.feature_digest(),
                    stable_rustc_version: version,
                },
            },
        };
        let output = root.join(label);
        reset_build_observer();
        let (bundle, frontier) = build_guarded_indexed_project_native_rust_sdk(
            &root.join("semaprax.toml"),
            &[selected],
            GuardedForeignLawSelection {
                import_id: "host.add",
                declared,
                law,
            },
            &output,
        )
        .unwrap();
        assert_ne!(
            bundle.manifest_digest(),
            original.manifest_digest(),
            "changed crate needs fresh artifact evidence"
        );
        assert_ne!(frontier.summary_digest(), last_summary);
        last_summary = frontier.summary_digest().into();
        let bindings = prepare_project_bindings(&[selected]).unwrap();
        semaprax::project::with_authenticated_indexed_rust_project(
            &root.join("semaprax.toml"),
            &bindings,
            |snapshot| {
                let revision = snapshot.retain_revision();
                let import = revision
                    .entry_program()
                    .interfaces
                    .iter()
                    .flat_map(|i| &i.imports)
                    .find(|i| i.id.as_str() == "host.add")
                    .unwrap();
                let plan = crate::indexed_binding::prepare_indexed_scalar_binding(
                    import,
                    &index,
                    selected.selection.package,
                    import.rust_path.as_deref().unwrap(),
                )
                .map_err(|e| vec![e])?;
                let caller = revision.foreign_caller_certificate(
                    "interop.add",
                    &plan,
                    plan.target.as_str(),
                    bundle.manifest_digest(),
                    declared,
                    law,
                )?;
                let view: Value = serde_json::from_str(&caller.public_view()).unwrap();
                assert_eq!(view["source_route_proved"], true);
                assert_eq!(view["foreign_internals_proved"], false);
                assert_eq!(caller.conditions().len(), 4);
                assert!(
                    original
                        .bind_guarded_foreign_caller(&revision, caller.clone())
                        .is_err(),
                    "old package cannot certify changed Rust bytes"
                );
                bundle
                    .bind_guarded_foreign_caller(&revision, caller)?
                    .replay(&revision)?;
                Ok(())
            },
        )
        .unwrap();
        let assertion=match expected {
            Some(value)=>format!("assert_eq!(sdk.spx_interop_dot_add(20,22),Ok({value}));"),
            None=>r#"match sdk.spx_interop_dot_add(20,22){
 Err(indexed_sdk::NativeRustSdkCallError::Semantic{domain_id,code,class,retryable})
  if domain_id=="host.math.v1" && code.get()==40909 && class==indexed_sdk::NativeRustSdkStatusClass::Import && !retryable=>{},
 other=>panic!("bad Rust return became successful Semaprax value: {other:?}"),
}"#.into(),
        };
        assert_eq!(run_published_sdk_with_consumer(rustc,clang,root,&output,&format!("fn main(){{let mut sdk=indexed_sdk::indexed_scalar_sdk(&[\"host.math\"]).unwrap();{assertion}}}")),0);
        assert_eq!(declared, &fixed_assumptions);
        assert_eq!(law, &fixed_law);
        assert_eq!(
            std::fs::read_to_string(root.join("src/app.spx")).unwrap(),
            APP
        );
        eprintln!(
            "LAW15 foreign-boundary {label}: {}",
            if expected.is_none() {
                "host.math.v1 / Import / 40909 / nonretryable"
            } else if label == "repaired" {
                "Ok(42), unchanged range law"
            } else {
                "Ok(0), range law does not prove addition"
            }
        );
    }
}
