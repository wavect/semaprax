use super::*;

#[test]
fn internal_strings_profile_is_explicit_and_source_only() {
    let options = parse(&strings(&[
        "app.spx",
        "--target",
        "web",
        "--profile",
        "internal-strings-v1",
        "--export=--profile",
    ]))
    .unwrap();
    assert_eq!(options.profile.as_deref(), Some("internal-strings-v1"));
    assert_eq!(options.exports, strings(&["--profile"]));
    for arguments in [
        strings(&[
            "app.spx",
            "--target",
            "web",
            "--profile",
            "internal-strings-v1",
        ]),
        strings(&[
            "app.spx",
            "--profile",
            "internal-strings-v1",
            "--export",
            "main",
        ]),
        strings(&[
            "--target",
            "web",
            "--profile",
            "internal-strings-v1",
            "--export",
            "main",
        ]),
        strings(&[
            "app.spx",
            "--target",
            "web",
            "--profile",
            "unknown",
            "--export",
            "main",
        ]),
        strings(&[
            "app.spx",
            "--target",
            "web",
            "--profile",
            "internal-strings-v1",
            "--profile",
            "internal-strings-v1",
            "--export",
            "main",
        ]),
        strings(&[
            "app.spx",
            "--target",
            "web",
            "--export",
            "--profile",
            "internal-strings-v1",
        ]),
    ] {
        assert!(parse(&arguments).is_err());
    }
    assert!(parse(&strings(&["app.spx", "--target", "web"]))
        .unwrap()
        .profile
        .is_none());
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn repeated_scalar_exports_preserve_caller_order() {
    let options = parse(&strings(&[
        "calculator.spx",
        "--target",
        "web",
        "--export",
        "calculator.subtract",
        "--export",
        "calculator.add",
        "-o",
        "site",
    ]))
    .unwrap();
    assert_eq!(
        options.input,
        BuildInput::Source(PathBuf::from("calculator.spx"))
    );
    assert_eq!(options.output, Some(PathBuf::from("site")));
    assert_eq!(
        options.exports,
        strings(&["calculator.subtract", "calculator.add"])
    );

    let hyphenated = parse(&strings(&[
        "calculator.spx",
        "--target",
        "web",
        "--export",
        "-x",
        "--export=--target",
    ]))
    .unwrap();
    assert_eq!(hyphenated.exports, strings(&["-x", "--target"]));
}

#[test]
fn rejects_unknown_repeated_and_cross_target_flags() {
    assert!(parse(&strings(&["app.spx", "--unknown", "x"])).is_err());
    assert!(parse(&strings(&[
        "app.spx", "--target", "web", "--target", "wasm",
    ]))
    .is_err());
    assert!(parse(&strings(&[
        "app.spx", "--target", "native", "--export", "app.main",
    ]))
    .is_err());
}

#[test]
fn project_selectors_do_not_confuse_legacy_sources() {
    let implicit = parse(&[]).unwrap();
    assert_eq!(
        implicit.input,
        BuildInput::Project(PathBuf::from(DEFAULT_MANIFEST))
    );
    assert_eq!(implicit.target, "web");
    assert_eq!(implicit.output, None);

    let explicit = parse(&strings(&[
        "--manifest-path",
        "fixtures/semaprax.toml",
        "--target",
        "web",
        "-o",
        "site",
    ]))
    .unwrap();
    assert_eq!(
        explicit.input,
        BuildInput::Project(PathBuf::from("fixtures/semaprax.toml"))
    );
    assert_eq!(explicit.output, Some(PathBuf::from("site")));
    assert!(parse(&strings(&["app.spx", "--manifest-path", DEFAULT_MANIFEST,])).is_err());
    assert!(parse(&strings(&[
        DEFAULT_MANIFEST,
        "--target",
        "web",
        "--export",
        "app.main",
    ]))
    .is_err());

    let npm = parse(&strings(&[
        "--manifest-path",
        "fixtures/semaprax.toml",
        "--target",
        "npm",
        "-o",
        "package",
    ]))
    .unwrap();
    assert_eq!(npm.target, "npm");
    assert_eq!(npm.output, Some(PathBuf::from("package")));
    assert!(matches!(npm.input, BuildInput::Project(_)));
    assert!(parse(&strings(&["app.spx", "--target", "npm"])).is_err());
    assert!(parse(&strings(&[
        "--manifest-path",
        DEFAULT_MANIFEST,
        "--target",
        "npm",
        "--export",
        "app.main",
    ]))
    .is_err());

    let rust = parse(&strings(&[
        "--manifest-path",
        "fixtures/semaprax.toml",
        "--target",
        "rust",
        "-o",
        "sdk",
    ]))
    .unwrap();
    assert_eq!(rust.target, "rust");
    assert_eq!(rust.output, Some(PathBuf::from("sdk")));
    assert!(matches!(rust.input, BuildInput::Project(_)));
    assert!(parse(&strings(&["app.spx", "--target", "rust"])).is_err());
    assert!(parse(&strings(&[
        "--manifest-path",
        DEFAULT_MANIFEST,
        "--target",
        "rust",
        "--export",
        "app.main",
    ]))
    .is_err());
    let normalized = absolute_rust_output(Path::new("dist/rust")).unwrap();
    assert!(normalized.is_absolute());
    assert!(normalized.ends_with(Path::new("dist/rust")));
    assert!(absolute_rust_output(Path::new("dist/../rust")).is_err());
}

#[test]
fn rust_output_binds_the_existing_parent_canonical_spelling() {
    let current = std::env::current_dir().unwrap();
    let output = current.join("semaprax-rust-output-binding-test");
    let bound = bind_rust_output_parent(&output).unwrap();
    #[cfg(windows)]
    assert_eq!(
        bound,
        current
            .canonicalize()
            .unwrap()
            .join("semaprax-rust-output-binding-test")
    );
    #[cfg(not(windows))]
    assert_eq!(bound, output);
}
