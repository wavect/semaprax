use super::support::*;

#[test]
fn resolver_root_and_selected_subject_must_be_one_exact_package() {
    let first_report = report_from_source("alpha", &simple_source("alpha", 1));
    let second_report = report_from_source("beta", &simple_source("beta", 2));
    let first = subject(&first_report, "alpha", "1.0.0", &[], &[]);
    let second = subject(&second_report, "beta", "1.0.0", &[], &[]);
    let input = input(
        &[("alpha", "1.0.0"), ("beta", "1.0.0")],
        vec![first, second],
        "wasm32",
        &[],
    );
    let resolution = package_resolver::generate(&input, &ResolutionOptions::default())
        .expect("two-root resolver evidence remains valid Resolver v1");
    let options = build_options("alpha", &["alpha.answer"], MAX_BYTES, MAX_BYTES);
    assert_eq!(generate_error(&resolution, &input, &options), "SPX-PB503");

    let fixture = fixture();
    let wrong_root = build_options("foreign", &["calculator.add"], MAX_BYTES, MAX_BYTES);
    assert_eq!(
        generate_error(&fixture.resolution, &fixture.input, &wrong_root),
        "SPX-PB503"
    );
}

#[test]
fn dependency_metadata_cannot_substitute_for_linked_source() {
    let dependency_report = report_from_source("dependency", &simple_source("dependency", 7));
    let dependency_coordinate = coordinate("dependency", "1.0.0");
    let dependency = subject(&dependency_report, "dependency", "1.0.0", &[], &[]);
    let root_report = report_from_source("root", &simple_source("root", 9));
    let root = subject(&root_report, "root", "1.0.0", &[dependency_coordinate], &[]);
    let input = input(&[("root", "1.0.0")], vec![dependency, root], "wasm32", &[]);
    let resolution = package_resolver::generate(&input, &ResolutionOptions::default())
        .expect("dependency closure is valid Resolver v1 evidence");
    let options = build_options("root", &["root.answer"], MAX_BYTES, MAX_BYTES);
    assert_eq!(generate_error(&resolution, &input, &options), "SPX-PB503");
}

#[test]
fn nonempty_capability_or_wrong_target_is_outside_the_effect_free_profile() {
    let report = report_from_source("authority", &simple_source("authority", 1));
    let authority_subject = subject(&report, "authority", "1.0.0", &[], &["fs.read"]);
    let authority_input = input(
        &[("authority", "1.0.0")],
        vec![authority_subject],
        "wasm32",
        &["fs.read"],
    );
    let resolution = package_resolver::generate(&authority_input, &ResolutionOptions::default())
        .expect("resolver allowlist admits the declared capability");
    let options = build_options("authority", &["authority.answer"], MAX_BYTES, MAX_BYTES);
    assert_eq!(
        generate_error(&resolution, &authority_input, &options),
        "SPX-PB504"
    );

    let report = report_from_source("native_only", &simple_source("native_only", 1));
    let native_subject = subject(&report, "native_only", "1.0.0", &[], &[]);
    let native_input = input(
        &[("native_only", "1.0.0")],
        vec![native_subject],
        "native64",
        &[],
    );
    let resolution = package_resolver::generate(&native_input, &ResolutionOptions::default())
        .expect("native target is valid Resolver v1 evidence");
    let options = build_options("native_only", &["native_only.answer"], MAX_BYTES, MAX_BYTES);
    assert_eq!(
        generate_error(&resolution, &native_input, &options),
        "SPX-PB504"
    );
}

