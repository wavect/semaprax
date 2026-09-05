//! Package Manifest v1: the extensible `semaprax.manifest.v1` table layout.
//!
//! Every admitted table manifest lowers onto the frozen profile contract the
//! frozen `semaprax.project.vN` layouts name directly, so these cases pin the
//! lowering for all eleven profiles, the closed table/key catalog, the exact
//! canonical bytes, the dependency and target grammars, and the two project
//! routes the layout gates: dependency-free builds and the declared target
//! matrix. The end-to-end cases run the CLI over the committed examples with
//! their manifests rewritten into the table layout.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    ManifestLayout, ProjectManifest, PACKAGE_MANIFEST_RESERVED_TABLES, PACKAGE_MANIFEST_SCHEMA,
    PACKAGE_RESERVED_KEYS, PROJECT_SCHEMA, PROJECT_SCHEMA_V10, PROJECT_SCHEMA_V11,
    PROJECT_SCHEMA_V2, PROJECT_SCHEMA_V3, PROJECT_SCHEMA_V4, PROJECT_SCHEMA_V5, PROJECT_SCHEMA_V6,
    PROJECT_SCHEMA_V7, PROJECT_SCHEMA_V8, PROJECT_SCHEMA_V9,
};
use semaprax::{package_lock_v3, package_report_v2};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const CALCULATOR_TABLES: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"calculator\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"calculator.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"calculator.tests\"]\n\n[exports]\nweb = [\"calculator.add\", \"calculator.divide\", \"calculator.is-negative\", \"calculator.multiply\", \"calculator.not\", \"calculator.subtract\"]\n";

const SPXGREP_TABLES: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"spxgrep\"\nversion = \"0.1.0\"\nprofile = \"useful-data-command.v1\"\n\n[modules]\nentry = \"spxgrep.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"spxgrep.tests\"]\n\n[exports]\nweb = [\"spxgrep.contains\"]\n\n[command]\nfunction = \"spxgrep.contains\"\n\n[capabilities]\nrequired = [\"process.stdout.write\"]\n";

const ADAPTER_CAPABILITIES: &str = "[\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]";

struct Profile {
    name: Option<&'static str>,
    frozen_schema: &'static str,
    input: Option<&'static str>,
    capabilities: Option<&'static str>,
}

const PROFILES: &[Profile] = &[
    Profile {
        name: None,
        frozen_schema: PROJECT_SCHEMA,
        input: None,
        capabilities: None,
    },
    Profile {
        name: Some("useful-text-consumer.v1"),
        frozen_schema: PROJECT_SCHEMA_V2,
        input: None,
        capabilities: None,
    },
    Profile {
        name: Some("useful-data.v1"),
        frozen_schema: PROJECT_SCHEMA_V3,
        input: None,
        capabilities: None,
    },
    Profile {
        name: Some("useful-data-command.v1"),
        frozen_schema: PROJECT_SCHEMA_V4,
        input: None,
        capabilities: Some("[\"process.stdout.write\"]"),
    },
    Profile {
        name: Some("useful-data-command.v2"),
        frozen_schema: PROJECT_SCHEMA_V5,
        input: Some("stdin-bytes+one-utf8-arg.v1"),
        capabilities: Some(ADAPTER_CAPABILITIES),
    },
    Profile {
        name: Some("language-command-io.v1"),
        frozen_schema: PROJECT_SCHEMA_V6,
        input: Some("argv-utf8+stdin-bytes.v1"),
        capabilities: Some(ADAPTER_CAPABILITIES),
    },
    Profile {
        name: Some("line-command-io.v1"),
        frozen_schema: PROJECT_SCHEMA_V7,
        input: Some("argv-utf8+stdin-bytes.v1"),
        capabilities: Some(ADAPTER_CAPABILITIES),
    },
    Profile {
        name: Some("owned-data-api.v1"),
        frozen_schema: PROJECT_SCHEMA_V8,
        input: None,
        capabilities: None,
    },
    Profile {
        name: Some("flat-owned-record-api.v1"),
        frozen_schema: PROJECT_SCHEMA_V9,
        input: None,
        capabilities: None,
    },
    Profile {
        name: Some("owned-utf8-api.v1"),
        frozen_schema: PROJECT_SCHEMA_V10,
        input: None,
        capabilities: None,
    },
    Profile {
        name: Some("nested-owned-record-api.v1"),
        frozen_schema: PROJECT_SCHEMA_V11,
        input: None,
        capabilities: None,
    },
];

