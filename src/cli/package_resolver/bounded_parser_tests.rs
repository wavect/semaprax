use super::*;

fn is_usage<T>(result: Result<T, PackageResolverCliError>) -> bool {
    matches!(result, Err(PackageResolverCliError::Usage(_)))
}

#[test]
fn range_bounds_precede_digit_scanning() {
    assert!(validate_range("=4294967295.0.0").is_ok());
    assert!(is_usage(validate_range("=10000000000.0.0")));
    let hostile = format!("={}.0.0", "9".repeat(1024 * 1024));
    assert!(is_usage(validate_range(&hostile)));
    assert!(is_usage(parse_requirement(&"x".repeat(1024 * 1024))));
}

#[test]
fn scalar_and_group_bounds_reject_before_materializing_the_excess_item() {
    assert!(is_usage(canonical_number(
        "--max-bytes",
        &"9".repeat(1024 * 1024),
    )));

    let huge = "x".repeat(1024 * 1024);
    let mut subjects = vec!["subject".to_owned(); MAX_SUBJECTS];
    subjects.push(huge.clone());
    assert!(is_usage(parse(&subjects)));

    let mut requirements = vec!["subject".to_owned()];
    for index in 0..MAX_REQUIREMENTS {
        requirements.extend(["--require".to_owned(), format!("p{index}:=1.0.0")]);
    }
    requirements.extend(["--require".to_owned(), huge.clone()]);
    assert!(is_usage(parse(&requirements)));

    let mut capabilities = vec![
        "subject".to_owned(),
        "--require".to_owned(),
        "p:=1.0.0".to_owned(),
        "--target".to_owned(),
        "native64".to_owned(),
    ];
    for index in 0..MAX_CAPABILITIES {
        capabilities.extend([
            "--allow-capability".to_owned(),
            format!("capability{index:03}"),
        ]);
    }
    capabilities.extend(["--allow-capability".to_owned(), huge]);
    assert!(is_usage(parse(&capabilities)));
}
