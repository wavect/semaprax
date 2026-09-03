use super::*;

#[test]
fn current_source_graph_schemas_are_admitted_without_widening_unknown_schemas() {
    assert!(is_source_graph_schema("semaprax.graph.v20"));
    assert!(is_source_graph_schema("semaprax.graph.v21"));
    assert!(is_source_graph_schema("semaprax.graph.v22"));
    assert!(is_source_graph_schema("semaprax.graph.v23"));
    assert!(is_source_graph_schema("semaprax.graph.v24"));
    assert!(is_source_graph_schema("semaprax.graph.v25"));
    assert!(is_source_graph_schema("semaprax.graph.v26"));
    assert!(is_source_graph_schema("semaprax.graph.v27"));
    for mutation in [
        "semaprax.graph.v26 ",
        "semaprax.graph.v026",
        "semaprax.graph.v27+v25",
        "semaprax.graph.v28",
    ] {
        assert!(!is_source_graph_schema(mutation));
    }
}
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn canonical_source(path: &str, source: &str) -> SemanticWorkspaceSource {
    let program = crate::parse(source, Path::new(path)).expect("semantic fixture must parse");
    SemanticWorkspaceSource {
        path: path.to_owned(),
        source: crate::format::canonical(&program),
    }
}

fn importing_sources() -> Vec<SemanticWorkspaceSource> {
    vec![
        canonical_source(
            "z/app.spx",
            r#"
module semantic.app;
use type @id("semantic.point") from semantic.provider as Point;
use function @id("semantic.work") from semantic.provider as work;
permit { audit.write }

@id("semantic.main")
fn main() -> i64 uses { audit.write } {
    work(Point { value: 1 })
}
"#,
        ),
        canonical_source(
            "a/provider.spx",
            r#"
module semantic.provider;
permit { audit.write }

@id("semantic.point")
record Point { @id("semantic.point.value") value: i64, }

@id("semantic.work")
fn work(value: Point) -> i64 uses { audit.write } { value.value }

@id("semantic.provider.main")
fn main() -> i64 { 0 }
"#,
        ),
    ]
}

