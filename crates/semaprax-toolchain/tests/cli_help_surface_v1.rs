use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
#[cfg(unix)]
#[path = "cli_help_surface_v1/native_pure_routes.rs"]
mod native_pure_routes;
#[cfg(unix)]
#[path = "cli_help_surface_v1/source_agent_hot_reload.rs"]
mod source_agent_hot_reload;
#[cfg(unix)]
#[path = "../examples/fixtures/opencode_source_fixture.rs"]
mod source_fixture;
const SHAPES_CATALOG_PATH: &str = "../../docs/LANGUAGE-SHAPES-CATALOG.md";
const DOCTOR_LINE: &str = "semaprax doctor [--profile <id>] [--target native|web|all] [--json]\n";
const NEW_LINE: &str = "semaprax new <destination> [--name project-name] [--template calculator|library|service|stdin-stream-text|stdin-stream-data|source-command-file-text]\n";
const PROJECT_SCAFFOLD_LINE: &str = "semaprax project-scaffold --name project-name [--template calculator|library|service|stdin-stream-text|stdin-stream-data|source-command-file-text] [--layout frozen|tables]\n";
const BUILD_SOURCE_LINE: &str =
    "semaprax build <file> [--target native] [-o|--output path] [--json]\n";
const BUILD_PROJECT_LINE: &str = "semaprax build [<dir>|semaprax.toml|--manifest-path path] [--target native|web|wasm|npm|oci|rust] [-o|--output path] [--json]\n";
const BANNER: &str = "SEMAPRAX — Meaning in. Verified machine code out.\n";
const GUIDE_MAX_BYTES: usize = 2048;
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
    "  cli             Command-line programs\n",
    "  maps            String-keyed maps\n",
    "  lists           Lists and iterators\n",
    "  mistakes-code   Habits from other languages: diagnostic examples\n",
    "  mistakes-index  Habits from other languages: diagnostic index\n",
    "  web             Web applications\n",
    "  projects        Projects\n",
    "  json            JSON documents and cursors\n",
    "  specifications  Where the rules live\n",
);
const DIAGNOSTIC_CODES: &str = concat!(
    "Common diagnostic codes:\n",
    "  SPX-P106 SPX-H006 SPX-T252 SPX-T203 SPX-G170 SPX-P003 SPX-P201 SPX-T205 SPX-T266 SPX-B104 SPX-F102 SPX-G172\n",
    "Fix: semaprax help diagnostic <code>\n",
    "All: semaprax help language mistakes-index\n",
);
const DIAGNOSTIC_T208: &str = concat!(
    "SPX-T208\n",
    "wrote: index + 1 when index: usize\n",
    "fix: Default i64; index + 1usize\n",
);

fn guide_commands(guide: &str) -> Vec<&str> {
    guide
        .lines()
        .filter_map(|line| line.strip_prefix("  "))
        .map(|entry| entry.split_whitespace().next().unwrap())
        .collect()
}

fn empty_working_directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-cli-help-full-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

fn invoke(arguments: &[&str]) -> (Output, PathBuf) {
    let working_directory = empty_working_directory();
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax-full"));
    command.current_dir(&working_directory);
    for argument in arguments {
        command.arg(argument);
    }
    let output = command.output().unwrap();
    assert_eq!(std::fs::read_dir(&working_directory).unwrap().count(), 0);
    (output, working_directory)
}

