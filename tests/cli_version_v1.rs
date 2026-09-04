use std::process::{Command, Output};

#[path = "../src/cli/version.rs"]
mod version;

const COMMIT: &str = "8b2d397f164ff93338d7b3935d1c2df291434458";

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn human_output_is_deterministic_with_known_and_absent_commits() {
    assert_eq!(
        version::render_human_with_commit(Some(COMMIT)).unwrap(),
        "semaprax 0.3.0 (8b2d397f164ff93338d7b3935d1c2df291434458)\n"
    );
    assert_eq!(
        version::render_human_with_commit(None).unwrap(),
        "semaprax 0.3.0 (commit unknown)\n"
    );
}

#[test]
fn json_output_is_canonical_and_has_one_terminal_lf() {
    assert_eq!(
        version::render_json_with_commit(Some(COMMIT)).unwrap(),
        "{\"schema\":\"semaprax.version.v1\",\"version\":\"0.3.0\",\"commit\":\"8b2d397f164ff93338d7b3935d1c2df291434458\",\"maturity\":\"pre-alpha\",\"rust_min\":\"1.88\"}\n"
    );
    assert_eq!(
        version::render_json_with_commit(None).unwrap(),
        "{\"schema\":\"semaprax.version.v1\",\"version\":\"0.3.0\",\"commit\":null,\"maturity\":\"pre-alpha\",\"rust_min\":\"1.88\"}\n"
    );
}

#[test]
fn malformed_injected_commit_facts_are_rejected() {
    for malformed in [
        "8b2d397",
        "8B2D397F164FF93338D7B3935D1C2DF291434458",
        "gb2d397f164ff93338d7b3935d1c2df291434458",
        "8b2d397f164ff93338d7b3935d1c2df2914344580",
    ] {
        assert_eq!(
            version::render_human_with_commit(Some(malformed)).unwrap_err(),
            "invalid SEMAPRAX_BUILD_COMMIT: expected exactly 40 lowercase hexadecimal characters"
        );
        assert!(version::render_json_with_commit(Some(malformed)).is_err());
    }
}

#[test]
fn version_flag_and_command_have_identical_human_output() {
    let flag = cli(&["--version"]);
    let command = cli(&["version"]);
    assert!(
        flag.status.success(),
        "{}",
        String::from_utf8_lossy(&flag.stderr)
    );
    assert!(
        command.status.success(),
        "{}",
        String::from_utf8_lossy(&command.stderr)
    );
    assert_eq!(flag.stdout, command.stdout);
    assert_eq!(
        String::from_utf8(flag.stdout).unwrap(),
        version::render_human_with_commit(option_env!("SEMAPRAX_BUILD_COMMIT")).unwrap()
    );
}

#[test]
fn version_json_cli_matches_the_canonical_renderer() {
    let output = cli(&["version", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        version::render_json_with_commit(option_env!("SEMAPRAX_BUILD_COMMIT")).unwrap()
    );
}

#[test]
fn unexpected_version_arguments_fail_without_stdout() {
    assert!(version::render(version::Invocation::Command, &["--verbose".to_owned()]).is_err());
    assert!(version::render(version::Invocation::Flag, &["--json".to_owned()]).is_err());
    for arguments in [
        &["version", "--verbose"][..],
        &["version", "--json", "extra"][..],
        &["--version", "--json"][..],
    ] {
        let output = cli(arguments);
        assert!(!output.status.success(), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(!output.stderr.is_empty(), "{arguments:?}");
    }
}
