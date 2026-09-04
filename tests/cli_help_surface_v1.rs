use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BUILD_LINE: &str = "semaprax build [<file>|<dir>|semaprax.toml|--manifest-path path] [--target native|native-callable|web|wasm|npm] [--profile internal-strings-v1] [--function stable-id] [--export stable-id ...] [-o path]\n";
const DOCTOR_LINE: &str = "semaprax doctor [--profile <id>] [--target native|web|all] [--json]\n";
const NEW_LINE: &str = "semaprax new <destination> [--name project-name] [--template calculator]\n";
const PROJECT_SCAFFOLD_LINE: &str =
    "semaprax project-scaffold --name project-name [--template calculator]\n";
const BANNER: &str = "SEMAPRAX — Meaning in. Verified machine code out.\n";
/// The guided overview must stay one screen; CLI Help v4 fixes the bound.
const GUIDE_MAX_BYTES: usize = 2048;
const GUIDE_HEADINGS: &[&str] = &[
    "Write, check, and run",
    "Inspect meaning",
    "Change by meaning",
    "Start a project",
    "Toolchain",
];

/// The command named by every indented entry of the guided overview.
fn guide_commands(guide: &str) -> Vec<&str> {
    guide
        .lines()
        .filter_map(|line| line.strip_prefix("  "))
        .map(|entry| entry.split_whitespace().next().unwrap())
        .collect()
}

fn empty_working_directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-cli-help-standalone-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

fn invoke(arguments: &[&str]) -> (Output, PathBuf) {
    let working_directory = empty_working_directory();
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
    command.current_dir(&working_directory);
    for argument in arguments {
        command.arg(argument);
    }
    let output = command.output().unwrap();
    assert_eq!(std::fs::read_dir(&working_directory).unwrap().count(), 0);
    (output, working_directory)
}