impl Profile {
    fn is_command(&self) -> bool {
        self.capabilities.is_some()
    }

    fn tables(&self) -> String {
        let mut text = format!(
            "schema = \"{PACKAGE_MANIFEST_SCHEMA}\"\n\n[package]\nname = \"demo\"\nversion = \"1.2.3\"\n"
        );
        if let Some(profile) = self.name {
            text.push_str(&format!("profile = \"{profile}\"\n"));
        }
        text.push_str(
            "\n[modules]\nentry = \"demo.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"demo.tests\"]\n\n[exports]\nweb = [\"demo.run\"]\n",
        );
        if self.is_command() {
            text.push_str("\n[command]\nfunction = \"demo.run\"\n");
            if let Some(input) = self.input {
                text.push_str(&format!("input = \"{input}\"\n"));
            }
        }
        if let Some(capabilities) = self.capabilities {
            text.push_str(&format!("\n[capabilities]\nrequired = {capabilities}\n"));
        }
        text
    }

    fn frozen(&self) -> String {
        let mut text = format!("schema = \"{}\"\nname = \"demo\"\n", self.frozen_schema);
        if let Some(profile) = self.name {
            text.push_str(&format!("version = \"1.2.3\"\nprofile = \"{profile}\"\n"));
        }
        text.push_str(
            "entry = \"demo.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"demo.run\"]\n",
        );
        if self.is_command() {
            text.push_str("command = \"demo.run\"\n");
            if let Some(input) = self.input {
                text.push_str(&format!("input = \"{input}\"\n"));
            }
        }
        if let Some(capabilities) = self.capabilities {
            text.push_str(&format!("capabilities = {capabilities}\n"));
        }
        text.push_str("tests = [\"demo.tests\"]\n");
        text
    }
}

fn codes(errors: &[semaprax::diagnostic::Diagnostic]) -> Vec<&str> {
    errors.iter().map(|error| error.code).collect()
}

fn reject(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    ProjectManifest::parse(source).expect_err("manifest must reject")
}

#[test]
fn table_layout_lowers_every_profile_to_its_frozen_contract() {
    for profile in PROFILES {
        let tables = profile.tables();
        let lowered =
            ProjectManifest::parse(&tables).unwrap_or_else(|errors| panic!("{tables}\n{errors:?}"));
        let frozen = ProjectManifest::parse(&profile.frozen()).unwrap();

        assert_eq!(lowered.schema(), profile.frozen_schema, "{tables}");
        assert_eq!(lowered.manifest_schema(), PACKAGE_MANIFEST_SCHEMA);
        assert_eq!(lowered.layout(), ManifestLayout::Tables);
        assert_eq!(frozen.layout(), ManifestLayout::Frozen);
        assert_eq!(frozen.manifest_schema(), profile.frozen_schema);
        assert_eq!(lowered.to_canonical_toml(), tables);

        assert_eq!(lowered.project_profile(), frozen.project_profile());
        assert_eq!(lowered.profile(), frozen.profile());
        assert_eq!(lowered.name(), frozen.name());
        assert_eq!(lowered.entry(), frozen.entry());
        assert_eq!(lowered.sources(), frozen.sources());
        assert_eq!(lowered.web_exports(), frozen.web_exports());
        assert_eq!(lowered.test_module(), frozen.test_module());
        assert_eq!(lowered.command(), frozen.command());
        assert_eq!(lowered.command_input(), frozen.command_input());
        assert_eq!(lowered.capabilities(), frozen.capabilities());
        assert_eq!(lowered.package_version(), Some("1.2.3"));
        assert!(lowered.dependencies().is_empty());
        assert!(lowered.dependency_sources().is_empty());
        assert!(lowered.rust_dependencies().is_empty());
        assert!(lowered.target_matrix().is_none());
        assert_eq!(
            frozen.package_version(),
            profile.name.map(|_| "1.2.3"),
            "the frozen v1 layout carries no version; the table layout always does"
        );

        let is_v = [
            lowered.is_v2(),
            lowered.is_v3(),
            lowered.is_v4(),
            lowered.is_v5(),
            lowered.is_v6(),
            lowered.is_v7(),
            lowered.is_v8(),
            lowered.is_v9(),
            lowered.is_v10(),
            lowered.is_v11(),
        ];
        let frozen_is_v = [
            frozen.is_v2(),
            frozen.is_v3(),
            frozen.is_v4(),
            frozen.is_v5(),
            frozen.is_v6(),
            frozen.is_v7(),
            frozen.is_v8(),
            frozen.is_v9(),
            frozen.is_v10(),
            frozen.is_v11(),
        ];
        assert_eq!(is_v, frozen_is_v, "{tables}");
    }
}

