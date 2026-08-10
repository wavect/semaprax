use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::agent_economics;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn manifest() -> PathBuf {
    root().join("benchmarks/agent-context-v1/corpus.tsv")
}

#[test]
fn offline_corpus_is_deterministic_and_matches_exact_economics_golden() {
    let first = agent_economics::benchmark_manifest(&manifest()).unwrap();
    let second = agent_economics::benchmark_manifest(&manifest()).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        format!("{first}\n"),
        include_str!("snapshots/agent_economics.v1.json")
    );
    assert!(first.contains("\"model_tokens\":false"));
    assert!(first.contains(
        "\"manifest\":{\"schema\":\"semaprax.agent-context-benchmark.v1\",\"sha256\":\"sha256:"
    ));
    assert!(first.contains("\"labels\":{\"relevant_ids\":["));
    assert!(first.contains("\"evidence_ids\":["));
    assert!(first.contains("\"to_source_bytes\":\"243/77\""));
    assert!(first.contains("\"evidence_recall\":{\"hits\":4,\"expected\":6,\"ratio\":\"2/3\"}"));
    assert_node_json(&first);

    let cli = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("context-benchmark")
        .arg(manifest())
        .output()
        .unwrap();
    assert!(cli.status.success());
    assert_eq!(String::from_utf8(cli.stdout).unwrap(), format!("{first}\n"));
    let extra = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("context-benchmark")
        .arg(manifest())
        .arg("unexpected")
        .output()
        .unwrap();
    assert_eq!(extra.status.code(), Some(2));
}

