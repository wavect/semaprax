//! Physical RI-04 scalar call: checked source -> indexed plan -> generated
//! Rust adapter -> C bridge -> Semaprax export -> selected Rust crate API.

use super::*;
use semaprax_rust_api_index::RustApiIndex;

#[test]
fn indexed_scalar_adapter_executes_and_rejects_flipped_rust_result() {
    let source = SOURCE.replacen(
        "import rust fn host_add(left: i64, right: i64) -> i64",
        "import rust fn host_add(left: i64, right: i64) -> i64 from \"fixture_math::add\"",
        1,
    );
    let program = crate::parse(&source, Path::new("indexed-scalar.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    assert_eq!(canonical, source);
    let resolved = hir::resolve(&program).unwrap();
    let import = &resolved.interfaces[0].imports[0];
    let target = current_target().unwrap();
    let crate_source = "pub fn add(left:i64,right:i64)->i64{left+right}\n";
    let crate_source_digest = raw_digest(crate_source.as_bytes());
    let mut envelope: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json"
    ))
    .unwrap();
    let index_value = &mut envelope["index"];
    index_value["package"]["name"] = "fixture_math".into();
    index_value["package"]["version"] = "0.0.1".into();
    index_value["package"]["source_sha256"] = crate_source_digest.clone().into();
    index_value["target"] = target.triple.clone().into();
    let rustc = configured_tool("RUSTC").unwrap();
    let selected_compiler = Command::new(&rustc.path)
        .env_clear()
        .arg("--version")
        .output()
        .unwrap();
    assert!(selected_compiler.status.success());
    index_value["stable_rustc_version"] = std::str::from_utf8(&selected_compiler.stdout)
        .unwrap()
        .trim()
        .into();
    let mut item = index_value["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["path"] == "local_api_fixture::cfg_selected")
        .unwrap()
        .clone();
    item["path"] = "fixture_math::add".into();
    item["signature"] = "fn add(left: i64, right: i64) -> i64".into();
    index_value["items"] = serde_json::json!([item]);
    index_value["types"] = serde_json::json!([]);
    let mut extractor_bytes = serde_json::to_vec(&envelope).unwrap();
    extractor_bytes.push(b'\n');
    let admitted = RustApiIndex::admit_extractor_output(&extractor_bytes).unwrap();
    let index_bytes = admitted.canonical_json().as_bytes();
    let package = crate::indexed_binding::SelectedPackage {
        cargo_alias: "fixture_math",
        name: "fixture_math",
        version: "0.0.1",
        source_sha256: &crate_source_digest,
        target: &target.triple,
        feature_digest: admitted.feature_digest(),
        stable_rustc_version: admitted.stable_rustc_version(),
    };
    let plan = crate::indexed_binding::prepare_indexed_scalar_binding(
        import,
        index_bytes,
        package,
        "fixture_math::add",
    )
    .unwrap();
    let collision_source = source.replacen("@id(\"host.add\")", "@id(\"host.add_2\")", 1);
    let collision_program =
        crate::parse(&collision_source, Path::new("indexed-scalar-collision.spx")).unwrap();
    let collision_hir = hir::resolve(&collision_program).unwrap();
    let collision_import = &collision_hir.interfaces[0].imports[0];
    let collision_plan = crate::indexed_binding::prepare_indexed_scalar_binding(
        collision_import,
        index_bytes,
        package,
        "fixture_math::add",
    )
    .unwrap();
    assert_ne!(plan.physical_symbol, collision_plan.physical_symbol);
    let spec = render_spec(&Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical.as_bytes())),
        target: target.clone(),
        exports: vec!["interop.add".to_owned()],
        imports: vec!["host.add".to_owned()],
        capabilities: vec!["host.math".to_owned()],
    });
    // The existing callback builder must still refuse this indexed source.
    assert_eq!(
        prepare_native_rust_interop(&program, spec.as_bytes())
            .err()
            .unwrap()[0]
            .code,
        "SPX-B107"
    );
    let (prepared, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
        phase_a::prepare_indexed_native_rust_interop_bounded(
            &program,
            spec.as_bytes(),
            std::slice::from_ref(&plan),
        )
    });
    assert!(!overflowed);
    let prepared = prepared.unwrap();
    let mut stale = plan.clone();
    stale.physical_symbol.push('0');
    let (stale_result, stale_overflowed) =
        crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
            phase_a::prepare_indexed_native_rust_interop_bounded(
                &program,
                spec.as_bytes(),
                &[stale],
            )
        });
    assert!(!stale_overflowed);
    let stale_error = stale_result.err().unwrap();
    assert_eq!(stale_error.code, "SPX-B142");
    assert_eq!(stale_error.span, Some(import.span));
    let expected_adapter = crate::indexed_binding::render_checked_scalar_adapter(
        import,
        &plan,
        &prepared.imports[0].rust_method,
    )
    .unwrap();
    let bad_method = crate::indexed_binding::render_checked_scalar_adapter(
        import,
        &plan,
        "import_valid;panic!()",
    )
    .unwrap_err();
    assert_eq!(bad_method.code, "SPX-B145");
    assert_eq!(bad_method.span, Some(import.span));
    assert!(expected_adapter.contains(&plan.physical_symbol));
    assert!(expected_adapter.contains("let target:fn(i64,i64)->i64=fixture_math::add"));

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!("semaprax-ri04-scalar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    for (label, rejected_package) in [
        (
            "alias",
            crate::indexed_binding::SelectedPackage {
                cargo_alias: "fixture_math_other",
                ..package
            },
        ),
        (
            "target",
            crate::indexed_binding::SelectedPackage {
                target: "wasm32-unknown-unknown",
                ..package
            },
        ),
        (
            "compiler",
            crate::indexed_binding::SelectedPackage {
                stable_rustc_version: "rustc 1.97.1",
                ..package
            },
        ),
    ] {
        let error = crate::build_indexed_scalar_native_rust(
            &source,
            Path::new("indexed-scalar.spx"),
            crate::NativeRustSdkOptions {
                exports: vec!["interop.add".to_owned()],
                imports: vec!["host.add".to_owned()],
                capabilities: vec!["host.math".to_owned()],
            },
            index_bytes,
            rejected_package,
            crate_source.as_bytes(),
            &output,
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-B142", "{label}");
        assert_eq!(error[0].span, Some(import.span), "{label}");
        assert!(!output.exists(), "{label} must refuse before publication");
    }
    let built = crate::build_indexed_scalar_native_rust(
        &source,
        Path::new("indexed-scalar.spx"),
        crate::NativeRustSdkOptions {
            exports: vec!["interop.add".to_owned()],
            imports: vec!["host.add".to_owned()],
            capabilities: vec!["host.math".to_owned()],
        },
        index_bytes,
        package,
        crate_source.as_bytes(),
        &output,
    )
    .unwrap();
    assert_eq!(built.output_directory(), output);
    assert_eq!(built.adapter_source(), expected_adapter);
    assert_eq!(
        built.adapter_sha256(),
        raw_digest(expected_adapter.as_bytes())
    );
    assert_eq!(built.physical_symbol(), plan.physical_symbol);
    let adapter = built.adapter_source();
    assert_eq!(
        std::str::from_utf8(&selected_compiler.stdout)
            .unwrap()
            .trim(),
        package.stable_rustc_version,
        "the physical adapter must use the selected stable compiler"
    );
    let clang = configured_tool("CLANG").unwrap();
    let object = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let export_method = &prepared.exports[0].rust_method;
    let harness = format!(
        "#[path=\"semaprax_native_rust_interop.rs\"] mod semaprax_native_rust_interop;\nuse semaprax_native_rust_interop::*;\n{adapter}\nfn main(){{let capabilities=NativeRustCapabilities::new(&[\"host.math\"]).unwrap_or_else(|_|std::process::exit(13));let mut bridge=NativeRustBridge::new(GeneratedIndexedAdapter,capabilities);match bridge.{export_method}(20,22){{Ok(64)=>{{}},_=>std::process::exit(12)}}}}\n"
    );
    std::fs::write(output.join("roundtrip.rs"), harness).unwrap();

    for (label, body, should_run) in [
        ("correct", crate_source, true),
        (
            "flipped",
            "pub fn add(left:i64,right:i64)->i64{left+right+1}\n",
            false,
        ),
    ] {
        let rust_file = format!("fixture_math_{label}.rs");
        let rlib = format!("libfixture_math_{label}.rlib");
        let executable = if cfg!(windows) {
            format!("roundtrip_{label}.exe")
        } else {
            format!("roundtrip_{label}")
        };
        std::fs::write(output.join(&rust_file), body).unwrap();
        let mut crate_compile = Command::new(&rustc.path);
        crate_compile.env_clear().current_dir(&output).args([
            "--edition=2021",
            "--crate-name",
            "fixture_math",
            "--crate-type=rlib",
            &rust_file,
            "-o",
            &rlib,
        ]);
        bind_test_tool_environment(&mut crate_compile);
        assert!(crate_compile.status().unwrap().success());
        let mut harness_compile = Command::new(&rustc.path);
        harness_compile.env_clear().current_dir(&output).args([
            "--edition=2021",
            "-C",
            "panic=unwind",
            "-C",
            &format!("link-arg={object}"),
            "--extern",
            &format!("fixture_math={rlib}"),
            "roundtrip.rs",
            "-o",
            &executable,
        ]);
        bind_test_tool_environment(&mut harness_compile);
        bind_test_rust_linker(&mut harness_compile, &clang);
        assert!(harness_compile.status().unwrap().success());
        let status = Command::new(output.join(&executable))
            .env_clear()
            .current_dir(&output)
            .status()
            .unwrap();
        assert_eq!(
            status.code(),
            Some(if should_run { 0 } else { 12 }),
            "{label} Rust result must change the Semaprax assertion"
        );
    }
    // A stale metadata signature cannot enter the bridge: stable rustc must
    // type-check the generated function-pointer assignment against the crate.
    std::fs::write(
        output.join("fixture_math_wrong_type.rs"),
        "pub fn add(_left:i64,_right:i64)->bool{true}\n",
    )
    .unwrap();
    let mut wrong_crate = Command::new(&rustc.path);
    wrong_crate.env_clear().current_dir(&output).args([
        "--edition=2021",
        "--crate-name",
        "fixture_math",
        "--crate-type=rlib",
        "fixture_math_wrong_type.rs",
        "-o",
        "libfixture_math_wrong_type.rlib",
    ]);
    bind_test_tool_environment(&mut wrong_crate);
    assert!(wrong_crate.status().unwrap().success());
    let mut wrong_harness = Command::new(&rustc.path);
    wrong_harness.env_clear().current_dir(&output).args([
        "--edition=2021",
        "-C",
        "panic=unwind",
        "-C",
        &format!("link-arg={object}"),
        "--extern",
        "fixture_math=libfixture_math_wrong_type.rlib",
        "roundtrip.rs",
        "-o",
        if cfg!(windows) {
            "roundtrip_wrong_type.exe"
        } else {
            "roundtrip_wrong_type"
        },
    ]);
    bind_test_tool_environment(&mut wrong_harness);
    bind_test_rust_linker(&mut wrong_harness, &clang);
    let rejected_signature = wrong_harness.output().unwrap();
    assert!(!rejected_signature.status.success());
    assert!(
        String::from_utf8_lossy(&rejected_signature.stderr).contains("mismatched types"),
        "stable rustc must reject the selected generated function-pointer type"
    );
    assert!(!output
        .join(if cfg!(windows) {
            "roundtrip_wrong_type.exe"
        } else {
            "roundtrip_wrong_type"
        })
        .exists());
    std::fs::remove_dir_all(&root).unwrap();
}