#[test]
fn full_help_is_exact_capability_aware_and_inert() {
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

    let guide = String::from_utf8(empty.stdout.clone()).unwrap();
    assert!(guide.starts_with(BANNER));
    assert!(guide.len() <= GUIDE_MAX_BYTES, "{} bytes", guide.len());
    assert_eq!(guide.matches("\n  new ").count(), 1);
    assert_eq!(guide.matches("\n  doctor ").count(), 1);
    assert_eq!(guide.matches("\n  help all ").count(), 1);
    assert!(guide.contains("semaprax help diagnostic <code>`\n"));
    assert_eq!(guide.matches("\nsemaprax ").count(), 0);
    for name in guide_commands(&guide) {
        let (output, directory) = invoke(&["help", name]);
        assert!(
            output.status.success(),
            "guided entry `{name}` must have scoped help"
        );
        assert!(output.stderr.is_empty(), "{name}");
        std::fs::remove_dir(directory).unwrap();
    }

    let (all, all_dir) = invoke(&["help", "all"]);
    assert!(all.status.success());
    assert!(all.stderr.is_empty());
    let help = String::from_utf8(all.stdout.clone()).unwrap();
    assert!(help.starts_with(&format!("{BANNER}\nUsage:\nsemaprax dev ")));
    assert_eq!(help.matches("\nsemaprax check ").count(), 1);
    assert_eq!(help.matches(DOCTOR_LINE).count(), 1);
    assert_eq!(help.matches(NEW_LINE).count(), 1);
    assert_eq!(help.matches(PROJECT_SCAFFOLD_LINE).count(), 1);
    assert_eq!(help.matches(BUILD_SOURCE_LINE).count(), 1);
    assert_eq!(help.matches(BUILD_PROJECT_LINE).count(), 1);
    let doctor = help.find(DOCTOR_LINE).unwrap();
    let new = help.find(NEW_LINE).unwrap();
    let scaffold = help.find(PROJECT_SCAFFOLD_LINE).unwrap();
    let build = help.find(BUILD_SOURCE_LINE).unwrap();
    assert!(doctor < new && new < scaffold && scaffold < build);
    std::fs::remove_dir(all_dir).unwrap();

    let (unknown, unknown_dir) = invoke(&["not-a-command"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(unknown.stdout, empty.stdout);
    assert_eq!(unknown.stderr, b"unknown command `not-a-command`\n\n");

    let (typo, typo_dir) = invoke(&["doctro"]);
    assert_eq!(typo.status.code(), Some(2));
    assert_eq!(typo.stdout, empty.stdout);
    assert_eq!(
        typo.stderr,
        b"unknown command `doctro`; did you mean `doctor`?\n\n"
    );

    let (malformed_known, malformed_known_dir) = invoke(&["doctor", "--unknown"]);
    assert_eq!(malformed_known.status.code(), Some(2));
    assert!(malformed_known.stdout.is_empty());
    assert_eq!(
        malformed_known.stderr,
        b"doctor: unknown doctor option `--unknown`\nhint: run `semaprax doctor --help` for usage\n"
    );

    std::fs::remove_dir(malformed_known_dir).unwrap();
    std::fs::remove_dir(typo_dir).unwrap();
    std::fs::remove_dir(unknown_dir).unwrap();
    std::fs::remove_dir(empty_dir).unwrap();
}

#[test]
fn full_scoped_help_is_exhaustive_exact_capability_aware_and_inert() {
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

    let (language, language_dir) = invoke(&["help", "language"]);
    assert!(language.status.success());
    assert!(language.stderr.is_empty());
    let card = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/AGENT-QUICK-REFERENCE.md"),
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
    let (json_topic, json_topic_dir) = invoke(&["help", "language", "json"]);
    assert!(json_topic.status.success());
    assert!(json_topic.stderr.is_empty());
    let card_text = std::str::from_utf8(&card).unwrap();
    let start = card_text.find("## JSON documents and cursors\n").unwrap();
    let section = &card_text[start..];
    let end = section.find("\n## ").unwrap_or(section.len());
    assert_eq!(json_topic.stdout, &section.as_bytes()[..end]);
    std::fs::remove_dir(json_topic_dir).unwrap();
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
        9
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
    let (library_index, library_index_dir) = invoke(&["help", "library"]);
    assert!(library_index.status.success());
    assert!(library_index.stderr.is_empty());
    assert!(library_index.stdout.len() <= 1_024);
    let index = String::from_utf8(library_index.stdout).unwrap();
    assert!(index.contains("std.int.decimal"));
    assert!(index.contains("semaprax help library all"));
    std::fs::remove_dir(library_index_dir).unwrap();
    let (library, library_dir) = invoke(&["help", "library", "all"]);
    assert!(library.status.success());
    assert!(library.stderr.is_empty());
    let catalog = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/STANDARD-LIBRARY-CATALOG.md"),
    )
    .unwrap();
    assert_eq!(library.stdout, catalog);
    assert!(library.stdout.starts_with(b"# Standard library catalog\n"));
    assert!(library.stdout.ends_with(b"\n"));
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
    let expected_kinds = concat!(
        "Language shape kinds (8):\n",
        "  class\n",
        "  function\n",
        "  interface\n",
        "  method\n",
        "  record\n",
        "  resource\n",
        "  session_protocol\n",
        "  variant\n",
        "Exact exemplar: semaprax help shapes <kind>\n",
        "Full catalog: semaprax help shapes\n",
    );
    let (kinds, kinds_dir) = invoke(&["help", "shapes", "kinds"]);
    assert!(kinds.status.success());
    assert!(kinds.stderr.is_empty());
    assert_eq!(kinds.stdout, expected_kinds.as_bytes());
    assert!(kinds.stdout.len() <= 2_048);
    assert!(semaprax::agent_economics::lexical_tokens(expected_kinds) <= 256);
    std::fs::remove_dir(kinds_dir).unwrap();
    for selector in ["Kinds", "kin"] {
        let (output, directory) = invoke(&["help", "shapes", selector]);
        assert_eq!(output.status.code(), Some(2), "{selector}");
        assert!(output.stdout.is_empty(), "{selector}");
        assert_eq!(
            output.stderr,
            format!("language shapes catalog has no exact match for `{selector}`\n").as_bytes()
        );
        std::fs::remove_dir(directory).unwrap();
    }
    let (kinds_extra, kinds_extra_dir) = invoke(&["help", "shapes", "kinds", "extra"]);
    assert_eq!(kinds_extra.status.code(), Some(2));
    assert!(kinds_extra.stdout.is_empty());
    assert_eq!(
        kinds_extra.stderr,
        b"help accepts exactly one operand; unexpected extra operand `extra`\n"
    );
    std::fs::remove_dir(kinds_extra_dir).unwrap();
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
    let expected_decimal = b"std.int.decimal.compare\ndependency std.int.decimal = \"^0.1.0\"\nprofile owned-data-api.v1\nfn compare(left: borrow str, right: borrow str) -> i64\n    requires valid(left) && valid(right)\n    ensures result >= -1 && result <= 1\n";
    for (selector, expected) in [
        ("std.core.compare", expected_compare.as_slice()),
        ("std.int.decimal.compare", expected_decimal.as_slice()),
    ] {
        let (entry, directory) = invoke(&["help", "library", selector]);
        assert!(entry.status.success());
        assert!(entry.stderr.is_empty());
        assert_eq!(entry.stdout, expected);
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
    let (ambiguous, ambiguous_dir) = invoke(&["help", "library", "compare"]);
    assert!(ambiguous.status.success());
    assert!(ambiguous.stderr.is_empty());
    assert_eq!(
        ambiguous.stdout,
        [
            expected_compare.as_slice(),
            b"\n",
            expected_decimal.as_slice()
        ]
        .concat()
    );
    assert!(ambiguous.stdout.len() <= 512);
    assert!(ambiguous.stdout.len() * 50 < catalog.len());
    let combined_units =
        semaprax::agent_economics::lexical_tokens(std::str::from_utf8(&ambiguous.stdout).unwrap());
    assert!(combined_units <= 2 * 128);
    assert!(
        combined_units * 50
            < semaprax::agent_economics::lexical_tokens(std::str::from_utf8(&catalog).unwrap())
    );
    std::fs::remove_dir(ambiguous_dir).unwrap();
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
    let (all_extra, all_extra_dir) = invoke(&["help", "all", "extra"]);
    assert_eq!(all_extra.status.code(), Some(2));
    assert!(all_extra.stdout.is_empty());
    assert_eq!(
        all_extra.stderr,
        b"help accepts exactly one operand; unexpected extra operand `extra`\n"
    );
    std::fs::remove_dir(all_extra_dir).unwrap();
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
                "  semaprax help library all\n",
                "  semaprax help library <module|name|stable-id>\n",
                "  semaprax help shapes\n",
                "  semaprax help shapes kinds\n",
                "  semaprax help shapes <kind|stable-id|path#stable-id>\n"
            )
            .as_bytes()
        );
        assert!(output.stderr.is_empty());
        std::fs::remove_dir(directory).unwrap();
    }

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
