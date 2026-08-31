use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const DOCTOR_LINE: &str = "semaprax doctor [--profile <id>] [--target native|web|all] [--json]\n";
const NEW_LINE: &str = "semaprax new <destination> [--name project-name] [--template calculator]\n";
const BUILD_LINE: &str = "semaprax build [<file>|semaprax.toml|--manifest-path path] [--target native|native-callable|web|wasm|npm|rust] [--profile internal-strings-v1] [--function stable-id] [--export stable-id ...] [-o path]\n";

fn empty_working_directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-cli-help-full-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

fn invoke(argument: Option<&str>) -> (Output, PathBuf) {
    let working_directory = empty_working_directory();
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax-full"));
    command.current_dir(&working_directory);
    if let Some(argument) = argument {
        command.arg(argument);
    }
    let output = command.output().unwrap();
    assert_eq!(std::fs::read_dir(&working_directory).unwrap().count(), 0);
    (output, working_directory)
}

#[test]
fn full_help_is_exact_capability_aware_and_inert() {
    let (empty, empty_dir) = invoke(None);
    assert_eq!(empty.status.code(), Some(2));
    assert!(empty.stderr.is_empty());

    for alias in ["help", "--help", "-h"] {
        let (output, working_directory) = invoke(Some(alias));
        assert!(output.status.success(), "{alias}");
        assert!(output.stderr.is_empty(), "{alias}");
        assert_eq!(output.stdout, empty.stdout, "{alias}");
        std::fs::remove_dir(working_directory).unwrap();
    }

    let help = String::from_utf8(empty.stdout.clone()).unwrap();
    assert_eq!(help.matches(DOCTOR_LINE).count(), 1);
    assert_eq!(help.matches(NEW_LINE).count(), 1);
    assert_eq!(help.matches(BUILD_LINE).count(), 1);
    let doctor = help.find(DOCTOR_LINE).unwrap();
    let new = help.find(NEW_LINE).unwrap();
    let build = help.find(BUILD_LINE).unwrap();
    assert!(doctor < new && new < build);

    let (unknown, unknown_dir) = invoke(Some("not-a-command"));
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(unknown.stdout, empty.stdout);
    assert_eq!(unknown.stderr, b"unknown command `not-a-command`\n\n");

    std::fs::remove_dir(unknown_dir).unwrap();
    std::fs::remove_dir(empty_dir).unwrap();
}