#[test]
fn profile_rules_reject_missing_or_foreign_command_facts() {
    let scalar = PROFILES[0].tables();
    let with_command = scalar.replace(
        "web = [\"demo.run\"]\n",
        "web = [\"demo.run\"]\n\n[command]\nfunction = \"demo.run\"\n",
    );
    let errors = reject(&with_command);
    assert_eq!(codes(&errors), ["SPX-J100"]);
    assert!(errors[0]
        .message
        .contains("does not admit a `[command]` table"));

    let with_capabilities = scalar.replace(
        "web = [\"demo.run\"]\n",
        "web = [\"demo.run\"]\n\n[capabilities]\nrequired = [\"process.stdout.write\"]\n",
    );
    assert!(reject(&with_capabilities)[0]
        .message
        .contains("does not admit a `[capabilities]` table"));

    let v4 = PROFILES[3].tables();
    let without_command = v4.replace("\n[command]\nfunction = \"demo.run\"\n", "");
    assert!(reject(&without_command)[0]
        .message
        .contains("requires a `[command]` table with `function`"));
    let wrong_capabilities = v4.replace(
        "required = [\"process.stdout.write\"]",
        "required = [\"process.stdin.read\"]",
    );
    let wrong_capability_errors = reject(&wrong_capabilities);
    assert!(
        wrong_capability_errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("requires `[capabilities] required = [\"process.stdout.write\"]`")),
        "{wrong_capability_errors:?}"
    );
    let with_input = v4.replace(
        "function = \"demo.run\"\n",
        "function = \"demo.run\"\ninput = \"stdin-bytes+one-utf8-arg.v1\"\n",
    );
    assert!(reject(&with_input)[0]
        .message
        .contains("does not admit `[command] input`"));

    let v5 = PROFILES[4].tables();
    let wrong_input = v5.replace("stdin-bytes+one-utf8-arg.v1", "argv-utf8+stdin-bytes.v1");
    assert!(reject(&wrong_input)[0]
        .message
        .contains("requires `[command] input = \"stdin-bytes+one-utf8-arg.v1\"`"));

    let unknown_profile = PROFILES[7]
        .tables()
        .replace("owned-data-api.v1", "owned-data-api.v9");
    assert!(reject(&unknown_profile)[0]
        .message
        .contains("`owned-data-api.v9` is not an admitted profile"));
}

#[test]
fn independent_manifest_structure_errors_arrive_together_with_scaffold_help() {
    let source = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"demo\"\nversion = \"0.1.0\"\nprofile = \"useful-data-command.v1\"\n\n[modules]\nentry = \"demo.app\"\nsources = [\"src/app.spx\"]\ntests = []\n\n[exports]\nweb = []\n";
    let errors = reject(source);
    assert_eq!(codes(&errors), ["SPX-J100"; 5]);
    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages
        .iter()
        .any(|message| message.contains("requires a `[command]` table")));
    assert!(messages
        .iter()
        .any(|message| message.contains("requires a `[capabilities]` table")));
    assert!(messages
        .iter()
        .any(|message| message.contains("requires 2..=16 explicit source paths")));
    assert!(messages
        .iter()
        .any(|message| message.contains("requires 1..=32 explicit web export identities")));
    assert!(messages
        .iter()
        .any(|message| message.contains("tests must contain exactly one bounded module name")));
    for diagnostic in errors {
        let help = diagnostic.help.unwrap();
        assert!(help.contains("semaprax new <destination>"), "{help}");
        assert!(
            help.contains("project-scaffold --name <name> --layout tables"),
            "{help}"
        );
    }
}

