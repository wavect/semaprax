use super::support::*;

#[test]
fn build_option_bounds_reject_before_artifact_return() {
    let fixture = fixture();
    for (artifact, evidence) in [
        (4095, MAX_BYTES),
        (MAX_BYTES + 1, MAX_BYTES),
        (MAX_BYTES, 4095),
        (MAX_BYTES, MAX_BYTES + 1),
    ] {
        let options = build_options(ROOT, &["calculator.add"], artifact, evidence);
        assert_eq!(
            generate_error(&fixture.resolution, &fixture.input, &options),
            "SPX-PB501"
        );
    }
}

#[test]
fn cumulative_artifact_limit_converges_to_exact_fixed_point_and_rejects_minus_one() {
    let fixture = fixture();
    let (options, build) = converge(&fixture, true);
    assert_eq!(artifact_bytes(&build), options.max_artifact_bytes);
    package_build::verify(
        &build,
        &fixture.resolution,
        &fixture.input,
        &ResolutionOptions::default(),
        &options,
    )
    .expect("exact cumulative artifact bound");

    let too_small = OfflinePackageBuildOptions {
        max_artifact_bytes: options.max_artifact_bytes - 1,
        ..options
    };
    assert_eq!(
        generate_error(&fixture.resolution, &fixture.input, &too_small),
        "SPX-PB505"
    );
}

#[test]
fn evidence_limit_converges_to_exact_fixed_point_and_cannot_widen_artifacts() {
    let (source, exports) = wide_source();
    let export_refs = exports.iter().map(String::as_str).collect::<Vec<_>>();
    let fixture = fixture_from_source("wide", "1.0.0", &source, &export_refs);
    let (options, build) = converge(&fixture, false);
    assert_eq!(build.evidence_json.len(), options.max_evidence_bytes);
    package_build::verify(
        &build,
        &fixture.resolution,
        &fixture.input,
        &ResolutionOptions::default(),
        &options,
    )
    .expect("exact evidence bound");

    let too_small = OfflinePackageBuildOptions {
        max_evidence_bytes: options.max_evidence_bytes - 1,
        ..options
    };
    assert_eq!(
        generate_error(&fixture.resolution, &fixture.input, &too_small),
        "SPX-PB505"
    );
}

fn converge(
    fixture: &Fixture,
    artifact: bool,
) -> (OfflinePackageBuildOptions, OfflinePackageBuild) {
    let mut options = OfflinePackageBuildOptions {
        root_package: fixture.options.root_package.clone(),
        exports: fixture.options.exports.clone(),
        max_artifact_bytes: MAX_BYTES,
        max_evidence_bytes: MAX_BYTES,
    };
    for _ in 0..32 {
        let build = package_build::generate(
            &fixture.resolution,
            &fixture.input,
            &ResolutionOptions::default(),
            &options,
        )
        .expect("fixed-point probe remains admitted");
        let used = if artifact {
            artifact_bytes(&build)
        } else {
            build.evidence_json.len()
        };
        let current = if artifact {
            options.max_artifact_bytes
        } else {
            options.max_evidence_bytes
        };
        if used == current {
            return (options, build);
        }
        if artifact {
            options.max_artifact_bytes = used;
        } else {
            options.max_evidence_bytes = used;
        }
    }
    panic!("canonical package byte accounting did not reach a fixed point")
}

fn wide_source() -> (String, Vec<String>) {
    let mut source = String::from("module wide;\n\n");
    let mut exports = Vec::new();
    for index in 0..32 {
        let stable_id = format!("wide.f{index:02}");
        source.push_str(&format!(
            "@id(\"{stable_id}\")\nfn f{index:02}(value: i64) -> i64\n{{\n    value + {index}\n}}\n\n"
        ));
        exports.push(stable_id);
    }
    source.push_str("@id(\"wide.main\")\nfn main() -> i64\n{\n    0\n}\n");
    (source, exports)
}
