use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SHAPES_CATALOG_PATH: &str = "docs/LANGUAGE-SHAPES-CATALOG.md";
const BUILD_SOURCE_LINE: &str = "semaprax build <file> [--target native|native-callable|web|wasm] [--profile internal-strings-v1] [--function stable-id] [--export stable-id ...] [-o|--output path] [--json]\n";
const BUILD_PROJECT_LINE: &str = "semaprax build [<dir>|semaprax.toml|--manifest-path path] [--target native|web|wasm|npm] [-o|--output path] [--json]\n";
const DOCTOR_LINE: &str = "semaprax doctor [--profile <id>] [--target native|web|all] [--json]\n";
const NEW_LINE: &str =
    "semaprax new <destination> [--name project-name] [--template calculator|library]\n";
const PROJECT_SCAFFOLD_LINE: &str =
    "semaprax project-scaffold --name project-name [--template calculator|library] [--layout frozen|tables]\n";
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
const LANGUAGE_TOPICS: &str = concat!(
    "Language topics:\n",
    "  workflow        Spend tokens on source, not on dumps\n",
    "  module          A complete file\n",
    "  scalars         Scalars and literals\n",
    "  control-flow    Control flow, mutation, contracts, effects\n",
    "  records         Records, variants, classes\n",
    "  ownership       Ownership and resources\n",
    "  strings         Strings and bytes\n",
    "  builtins        Compiler-owned functions\n",
    "  mistakes-code   Habits from other languages: diagnostic examples\n",
    "  mistakes-index  Habits from other languages: diagnostic index\n",
    "  projects        Projects\n",
    "  specifications  Where the rules live\n",
);
const DIAGNOSTIC_CODES: &str = concat!(
    "Diagnostic codes:\n",
    "  SPX-O101\n",
    "  SPX-P104\n",
    "  SPX-P105\n",
    "  SPX-P106\n",
    "  SPX-P201\n",
    "  SPX-P203\n",
    "  SPX-T001\n",
    "  SPX-T104\n",
    "  SPX-T202\n",
    "  SPX-T203\n",
    "  SPX-T205\n",
    "  SPX-T208\n",
    "  SPX-T209\n",
    "  SPX-T221\n",
    "  SPX-T225\n",
    "  SPX-T232\n",
    "  SPX-T250\n",
    "  SPX-T263\n",
    "  SPX-T266\n",
    "  SPX-U101\n",
);
const DIAGNOSTIC_T208: &str = concat!(
    "SPX-T208\n",
    "wrote: `index + 1` when `index: usize`\n",
    "fix: Integer literals default to `i64`; write `index + 1usize`\n",
);

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
    assert_eq!(guide.matches("\n  help shapes ").count(), 1);
    assert!(guide.contains("semaprax help diagnostic <code>`\n"));
    assert_eq!(guide.matches("\n  project-scaffold ").count(), 1);
    assert_eq!(guide.matches("\n  new ").count(), 1);
    assert_eq!(guide.matches("\n  doctor ").count(), 1);
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
    assert_eq!(help.matches(BUILD_SOURCE_LINE).count(), 1);
    assert_eq!(help.matches(BUILD_PROJECT_LINE).count(), 1);
    assert_eq!(help.matches(DOCTOR_LINE).count(), 1);
    assert_eq!(help.matches(NEW_LINE).count(), 1);
    assert_eq!(help.matches(PROJECT_SCAFFOLD_LINE).count(), 1);
    assert!(help.find(NEW_LINE).unwrap() < help.find(PROJECT_SCAFFOLD_LINE).unwrap());
    assert!(help.find(PROJECT_SCAFFOLD_LINE).unwrap() < help.find(BUILD_SOURCE_LINE).unwrap());
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

    // `doctor` is catalogued by both binaries, so its typo is suggested too.
    let (hidden_typo, hidden_typo_dir) = invoke(&["doctro"]);
    assert_eq!(hidden_typo.status.code(), Some(2));
    assert_eq!(hidden_typo.stdout, empty.stdout);
    assert_eq!(
        hidden_typo.stderr,
        b"unknown command `doctro`; did you mean `doctor`?\n\n"
    );

    let (malformed_known, malformed_known_dir) = invoke(&["check", "--unknown"]);
    assert_eq!(malformed_known.status.code(), Some(2));
    assert!(malformed_known.stdout.is_empty());
    assert_eq!(
        malformed_known.stderr,
        b"unknown check option `--unknown`\nhint: run `semaprax check --help` for usage\n"
    );

    // `doctor` is admitted by both binaries; its option grammar is closed.
    let (hidden_known, hidden_known_dir) = invoke(&["doctor", "--unknown"]);
    assert_eq!(hidden_known.status.code(), Some(2));
    assert!(hidden_known.stdout.is_empty());
    assert_eq!(
        hidden_known.stderr,
        b"doctor: unknown doctor option `--unknown`\nhint: run `semaprax doctor --help` for usage\n"
    );

    let (help_extra, help_extra_dir) = invoke(&["help", "all", "extra"]);
    assert_eq!(help_extra.status.code(), Some(2));
    assert!(help_extra.stdout.is_empty());
    assert_eq!(
        help_extra.stderr,
        b"help accepts exactly one operand; unexpected extra operand `extra`\n"
    );

    std::fs::remove_dir(help_extra_dir).unwrap();
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

    // No command is hidden by a capability boundary any more: `doctor` has
    // scoped help in the standalone binary too.
    let (hidden, hidden_dir) = invoke(&["help", "doctor"]);
    assert!(hidden.status.success());
    assert!(hidden.stderr.is_empty());
    assert_eq!(hidden.stdout, format!("Usage:\n  {DOCTOR_LINE}").as_bytes());
    std::fs::remove_dir(hidden_dir).unwrap();
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
    let (topics, topics_dir) = invoke(&["help", "language", "topics"]);
    assert!(topics.status.success());
    assert!(topics.stderr.is_empty());
    assert_eq!(topics.stdout, LANGUAGE_TOPICS.as_bytes());
    assert!(topics.stdout.len() <= 768);
    std::fs::remove_dir(topics_dir).unwrap();
    let (scalars, scalars_dir) = invoke(&["help", "language", "scalars"]);
    assert!(scalars.status.success());
    assert!(scalars.stderr.is_empty());
    assert!(scalars.stdout.starts_with(b"## Scalars and literals\n"));
    assert!(scalars
        .stdout
        .windows(b"- `u8`:".len())
        .any(|window| window == b"- `u8`:"));
    assert!(!scalars
        .stdout
        .windows(b"## Control flow".len())
        .any(|window| window == b"## Control flow"));
    assert!(scalars.stdout.len() <= 1_024);
    assert!(scalars.stdout.len() * 20 < card.len());
    let scalar_units =
        semaprax::agent_economics::lexical_tokens(std::str::from_utf8(&scalars.stdout).unwrap());
    let card_units = semaprax::agent_economics::lexical_tokens(std::str::from_utf8(&card).unwrap());
    assert!(scalar_units <= 300);
    assert!(scalar_units * 20 < card_units);
    std::fs::remove_dir(scalars_dir).unwrap();
    let (diagnostic_codes, diagnostic_codes_dir) = invoke(&["help", "diagnostic", "codes"]);
    assert!(diagnostic_codes.status.success());
    assert!(diagnostic_codes.stderr.is_empty());
    assert_eq!(diagnostic_codes.stdout, DIAGNOSTIC_CODES.as_bytes());
    assert!(diagnostic_codes.stdout.len() <= 256);
    std::fs::remove_dir(diagnostic_codes_dir).unwrap();
    let (diagnostic, diagnostic_dir) = invoke(&["help", "diagnostic", "SPX-T208"]);
    assert!(diagnostic.status.success());
    assert!(diagnostic.stderr.is_empty());
    assert_eq!(diagnostic.stdout, DIAGNOSTIC_T208.as_bytes());
    assert!(diagnostic.stdout.len() <= 256);
    let mistakes = invoke(&["help", "language", "mistakes-index"]);
    assert!(diagnostic.stdout.len() * 20 < mistakes.0.stdout.len());
    assert!(
        semaprax::agent_economics::lexical_tokens(DIAGNOSTIC_T208) * 20
            < semaprax::agent_economics::lexical_tokens(
                std::str::from_utf8(&mistakes.0.stdout).unwrap()
            )
    );
    std::fs::remove_dir(mistakes.1).unwrap();
    std::fs::remove_dir(diagnostic_dir).unwrap();
    let (p106, p106_dir) = invoke(&["help", "diagnostic", "SPX-P106"]);
    assert!(p106.status.success());
    assert!(p106.stderr.is_empty());
    assert_eq!(
        p106.stdout
            .windows(b"\nwrote: ".len())
            .filter(|window| *window == b"\nwrote: ")
            .count(),
        6
    );
    assert!(p106.stdout.len() <= 1_024);
    std::fs::remove_dir(p106_dir).unwrap();
    let (missing_diagnostic, missing_diagnostic_dir) = invoke(&["help", "diagnostic", "spx-t208"]);
    assert_eq!(missing_diagnostic.status.code(), Some(2));
    assert!(missing_diagnostic.stdout.is_empty());
    assert_eq!(
        missing_diagnostic.stderr,
        b"diagnostic help has no exact match for `spx-t208`\n"
    );
    std::fs::remove_dir(missing_diagnostic_dir).unwrap();
    let (diagnostic_extra, diagnostic_extra_dir) =
        invoke(&["help", "diagnostic", "SPX-T208", "extra"]);
    assert_eq!(diagnostic_extra.status.code(), Some(2));
    assert!(diagnostic_extra.stdout.is_empty());
    assert_eq!(
        diagnostic_extra.stderr,
        b"help accepts exactly one operand; unexpected extra operand `extra`\n"
    );
    std::fs::remove_dir(diagnostic_extra_dir).unwrap();
    let (library, library_dir) = invoke(&["help", "library"]);
    assert!(library.status.success());
    assert!(library.stderr.is_empty());
    let catalog = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/STANDARD-LIBRARY-CATALOG.md"),
    )
    .unwrap();
    assert_eq!(library.stdout, catalog);
    assert!(library.stdout.starts_with(b"# Standard library catalog\n"));
    std::fs::remove_dir(library_dir).unwrap();
    let (shapes, shapes_dir) = invoke(&["help", "shapes"]);
    assert!(shapes.status.success());
    assert!(shapes.stderr.is_empty());
    let shapes_catalog =
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SHAPES_CATALOG_PATH))
            .unwrap();
    assert_eq!(shapes.stdout, shapes_catalog);
    assert!(shapes.stdout.starts_with(b"# Language shapes catalog\n"));
    std::fs::remove_dir(shapes_dir).unwrap();
    let expected_add = b"function calculator.add\nsource examples/calculator.spx\n@id(\"calculator.add\")\nfn add(left: i64, right: i64) -> i64\n";
    let (shape, shape_dir) = invoke(&["help", "shapes", "calculator.add"]);
    assert!(shape.status.success());
    assert!(shape.stderr.is_empty());
    assert_eq!(shape.stdout, expected_add);
    assert!(shape.stdout.len() <= 512);
    assert!(shape.stdout.len() * 40 < shapes_catalog.len());
    let shape_units =
        semaprax::agent_economics::lexical_tokens(std::str::from_utf8(&shape.stdout).unwrap());
    let shapes_catalog_units =
        semaprax::agent_economics::lexical_tokens(std::str::from_utf8(&shapes_catalog).unwrap());
    assert!(shape_units <= 128);
    assert!(shape_units * 40 < shapes_catalog_units);
    std::fs::remove_dir(shape_dir).unwrap();
    let (representative, representative_dir) = invoke(&["help", "shapes", "record"]);
    assert!(representative.status.success());
    assert!(representative.stderr.is_empty());
    assert!(representative
        .stdout
        .starts_with(b"representative record\nsource "));
    assert!(representative.stdout.len() <= 512);
    assert!(representative.stdout.len() * 40 < shapes_catalog.len());
    assert!(
        semaprax::agent_economics::lexical_tokens(
            std::str::from_utf8(&representative.stdout).unwrap()
        ) <= 128
    );
    std::fs::remove_dir(representative_dir).unwrap();
    let (disambiguated, disambiguated_dir) =
        invoke(&["help", "shapes", "examples/calculator.spx#app.main"]);
    assert!(disambiguated.status.success());
    assert!(disambiguated.stderr.is_empty());
    assert!(disambiguated
        .stdout
        .starts_with(b"function app.main\nsource examples/calculator.spx\n"));
    std::fs::remove_dir(disambiguated_dir).unwrap();
    let (missing_shape, missing_shape_dir) = invoke(&["help", "shapes", "not_a_shape"]);
    assert_eq!(missing_shape.status.code(), Some(2));
    assert!(missing_shape.stdout.is_empty());
    assert_eq!(
        missing_shape.stderr,
        b"language shapes catalog has no exact match for `not_a_shape`\n"
    );
    std::fs::remove_dir(missing_shape_dir).unwrap();
    let (shape_extra, shape_extra_dir) = invoke(&["help", "shapes", "record", "extra"]);
    assert_eq!(shape_extra.status.code(), Some(2));
    assert!(shape_extra.stdout.is_empty());
    assert_eq!(
        shape_extra.stderr,
        b"help accepts exactly one operand; unexpected extra operand `extra`\n"
    );
    std::fs::remove_dir(shape_extra_dir).unwrap();
    let expected_compare = b"std.core.compare\ndependency std.core = \"^0.1.0\"\nprofile scalar\nfn compare(left: i64, right: i64) -> i64\n    ensures result >= -1 && result <= 1\n    ensures result != 0 || left == right\n    ensures result == 0 || left != right\n";
    for selector in ["std.core.compare", "compare"] {
        let (entry, directory) = invoke(&["help", "library", selector]);
        assert!(entry.status.success());
        assert!(entry.stderr.is_empty());
        assert_eq!(entry.stdout, expected_compare);
        assert!(entry.stdout.len() <= 512);
        assert!(entry.stdout.len() * 50 < catalog.len());
        let entry_units =
            semaprax::agent_economics::lexical_tokens(std::str::from_utf8(&entry.stdout).unwrap());
        let catalog_units =
            semaprax::agent_economics::lexical_tokens(std::str::from_utf8(&catalog).unwrap());
        assert!(entry_units <= 128);
        assert!(entry_units * 50 < catalog_units);
        std::fs::remove_dir(directory).unwrap();
    }
    let (module, module_dir) = invoke(&["help", "library", "std.core"]);
    assert!(module.status.success());
    assert!(module.stderr.is_empty());
    let module = String::from_utf8(module.stdout).unwrap();
    assert!(module.starts_with("std.core.ordering.less\n"));
    assert!(module.contains("\nstd.core.compare\n"));
    assert!(!module.contains("std.bytes."));
    std::fs::remove_dir(module_dir).unwrap();
    let (missing_library, missing_library_dir) =
        invoke(&["help", "library", "not_a_library_function"]);
    assert_eq!(missing_library.status.code(), Some(2));
    assert!(missing_library.stdout.is_empty());
    assert_eq!(
        missing_library.stderr,
        b"standard library has no exact match for `not_a_library_function`\n"
    );
    std::fs::remove_dir(missing_library_dir).unwrap();
    let (missing_topic, missing_topic_dir) = invoke(&["help", "language", "not-a-topic"]);
    assert_eq!(missing_topic.status.code(), Some(2));
    assert!(missing_topic.stdout.is_empty());
    assert_eq!(
        missing_topic.stderr,
        b"language card has no exact topic `not-a-topic`\n"
    );
    std::fs::remove_dir(missing_topic_dir).unwrap();
    let (language_extra, language_extra_dir) = invoke(&["help", "language", "scalars", "extra"]);
    assert_eq!(language_extra.status.code(), Some(2));
    assert!(language_extra.stdout.is_empty());
    assert_eq!(
        language_extra.stderr,
        b"help accepts exactly one operand; unexpected extra operand `extra`\n"
    );
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
            concat!(
                "Usage:\n",
                "  semaprax help <command>\n",
                "  semaprax help all\n",
                "  semaprax help diagnostic <SPX-code|codes>\n",
                "  semaprax help language\n",
                "  semaprax help language <topic|topics>\n",
                "  semaprax help library\n",
                "  semaprax help library <module|name|stable-id>\n",
                "  semaprax help shapes\n",
                "  semaprax help shapes <kind|stable-id|path#stable-id>\n"
            )
            .as_bytes()
        );
        assert!(output.stderr.is_empty());
        std::fs::remove_dir(directory).unwrap();
    }
    let (all_extra, all_extra_dir) = invoke(&["help", "all", "extra"]);
    assert_eq!(all_extra.status.code(), Some(2));
    assert!(all_extra.stdout.is_empty());
    assert_eq!(
        all_extra.stderr,
        b"help accepts exactly one operand; unexpected extra operand `extra`\n"
    );
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
    assert!(malformed.stdout.is_empty());
    assert_eq!(
        malformed.stderr,
        b"help accepts exactly one operand; unexpected extra operand `extra`\n"
    );
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

