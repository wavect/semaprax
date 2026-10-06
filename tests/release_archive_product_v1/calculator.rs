use super::{command, Release};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

const README: &str = "# archive-calculator\n\nA small calculator project created by SEMAPRAX.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax run .\nsemaprax build . --target web -o web\n```\n\nRead `AGENTS.md` before editing the source, whether you are a person or a\ncoding agent: it lists the commands and the rules that differ from other\nlanguages.\n";
const MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"archive-calculator\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"archive_calculator.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"archive_calculator.tests\"]\n\n[exports]\nweb = [\"archive-calculator.add\"]\n";
const APP: &str = "module archive_calculator.app;\nuse function @id(\"archive-calculator.add\") from archive_calculator.core as add;\n\n@id(\"archive-calculator.app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n";
const CORE: &str = "module archive_calculator.core;\n\n@id(\"archive-calculator.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n";
const TESTS: &str = "module archive_calculator.tests;\n\n@id(\"archive-calculator.tests.main\")\nfn main() -> i64\n{\n    if 19 + 23 == 42 { 0 } else { 1 }\n}\n";

/// `AGENTS.md` embeds a generated skill-workflow section that tracks the
/// installed command catalog, so its exact bytes are not pinned here. Its
/// stable structure is, and the bytes read once are then pinned for the
/// "nothing else changed" inventory comparisons.
fn agents_guide(bytes: &[u8]) {
    let text = std::str::from_utf8(bytes).unwrap();
    assert!(text.starts_with(
        "# Agent guide for archive-calculator\n\nThis is a SEMAPRAX project. `semaprax.toml` lists its modules;"
    ));
    for section in [
        "\n## Commands\n",
        "\n## Rules that differ from other languages\n",
        "\n## Project v1 function boundaries\n",
        "\n## Installed Agent Skill workflow\n",
    ] {
        assert!(text.contains(section), "AGENTS.md lacks {section:?}");
    }
    assert!(text.contains("`semaprax check .`") && text.ends_with('\n'));
    assert!(text.len() < 64 * 1024);
}

pub(super) fn inventory(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn metadata(path: &Path) -> fs::Metadata {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt as _;
            assert_eq!(metadata.file_attributes() & 0x400, 0);
        }
        metadata
    }
    fn visit(root: &Path, path: &Path, rows: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            assert!(rows.len() < 64, "calculator inventory exceeded fixed bound");
            let path = entry.unwrap().path();
            let metadata = metadata(&path);
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            if metadata.is_dir() {
                rows.insert(format!("{name}/"), Vec::new());
                visit(root, &path, rows);
            } else {
                assert!(metadata.is_file() && metadata.len() <= 16 * 1024 * 1024);
                let mut bytes = Vec::new();
                fs::File::open(&path)
                    .unwrap()
                    .take(16 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .unwrap();
                assert!(bytes.len() <= 16 * 1024 * 1024);
                rows.insert(name, bytes);
            }
        }
    }
    let mut rows = BTreeMap::new();
    assert!(metadata(root).is_dir());
    visit(root, root, &mut rows);
    rows
}

fn cli(release: &Release, cwd: &Path, captures: &Path, arguments: &[&str]) -> Output {
    command::run(
        Command::new(&release.cli).args(arguments).current_dir(cwd),
        b"",
        captures,
        Duration::from_secs(60),
        4 * 1024 * 1024,
        64 * 1024,
    )
}

fn success(output: Output) -> Vec<u8> {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    output.stdout
}