#[test]
fn reserved_and_unknown_tables_and_keys_fail_closed_with_spx_j120() {
    for table in PACKAGE_MANIFEST_RESERVED_TABLES {
        let source = format!("{CALCULATOR_TABLES}\n[{table}]\n");
        let errors = reject(&source);
        assert_eq!(codes(&errors), ["SPX-J120"], "[{table}]");
        assert!(
            errors[0]
                .message
                .contains(&format!("table `[{table}]` is reserved")),
            "{}",
            errors[0].message
        );
    }
    let unknown = format!("{CALCULATOR_TABLES}\n[plugins]\nnames = []\n");
    let errors = reject(&unknown);
    assert_eq!(codes(&errors), ["SPX-J120"]);
    assert!(errors[0]
        .message
        .contains("does not admit table `[plugins]`; admitted tables are `[package]`"));

    for key in PACKAGE_RESERVED_KEYS {
        let source = CALCULATOR_TABLES.replace(
            "version = \"0.1.0\"\n",
            &format!("version = \"0.1.0\"\n{key} = \"x\"\n"),
        );
        let errors = reject(&source);
        assert_eq!(codes(&errors), ["SPX-J120"], "{key}");
        assert!(errors[0]
            .message
            .contains(&format!("key `[package] {key}` is reserved")));
    }
    let unknown_key = CALCULATOR_TABLES.replace(
        "entry = \"calculator.app\"\n",
        "entry = \"calculator.app\"\nmain = \"calculator.app\"\n",
    );
    let errors = reject(&unknown_key);
    assert_eq!(codes(&errors), ["SPX-J120"]);
    assert!(errors[0]
        .message
        .contains("table `[modules]` does not admit key `main`"));

    let missing_table = CALCULATOR_TABLES.replace(
        "\n[exports]\nweb = [\"calculator.add\", \"calculator.divide\", \"calculator.is-negative\", \"calculator.multiply\", \"calculator.not\", \"calculator.subtract\"]\n",
        "",
    );
    let errors = reject(&missing_table);
    assert_eq!(codes(&errors), ["SPX-J100"]);
    assert!(errors[0].message.contains("requires a `[exports]` table"));
    let missing_key = CALCULATOR_TABLES.replace("version = \"0.1.0\"\n", "");
    assert!(reject(&missing_key)[0]
        .message
        .contains("table `[package]` requires `version`"));
    let duplicate_table = format!("{CALCULATOR_TABLES}\n[package]\nname = \"again\"\n");
    assert!(reject(&duplicate_table)[0]
        .message
        .contains("table `[package]` appears twice"));
    let duplicate_key = CALCULATOR_TABLES.replace(
        "version = \"0.1.0\"\n",
        "version = \"0.1.0\"\nversion = \"0.1.0\"\n",
    );
    assert!(reject(&duplicate_key)[0]
        .message
        .contains("key `version` appears twice in `[package]`"));
    let list_for_text =
        CALCULATOR_TABLES.replace("name = \"calculator\"", "name = [\"calculator\"]");
    assert!(reject(&list_for_text)[0]
        .message
        .contains("`[package] name` must be one string"));
    let text_for_list = CALCULATOR_TABLES.replace(
        "tests = [\"calculator.tests\"]",
        "tests = \"calculator.tests\"",
    );
    assert!(reject(&text_for_list)[0]
        .message
        .contains("`[modules] tests` must be an array of strings"));
}