#[test]
fn export_selection_and_authored_aggregate_surface_remain_closed() {
    let fixture = fixture();
    for exports in [Vec::<&str>::new(), vec!["calculator.add", "calculator.add"]] {
        let options = build_options(ROOT, &exports, MAX_BYTES, MAX_BYTES);
        assert_eq!(
            generate_error(&fixture.resolution, &fixture.input, &options),
            "SPX-PB501"
        );
    }
    let unsorted = build_options(
        ROOT,
        &["calculator.not", "calculator.add"],
        MAX_BYTES,
        MAX_BYTES,
    );
    assert_eq!(
        generate_error(&fixture.resolution, &fixture.input, &unsorted),
        "SPX-PB501"
    );

    let report = report_from_source(
        "examples.records",
        include_str!("../../examples/records.spx"),
    );
    let subject = subject(&report, "examples.records", "1.0.0", &[], &[]);
    let input = input(
        &[("examples.records", "1.0.0")],
        vec![subject],
        "wasm32",
        &[],
    );
    let resolution = package_resolver::generate(&input, &ResolutionOptions::default())
        .expect("aggregate source is valid Resolver v1 target evidence");
    let options = build_options("examples.records", &["app.main"], MAX_BYTES, MAX_BYTES);
    let error =
        package_build::generate(&resolution, &input, &ResolutionOptions::default(), &options)
            .expect_err("authored aggregates remain outside scalar package v1");
    assert_eq!(error[0].code, "SPX-PB504");
}

#[test]
fn root_option_uses_the_exact_source_module_grammar() {
    let fixture = fixture();
    for root in ["bad-name", "1bad", "bad..name", "bad."] {
        let options = build_options(root, &["calculator.add"], MAX_BYTES, MAX_BYTES);
        assert_eq!(
            generate_error(&fixture.resolution, &fixture.input, &options),
            "SPX-PB501"
        );
    }
}

/// The package lane admits the same Copy-scalar surface as the Public Scalar
/// Export Profile v1 it emits: the interface report, the resolver subject, and
/// the emitted manifest all describe one widened ABI. `usize` stays outside.
#[test]
fn package_lane_admits_the_widened_copy_scalar_surface() {
    const WIDENED: &str = "module widen.pkg;\n\n@id(\"widen.pkg.char\")\nfn pick_char(value: char) -> char\n{\n    value\n}\n\n@id(\"widen.pkg.mixed\")\nfn mixed(flag: bool, small: u8, medium: i32, ratio: f32) -> f64\n{\n    2.5\n}\n\n@id(\"widen.pkg.main\")\nfn main() -> i64\n{\n    0\n}\n";

    let fixture = fixture_from_source(
        "widen.pkg",
        "1.0.0",
        WIDENED,
        &["widen.pkg.char", "widen.pkg.mixed"],
    );
    let manifest = &fixture.build.manifest_json;
    assert!(manifest.contains("\"stable_id\":\"widen.pkg.char\",\"wasm_export\":\"spx_scalar_"));
    assert!(manifest.contains("\"parameters\":[\"char\"],\"result\":\"char\""));
    assert!(
        manifest.contains("\"parameters\":[\"bool\",\"u8\",\"i32\",\"f32\"],\"result\":\"f64\"")
    );
}

/// `usize` is outside the profile in the package lane too. The lane maps the
/// `SPX-W115` profile rejection to its own `SPX-PB504`, before any artifact
/// exists.
#[test]
fn package_lane_still_excludes_usize() {
    const USIZE_SOURCE: &str = "module widen.usz;\n\n@id(\"widen.usz.width\")\nfn width(value: usize) -> usize\n{\n    value\n}\n\n@id(\"widen.usz.main\")\nfn main() -> i64\n{\n    0\n}\n";

    let report = report_from_source("widen.usz", USIZE_SOURCE);
    let subject = subject(&report, "widen.usz", "1.0.0", &[], &[]);
    let input = input(&[("widen.usz", "1.0.0")], vec![subject], "wasm32", &[]);
    let resolution = package_resolver::generate(&input, &ResolutionOptions::default())
        .expect("usize source is still valid resolver evidence");
    let options = build_options("widen.usz", &["widen.usz.width"], MAX_BYTES, MAX_BYTES);
    assert_eq!(generate_error(&resolution, &input, &options), "SPX-PB504");
}