fn path_set(paths: &[&str]) -> String {
    render_path_set(
        &paths
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn raw_path_set(paths: &[String]) -> String {
    format!(
        "{{\"schema\":{},\"files\":[{}]}}\n",
        quote_json(PATH_SET_SCHEMA),
        paths
            .iter()
            .map(|path| format!("{{\"path\":{}}}", quote_json(path)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn raw_manifest(files: &[SemanticWorkspaceFileFact]) -> String {
    format!(
            "{{\"schema\":{},\"files\":[{}]}}\n",
            quote_json(MANIFEST_SCHEMA),
            files
                .iter()
                .map(|file| format!(
                    "{{\"path\":{},\"source_graph_schema\":{},\"source_revision\":{},\"source_digest\":{},\"bytes\":{}}}",
                    quote_json(&file.path),
                    quote_json(&file.source_graph_schema),
                    quote_json(&file.source_revision),
                    quote_json(&file.source_digest),
                    file.bytes
                ))
                .collect::<Vec<_>>()
                .join(",")
        )
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) -> Vec<Diagnostic> {
    let error = result.err().expect("hostile input must fail closed");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, code);
    error
}

#[test]
fn exact_path_set_active_manifest_and_revision_kat_replay() {
    let paths = path_set(&["a/provider.spx", "z/app.spx"]);
    assert_eq!(
            paths,
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n"
        );
    assert_eq!(
        parse_path_set(&paths).unwrap(),
        ["a/provider.spx", "z/app.spx"]
    );

    let preflight = preflight_owned(&paths, importing_sources()).unwrap();
    assert_eq!(preflight.path_set(), parse_path_set(&paths).unwrap());
    assert_eq!(
        preflight
            .files()
            .iter()
            .map(SemanticWorkspaceFileFact::path)
            .collect::<Vec<_>>(),
        ["a/provider.spx", "z/app.spx"]
    );
    assert!(preflight.files().iter().all(|file| {
        file.source_graph_schema() == "semaprax.graph.v10"
            && file.bytes() == file.source().len()
            && file.source_revision().starts_with("sha256:")
            && file.source_digest().starts_with("sha256:")
    }));
    assert_eq!(
            preflight.manifest(),
            "{\"schema\":\"semaprax.workspace-semantic-manifest.v1\",\"files\":[{\"path\":\"a/provider.spx\",\"source_graph_schema\":\"semaprax.graph.v10\",\"source_revision\":\"sha256:e9e29bfe3a186fd9c9e1a7d8f3c10dc7ebcc006ed92b407344adacbb0248b7c0\",\"source_digest\":\"sha256:92041de1eebfe58bac89d26f743f7b09c21e57b9203094fc8a5667d40c1592a7\",\"bytes\":292},{\"path\":\"z/app.spx\",\"source_graph_schema\":\"semaprax.graph.v10\",\"source_revision\":\"sha256:df8274579bffda63bfed85c486f8dc30b54c698a8d051bf4ee165b947e3e370a\",\"source_digest\":\"sha256:943ec92f277f75089f8a4b7db0a3a4bf66fa90787d94ea494230195d2604f10b\",\"bytes\":272}]}\n"
        );
    assert_eq!(
        preflight.workspace_revision(),
        "sha256:88181393a052db1605145236cd3fd2e7f3f24256ce0c90d7968d939fc6a4c4ef"
    );
    assert_eq!(
        semantic_workspace_revision(preflight.manifest()),
        preflight.workspace_revision()
    );
    let parsed_manifest = parse_manifest(preflight.manifest()).unwrap();
    assert_eq!(
        render_manifest(&parsed_manifest).unwrap(),
        preflight.manifest()
    );
    for (actual, replayed) in preflight.files().iter().zip(parsed_manifest) {
        assert_eq!(actual.path(), replayed.path());
        assert_eq!(actual.source_graph_schema(), replayed.source_graph_schema());
        assert_eq!(actual.source_revision(), replayed.source_revision());
        assert_eq!(actual.source_digest(), replayed.source_digest());
        assert_eq!(actual.bytes(), replayed.bytes());
    }
    let active = render_root(preflight.workspace_revision()).unwrap();
    assert_eq!(
            active,
            format!(
                "{{\"schema\":\"semaprax.workspace-semantic-root.v1\",\"workspace_revision\":\"{}\"}}\n",
                preflight.workspace_revision()
            )
        );
    assert_eq!(parse_root(&active).unwrap(), preflight.workspace_revision());
    let schemas = preflight.graph().source_graph_schemas().unwrap();
    assert_eq!(schemas["a/provider.spx"], "semaprax.graph.v10");
    assert_eq!(schemas["z/app.spx"], "semaprax.graph.v10");
}

#[test]
fn control_parsers_reject_noncanonical_and_hostile_forms() {
    let valid = path_set(&["a.spx", "b.spx"]);
    for hostile in [
            "{".to_owned(),
            "{\"files\":[{\"path\":\"a.spx\"},{\"path\":\"b.spx\"}],\"schema\":\"semaprax.workspace-semantic-path-set.v1\"}\n".to_owned(),
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\"}\n".to_owned(),
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[],\"extra\":0}\n".to_owned(),
            format!("\u{feff}{valid}"),
            valid.replace('\n', "\r\n"),
            valid.trim_end().to_owned(),
            format!("{valid}\n"),
        ] {
            assert_code(parse_path_set(&hostile), "SPX-G174");
        }

    let deep = format!(
        "{{\"schema\":\"{}\",\"files\":[[[[[[[[[]]]]]]]]]}}\n",
        PATH_SET_SCHEMA
    );
    assert_code(parse_path_set(&deep), "SPX-G175");
    let one = vec!["a.spx".to_owned()];
    assert_code(render_path_set(&one), "SPX-G174");
    let error = assert_code(parse_path_set(&raw_path_set(&one)), "SPX-G174");
    assert_eq!(
        error[0].message,
        "Semantic Workspace requires 2..16 source files"
    );
    let over = (0..17)
        .map(|index| format!("f{index:02}.spx"))
        .collect::<Vec<_>>();
    assert_code(render_path_set(&over), "SPX-G175");
    let error = assert_code(parse_path_set(&raw_path_set(&over)), "SPX-G175");
    assert_eq!(
        error[0].message,
        "Semantic Workspace `managed_files` exceeds 16"
    );
    for paths in [
        vec!["b.spx", "a.spx"],
        vec!["a.spx", "a.spx"],
        vec!["A.spx", "b.spx"],
        vec!["a.spx", "con.spx"],
        vec!["/a.spx", "b.spx"],
        vec!["a/../b.spx", "c.spx"],
        vec!["a.spx", "a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/b.spx"],
    ] {
        let paths = paths.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_code(render_path_set(&paths), "SPX-G174");
        assert_code(parse_path_set(&raw_path_set(&paths)), "SPX-G174");
    }

    let revision = format!("sha256:{}", "1".repeat(64));
    let active = render_root(&revision).unwrap();
    for hostile in [
        "{".to_owned(),
        format!("{{\"workspace_revision\":\"{revision}\",\"schema\":\"{ROOT_SCHEMA}\"}}\n"),
        format!("{{\"schema\":\"{ROOT_SCHEMA}\"}}\n"),
        format!(
            "{{\"schema\":\"{ROOT_SCHEMA}\",\"workspace_revision\":\"{revision}\",\"extra\":0}}\n"
        ),
        format!("\u{feff}{active}"),
        active.replace('\n', "\r\n"),
        active.trim_end().to_owned(),
        format!("{active}\n"),
        format!("{{\"schema\":\"{ROOT_SCHEMA}\",\"workspace_revision\":\"sha256:ABC\"}}\n"),
    ] {
        assert_code(parse_root(&hostile), "SPX-G174");
    }
}

#[test]
fn semantic_storage_boundaries_are_exact_and_one_over() {
    let exact = "x".repeat(MAX_CONTROL_JSON_BYTES);
    for field in ["path_set_bytes", "active_bytes", "manifest_bytes"] {
        require_bounded_control_json(&exact, field).unwrap();
        let error = assert_code(
            require_bounded_control_json(&format!("{exact}x"), field),
            "SPX-G175",
        );
        assert_eq!(
            error[0].message,
            format!("Semantic Workspace `{field}` exceeds {MAX_CONTROL_JSON_BYTES}")
        );
    }

    let paths = path_set(&["a.spx", "b.spx"]);
    let first = SemanticWorkspaceSource {
        path: "a.spx".to_owned(),
        source: "x".repeat(MAX_TOTAL_SOURCE_BYTES - 1),
    };
    let second = SemanticWorkspaceSource {
        path: "b.spx".to_owned(),
        source: "x".to_owned(),
    };
    let exact_error = preflight_owned(&paths, vec![first, second])
        .err()
        .expect("invalid exact-boundary source must fail after storage admission");
    assert_ne!(exact_error[0].code, "SPX-G175");

    let first = SemanticWorkspaceSource {
        path: "a.spx".to_owned(),
        source: "x".repeat(MAX_TOTAL_SOURCE_BYTES),
    };
    let second = SemanticWorkspaceSource {
        path: "b.spx".to_owned(),
        source: "x".to_owned(),
    };
    let error = assert_code(preflight_owned(&paths, vec![first, second]), "SPX-G175");
    assert_eq!(
        error[0].message,
        "Semantic Workspace `total_source_bytes` exceeds 16777216"
    );
}

fn manifest_fact(path: &str, bytes: usize) -> SemanticWorkspaceFileFact {
    SemanticWorkspaceFileFact {
        path: path.to_owned(),
        source_graph_schema: "semaprax.graph.v10".to_owned(),
        source_revision: format!("sha256:{}", "1".repeat(64)),
        source_digest: format!("sha256:{}", "2".repeat(64)),
        bytes,
        source: String::new(),
    }
}

#[test]
fn typed_cardinality_and_manifest_byte_replay_fail_before_unbounded_work() {
    let paths = path_set(&["a.spx", "b.spx"]);
    for sources in [
        vec![SemanticWorkspaceSource {
            path: "not-canonical".to_owned(),
            source: String::new(),
        }],
        (0..4096)
            .map(|index| SemanticWorkspaceSource {
                path: format!("NOT-CANONICAL-{index}"),
                source: String::new(),
            })
            .collect::<Vec<_>>(),
    ] {
        let error = assert_code(preflight_owned(&paths, sources), "SPX-G174");
        assert_eq!(
            error[0].message,
            "semantic workspace owned sources disagree with the canonical path set"
        );
    }

    let exact = vec![
        manifest_fact("a.spx", MAX_TOTAL_SOURCE_BYTES - 1),
        manifest_fact("b.spx", 1),
    ];
    let exact_manifest = render_manifest(&exact).unwrap();
    let replayed = parse_manifest(&exact_manifest).unwrap();
    assert_eq!(
        replayed.iter().map(|fact| fact.bytes()).sum::<usize>(),
        MAX_TOTAL_SOURCE_BYTES
    );

    let over = vec![
        manifest_fact("a.spx", MAX_TOTAL_SOURCE_BYTES),
        manifest_fact("b.spx", 1),
    ];
    assert_code(render_manifest(&over), "SPX-G175");
    let error = assert_code(parse_manifest(&raw_manifest(&over)), "SPX-G175");
    assert_eq!(
        error[0].message,
        "Semantic Workspace `total_source_bytes` exceeds 16777216"
    );

    let one = [manifest_fact("a.spx", 1)];
    assert_code(render_manifest(&one), "SPX-G174");
    let error = assert_code(parse_manifest(&raw_manifest(&one)), "SPX-G174");
    assert_eq!(
        error[0].message,
        "Semantic Workspace requires 2..16 source files"
    );
    let seventeen = (0..17)
        .map(|index| manifest_fact(&format!("f{index:02}.spx"), 1))
        .collect::<Vec<_>>();
    assert_code(render_manifest(&seventeen), "SPX-G175");
    let error = assert_code(parse_manifest(&raw_manifest(&seventeen)), "SPX-G175");
    assert_eq!(
        error[0].message,
        "Semantic Workspace `managed_files` exceeds 16"
    );
}

#[test]
fn preflight_replay_rejects_malformed_reordered_and_substituted_manifest() {
    let paths = path_set(&["a/provider.spx", "z/app.spx"]);
    let mut malformed = preflight_owned(&paths, importing_sources()).unwrap();
    malformed.manifest = "{\n".to_owned();
    assert_code(validate_preflight_replay(&malformed), "SPX-G174");

    let mut reordered = preflight_owned(&paths, importing_sources()).unwrap();
    let value: Value = serde_json::from_str(reordered.manifest.trim_end()).unwrap();
    reordered.manifest = format!(
        "{{\"files\":{},\"schema\":{}}}\n",
        serde_json::to_string(&value["files"]).unwrap(),
        quote_json(MANIFEST_SCHEMA)
    );
    let error = assert_code(validate_preflight_replay(&reordered), "SPX-G174");
    assert_eq!(
        error[0].message,
        "semantic workspace manifest is not canonical semaprax.workspace-semantic-manifest.v1"
    );

    let mut substituted = preflight_owned(&paths, importing_sources()).unwrap();
    let mut facts = parse_manifest(substituted.manifest()).unwrap();
    facts[0].source_digest = format!("sha256:{}", "f".repeat(64));
    substituted.manifest = render_manifest(&facts).unwrap();
    let error = assert_code(validate_preflight_replay(&substituted), "SPX-G174");
    assert_eq!(
        error[0].message,
        "Semantic Workspace manifest facts disagree with independent grammar replay"
    );
}

#[test]
fn per_file_graph_v10_through_v17_facts_replay_exactly() {
    let cases = [
            (
                "v10.spx",
                "module schema.v10; @id(\"v10.main\") fn main()->i64{0}",
                "semaprax.graph.v10",
            ),
            (
                "v11.spx",
                "module schema.v11; @id(\"v11.target\") fn target(input:Option<i64>)->Option<bool>{let checked=input?;Option<bool>::Some { value: checked>0 }} @id(\"v11.main\") fn main()->i64{0}",
                "semaprax.graph.v11",
            ),
            (
                "v12.spx",
                "module schema.v12; @id(\"v12.box\") record Box<T>{@id(\"v12.box.value\") value:T,} @id(\"v12.main\") fn main()->i64{0}",
                "semaprax.graph.v12",
            ),
            (
                "v13.spx",
                "module schema.v13; @id(\"v13.box\") record Box{@id(\"v13.box.value\") value:i64,} @id(\"v13.read\") fn read(input:Box)->i64{match input { Box { value } => value, }} @id(\"v13.main\") fn main()->i64{0}",
                "semaprax.graph.v13",
            ),
            (
                "v14.spx",
                "module schema.v14; @id(\"v14.target\") fn target<T>()->bool{true} @id(\"v14.main\") fn main()->i64{if target<i64>(){1}else{0}}",
                "semaprax.graph.v14",
            ),
            (
                "v15.spx",
                "module schema.v15; @id(\"v15.main\") fn main()->i64{let mut n=0;while n<1{n=n+1;n<1}n}",
                "semaprax.graph.v15",
            ),
            (
                "v16.spx",
                "module schema.v16; @id(\"v16.pick\") fn pick(value:i64)->i64{match value { 0 => 1, _ => 2, }} @id(\"v16.main\") fn main()->i64{pick(0)}",
                "semaprax.graph.v16",
            ),
            (
                "v17.spx",
                "module schema.v17; @id(\"v17.length\") fn length(value:borrow Slice<u8>)->usize{byte_len(value)} @id(\"v17.main\") fn main()->i64{0}",
                "semaprax.graph.v17",
            ),
        ];
    let paths = cases.iter().map(|(path, _, _)| *path).collect::<Vec<_>>();
    let sources = cases
        .iter()
        .map(|(path, source, _)| canonical_source(path, source))
        .collect::<Vec<_>>();
    let preflight = preflight_owned(&path_set(&paths), sources).unwrap();
    assert_eq!(
        preflight
            .files()
            .iter()
            .map(|file| (file.path(), file.source_graph_schema()))
            .collect::<Vec<_>>(),
        cases
            .iter()
            .map(|(path, _, schema)| (*path, *schema))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        render_manifest(&parse_manifest(preflight.manifest()).unwrap()).unwrap(),
        preflight.manifest()
    );
}

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-semantic-workspace-ordinary-preservation-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct SemanticFixture {
    root: TempRoot,
    path_set: PathBuf,
    sources: Vec<SemanticWorkspaceSource>,
    path_set_bytes: String,
}

impl SemanticFixture {
    fn new() -> Self {
        let root = TempRoot::new();
        let sources = importing_sources();
        for source in &sources {
            let destination = root.0.join(&source.path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::write(destination, &source.source).unwrap();
        }
        let path_set_bytes = path_set(&["a/provider.spx", "z/app.spx"]);
        let path_set = root.0.join("semantic-paths.json");
        std::fs::write(&path_set, &path_set_bytes).unwrap();
        Self {
            root,
            path_set,
            sources,
            path_set_bytes,
        }
    }

    fn control(&self) -> PathBuf {
        self.root.0.join(".semaprax-workspace")
    }

    fn active(&self) -> PathBuf {
        self.control().join("ACTIVE")
    }

    fn generation(&self, revision: &str) -> PathBuf {
        self.control()
            .join("generations")
            .join(revision.strip_prefix("sha256:").unwrap())
    }

    fn expected_preflight(&self) -> SemanticWorkspacePreflight {
        preflight_owned(
            &self.path_set_bytes,
            self.sources
                .iter()
                .map(|source| SemanticWorkspaceSource {
                    path: source.path.clone(),
                    source: source.source.clone(),
                })
                .collect(),
        )
        .unwrap()
    }

    fn assert_inputs_unchanged(&self) {
        assert_eq!(
            std::fs::read_to_string(&self.path_set).unwrap(),
            self.path_set_bytes
        );
        for source in &self.sources {
            assert_eq!(
                std::fs::read_to_string(self.root.0.join(&source.path)).unwrap(),
                source.source
            );
        }
    }
}

fn assert_lock_reacquirable(fixture: &SemanticFixture) {
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.control().join("LOCK"))
        .unwrap();
    fs2::FileExt::try_lock_exclusive(&lock).expect("initializer must release the lock");
    fs2::FileExt::unlock(&lock).unwrap();
}

fn replace_with_same_bytes(path: &Path) {
    let bytes = std::fs::read(path).unwrap();
    std::fs::remove_file(path).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn semantic_initialize_publishes_exact_generation_and_all_six_edge_families() {
    let first = SemanticFixture::new();
    let expected = first.expected_preflight();
    let revision = initialize_from_preflight(&first.root.0, &first.path_set).unwrap();
    assert_eq!(revision, expected.workspace_revision());
    assert_eq!(
        std::fs::read_to_string(first.active()).unwrap(),
        render_root(&revision).unwrap()
    );
    let generation = first.generation(&revision);
    assert_eq!(
        std::fs::read_to_string(generation.join("manifest.json")).unwrap(),
        expected.manifest()
    );
    for file in expected.files() {
        assert_eq!(
            std::fs::read_to_string(generation.join("files").join(file.path())).unwrap(),
            file.source()
        );
    }
    first.assert_inputs_unchanged();

    let graph = crate::workspace_graph::snapshot(&first.root.0, "semantic.app").unwrap();
    assert_eq!(graph.workspace_revision(), revision);
    let kinds = graph
        .edges()
        .iter()
        .map(|edge| edge.kind())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        kinds,
        BTreeSet::from([
            "call",
            "capability_authority",
            "effect_requirement",
            "function_import",
            "type_import",
            "type_reference",
        ])
    );
    assert_lock_reacquirable(&first);

    let second = SemanticFixture::new();
    let replayed = initialize_from_preflight(&second.root.0, &second.path_set).unwrap();
    let replayed_graph = crate::workspace_graph::snapshot(&second.root.0, "semantic.app").unwrap();
    assert_eq!(replayed, revision);
    assert_eq!(replayed_graph.to_json(), graph.to_json());
    assert_lock_reacquirable(&second);
}

#[test]
fn semantic_preflight_failures_and_post_preflight_input_replacement_publish_no_control() {
    let malformed = SemanticFixture::new();
    std::fs::write(
        malformed.root.0.join("z/app.spx"),
        "module semantic.app; this is not source\n",
    )
    .unwrap();
    assert!(initialize_from_preflight(&malformed.root.0, &malformed.path_set).is_err());
    assert!(!malformed.control().exists());

    for target in ["source", "path-set"] {
        let fixture = SemanticFixture::new();
        let path = if target == "source" {
            fixture.root.0.join("z/app.spx")
        } else {
            fixture.path_set.clone()
        };
        let error =
            initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                if matches!(point, SemanticInitializePoint::SemanticPreflightComplete) {
                    replace_with_same_bytes(&path);
                }
            })
            .unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G153", "{target}");
        assert!(!fixture.control().exists(), "{target}");
    }

    let fixture = SemanticFixture::new();
    let source = fixture.root.0.join("z/app.spx");
    let donor = fixture.root.0.join("source-donor.spx");
    std::fs::write(&donor, std::fs::read(&source).unwrap()).unwrap();
    let error = initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
        if matches!(point, SemanticInitializePoint::SemanticPreflightComplete) {
            std::fs::remove_file(&source).unwrap();
            std::fs::hard_link(&donor, &source).unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G153");
    assert!(!fixture.control().exists());
    assert_eq!(
        std::fs::read(&source).unwrap(),
        std::fs::read(&donor).unwrap()
    );
}

#[test]
fn semantic_initializer_preserves_foreign_control_generation_active_and_staging() {
    for kind in ["file", "directory"] {
        let fixture = SemanticFixture::new();
        let control = fixture.control();
        if kind == "file" {
            std::fs::write(&control, "foreign-control\n").unwrap();
        } else {
            std::fs::create_dir(&control).unwrap();
            std::fs::write(control.join("foreign"), "preserve\n").unwrap();
        }
        let error = initialize_from_preflight(&fixture.root.0, &fixture.path_set).unwrap_err();
        assert_eq!(error[0].code, "SPX-I209");
        if kind == "file" {
            assert_eq!(
                std::fs::read_to_string(&control).unwrap(),
                "foreign-control\n"
            );
        } else {
            assert_eq!(
                std::fs::read_to_string(control.join("foreign")).unwrap(),
                "preserve\n"
            );
        }
    }

    let fixture = SemanticFixture::new();
    let revision = fixture.expected_preflight().workspace_revision().to_owned();
    let generation = fixture.generation(&revision);
    let error = initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
        if matches!(point, SemanticInitializePoint::GenerationDestinationChecked) {
            std::fs::write(&generation, "foreign-generation\n").unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I211");
    assert_eq!(
        std::fs::read_to_string(&generation).unwrap(),
        "foreign-generation\n"
    );
    assert!(!fixture.active().exists());
    assert_lock_reacquirable(&fixture);

    let fixture = SemanticFixture::new();
    let active = fixture.active();
    let error = initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
        if matches!(point, SemanticInitializePoint::ActiveDestinationChecked) {
            std::fs::write(&active, "foreign-active\n").unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert_eq!(
        std::fs::read_to_string(&active).unwrap(),
        "foreign-active\n"
    );
    assert_lock_reacquirable(&fixture);

    let fixture = SemanticFixture::new();
    let foreign_slot = fixture.control().join("staging/31");
    let error = initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
        if matches!(point, SemanticInitializePoint::GenerationBeforeRename) {
            std::fs::create_dir(&foreign_slot).unwrap();
            std::fs::write(foreign_slot.join("foreign"), "preserve\n").unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert_eq!(
        std::fs::read_to_string(foreign_slot.join("foreign")).unwrap(),
        "preserve\n"
    );
    assert!(!fixture.active().exists());
    assert_lock_reacquirable(&fixture);
}

#[test]
fn semantic_staging_slot_zero_race_and_all_slots_exhaustion_fail_closed() {
    let fixture = SemanticFixture::new();
    let slot_zero = fixture.control().join("staging/0");
    let error = initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
        if matches!(point, SemanticInitializePoint::SemanticStagingReady) {
            std::fs::create_dir(&slot_zero).unwrap();
            std::fs::write(slot_zero.join("foreign"), "slot-zero\n").unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G153");
    assert_eq!(
        std::fs::read_to_string(slot_zero.join("foreign")).unwrap(),
        "slot-zero\n"
    );
    assert!(!fixture.active().exists());
    assert_lock_reacquirable(&fixture);

    let fixture = SemanticFixture::new();
    let staging = fixture.control().join("staging");
    let error = initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
        if matches!(point, SemanticInitializePoint::SemanticStagingReady) {
            for ordinal in 0..32 {
                let slot = staging.join(ordinal.to_string());
                std::fs::create_dir(&slot).unwrap();
                std::fs::write(slot.join("foreign"), ordinal.to_string()).unwrap();
            }
        }
    })
    .unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G175");
    assert_eq!(
        error[0].message,
        "Semantic Workspace `staging_attempts` exceeds 32"
    );
    for ordinal in 0..32 {
        assert_eq!(
            std::fs::read_to_string(staging.join(ordinal.to_string()).join("foreign")).unwrap(),
            ordinal.to_string()
        );
    }
    assert!(!fixture.active().exists());
    assert_lock_reacquirable(&fixture);
}

#[test]
fn semantic_final_boundary_rechecks_sources_paths_control_generation_and_active_stage() {
    for target in [
        "source",
        "path-set",
        "control",
        "generation",
        "active-stage",
    ] {
        let fixture = SemanticFixture::new();
        let revision = fixture.expected_preflight().workspace_revision().to_owned();
        let error =
            initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                if !matches!(point, SemanticInitializePoint::ActiveBeforeRename) {
                    return;
                }
                match target {
                    "source" => {
                        replace_with_same_bytes(&fixture.root.0.join("z/app.spx"));
                    }
                    "path-set" => replace_with_same_bytes(&fixture.path_set),
                    "control" => {
                        std::fs::write(fixture.control().join("foreign"), "drift\n").unwrap();
                    }
                    "generation" => replace_with_same_bytes(
                        &fixture.generation(&revision).join("manifest.json"),
                    ),
                    "active-stage" => replace_with_same_bytes(&fixture.control().join("staging/0")),
                    _ => unreachable!(),
                }
            })
            .unwrap_err();
        assert_eq!(error.len(), 1, "{target}");
        assert_eq!(error[0].code, "SPX-G153", "{target}");
        assert!(!fixture.active().exists(), "{target}");
        assert_lock_reacquirable(&fixture);
    }
}

#[test]
fn semantic_post_pivot_drift_is_i212_and_releases_the_lock() {
    for target in ["active", "generation"] {
        let fixture = SemanticFixture::new();
        let revision = fixture.expected_preflight().workspace_revision().to_owned();
        let result =
            initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                if matches!(point, SemanticInitializePoint::ActiveRelocated) {
                    if target == "active" {
                        replace_with_same_bytes(&fixture.active());
                    } else {
                        replace_with_same_bytes(
                            &fixture.generation(&revision).join("manifest.json"),
                        );
                    }
                }
            });
        let error = match result {
            Err(error) => error,
            Ok(revision) => {
                panic!("post-pivot {target} drift unexpectedly succeeded as {revision}")
            }
        };
        assert_eq!(error.len(), 1, "{target}");
        assert_eq!(error[0].code, "SPX-I212", "{target}");
        assert!(fixture.active().exists(), "{target}");
        assert_lock_reacquirable(&fixture);
    }
}

#[test]
fn semantic_and_ordinary_initializers_reject_the_other_control_schema() {
    let semantic = SemanticFixture::new();
    let ordinary_bytes = "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n";
    std::fs::write(&semantic.path_set, ordinary_bytes).unwrap();
    let error = initialize_from_preflight(&semantic.root.0, &semantic.path_set).unwrap_err();
    assert_eq!(error[0].code, "SPX-G174");
    assert!(!semantic.control().exists());

    let ordinary = SemanticFixture::new();
    let error = crate::workspace::initialize(&ordinary.root.0, &ordinary.path_set).unwrap_err();
    assert_eq!(error[0].code, "SPX-G150");
    assert!(!ordinary.control().exists());
}

#[test]
fn ordinary_workspace_initializer_still_rejects_imports_without_control_writes() {
    let root = TempRoot::new();
    let sources = importing_sources();
    for source in &sources {
        let destination = root.0.join(&source.path);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(destination, &source.source).unwrap();
    }
    let ordinary_path_set = root.0.join("paths.json");
    let path_set_bytes = "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n";
    std::fs::write(&ordinary_path_set, path_set_bytes).unwrap();
    let before = sources
        .iter()
        .map(|source| {
            (
                source.path.clone(),
                std::fs::read(root.0.join(&source.path)).unwrap(),
            )
        })
        .collect::<Vec<_>>();

    let error = crate::workspace::initialize(&root.0, &ordinary_path_set).unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G172");
    assert_eq!(
        error[0].message,
        "source module imports require Workspace Semantic Graph resolution"
    );
    assert!(!root.0.join(".semaprax-workspace").exists());
    assert_eq!(
        std::fs::read_to_string(&ordinary_path_set).unwrap(),
        path_set_bytes
    );
    assert_eq!(
        before,
        sources
            .iter()
            .map(|source| (
                source.path.clone(),
                std::fs::read(root.0.join(&source.path)).unwrap()
            ))
            .collect::<Vec<_>>()
    );
}