#[test]
fn non_canonical_bytes_name_the_first_differing_line() {
    let reordered = CALCULATOR_TABLES.replace(
        "\n[modules]\nentry = \"calculator.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"calculator.tests\"]\n",
        "\n[modules]\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\nentry = \"calculator.app\"\ntests = [\"calculator.tests\"]\n",
    );
    let errors = reject(&reordered);
    assert_eq!(codes(&errors), ["SPX-J100"]);
    assert_eq!(
        errors[0].message,
        "Package Manifest v1 manifest is not canonical"
    );
    let help = errors[0].help.as_deref().unwrap();
    assert!(
        help.starts_with(
            "line 8: expected `entry = \"calculator.app\"`, found `sources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]`"
        ),
        "{help}"
    );

    let table_order = CALCULATOR_TABLES
        .replace(
            "\n[modules]\nentry = \"calculator.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"calculator.tests\"]\n",
            "",
        )
        + "\n[modules]\nentry = \"calculator.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"calculator.tests\"]\n";
    let help = reject(&table_order)[0].help.clone().unwrap();
    assert!(
        help.starts_with("line 7: expected `[modules]`, found `[exports]`"),
        "{help}"
    );

    let missing_blank = CALCULATOR_TABLES.replace("\n\n[package]", "\n[package]");
    let help = reject(&missing_blank)[0].help.clone().unwrap();
    assert!(
        help.starts_with("line 2: expected ``, found `[package]`"),
        "{help}"
    );

    let trailing_blank = format!("{CALCULATOR_TABLES}\n");
    let help = reject(&trailing_blank)[0].help.clone().unwrap();
    assert!(
        help.starts_with("line 15: expected end of manifest, found ``"),
        "{help}"
    );

    let comment = CALCULATOR_TABLES.replace("[package]\n", "[package]\n# identity\n");
    let errors = reject(&comment);
    assert_eq!(codes(&errors), ["SPX-J100"]);
    assert!(errors[0]
        .message
        .contains("expected `key = value` in `[package]`; found `# identity`"));

    let spaced = CALCULATOR_TABLES.replace("name = \"calculator\"", "name  = \"calculator\"");
    assert_eq!(codes(&reject(&spaced)), ["SPX-J100"]);

    assert!(reject(CALCULATOR_TABLES.trim_end())[0]
        .message
        .contains("must end with one terminal LF"));
}