pub(super) fn run(release: &Release, root: &Path) -> PathBuf {
    success(cli(
        release,
        root,
        &root.join("new"),
        &["new", "archive-calculator"],
    ));
    let project = root.join("archive-calculator");
    let mut found = inventory(&project);
    agents_guide(&found["AGENTS.md"]);
    let expected = [
        ("README.md", README),
        ("semaprax.toml", MANIFEST),
        ("src/app.spx", APP),
        ("src/core.spx", CORE),
        ("src/tests.spx", TESTS),
        ("src/", ""),
    ]
    .into_iter()
    .map(|(name, text)| (name.to_owned(), text.as_bytes().to_vec()))
    .chain([("AGENTS.md".to_owned(), found.remove("AGENTS.md").unwrap())])
    .collect::<BTreeMap<_, _>>();
    assert_eq!(inventory(&project), expected);
    success(cli(
        release,
        &project,
        &root.join("check"),
        &["check", "semaprax.toml"],
    ));
    assert_eq!(
        success(cli(
            release,
            &project,
            &root.join("test"),
            &["test", "semaprax.toml"]
        )),
        b"project tests passed\n"
    );
    assert_eq!(
        success(cli(
            release,
            &project,
            &root.join("run"),
            &["run", "semaprax.toml"]
        )),
        b"42\n"
    );
    // A source module that imports another needs Workspace Semantic Graph
    // resolution, so the single-file route must stay refused.
    let single = cli(
        release,
        &project,
        &root.join("graph-single-file"),
        &["graph", "src/app.spx"],
    );
    assert!(!single.status.success(), "{single:?}");
    assert!(single.stdout.is_empty(), "{single:?}");
    assert!(
        String::from_utf8_lossy(&single.stderr).contains("SPX-G172"),
        "{single:?}"
    );
    let graph = success(cli(
        release,
        &project,
        &root.join("graph"),
        &["graph", "semaprax.toml"],
    ));
    let text = std::str::from_utf8(&graph).unwrap();
    assert!(text.starts_with("{\"schema\":\"semaprax.project-semantic-graph.v"));
    serde_json::from_slice::<serde_json::Value>(&graph).unwrap();
    for needle in [
        "\"module\":\"archive_calculator.app\"",
        "\"module\":\"archive_calculator.core\"",
        "\"module\":\"archive_calculator.tests\"",
        "archive-calculator.add",
        "archive-calculator.app.main",
    ] {
        assert!(text.contains(needle), "graph lacks {needle}");
    }
    assert_eq!(
        success(cli(
            release,
            &project,
            &root.join("graph-again"),
            &["graph", "semaprax.toml"]
        )),
        graph
    );
    assert_eq!(inventory(&project), expected);
    let collision = cli(
        release,
        root,
        &root.join("new-collision"),
        &["new", "archive-calculator"],
    );
    assert!(!collision.status.success());
    assert_eq!(inventory(&project), expected);
    success(cli(
        release,
        &project,
        &root.join("web"),
        &[
            "build",
            "semaprax.toml",
            "--target",
            "web",
            "-o",
            "dist/web",
        ],
    ));
    let web = inventory(&project.join("dist/web"));
    assert_eq!(
        web.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "app.wasm",
            "index.html",
            "package.json",
            "semaprax.bindings.d.ts",
            "semaprax.bindings.js",
            "semaprax.js",
            "semaprax.scalar-exports.json"
        ]
    );
    assert!(web["app.wasm"].starts_with(b"\0asm\x01\0\0\0"));
    let exports: serde_json::Value =
        serde_json::from_slice(&web["semaprax.scalar-exports.json"]).unwrap();
    assert_eq!(exports["schema"], "semaprax.web-project.v1");
    assert_eq!(exports["project"], "archive-calculator");
    assert_eq!(exports["entry_module"], "archive_calculator.app");
    let functions = exports["scalar_abi"]["functions"].as_array().unwrap();
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0]["stable_id"], "archive-calculator.add");
    assert_eq!(
        functions[0]["parameters"],
        serde_json::json!(["i64", "i64"])
    );
    assert_eq!(functions[0]["result"], "i64");
    let published = inventory(&project);
    let mut expected_published = expected.clone();
    expected_published.insert("dist/".into(), Vec::new());
    expected_published.insert("dist/web/".into(), Vec::new());
    for (name, bytes) in &web {
        expected_published.insert(format!("dist/web/{name}"), bytes.clone());
    }
    assert_eq!(published, expected_published);
    let collision = cli(
        release,
        &project,
        &root.join("web-collision"),
        &[
            "build",
            "semaprax.toml",
            "--target",
            "web",
            "-o",
            "dist/web",
        ],
    );
    assert!(!collision.status.success());
    assert_eq!(inventory(&project), published);
    for (name, bytes) in expected {
        assert_eq!(published[&name], bytes);
    }
    project
}