#[test]
fn fmt_and_context_failures_keep_stable_diagnostic_codes() {
    let directory = empty_working_directory();
    let source_path = directory.join("module.spx");
    std::fs::write(
        &source_path,
        "module cli.diagnostics;\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    )
    .unwrap();

    let context = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(&directory)
        .args(["context", "module.spx", "missing"])
        .output()
        .unwrap();
    assert_eq!(context.status.code(), Some(1));
    let stderr = String::from_utf8(context.stderr).unwrap();
    assert!(stderr.contains("error[SPX-G404]"), "{stderr}");
    assert!(stderr.contains("module.spx"), "{stderr}");

    let missing_source = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(&directory)
        .args(["fmt", "missing.spx"])
        .output()
        .unwrap();
    assert_eq!(missing_source.status.code(), Some(1));
    assert!(String::from_utf8(missing_source.stderr)
        .unwrap()
        .contains("error[SPX-I001]"));

    let empty_project = directory.join("empty-project");
    std::fs::create_dir(&empty_project).unwrap();
    let missing_manifest = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(&directory)
        .args(["fmt", "empty-project"])
        .output()
        .unwrap();
    assert_eq!(missing_manifest.status.code(), Some(1));
    let stderr = String::from_utf8(missing_manifest.stderr).unwrap();
    assert!(stderr.contains("error[SPX-J102]"), "{stderr}");
    assert!(stderr.contains("help:"), "{stderr}");

    std::fs::remove_dir(empty_project).unwrap();
    std::fs::remove_file(source_path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
