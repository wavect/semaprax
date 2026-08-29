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
    let dependency = subject(
        &dependency_report,
        "dependency",
        "1.0.0",
        &[],
        &[],
    );
    let root_report = report_from_source("root", &simple_source("root", 9));
    let root = subject(
        &root_report,
        "root",
        "1.0.0",
        &[dependency_coordinate],
        &[],
    );
    let input = input(&[("root", "1.0.0")], vec![dependency, root], "wasm32", &[]);
    let resolution = package_resolver::generate(&input, &ResolutionOptions::default())
        .expect("dependency closure is valid Resolver v1 evidence");
    let options = build_options("root", &["root.answer"], MAX_BYTES, MAX_BYTES);
    assert_eq!(generate_error(&resolution, &input, &options), "SPX-PB503");
}

#[test]
fn nonempty_capability_or_wrong_target_is_outside_the_effect_free_profile() {
    let report = report_from_source("authority", &simple_source("authority", 1));
    let subject = subject(&report, "authority", "1.0.0", &[], &["fs.read"]);
    let input = input(
        &[("authority", "1.0.0")],
        vec![subject],
        "wasm32",
        &["fs.read"],
    );
    let resolution = package_resolver::generate(&input, &ResolutionOptions::default())
        .expect("resolver allowlist admits the declared capability");
    let options = build_options("authority", &["authority.answer"], MAX_BYTES, MAX_BYTES);
    assert_eq!(generate_error(&resolution, &input, &options), "SPX-PB504");

    let report = report_from_source("native_only", &simple_source("native_only", 1));
    let subject = subject(&report, "native_only", "1.0.0", &[], &[]);
    let input = input(
        &[("native_only", "1.0.0")],
        vec![subject],
        "native64",
        &[],
    );
    let resolution = package_resolver::generate(&input, &ResolutionOptions::default())
        .expect("native target is valid Resolver v1 evidence");
    let options = build_options("native_only", &["native_only.answer"], MAX_BYTES, MAX_BYTES);
    assert_eq!(generate_error(&resolution, &input, &options), "SPX-PB504");
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

    let report = report_from_source("examples.records", include_str!("../../examples/records.spx"));
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
    let error = package_build::generate(
        &resolution,
        &input,
        &ResolutionOptions::default(),
        &options,
    )
    .expect_err("authored aggregates remain outside scalar package v1");
    assert_eq!(error[0].code, "SPX-PB504");
}