#[test]
fn standalone_help_is_exact_capability_aware_and_inert() {
    let (empty, empty_dir) = invoke(&[]);
    assert_eq!(empty.status.code(), Some(2));
    assert!(empty.stderr.is_empty());

    for alias in ["help", "--help", "-h"] {
        let (output, working_directory) = invoke(&[alias]);
        assert!(output.status.success(), "{alias}");
        assert!(output.stderr.is_empty(), "{alias}");
        assert_eq!(output.stdout, empty.stdout, "{alias}");
        std::fs::remove_dir(working_directory).unwrap();
    }

    // The global form is the guided one-screen overview owned by CLI Help v4.
    let guide = String::from_utf8(empty.stdout.clone()).unwrap();
    assert!(guide.starts_with(BANNER));
    assert!(guide.len() <= GUIDE_MAX_BYTES, "{} bytes", guide.len());
    assert!(guide.contains("\nUsage: semaprax <command> [arguments]\n"));
    for heading in GUIDE_HEADINGS {
        assert_eq!(
            guide.matches(&format!("\n{heading}:\n")).count(),
            1,
            "{heading}"
        );
    }
    assert_eq!(guide.matches("\n  help all ").count(), 1);
    assert_eq!(guide.matches("\n  help language ").count(), 1);
    assert_eq!(guide.matches("\n  project-scaffold ").count(), 1);
    assert_eq!(guide.matches("\n  new ").count(), 0);
    assert_eq!(guide.matches("\n  doctor ").count(), 0);
    assert_eq!(guide.matches("|rust").count(), 0);
    assert_eq!(guide.matches("\nsemaprax ").count(), 0);
    for name in guide_commands(&guide) {
        let (output, directory) = invoke(&["help", name]);
        assert!(
            output.status.success(),
            "guided entry `{name}` must have scoped help"
        );
        assert!(output.stderr.is_empty(), "{name}");
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("Usage:\n  semaprax "));
        std::fs::remove_dir(directory).unwrap();
    }

    // `help all` is the exhaustive catalog: the exact global bytes of v1..v3.
    let (all, all_dir) = invoke(&["help", "all"]);
    assert!(all.status.success());
    assert!(all.stderr.is_empty());
    let help = String::from_utf8(all.stdout.clone()).unwrap();
    assert!(help.starts_with(&format!("{BANNER}\nUsage:\nsemaprax check ")));
    assert_eq!(help.matches(BUILD_LINE).count(), 1);
    assert_eq!(help.matches(DOCTOR_LINE).count(), 0);
    assert_eq!(help.matches(NEW_LINE).count(), 0);
    assert_eq!(help.matches(PROJECT_SCAFFOLD_LINE).count(), 1);
    assert!(help.find(PROJECT_SCAFFOLD_LINE).unwrap() < help.find(BUILD_LINE).unwrap());
    assert_eq!(
        help.matches("native|native-callable|web|wasm|npm|rust")
            .count(),
        0
    );
    assert!(help.len() > guide.len());
    std::fs::remove_dir(all_dir).unwrap();

    let (unknown, unknown_dir) = invoke(&["not-a-command"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(unknown.stdout, empty.stdout);
    assert_eq!(unknown.stderr, b"unknown command `not-a-command`\n\n");

    let (typo, typo_dir) = invoke(&["chek"]);
    assert_eq!(typo.status.code(), Some(2));
    assert_eq!(typo.stdout, empty.stdout);
    assert_eq!(
        typo.stderr,
        b"unknown command `chek`; did you mean `check`?\n\n"
    );

    let (hidden_typo, hidden_typo_dir) = invoke(&["doctro"]);
    assert_eq!(hidden_typo.status.code(), Some(2));
    assert_eq!(hidden_typo.stdout, empty.stdout);
    assert_eq!(hidden_typo.stderr, b"unknown command `doctro`\n\n");

    let (malformed_known, malformed_known_dir) = invoke(&["check", "--unknown"]);
    assert_eq!(malformed_known.status.code(), Some(2));
    assert!(malformed_known.stdout.is_empty());
    assert_eq!(
        malformed_known.stderr,
        b"unknown check option `--unknown`\nhint: run `semaprax check --help` for usage\n"
    );

    let (hidden_known, hidden_known_dir) = invoke(&["doctor", "--unknown"]);
    assert_eq!(hidden_known.status.code(), Some(2));
    assert!(hidden_known.stdout.is_empty());
    assert_eq!(hidden_known.stderr, b"doctor is unavailable in the standalone crates.io package; use the unpublished semaprax-full toolchain CLI\n");

    std::fs::remove_dir(hidden_known_dir).unwrap();
    std::fs::remove_dir(malformed_known_dir).unwrap();
    std::fs::remove_dir(hidden_typo_dir).unwrap();
    std::fs::remove_dir(typo_dir).unwrap();
    std::fs::remove_dir(unknown_dir).unwrap();
    std::fs::remove_dir(empty_dir).unwrap();
}

#[test]
fn standalone_scoped_help_is_exhaustive_exact_capability_aware_and_inert() {
    let (global, global_dir) = invoke(&["--help"]);
    let (catalog, catalog_dir) = invoke(&["help", "all"]);
    let global_text = String::from_utf8(catalog.stdout.clone()).unwrap();
    std::fs::remove_dir(catalog_dir).unwrap();
    let usages: Vec<_> = global_text
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("semaprax "))
        .collect();
    assert!(!usages.is_empty());
    for usage in usages {
        let command = usage.split_whitespace().next().unwrap();
        let expected: String = global_text
            .lines()
            .filter_map(|line| {
                let line = line.trim_start();
                let prefix = format!("semaprax {command}");
                (line == prefix
                    || line
                        .strip_prefix(&prefix)
                        .is_some_and(|tail| tail.starts_with(' ')))
                .then(|| format!("  {line}\n"))
            })
            .collect();
        let expected = format!("Usage:\n{expected}");
        for arguments in [
            vec!["help", command],
            vec![command, "--help"],
            vec![command, "-h"],
        ] {
            let (output, directory) = invoke(&arguments);
            assert!(output.status.success(), "{arguments:?}");
            assert!(output.stderr.is_empty(), "{arguments:?}");
            assert_eq!(output.stdout, expected.as_bytes(), "{arguments:?}");
            std::fs::remove_dir(directory).unwrap();
        }
    }

    for private in ["doctor", "new"] {
        let (output, directory) = invoke(&["help", private]);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(output.stdout, global.stdout);
        assert_eq!(
            output.stderr,
            format!("unknown command `{private}`\n\n").as_bytes()
        );
        std::fs::remove_dir(directory).unwrap();
    }
    let (language, language_dir) = invoke(&["help", "language"]);
    assert!(language.status.success());
    assert!(language.stderr.is_empty());
    let card = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/AGENT-QUICK-REFERENCE.md"),
    )
    .unwrap();
    assert_eq!(language.stdout, card);
    assert!(language.stdout.starts_with(b"# Agent quick reference\n"));
    std::fs::remove_dir(language_dir).unwrap();
    let (language_extra, language_extra_dir) = invoke(&["help", "language", "extra"]);
    assert_eq!(language_extra.status.code(), Some(2));
    assert_eq!(language_extra.stdout, global.stdout);
    assert_eq!(language_extra.stderr, b"unknown command `help`\n\n");
    std::fs::remove_dir(language_extra_dir).unwrap();
    let (version_alias, version_alias_dir) = invoke(&["-V", "--help"]);
    assert!(version_alias.status.success());
    assert!(version_alias.stderr.is_empty());
    assert_eq!(version_alias.stdout, b"Usage:\n  semaprax --version\n");
    std::fs::remove_dir(version_alias_dir).unwrap();
    for name in ["help", "--help", "-h"] {
        let (output, directory) = invoke(&["help", name]);
        assert!(output.status.success(), "{name}");
        assert_eq!(
            output.stdout,
            b"Usage:\n  semaprax help <command>\n  semaprax help all\n  semaprax help language\n"
        );
        assert!(output.stderr.is_empty());
        std::fs::remove_dir(directory).unwrap();
    }
    let (all_extra, all_extra_dir) = invoke(&["help", "all", "extra"]);
    assert_eq!(all_extra.status.code(), Some(2));
    assert_eq!(all_extra.stdout, global.stdout);
    assert_eq!(all_extra.stderr, b"unknown command `help`\n\n");
    std::fs::remove_dir(all_extra_dir).unwrap();
    let (typo, typo_dir) = invoke(&["help", "buidl"]);
    assert_eq!(typo.status.code(), Some(2));
    assert_eq!(typo.stdout, global.stdout);
    assert_eq!(
        typo.stderr,
        b"unknown command `buidl`; did you mean `build`?\n\n"
    );
    std::fs::remove_dir(typo_dir).unwrap();
    let (malformed, malformed_dir) = invoke(&["help", "build", "extra"]);
    assert_eq!(malformed.status.code(), Some(2));
    assert_eq!(malformed.stdout, global.stdout);
    assert_eq!(malformed.stderr, b"unknown command `help`\n\n");
    let (embedded, embedded_dir) = invoke(&["fmt", "effectful.spx", "--help"]);
    assert_eq!(embedded.status.code(), Some(2));
    assert!(embedded.stdout.is_empty());
    assert_eq!(
        embedded.stderr,
        b"help flags are admitted only as the sole operand of a command\n"
    );
    assert!(!embedded_dir.join("effectful.spx").exists());
    let (embedded_short, embedded_short_dir) = invoke(&["fmt", "effectful.spx", "-h"]);
    assert_eq!(embedded_short.status.code(), Some(2));
    assert!(embedded_short.stdout.is_empty());
    assert_eq!(embedded_short.stderr, embedded.stderr);
    assert!(!embedded_short_dir.join("effectful.spx").exists());
    std::fs::remove_dir(embedded_short_dir).unwrap();
    std::fs::remove_dir(embedded_dir).unwrap();
    std::fs::remove_dir(malformed_dir).unwrap();
    std::fs::remove_dir(global_dir).unwrap();
}