#[test]
fn context_cli_outputs_match_maintenance_goldens_exactly() {
    let source = root().join("benchmarks/agent-context-v1/maintenance.spx");
    let cases = [
        (
            [
                "--depth",
                "0",
                "--max-bytes",
                "8192",
                "--max-nodes",
                "4",
                "--filters",
                "contracts,effects",
            ],
            include_str!("snapshots/agent_context_maintenance_contract.json"),
        ),
        (
            [
                "--depth",
                "1",
                "--max-bytes",
                "16384",
                "--max-nodes",
                "8",
                "--filters",
                "effects,types",
            ],
            include_str!("snapshots/agent_context_maintenance_dependencies.json"),
        ),
    ];
    for (arguments, expected) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("context")
            .arg(&source)
            .arg("service.fetch")
            .args(arguments)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn same_case_source_mutation_changes_revision_and_context_digest() {
    let fixture = Fixture::new();
    let original_source =
        fs::read_to_string(root().join("benchmarks/agent-context-v1/maintenance.spx")).unwrap();
    fs::write(fixture.directory.join("maintenance.spx"), &original_source).unwrap();
    fs::write(
        fixture.directory.join("corpus.tsv"),
        "schema\tsemaprax.agent-context-benchmark.v1\ncase\tWhich function refreshes the cache?\tmaintenance.spx\tservice.fetch\t1\t16384\t8\teffects\tservice.fetch,cache.lookup,cache.refresh\tcache.refresh\n",
    )
    .unwrap();
    let path = fixture.directory.join("corpus.tsv");
    let first = agent_economics::benchmark_manifest(&path).unwrap();
    fs::write(
        fixture.directory.join("maintenance.spx"),
        original_source.replace("key + 2", "key + 3"),
    )
    .unwrap();
    let mutated = agent_economics::benchmark_manifest(&path).unwrap();
    assert_ne!(first, mutated);
    assert_eq!(mutated, agent_economics::benchmark_manifest(&path).unwrap());
    assert!(first.contains("\"evidence_recall\":{\"hits\":1,\"expected\":1,\"ratio\":\"1/1\"}"));
    assert!(mutated.contains("\"evidence_recall\":{\"hits\":1,\"expected\":1,\"ratio\":\"1/1\"}"));
}

#[test]
fn manifests_reject_unknown_malformed_and_escaping_fields() {
    let fixture = Fixture::new();
    for (name, body) in [
        ("schema", "schema\tunknown\n"),
        (
            "escape",
            "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\t../x.spx\tapp.main\t0\t1024\t1\teffects\tapp.main\tapp.main\n",
        ),
        (
            "current",
            "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\t./x.spx\tapp.main\t0\t1024\t1\teffects\tapp.main\tapp.main\n",
        ),
        (
            "absolute",
            "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\t/tmp/x.spx\tapp.main\t0\t1024\t1\teffects\tapp.main\tapp.main\n",
        ),
        (
            "trailing-dot",
            "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\tx.spx.\tapp.main\t0\t1024\t1\teffects\tapp.main\tapp.main\n",
        ),
        (
            "trailing-space",
            "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\tx.spx \tapp.main\t0\t1024\t1\teffects\tapp.main\tapp.main\n",
        ),
        (
            "alternate-stream",
            "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\tx.spx:stream\tapp.main\t0\t1024\t1\teffects\tapp.main\tapp.main\n",
        ),
        (
            "filter",
            "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\tx.spx\tapp.main\t0\t1024\t1\tunknown\tapp.main\tapp.main\n",
        ),
        (
            "unavailable-filter",
            "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\tx.spx\tapp.main\t0\t1024\t1\ttargets\tapp.main\tapp.main\n",
        ),
        (
            "control",
            "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\rx.spx\tapp.main\t0\t1024\t1\teffects\tapp.main\tapp.main\n",
        ),
        (
            "subset",
            "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\tx.spx\tapp.main\t0\t1024\t1\teffects\tapp.main\tother\n",
        ),
    ] {
        let path = fixture.directory.join(format!("{name}.tsv"));
        fs::write(&path, body).unwrap();
        assert_eq!(
            agent_economics::benchmark_manifest(&path).unwrap_err().code,
            "SPX-G005"
        );
    }
    fs::write(
        fixture.directory.join("x.spx"),
        "module test.labels; @id(\"app.main\") fn main() -> i64 { 0 }",
    )
    .unwrap();
    let unknown_label = fixture.directory.join("unknown-label.tsv");
    fs::write(
        &unknown_label,
        "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\tx.spx\tapp.main\t0\t1024\t1\teffects\tmissing\tmissing\n",
    )
    .unwrap();
    let error = agent_economics::benchmark_manifest(&unknown_label).unwrap_err();
    assert_eq!(error.code, "SPX-G005");
    assert!(error
        .message
        .contains("relevant ID `missing` was not found"));
}

#[test]
fn manifests_reject_noncanonical_and_windows_reserved_source_aliases() {
    let fixture = Fixture::new();
    let mut aliases = vec!["dir//x.spx".to_owned(), "dir/".to_owned()];
    for character in ['<', '>', '"', '|', '?', '*'] {
        aliases.push(format!("bad{character}.spx"));
    }
    for device in ["CON", "con", "PRN", "AUX", "NUL"] {
        aliases.push(format!("{device}.spx"));
    }
    for prefix in ["COM", "LPT"] {
        for digit in 1..=9 {
            aliases.push(format!("{prefix}{digit}.spx"));
        }
    }
    for device in ["COM¹", "com²", "CoM³", "LPT¹", "lpt²", "LpT³"] {
        aliases.push(format!("{device}.spx"));
    }
    for (index, alias) in aliases.into_iter().enumerate() {
        let manifest = fixture.directory.join(format!("hostile-{index}.tsv"));
        fs::write(
            &manifest,
            format!(
                "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\t{alias}\tapp.main\t0\t1024\t1\teffects\tapp.main\tapp.main\n"
            ),
        )
        .unwrap();
        assert_eq!(
            agent_economics::benchmark_manifest(&manifest)
                .unwrap_err()
                .code,
            "SPX-G005",
            "hostile alias was accepted: {alias:?}"
        );
    }
}

#[test]
fn source_alias_requires_exact_directory_entry_spelling() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.directory.join("ExactCase")).unwrap();
    fs::write(
        fixture.directory.join("ExactCase/source.spx"),
        "module test.labels; @id(\"app.main\") fn main() -> i64 { 0 }",
    )
    .unwrap();
    let manifest = fixture.directory.join("case-alias.tsv");
    fs::write(
        &manifest,
        "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\texactcase/source.spx\tapp.main\t0\t1024\t1\teffects\tapp.main\tapp.main\n",
    )
    .unwrap();
    assert_eq!(
        agent_economics::benchmark_manifest(&manifest)
            .unwrap_err()
            .code,
        "SPX-G005"
    );
}

#[cfg(unix)]
#[test]
fn manifests_and_sources_reject_symlink_aliases() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fs::write(
        fixture.directory.join("real.spx"),
        "module test.labels; @id(\"app.main\") fn main() -> i64 { 0 }",
    )
    .unwrap();
    symlink(
        fixture.directory.join("real.spx"),
        fixture.directory.join("link.spx"),
    )
    .unwrap();
    let manifest = fixture.directory.join("corpus.tsv");
    fs::write(
        &manifest,
        "schema\tsemaprax.agent-context-benchmark.v1\nx\tq\tlink.spx\tapp.main\t0\t1024\t1\teffects\tapp.main\tapp.main\n",
    )
    .unwrap();
    assert_eq!(
        agent_economics::benchmark_manifest(&manifest)
            .unwrap_err()
            .code,
        "SPX-G005"
    );

    let manifest_link = fixture.directory.join("manifest-link.tsv");
    symlink(&manifest, &manifest_link).unwrap();
    assert_eq!(
        agent_economics::benchmark_manifest(&manifest_link)
            .unwrap_err()
            .code,
        "SPX-G005"
    );
}

fn assert_node_json(json: &str) {
    let output = Command::new("node")
        .arg("-e")
        .arg("JSON.parse(process.argv[1]);")
        .arg(json)
        .output()
        .expect("Node.js is required for the independent economics JSON gate");
    assert!(
        output.status.success(),
        "independent JSON parse failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Fixture {
    directory: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-agent-economics-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        Self { directory }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}