#[test]
fn dependency_grammar_is_admitted_and_ordinary_builds_fail_closed_with_spx_j121() {
    let source = format!(
        "{CALCULATOR_TABLES}\n[dependencies]\nalpha = \"^1.2.0\"\nexamples.meaning = \"~0.4.1\"\nnum_util-2 = \"=3.0.0\"\n"
    );
    let manifest = ProjectManifest::parse(&source).unwrap();
    assert_eq!(manifest.to_canonical_toml(), source);
    let rows = manifest
        .dependencies()
        .iter()
        .map(|dependency| (dependency.name(), dependency.range()))
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        [
            ("alpha", "^1.2.0"),
            ("examples.meaning", "~0.4.1"),
            ("num_util-2", "=3.0.0"),
        ],
        "dotted and underscored package identities are admitted dependency names"
    );

    for (bad, fragment) in [
        ("alpha = \">=1.0.0\"\n", "Package Manifest v1 dependency"),
        ("alpha = \"^1.2\"\n", "missing patch component"),
        ("alpha = \"^01.2.0\"\n", "dependency"),
        (
            "Alpha = \"^1.2.0\"\n",
            "keys are lowercase [a-z0-9._-]+; found `Alpha` in `[dependencies]`",
        ),
        (
            "1alpha = \"^1.2.0\"\n",
            "dependency names are dotted lowercase package identities",
        ),
        (
            ".alpha = \"^1.2.0\"\n",
            "dependency names are dotted lowercase package identities",
        ),
        (
            "alpha. = \"^1.2.0\"\n",
            "dependency names are dotted lowercase package identities",
        ),
        (
            "a..b = \"^1.2.0\"\n",
            "dependency names are dotted lowercase package identities",
        ),
        ("alpha = [\"^1.2.0\"]\n", "must be one range string"),
        (
            "beta = \"^1.0.0\"\nalpha = \"^1.0.0\"\n",
            "strictly byte-sorted by name",
        ),
    ] {
        let errors = reject(&format!("{CALCULATOR_TABLES}\n[dependencies]\n{bad}"));
        assert_eq!(codes(&errors), ["SPX-J100"], "{bad}");
        assert!(
            errors[0].message.contains(fragment),
            "{bad}: {}",
            errors[0].message
        );
    }

    let fixture = calculator_fixture("dependencies", &source);
    let output = cli(&fixture.root, &["check", "semaprax.toml"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SPX-J121"), "{stderr}");
    assert!(
        stderr.contains("dependency `alpha` is not a compiler-bundled standard-library package"),
        "{stderr}"
    );
    assert!(!fixture.root.join("calculator-web").exists());
}

#[test]
fn exact_semaprax_sources_and_rust_crates_are_canonical_manifest_inputs() {
    let source = format!(
        "{CALCULATOR_TABLES}\n[dependencies]\nalpha = \"^1.2.0\"\n\n[dependency-sources]\nalpha = \"vendor/alpha.subject.json\"\nbeta = \"vendor/beta.subject.json\"\n\n[rust-dependencies]\nitoa = [\"=1.0.15\"]\nserde = [\"=1.0.228\", \"derive\", \"std\"]\n\n[targets]\nmatrix = [\"native64\"]\n"
    );
    let manifest = ProjectManifest::parse(&source).unwrap();
    assert_eq!(manifest.to_canonical_toml(), source);
    assert_eq!(
        manifest
            .dependency_sources()
            .iter()
            .map(|dependency| (dependency.name(), dependency.path()))
            .collect::<Vec<_>>(),
        [
            ("alpha", "vendor/alpha.subject.json"),
            ("beta", "vendor/beta.subject.json"),
        ]
    );
    assert_eq!(
        manifest
            .rust_dependencies()
            .iter()
            .map(|dependency| (
                dependency.name(),
                dependency.version(),
                dependency.features()
            ))
            .collect::<Vec<_>>(),
        [
            ("itoa", "=1.0.15", &[][..]),
            (
                "serde",
                "=1.0.228",
                &["derive".to_owned(), "std".to_owned()][..]
            ),
        ]
    );

    for (suffix, fragment) in [
        (
            "\n[dependency-sources]\nalpha = \"../alpha.json\"\n",
            "canonical project-relative `.json` path",
        ),
        (
            "\n[dependency-sources]\nalpha = \"vendor/alpha.spx\"\n",
            "canonical project-relative `.json` path",
        ),
        (
            "\n[rust-dependencies]\nserde = [\"^1.0.0\"]\n",
            "must use exact `=x.y.z` syntax",
        ),
        (
            "\n[rust-dependencies]\nserde = [\"=1.0.0\", \"std\", \"derive\"]\n",
            "features must be strictly byte-sorted",
        ),
        (
            "\n[rust-dependencies]\nfoo-bar = [\"=1.0.0\"]\nfoo_bar = [\"=1.0.0\"]\n",
            "collide after Cargo maps `-` to `_`",
        ),
    ] {
        let errors = reject(&format!(
            "{CALCULATOR_TABLES}\n[dependencies]\nalpha = \"^1.0.0\"{suffix}"
        ));
        assert_eq!(codes(&errors), ["SPX-J100"], "{suffix}");
        assert!(
            errors[0].message.contains(fragment),
            "{suffix}: {}",
            errors[0].message
        );
    }

    for table in [
        "[dependencies]\nalpha = \"^1.0.0\"\n\n[dependency-sources]\nalpha = \"vendor/alpha.json\"\n",
        "[rust-dependencies]\nsame-file = [\"=1.0.6\"]\n",
    ] {
        let errors = reject(&format!("{SPXGREP_TABLES}\n{table}"));
        assert_eq!(codes(&errors), ["SPX-J100"]);
        assert!(errors[0].message.contains("require the scalar profile"));
    }

    let reserved = CALCULATOR_TABLES.replacen("src/app.spx", "dependencies/app.spx", 1);
    let errors = reject(&format!(
        "{reserved}\n[dependencies]\nalpha = \"^1.0.0\"\n\n[dependency-sources]\nalpha = \"vendor/alpha.json\"\n"
    ));
    assert_eq!(codes(&errors), ["SPX-J100"]);
    assert!(errors[0]
        .message
        .contains("reserved `dependencies/` prefix"));
}

#[test]
fn exact_local_semaprax_subjects_link_through_every_project_route() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-external-dependency-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("vendor")).unwrap();
    let provider = root.join("provider.spx");
    std::fs::write(
        &provider,
        "module acme.math;\n\n@id(\"acme.math.double\")\nfn double(value: i64) -> i64\n{\n    value * 2\n}\n\n@id(\"acme.math.main\")\nfn main() -> i64\n{\n    double(21)\n}\n",
    )
    .unwrap();
    let report = package_report_v2::generate(
        &provider,
        &package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let subject = package_lock_v3::create_subject(
        &package_lock_v3::Coordinate {
            package: "acme.math".to_owned(),
            version: "1.2.0".to_owned(),
        },
        &report,
        &[],
        &[],
    )
    .unwrap();
    std::fs::write(root.join("vendor/acme-math.subject.json"), &subject).unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        "module consumer.app;\nuse function @id(\"acme.math.double\") from acme.math as double;\n\n@id(\"consumer.answer\")\nfn answer() -> i64\n{\n    double(21)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    answer()\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    let manifest = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"consumer\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.answer\"]\n\n[dependencies]\nacme.math = \"^1.0.0\"\n\n[dependency-sources]\nacme.math = \"vendor/acme-math.subject.json\"\n\n[rust-dependencies]\nsame-file = [\"=1.0.6\"]\n";
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();

    for arguments in [
        vec!["check", "semaprax.toml"],
        vec!["test", "semaprax.toml"],
        vec!["project-image", "semaprax.toml"],
        vec!["lock", "semaprax.toml"],
    ] {
        let output = cli(&root, &arguments);
        assert!(
            output.status.success(),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = cli(&root, &["run", "semaprax.toml"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("42\n"));
    let output = cli(
        &root,
        &[
            "build",
            "semaprax.toml",
            "--target",
            "web",
            "-o",
            "consumer-web",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(root.join("consumer-web/app.wasm").is_file());

    let image = cli(&root, &["project-image", "semaprax.toml"]);
    assert!(image.status.success());
    let image: serde_json::Value = serde_json::from_slice(&image.stdout).unwrap();
    let subject_digest = package_lock_v3::verify_dependency_subject(&subject)
        .unwrap()
        .subject_digest;
    let dependency_path = format!(
        "dependencies/{}/{}.spx",
        &subject_digest[7..39],
        &subject_digest[39..]
    );
    assert!(image["sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["path"] == dependency_path));

    let tampered = subject.replacen("1.2.0", "1.3.0", 1);
    std::fs::write(root.join("vendor/acme-math.subject.json"), tampered).unwrap();
    let output = cli(&root, &["check", "semaprax.toml"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-J123"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn target_matrix_is_closed_and_gates_cli_build_targets() {
    for (bad, fragment) in [
        ("matrix = []\n", "must name at least one target"),
        (
            "matrix = [\"x86_64\"]\n",
            "admits only \"native64\" and \"wasm32\"; found `x86_64`",
        ),
        (
            "matrix = [\"wasm32\", \"native64\"]\n",
            "strictly byte-sorted and unique",
        ),
        (
            "matrix = [\"wasm32\", \"wasm32\"]\n",
            "strictly byte-sorted and unique",
        ),
    ] {
        let errors = reject(&format!("{CALCULATOR_TABLES}\n[targets]\n{bad}"));
        assert_eq!(codes(&errors), ["SPX-J100"], "{bad}");
        assert!(
            errors[0].message.contains(fragment),
            "{bad}: {}",
            errors[0].message
        );
    }

    let wasm_only = format!("{CALCULATOR_TABLES}\n[targets]\nmatrix = [\"wasm32\"]\n");
    let manifest = ProjectManifest::parse(&wasm_only).unwrap();
    assert_eq!(manifest.target_matrix(), Some(&["wasm32".to_owned()][..]));
    assert_eq!(manifest.to_canonical_toml(), wasm_only);
    assert!(manifest.admit_build_target("web").is_ok());
    assert!(manifest.admit_build_target("npm").is_ok());
    let errors = manifest.admit_build_target("native").unwrap_err();
    assert_eq!(codes(&errors), ["SPX-J122"]);
    assert_eq!(
        errors[0].message,
        "build target `native` needs `native64`, but the manifest `[targets] matrix` declares only [\"wasm32\"]"
    );
    assert!(ProjectManifest::parse(CALCULATOR_TABLES)
        .unwrap()
        .admit_build_target("native")
        .is_ok());

    let fixture = calculator_fixture("targets", &wasm_only);
    let output = cli(
        &fixture.root,
        &[
            "build",
            "semaprax.toml",
            "--target",
            "native",
            "-o",
            "out-native",
        ],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SPX-J122"), "{stderr}");
    assert!(!fixture.root.join("out-native").exists());

    let output = cli(
        &fixture.root,
        &["build", "semaprax.toml", "--target", "web", "-o", "out-web"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.root.join("out-web/app.wasm").is_file());
}

#[test]
fn cli_routes_accept_table_manifests_for_scalar_and_command_projects() {
    let calculator = calculator_fixture("routes", CALCULATOR_TABLES);
    for arguments in [
        &["check", "semaprax.toml"][..],
        &["test", "semaprax.toml"],
        &["run", "semaprax.toml"],
        &["project-image", "semaprax.toml"],
    ] {
        let output = cli(&calculator.root, arguments);
        assert!(
            output.status.success(),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let image =
        String::from_utf8(cli(&calculator.root, &["project-image", "semaprax.toml"]).stdout)
            .unwrap();
    let image: serde_json::Value = serde_json::from_str(&image).unwrap();
    assert_eq!(
        image["canonical_manifest"].as_str().unwrap(),
        CALCULATOR_TABLES,
        "the image binds the table bytes, not a frozen rewrite"
    );

    let output = cli(
        &calculator.root,
        &["build", "semaprax.toml", "--target", "web", "-o", "web"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let frozen = calculator_fixture(
        "routes-frozen",
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/semaprax.toml"),
        )
        .unwrap(),
    );
    let output = cli(
        &frozen.root,
        &["build", "semaprax.toml", "--target", "web", "-o", "web"],
    );
    assert!(output.status.success());
    let names = |root: &Path| {
        let mut names = std::fs::read_dir(root.join("web"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        names.sort();
        names
    };
    assert_eq!(names(&calculator.root), names(&frozen.root));
    assert_eq!(
        std::fs::read(calculator.root.join("web/app.wasm")).unwrap(),
        std::fs::read(frozen.root.join("web/app.wasm")).unwrap(),
        "the layout changes manifest bytes, never generated code"
    );

    let spxgrep = example_fixture("spxgrep", "examples/spxgrep-project", SPXGREP_TABLES);
    for arguments in [&["check", "semaprax.toml"][..], &["test", "semaprax.toml"]] {
        let output = cli(&spxgrep.root, arguments);
        assert!(
            output.status.success(),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn specification_examples_parse_canonically() {
    let specification = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/PACKAGE-MANIFEST-V1.md"),
    )
    .unwrap();
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in specification.lines() {
        match (&mut current, line) {
            (None, "```toml") => current = Some(String::new()),
            (Some(block), "```") => blocks.push(std::mem::take(block)),
            (Some(block), line) => {
                block.push_str(line);
                block.push('\n');
            }
            (None, _) => {}
        }
        if line == "```" {
            current = None;
        }
    }
    assert!(
        blocks.len() >= 2,
        "the specification shows at least two manifests"
    );
    for block in blocks {
        let manifest = ProjectManifest::parse(&block).unwrap_or_else(|errors| {
            panic!("specification manifest must be canonical:\n{block}\n{errors:?}")
        });
        assert_eq!(manifest.layout(), ManifestLayout::Tables);
        assert_eq!(manifest.to_canonical_toml(), block);
    }
}

struct Fixture {
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn calculator_fixture(label: &str, manifest: &str) -> Fixture {
    example_fixture(label, "examples/calculator-project", manifest)
}

fn example_fixture(label: &str, example: &str, manifest: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-package-manifest-v1-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(example);
    for entry in std::fs::read_dir(source.join("src")).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), root.join("src").join(entry.file_name())).unwrap();
    }
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
    Fixture {
        root: root.canonicalize().unwrap(),
    }
}

fn cli(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap()
}
