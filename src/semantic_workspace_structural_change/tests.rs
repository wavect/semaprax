use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::*;
use fs2::FileExt;
use sha2::{Digest, Sha256};

static MANAGED_SERIAL: AtomicU64 = AtomicU64::new(0);
const TEST_MAX_EVIDENCE_BYTES: usize = 1_048_576;

pub(super) struct BaseFixture {
    pub(super) revision: String,
    pub(super) manifest_bytes: usize,
    pub(super) sources: Vec<workspace::WorkspaceSemanticSource>,
    pub(super) graph: workspace_graph::WorkspaceGraphBuild,
}

struct ManagedFixture {
    root: PathBuf,
    proposal_path: PathBuf,
    proposal_source: String,
}

impl ManagedFixture {
    fn new(label: &str) -> Self {
        let base = base_fixture();
        let proposal_source = mixed_proposal(&base);
        let serial = MANAGED_SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-semantic-workspace-structural-change-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut paths = Vec::new();
        for source in &base.sources {
            let destination = root.join(&source.path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::write(&destination, &source.source).unwrap();
            paths.push(source.path.clone());
        }
        paths.sort();
        let path_set = root.join("paths.json");
        std::fs::write(
            &path_set,
            semantic_workspace::render_path_set(&paths).unwrap(),
        )
        .unwrap();
        assert_eq!(
            semantic_workspace::initialize(&root, &path_set).unwrap(),
            base.revision
        );
        let proposal_path = root.join("structural-change.json");
        std::fs::write(&proposal_path, &proposal_source).unwrap();
        Self {
            root,
            proposal_path,
            proposal_source,
        }
    }

    fn inventory(&self) -> Vec<(String, bool, Vec<u8>)> {
        fn walk(root: &Path, path: &Path, facts: &mut Vec<(String, bool, Vec<u8>)>) {
            let mut entries = std::fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap())
                .collect::<Vec<_>>();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                let metadata = std::fs::symlink_metadata(&path).unwrap();
                if metadata.is_dir() {
                    facts.push((relative, true, Vec::new()));
                    walk(root, &path, facts);
                } else {
                    facts.push((relative, false, std::fs::read(&path).unwrap()));
                }
            }
        }

        let mut facts = Vec::new();
        walk(&self.root, &self.root, &mut facts);
        facts
    }

    fn assert_exclusive_reacquire(&self) {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.root.join(".semaprax-workspace/LOCK"))
            .unwrap();
        FileExt::try_lock_exclusive(&lock).unwrap();
        FileExt::unlock(&lock).unwrap();
    }

    fn raw_inventory(&self) -> Vec<(String, bool, Vec<u8>)> {
        self.inventory()
            .into_iter()
            .filter(|(path, _, _)| !path.starts_with(".semaprax-workspace"))
            .collect()
    }

    fn authenticated_paths_and_storage(&self) -> (Vec<String>, usize, usize) {
        let mut authority = workspace::acquire_semantic_change_read(&self.root).unwrap();
        let retained = authority.retained_generations();
        let staging = authority.staging_attempts();
        let mut paths = authority
            .take_sources()
            .into_iter()
            .map(|source| source.path)
            .collect::<Vec<_>>();
        paths.sort();
        let _graph = authority.take_graph().unwrap();
        authority.finish(Ok(())).unwrap();
        (paths, retained, staging)
    }
}

impl Drop for ManagedFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn raw_sha(source: &str) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}

fn diagnostic(result: Result<impl Sized, Vec<Diagnostic>>) -> Diagnostic {
    let diagnostics = match result {
        Ok(_) => panic!("expected failure"),
        Err(diagnostics) => diagnostics,
    };
    assert_eq!(diagnostics.len(), 1);
    diagnostics.into_iter().next().unwrap()
}

fn read_only_failure<T>(
    fixture: &ManagedFixture,
    operation: impl FnOnce() -> Result<T, Vec<Diagnostic>>,
) -> Diagnostic {
    let before = fixture.inventory();
    let error = diagnostic(operation());
    assert_eq!(fixture.inventory(), before);
    fixture.assert_exclusive_reacquire();
    error
}

fn application_fixture(label: &str) -> (ManagedFixture, PathBuf) {
    let fixture = ManagedFixture::new(label);
    let artifacts = generate_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
    (fixture, evidence_path)
}

fn spawn_structural_apply_process(
    fixture: &ManagedFixture,
    evidence_path: &Path,
    boundary: &str,
) -> (Child, PathBuf) {
    let ready = fixture
        .root
        .join(format!("structural-apply-{boundary}.ready"));
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "semantic_workspace_structural_change::tests::structural_apply_process_child",
            "--nocapture",
        ])
        .env("SEMAPRAX_STRUCTURAL_APPLY_CHILD", "1")
        .env("SEMAPRAX_STRUCTURAL_APPLY_ROOT", &fixture.root)
        .env("SEMAPRAX_STRUCTURAL_APPLY_PROPOSAL", &fixture.proposal_path)
        .env("SEMAPRAX_STRUCTURAL_APPLY_EVIDENCE", evidence_path)
        .env("SEMAPRAX_STRUCTURAL_APPLY_BOUNDARY", boundary)
        .env("SEMAPRAX_STRUCTURAL_APPLY_READY", &ready)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !matches!(std::fs::read(&ready), Ok(bytes) if bytes == b"ready\n") {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("structural apply child exited before {boundary}: {status}");
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("structural apply child did not reach {boundary}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    (child, ready)
}

fn directory_names(path: &Path) -> Vec<String> {
    let mut names = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn apply_point_name(point: StructuralApplyPoint) -> &'static str {
    match point {
        StructuralApplyPoint::ProposalOwned => "proposal_owned",
        StructuralApplyPoint::EvidenceOwned => "evidence_owned",
        StructuralApplyPoint::AfterReplay => "after_replay",
        StructuralApplyPoint::ReceiptRendered => "receipt_rendered",
        StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
            workspace::GenerationPoint::AfterSlotCreate,
        )) => "generation_after_slot_create",
        StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
            workspace::GenerationPoint::AfterManifestWrite,
        )) => "generation_after_manifest_write",
        StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
            workspace::GenerationPoint::AfterFilesWrite,
        )) => "generation_after_files_write",
        StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
            workspace::GenerationPoint::BeforeStageValidation,
        )) => "generation_before_stage_validation",
        StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
            workspace::GenerationPoint::BeforeGenerationPublish,
        )) => "generation_before_publish",
        StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
            workspace::GenerationPoint::DestinationChecked,
        )) => "generation_destination_checked",
        StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
            workspace::GenerationPoint::AfterGenerationPublish,
        )) => "generation_after_publish",
        StructuralApplyPoint::Workspace(
            workspace::SemanticChangeApplyPoint::AfterCandidatePrepared,
        ) => "after_candidate_prepared",
        StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::AfterActiveStaged) => {
            "after_active_staged"
        }
        StructuralApplyPoint::Workspace(
            workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck,
        ) => "before_first_final_check",
        StructuralApplyPoint::Workspace(
            workspace::SemanticChangeApplyPoint::BeforeSecondFinalCheck,
        ) => "before_second_final_check",
        StructuralApplyPoint::Workspace(
            workspace::SemanticChangeApplyPoint::BeforeActiveReplace,
        ) => "before_active_replace",
        StructuralApplyPoint::Workspace(
            workspace::SemanticChangeApplyPoint::AfterActiveReplace,
        ) => "after_active_replace",
    }
}

fn replace_owned_path(path: &Path, replacement: &Path) {
    std::fs::remove_file(path).unwrap();
    std::fs::rename(replacement, path).unwrap();
}

fn object_after<'a>(source: &'a str, marker: &str) -> &'a str {
    let marker = source.find(marker).unwrap() + marker.len();
    let start = marker + source[marker..].find('{').unwrap();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in source[start..].bytes().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..=start + offset];
                }
            }
            _ => {}
        }
    }
    panic!("object after marker is unterminated")
}

fn replace_scalar_field(object: &str, field: &str, replacement: &str) -> String {
    let needle = format!("\"{field}\":");
    let start = object.find(&needle).unwrap() + needle.len();
    let bytes = object.as_bytes();
    let end = if bytes[start] == b'"' {
        let mut index = start + 1;
        let mut escaped = false;
        loop {
            if escaped {
                escaped = false;
            } else if bytes[index] == b'\\' {
                escaped = true;
            } else if bytes[index] == b'"' {
                break index + 1;
            }
            index += 1;
        }
    } else {
        let mut index = start;
        while !matches!(bytes[index], b',' | b'}') {
            index += 1;
        }
        index
    };
    format!("{}{}{}", &object[..start], replacement, &object[end..])
}

fn remove_nonfirst_scalar_field(object: &str, field: &str) -> String {
    let needle = format!(",\"{field}\":");
    let start = object.find(&needle).unwrap();
    let value_start = start + needle.len();
    let bytes = object.as_bytes();
    let end = if bytes[value_start] == b'"' {
        let mut index = value_start + 1;
        let mut escaped = false;
        loop {
            if escaped {
                escaped = false;
            } else if bytes[index] == b'\\' {
                escaped = true;
            } else if bytes[index] == b'"' {
                break index + 1;
            }
            index += 1;
        }
    } else {
        let mut index = value_start;
        while !matches!(bytes[index], b',' | b'}') {
            index += 1;
        }
        index
    };
    format!("{}{}", &object[..start], &object[end..])
}

fn duplicate_first_field(object: &str) -> String {
    let comma = object.find(',').unwrap();
    format!("{{{},{}", &object[1..comma], &object[1..])
}

fn reorder_first_two_fields(object: &str) -> String {
    let first_comma = object.find(',').unwrap();
    let second_end = object[first_comma + 1..]
        .find(',')
        .map_or(object.len() - 1, |offset| first_comma + 1 + offset);
    format!(
        "{{{},{}{}",
        &object[first_comma + 1..second_end],
        &object[1..first_comma],
        &object[second_end..]
    )
}

fn nested_shape_mutations(
    source: &str,
    marker: &str,
    first_field: &str,
    second_field: &str,
) -> Vec<String> {
    let object = object_after(source, marker);
    [
        remove_nonfirst_scalar_field(object, second_field),
        object.replacen('{', "{\"extra\":0,", 1),
        duplicate_first_field(object),
        reorder_first_two_fields(object),
        replace_scalar_field(object, first_field, "[]"),
    ]
    .into_iter()
    .map(|mutation| source.replacen(object, &mutation, 1))
    .collect()
}

fn canonical(source: &str, path: &str) -> String {
    crate::format::canonical(&crate::parse(source, path).unwrap())
}

fn provider() -> String {
    canonical(
        r#"
module structural.provider;
permit { audit.old }

@id("structural.point")
record Point { @id("structural.point.value") value: i64, }

@id("structural.work")
fn work(value: Point) -> i64 uses { audit.old } { value.value }

fn helper() -> i64 { 1 }

@id("structural.provider.main")
fn main() -> i64 { helper() }
"#,
        "a/provider.spx",
    )
}

fn consumer() -> String {
    canonical(
        r#"
module structural.consumer;
use type @id("structural.point") from structural.provider as Point;
use function @id("structural.work") from structural.provider as work;
permit { audit.old, audit.new }

@id("structural.consume")
fn consume() -> i64 uses { audit.old, audit.new } { work(Point { value: 3 }) }

@id("structural.consumer.main")
fn main() -> i64 uses { audit.old, audit.new } { consume() }
"#,
        "m/consumer.spx",
    )
}

fn island() -> String {
    canonical(
        r#"
module structural.island;
permit { island.old }

@id("structural.island.value")
fn value() -> i64 { 1 }

@id("structural.island.main")
fn main() -> i64 { value() }
"#,
        "n/island.spx",
    )
}

fn entry() -> String {
    canonical(
        r#"
module structural.entry;
use type @id("structural.point") from structural.provider as Point;
use function @id("structural.work") from structural.provider as work;
use function @id("structural.consume") from structural.consumer as consume;
permit { audit.old, audit.new }

@id("structural.entry.main")
fn main() -> i64 uses { audit.old, audit.new } { work(Point { value: 1 }) }
"#,
        "z/entry.spx",
    )
}

fn entry_replacement() -> String {
    canonical(
        r#"
module structural.entry;
use type @id("structural.point") from structural.provider as Point;
use function @id("structural.work") from structural.provider as work;
use function @id("structural.consume") from structural.consumer as consume;
permit { audit.old, audit.new }

@id("structural.entry.main")
fn main() -> i64 uses { audit.old, audit.new } { work(Point { value: 2 }) + consume() }
"#,
        "z/entry.spx",
    )
}

fn created() -> String {
    canonical(
        r#"
module structural.created;
permit { created.capability }

fn helper() -> i64 { 7 }

@id("structural.created.main")
fn main() -> i64 uses { created.capability } { helper() }
"#,
        "b/created.spx",
    )
}

pub(super) fn base_fixture() -> BaseFixture {
    base_fixture_with_order(false)
}

fn base_fixture_with_order(reverse_sources: bool) -> BaseFixture {
    let mut sources = vec![
        semantic_workspace::SemanticWorkspaceSource {
            path: "a/provider.spx".to_owned(),
            source: provider(),
        },
        semantic_workspace::SemanticWorkspaceSource {
            path: "m/consumer.spx".to_owned(),
            source: consumer(),
        },
        semantic_workspace::SemanticWorkspaceSource {
            path: "n/island.spx".to_owned(),
            source: island(),
        },
        semantic_workspace::SemanticWorkspaceSource {
            path: "z/entry.spx".to_owned(),
            source: entry(),
        },
    ];
    let mut paths = sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    paths.sort();
    if reverse_sources {
        sources.reverse();
    }
    let path_set = semantic_workspace::render_path_set(&paths).unwrap();
    let preflight = semantic_workspace::preflight_owned_for_change(
        &path_set,
        sources,
        semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
    )
    .unwrap();
    let (files, manifest, revision, graph) = preflight.into_snapshot_parts();
    let sources = files
        .into_iter()
        .map(|file| {
            let (path, source_graph_schema, source_revision, source_digest, source) =
                file.into_parts();
            workspace::WorkspaceSemanticSource {
                path,
                source_graph_schema,
                source_revision,
                source_digest,
                source,
            }
        })
        .collect();
    BaseFixture {
        revision,
        manifest_bytes: manifest.len(),
        sources,
        graph,
    }
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

fn binding(source: &workspace::WorkspaceSemanticSource) -> String {
    format!(
        ",\"base_source_graph_schema\":{},\"base_source_revision\":{},\"base_source_digest\":{}",
        quoted(&source.source_graph_schema),
        quoted(&source.source_revision),
        quoted(&source.source_digest)
    )
}

fn create_operation(path: &str, source: &str) -> String {
    format!(
        "{{\"kind\":\"create\",\"path\":{},\"source\":{}}}",
        quoted(path),
        quoted(source)
    )
}

fn delete_operation(source: &workspace::WorkspaceSemanticSource) -> String {
    format!(
        "{{\"kind\":\"delete\",\"path\":{}{}}}",
        quoted(&source.path),
        binding(source)
    )
}

fn move_operation(source: &workspace::WorkspaceSemanticSource, to_path: &str) -> String {
    format!(
        "{{\"kind\":\"move\",\"from_path\":{},\"to_path\":{}{}}}",
        quoted(&source.path),
        quoted(to_path),
        binding(source)
    )
}

fn replace_operation(source: &workspace::WorkspaceSemanticSource, replacement: &str) -> String {
    format!(
        "{{\"kind\":\"replace\",\"path\":{}{},\"replacement_source\":{}}}",
        quoted(&source.path),
        binding(source),
        quoted(replacement)
    )
}

fn proposal(revision: &str, entry_module: &str, operations: &[String]) -> String {
    format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":{},\"entry_module\":{},\"operations\":[{}]}}\n",
            quoted(revision),
            quoted(entry_module),
            operations.join(",")
        )
}

fn source<'a>(base: &'a BaseFixture, path: &str) -> &'a workspace::WorkspaceSemanticSource {
    base.sources
        .iter()
        .find(|source| source.path == path)
        .unwrap()
}

pub(super) fn mixed_proposal(base: &BaseFixture) -> String {
    proposal(
        &base.revision,
        "structural.entry",
        &[
            create_operation("b/created.spx", &created()),
            delete_operation(source(base, "n/island.spx")),
            move_operation(source(base, "a/provider.spx"), "c/provider.spx"),
            replace_operation(source(base, "z/entry.spx"), &entry_replacement()),
        ],
    )
}

fn error_code<T>(result: Result<T, Vec<Diagnostic>>) -> String {
    let diagnostics = match result {
        Ok(_) => panic!("expected failure"),
        Err(diagnostics) => diagnostics,
    };
    assert_eq!(diagnostics.len(), 1);
    diagnostics[0].code.to_owned()
}

#[test]
fn proposal_kat_has_all_four_operations_and_frozen_order() {
    let base = base_fixture();
    let proposal_source = mixed_proposal(&base);
    let parsed = parse_proposal(&proposal_source).unwrap();
    assert_eq!(parsed.source(), proposal_source);
    assert_eq!(parsed.base_workspace_revision(), base.revision);
    assert_eq!(parsed.entry_module(), "structural.entry");
    assert!(matches!(
        parsed.operations(),
        [
            SemanticWorkspaceStructuralOperation::Create { path, .. },
            SemanticWorkspaceStructuralOperation::Delete { .. },
            SemanticWorkspaceStructuralOperation::Move { .. },
            SemanticWorkspaceStructuralOperation::Replace { .. }
        ] if path == "b/created.spx"
    ));
    let digest = format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(proposal_source.as_bytes()))
    );
    assert_eq!(
        digest,
        "sha256:b13dcbf801bdb0fe1cd05a5cff26b58085bc32a576d9a5b8fc7264755c5548f8"
    );

    let mut reordered = parsed.source().to_owned();
    reordered = reordered.replace("{\"kind\":\"create\",\"path\"", "{\"path\"");
    assert_eq!(error_code(parse_proposal(&reordered)), "SPX-G188");
    let reversed = proposal(
        &base.revision,
        "structural.entry",
        &[
            move_operation(source(&base, "a/provider.spx"), "c/provider.spx"),
            create_operation("b/created.spx", &created()),
        ],
    );
    assert_eq!(error_code(parse_proposal(&reversed)), "SPX-G188");
}

#[test]
fn endpoint_conflicts_stale_bindings_and_structural_premise_fail_closed() {
    let base = base_fixture();
    let provider = source(&base, "a/provider.spx");
    let island = source(&base, "n/island.spx");
    let entry_source = source(&base, "z/entry.spx");
    let replacement = entry_replacement();
    let cases = [
        (
            vec![create_operation("a/provider.spx", &created())],
            "SPX-G189",
        ),
        (vec![move_operation(provider, "m/consumer.spx")], "SPX-G189"),
        (
            vec![
                delete_operation(provider),
                move_operation(provider, "c/provider.spx"),
            ],
            "SPX-G190",
        ),
        (
            vec![
                create_operation("c/provider.spx", &created()),
                move_operation(provider, "c/provider.spx"),
            ],
            "SPX-G190",
        ),
        (vec![move_operation(provider, "a/provider.spx")], "SPX-G190"),
        (
            vec![
                move_operation(provider, "n/island.spx"),
                move_operation(island, "a/provider.spx"),
            ],
            "SPX-G190",
        ),
        (
            vec![
                delete_operation(entry_source),
                replace_operation(entry_source, &replacement),
            ],
            "SPX-G190",
        ),
        (
            vec![
                move_operation(provider, "c/provider.spx"),
                replace_operation(provider, &provider.source),
            ],
            "SPX-G190",
        ),
        (
            vec![
                move_operation(provider, "n/island.spx"),
                move_operation(island, "c/island.spx"),
            ],
            "SPX-G190",
        ),
        (
            vec![
                create_operation("b/created.spx", &created()),
                replace_operation(entry_source, &entry_source.source),
            ],
            "SPX-G190",
        ),
    ];
    for (operations, expected) in cases {
        let parsed =
            parse_proposal(&proposal(&base.revision, "structural.entry", &operations)).unwrap();
        assert_eq!(
            error_code(derive_candidate_overlay(
                &base.revision,
                base_fixture().sources,
                &parsed,
            )),
            expected
        );
    }

    let replace_only = proposal(
        &base.revision,
        "structural.entry",
        &[replace_operation(
            source(&base, "z/entry.spx"),
            &replacement,
        )],
    );
    assert_eq!(error_code(parse_proposal(&replace_only)), "SPX-G190");

    let duplicate_create = proposal(
        &base.revision,
        "structural.entry",
        &[
            create_operation("b/created.spx", &created()),
            create_operation("b/created.spx", &created()),
        ],
    );
    assert_eq!(error_code(parse_proposal(&duplicate_create)), "SPX-G188");

    let mut stale = move_operation(provider, "c/provider.spx");
    stale = stale.replace(
        &provider.source_digest,
        &format!("sha256:{}", "0".repeat(64)),
    );
    let parsed = parse_proposal(&proposal(&base.revision, "structural.entry", &[stale])).unwrap();
    assert_eq!(
        error_code(derive_candidate_overlay(
            &base.revision,
            base_fixture().sources,
            &parsed,
        )),
        "SPX-G189"
    );

    let parsed = parse_proposal(&proposal(
        &base.revision,
        "structural.entry",
        &[move_operation(provider, "c/provider.spx")],
    ))
    .unwrap();
    assert_eq!(
        error_code(derive_candidate_overlay(
            &format!("sha256:{}", "f".repeat(64)),
            base_fixture().sources,
            &parsed,
        )),
        "SPX-G189"
    );
}

#[test]
fn overlay_preserves_exact_move_bytes_and_enforces_final_cardinality() {
    let base = base_fixture();
    let provider_bytes = source(&base, "a/provider.spx").source.clone();
    let parsed = parse_proposal(&mixed_proposal(&base)).unwrap();
    let overlay = derive_candidate_overlay(&base.revision, base.sources, &parsed).unwrap();
    let (base_files, candidate, changed_paths, supplied_bytes) = overlay.into_parts();
    assert_eq!(base_files.len(), 4);
    assert_eq!(
        candidate
            .iter()
            .map(|source| source.path.as_str())
            .collect::<Vec<_>>(),
        [
            "b/created.spx",
            "c/provider.spx",
            "m/consumer.spx",
            "z/entry.spx"
        ]
    );
    assert_eq!(
        candidate
            .iter()
            .find(|source| source.path == "c/provider.spx")
            .unwrap()
            .source,
        provider_bytes
    );
    assert_eq!(
        changed_paths.into_iter().collect::<Vec<_>>(),
        [
            "a/provider.spx",
            "b/created.spx",
            "c/provider.spx",
            "n/island.spx",
            "z/entry.spx"
        ]
    );
    assert_eq!(supplied_bytes, created().len() + entry_replacement().len());

    let base = base_fixture();
    let exact_operations = (0..12)
        .map(|index| create_operation(&format!("x/{index:02}.spx"), ""))
        .collect::<Vec<_>>();
    let exact = parse_proposal(&proposal(
        &base.revision,
        "structural.entry",
        &exact_operations,
    ))
    .unwrap();
    assert_eq!(
        derive_candidate_overlay(&base.revision, base.sources, &exact)
            .unwrap()
            .into_parts()
            .1
            .len(),
        16
    );
    let base = base_fixture();
    let over_operations = (0..13)
        .map(|index| create_operation(&format!("x/{index:02}.spx"), ""))
        .collect::<Vec<_>>();
    let over = parse_proposal(&proposal(
        &base.revision,
        "structural.entry",
        &over_operations,
    ))
    .unwrap();
    assert_eq!(
        error_code(derive_candidate_overlay(
            &base.revision,
            base.sources,
            &over
        )),
        "SPX-G190"
    );

    let base = base_fixture();
    let exact_min = parse_proposal(&proposal(
        &base.revision,
        "structural.entry",
        &[
            delete_operation(source(&base, "a/provider.spx")),
            delete_operation(source(&base, "n/island.spx")),
        ],
    ))
    .unwrap();
    assert_eq!(
        derive_candidate_overlay(&base.revision, base.sources, &exact_min)
            .unwrap()
            .into_parts()
            .1
            .len(),
        2
    );
    let base = base_fixture();
    let under_min = parse_proposal(&proposal(
        &base.revision,
        "structural.entry",
        &[
            delete_operation(source(&base, "a/provider.spx")),
            delete_operation(source(&base, "m/consumer.spx")),
            delete_operation(source(&base, "n/island.spx")),
        ],
    ))
    .unwrap();
    assert_eq!(
        error_code(derive_candidate_overlay(
            &base.revision,
            base.sources,
            &under_min,
        )),
        "SPX-G190"
    );
}

#[test]
fn parser_limits_are_exact_and_one_over_is_named() {
    let base = base_fixture();
    let exact_path = format!(
        "{}/{}/{}/{}.spx",
        "a".repeat(59),
        "b".repeat(59),
        "c".repeat(59),
        "d".repeat(56)
    );
    assert_eq!(exact_path.len(), MAX_PATH_BYTES);
    let exact = proposal(
        &base.revision,
        "structural.entry",
        &[create_operation(&exact_path, "")],
    );
    parse_proposal(&exact).unwrap();
    let over_path = format!("{exact_path}a");
    assert_eq!(
        error_code(parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &[create_operation(&over_path, "")],
        ))),
        "SPX-G191"
    );

    let exact_source = "x".repeat(MAX_SOURCE_BYTES_PER_OPERATION);
    parse_proposal(&proposal(
        &base.revision,
        "structural.entry",
        &[create_operation("x/exact.spx", &exact_source)],
    ))
    .unwrap();
    assert_eq!(
        error_code(parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &[create_operation("x/over.spx", &format!("{exact_source}x"))],
        ))),
        "SPX-G191"
    );

    let exact_operations = (0..4)
        .map(|index| create_operation(&format!("x/{index}.spx"), &exact_source))
        .collect::<Vec<_>>();
    parse_proposal(&proposal(
        &base.revision,
        "structural.entry",
        &exact_operations,
    ))
    .unwrap();
    let mut over_operations = exact_operations;
    over_operations.push(create_operation("x/4.spx", "x"));
    assert_eq!(
        error_code(parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &over_operations,
        ))),
        "SPX-G191"
    );

    let exact_entry = "a".repeat(MAX_ENTRY_MODULE_BYTES);
    parse_proposal(&proposal(
        &base.revision,
        &exact_entry,
        &[create_operation("x/entry-exact.spx", "")],
    ))
    .unwrap();
    assert_eq!(
        error_code(parse_proposal(&proposal(
            &base.revision,
            &format!("{exact_entry}a"),
            &[create_operation("x/entry-over.spx", "")],
        ))),
        "SPX-G191"
    );

    let operations = (0..=MAX_OPERATIONS)
        .map(|index| create_operation(&format!("x/{index:02}.spx"), ""))
        .collect::<Vec<_>>();
    assert_eq!(
        error_code(parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &operations,
        ))),
        "SPX-G191"
    );
}

#[test]
fn mixed_full_graph_candidate_is_deterministic_and_entry_removal_fails() {
    let base = base_fixture();
    let parsed = parse_proposal(&mixed_proposal(&base)).unwrap();
    let expected_operations = parsed.operations().to_vec();
    let prepared = prepare_owned(
        base.revision,
        base.sources,
        base.graph,
        (base.manifest_bytes, 1, 0),
        parsed,
    )
    .unwrap();
    assert_ne!(
        prepared.base_workspace_revision(),
        prepared.candidate_workspace_revision()
    );
    assert_eq!(prepared.entry_module(), "structural.entry");
    assert_eq!(prepared.operations(), expected_operations);
    assert_eq!(
        prepared.used_total_supplied_source_bytes(),
        created().len() + entry_replacement().len()
    );
    assert!(!prepared.roots().is_empty());
    assert!(!prepared.delta_edges().is_empty());
    assert!(!prepared.context_nodes().is_empty());
    assert!(!prepared.impact().is_empty());
    assert!(!prepared.impact_edges().is_empty());
    assert!(prepared.used_analysis_builder_bytes() > 0);
    assert_eq!(
        prepared
            .delta_edges()
            .iter()
            .map(|edge| edge.edge().kind())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "call",
            "capability_authority",
            "effect_requirement",
            "function_import",
            "type_import",
            "type_reference",
        ])
    );
    assert!(prepared.roots().iter().any(|root| {
        root.id() == "structural.point"
            && root.identity_origin() == Some("explicit")
            && root.path() == Some("a/provider.spx")
            && root.change() == "modified_before"
    }));
    assert!(prepared.roots().iter().any(|root| {
        root.id() == "structural.point"
            && root.identity_origin() == Some("explicit")
            && root.path() == Some("c/provider.spx")
            && root.change() == "modified_after"
    }));
    assert!(prepared.roots().iter().any(|root| {
        root.identity_origin() == Some("automatic")
            && root.path() == Some("a/provider.spx")
            && root.change() == "removed"
    }));
    assert!(prepared.roots().iter().any(|root| {
        root.identity_origin() == Some("automatic")
            && root.path() == Some("c/provider.spx")
            && root.change() == "added"
    }));
    assert!(prepared
        .roots()
        .iter()
        .any(|root| root.path() == Some("n/island.spx") && root.state() == "base"));
    assert!(prepared
        .roots()
        .iter()
        .any(|root| root.path() == Some("b/created.spx") && root.state() == "candidate"));
    assert!(prepared.roots().iter().any(|root| {
        root.state() == "base"
            && root.kind() == "module"
            && root.id() == "structural.consumer"
            && root.path() == Some("m/consumer.spx")
            && root.change() == "modified_before"
    }));
    assert!(prepared.roots().iter().any(|root| {
        root.state() == "candidate"
            && root.kind() == "module"
            && root.id() == "structural.consumer"
            && root.path() == Some("m/consumer.spx")
            && root.change() == "modified_after"
    }));

    let replay = base_fixture_with_order(true);
    let parsed = parse_proposal(&mixed_proposal(&replay)).unwrap();
    let replayed = prepare_owned(
        replay.revision,
        replay.sources,
        replay.graph,
        (replay.manifest_bytes, 1, 0),
        parsed,
    )
    .unwrap();
    assert_eq!(prepared.proposal_source(), replayed.proposal_source());
    assert_eq!(
        prepared.candidate_workspace_revision(),
        replayed.candidate_workspace_revision()
    );
    assert_eq!(prepared.candidate_manifest(), replayed.candidate_manifest());
    assert_eq!(
        prepared.candidate_workspace_graph_digest(),
        replayed.candidate_workspace_graph_digest()
    );
    assert_eq!(prepared.roots(), replayed.roots());
    assert_eq!(prepared.delta_edges(), replayed.delta_edges());

    let base = base_fixture();
    let delete_entry = parse_proposal(&proposal(
        &base.revision,
        "structural.entry",
        &[delete_operation(source(&base, "z/entry.spx"))],
    ))
    .unwrap();
    assert_eq!(
        error_code(prepare_owned(
            base.revision,
            base.sources,
            base.graph,
            (base.manifest_bytes, 1, 0),
            delete_entry,
        )),
        "SPX-G190"
    );
}

fn build_with_analysis_limit(limit: usize) -> Result<usize, Vec<Diagnostic>> {
    let base = base_fixture();
    let parsed = parse_proposal(&mixed_proposal(&base)).unwrap();
    prepare_owned_with_analysis_limit(
        base.revision,
        base.sources,
        base.graph,
        (base.manifest_bytes, 1, 0),
        parsed,
        limit,
    )
    .map(|prepared| prepared.used_analysis_builder_bytes())
}

#[test]
fn each_structural_operation_builds_a_complete_candidate_independently() {
    for case in [
        "create",
        "delete",
        "move",
        "create-replace",
        "structural-three",
    ] {
        let base = base_fixture();
        let operations = match case {
            "create" => vec![create_operation("b/created.spx", &created())],
            "delete" => vec![delete_operation(source(&base, "n/island.spx"))],
            "move" => vec![move_operation(
                source(&base, "a/provider.spx"),
                "c/provider.spx",
            )],
            "create-replace" => vec![
                create_operation("b/created.spx", &created()),
                replace_operation(source(&base, "z/entry.spx"), &entry_replacement()),
            ],
            "structural-three" => vec![
                create_operation("b/created.spx", &created()),
                delete_operation(source(&base, "n/island.spx")),
                move_operation(source(&base, "a/provider.spx"), "c/provider.spx"),
            ],
            _ => unreachable!(),
        };
        let parsed =
            parse_proposal(&proposal(&base.revision, "structural.entry", &operations)).unwrap();
        if let Err(diagnostics) = prepare_owned(
            base.revision,
            base.sources,
            base.graph,
            (base.manifest_bytes, 1, 0),
            parsed,
        ) {
            panic!("{case} failed: {diagnostics:?}");
        }
    }
}

#[test]
fn analysis_builder_limit_has_an_exact_minimum_successful_boundary() {
    let mut low = 0usize;
    let mut high = MAX_ANALYSIS_BUILDER_BYTES;
    while low < high {
        let middle = low + (high - low) / 2;
        if build_with_analysis_limit(middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    assert!(low > 0);
    assert_eq!(build_with_analysis_limit(low).unwrap(), low);
    let diagnostics = build_with_analysis_limit(low - 1).unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-G191");
    assert_eq!(
            diagnostics[0].message,
            format!(
                "Semantic Workspace Structural Change limit exceeded: analysis_builder_bytes maximum {MAX_ANALYSIS_BUILDER_BYTES}"
            )
        );
}

#[test]
fn managed_generate_and_verify_are_exact_read_only_kats_under_one_shared_lock() {
    let fixture = ManagedFixture::new("generate-verify-kat");
    let before_generate = fixture.inventory();
    let generate_points = std::cell::RefCell::new(Vec::new());
    let artifacts = generate_with_hook(&fixture.root, &fixture.proposal_path, |point| {
        generate_points.borrow_mut().push(point);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(fixture.root.join(".semaprax-workspace/LOCK"))
            .unwrap();
        assert!(FileExt::try_lock_exclusive(&lock).is_err());
    })
    .unwrap();
    assert_eq!(
        *generate_points.borrow(),
        [
            StructuralGeneratePoint::ProposalOwned,
            StructuralGeneratePoint::ArtifactsRendered,
        ]
    );
    assert_eq!(fixture.inventory(), before_generate);
    assert_eq!(
        [
            raw_sha(artifacts.preview()),
            raw_sha(artifacts.context()),
            raw_sha(artifacts.impact()),
            raw_sha(artifacts.review()),
            raw_sha(artifacts.evidence()),
        ],
        [
            "sha256:0f6efe543aba015c57605af3813c68ce21b7f713c272a603d3e37c643787b8c2",
            "sha256:591a188353da9ede365e0d3201555dfd0cfda404ab4e21cf55dd6562c2b0df6d",
            "sha256:eb74c7c76c24e64ca61c42e33ed5c4af32b12af548dd5548d8d8739a22a435fd",
            "sha256:8b2e6d27de11a3a9422c19be5d31374b250876089e2e114da5bd12af15bd183d",
            "sha256:6396811d0418a82db6159cb46d1c274b48d3a33f97db1a6559c1f656448cf8df",
        ]
    );

    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
    let before_verify = fixture.inventory();
    let verify_points = std::cell::RefCell::new(Vec::new());
    let receipt = verify_with_hook(
        &fixture.root,
        &fixture.proposal_path,
        &evidence_path,
        |point| {
            verify_points.borrow_mut().push(point);
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .open(fixture.root.join(".semaprax-workspace/LOCK"))
                .unwrap();
            assert!(FileExt::try_lock_exclusive(&lock).is_err());
        },
    )
    .unwrap();
    assert_eq!(
        *verify_points.borrow(),
        [
            StructuralVerifyPoint::ProposalOwned,
            StructuralVerifyPoint::EvidenceOwned,
            StructuralVerifyPoint::ReceiptRendered,
        ]
    );
    assert_eq!(fixture.inventory(), before_verify);
    assert!(receipt.ends_with('\n'));
    assert!(!receipt[..receipt.len() - 1].contains('\n'));
    let value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.workspace-semantic-structural-change-evidence-verification.v1"
    );
    assert_eq!(value["result"], "exact_replay");
    assert_eq!(
        value["workspace_structural_change_evidence"]["bytes"],
        artifacts.evidence().len()
    );
    assert_eq!(value["budget"]["used_receipt_bytes"], receipt.len());
    assert_eq!(
        raw_sha(&receipt),
        "sha256:18f83d757c855caab3b8cc591a76eea38464d94450439c1325ff9f1e8a734494"
    );
    fixture.assert_exclusive_reacquire();
}

#[test]
fn input_ownership_precedence_and_exact_read_limits_are_fail_closed() {
    let fixture = ManagedFixture::new("input-precedence");
    let missing_proposal = fixture.root.join("missing-proposal.json");
    let missing_evidence = fixture.root.join("missing-evidence.json");
    let error = read_only_failure(&fixture, || {
        verify_with_hook(&fixture.root, &missing_proposal, &missing_evidence, |_| {})
    });
    assert_eq!(error.code, "SPX-I215");
    assert_eq!(
        error.message,
        "could not read Semantic Workspace Structural Change proposal: open failed"
    );

    let malformed_proposal = fixture.root.join("malformed-proposal.json");
    std::fs::write(&malformed_proposal, "{}\n").unwrap();
    let error = read_only_failure(&fixture, || {
        verify_with_hook(
            &fixture.root,
            &malformed_proposal,
            &missing_evidence,
            |_| {},
        )
    });
    assert_eq!(error.code, "SPX-I215");
    assert_eq!(
        error.message,
        "could not read Semantic Workspace Structural Change Evidence: open failed"
    );
    let malformed_evidence = fixture.root.join("malformed-evidence.json");
    std::fs::write(&malformed_evidence, "{}\n").unwrap();
    assert_eq!(
        read_only_failure(&fixture, || verify_with_hook(
            &fixture.root,
            &malformed_proposal,
            &malformed_evidence,
            |_| {},
        ))
        .code,
        "SPX-G193"
    );

    let proposal_dir = fixture.root.join("proposal-dir");
    std::fs::create_dir(&proposal_dir).unwrap();
    let error = read_only_failure(&fixture, || {
        generate_with_hook(&fixture.root, &proposal_dir, |_| {})
    });
    assert_eq!(error.code, "SPX-I215");
    #[cfg(windows)]
    assert_eq!(
        error.message,
        "could not read Semantic Workspace Structural Change proposal: open failed"
    );
    #[cfg(not(windows))]
    assert_eq!(
        error.message,
        "could not read Semantic Workspace Structural Change proposal: input is not a regular file"
    );

    let invalid_proposal = fixture.root.join("invalid-proposal.json");
    std::fs::write(&invalid_proposal, [0xff]).unwrap();
    let error = read_only_failure(&fixture, || {
        generate_with_hook(&fixture.root, &invalid_proposal, |_| {})
    });
    assert_eq!(error.code, "SPX-I215");
    assert_eq!(
        error.message,
        "could not read Semantic Workspace Structural Change proposal: input is not UTF-8"
    );

    let exact_proposal = fixture.root.join("exact-proposal.json");
    File::create(&exact_proposal)
        .unwrap()
        .set_len(MAX_PROPOSAL_BYTES as u64)
        .unwrap();
    assert_eq!(
        read_only_failure(&fixture, || {
            generate_with_hook(&fixture.root, &exact_proposal, |_| {})
        })
        .code,
        "SPX-G188"
    );
    let oversized_proposal = fixture.root.join("oversized-proposal.json");
    File::create(&oversized_proposal)
        .unwrap()
        .set_len(MAX_PROPOSAL_BYTES as u64 + 1)
        .unwrap();
    assert_eq!(
        read_only_failure(&fixture, || generate_with_hook(
            &fixture.root,
            &oversized_proposal,
            |_| {}
        ))
        .code,
        "SPX-G191"
    );

    let evidence_dir = fixture.root.join("evidence-dir");
    std::fs::create_dir(&evidence_dir).unwrap();
    let error = read_only_failure(&fixture, || {
        verify_with_hook(&fixture.root, &fixture.proposal_path, &evidence_dir, |_| {})
    });
    assert_eq!(error.code, "SPX-I215");
    #[cfg(windows)]
    assert_eq!(
        error.message,
        "could not read Semantic Workspace Structural Change Evidence: open failed"
    );
    #[cfg(not(windows))]
    assert_eq!(
        error.message,
        "could not read Semantic Workspace Structural Change Evidence: input is not a regular file"
    );
    let invalid_evidence = fixture.root.join("invalid-evidence.json");
    std::fs::write(&invalid_evidence, [0xff]).unwrap();
    let error = read_only_failure(&fixture, || {
        verify_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &invalid_evidence,
            |_| {},
        )
    });
    assert_eq!(error.code, "SPX-I215");
    assert_eq!(
        error.message,
        "could not read Semantic Workspace Structural Change Evidence: input is not UTF-8"
    );
    let exact_evidence = fixture.root.join("exact-evidence.json");
    File::create(&exact_evidence)
        .unwrap()
        .set_len(TEST_MAX_EVIDENCE_BYTES as u64)
        .unwrap();
    assert_eq!(
        read_only_failure(&fixture, || verify_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &exact_evidence,
            |_| {},
        ))
        .code,
        "SPX-G193"
    );
    let oversized_evidence = fixture.root.join("oversized-evidence.json");
    File::create(&oversized_evidence)
        .unwrap()
        .set_len(TEST_MAX_EVIDENCE_BYTES as u64 + 1)
        .unwrap();
    assert_eq!(
        read_only_failure(&fixture, || verify_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &oversized_evidence,
            |_| {},
        ))
        .code,
        "SPX-G191"
    );
}

#[test]
fn owned_inputs_are_never_reopened_and_final_drift_discards_outputs() {
    let fixture = ManagedFixture::new("owned-inputs");
    let baseline = generate_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
    let evidence_path = fixture.root.join("owned-evidence.json");
    std::fs::write(&evidence_path, baseline.evidence()).unwrap();
    let baseline_receipt = verify_with_hook(
        &fixture.root,
        &fixture.proposal_path,
        &evidence_path,
        |_| {},
    )
    .unwrap();

    for replacement in [fixture.proposal_source.as_bytes(), b"{}\n".as_slice()] {
        std::fs::write(&fixture.proposal_path, &fixture.proposal_source).unwrap();
        let replacement_path = fixture.root.join("generate-replacement.json");
        std::fs::write(&replacement_path, replacement).unwrap();
        let artifacts = generate_with_hook(&fixture.root, &fixture.proposal_path, |point| {
            if matches!(point, StructuralGeneratePoint::ProposalOwned) {
                replace_owned_path(&fixture.proposal_path, &replacement_path);
            }
        })
        .unwrap();
        assert_eq!(
            [
                artifacts.proposal_digest(),
                artifacts.candidate_manifest_digest(),
                artifacts.preview(),
                artifacts.preview_digest(),
                artifacts.context(),
                artifacts.context_digest(),
                artifacts.impact(),
                artifacts.impact_digest(),
                artifacts.review(),
                artifacts.review_digest(),
                artifacts.evidence(),
                artifacts.evidence_digest(),
            ],
            [
                baseline.proposal_digest(),
                baseline.candidate_manifest_digest(),
                baseline.preview(),
                baseline.preview_digest(),
                baseline.context(),
                baseline.context_digest(),
                baseline.impact(),
                baseline.impact_digest(),
                baseline.review(),
                baseline.review_digest(),
                baseline.evidence(),
                baseline.evidence_digest(),
            ]
        );
    }

    for replace_at in ["proposal", "evidence"] {
        std::fs::write(&fixture.proposal_path, &fixture.proposal_source).unwrap();
        std::fs::write(&evidence_path, baseline.evidence()).unwrap();
        let replacement_path = fixture.root.join(format!("{replace_at}-replacement.json"));
        std::fs::write(&replacement_path, "{}\n").unwrap();
        let receipt = verify_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point| match (replace_at, point) {
                ("proposal", StructuralVerifyPoint::ProposalOwned) => {
                    replace_owned_path(&fixture.proposal_path, &replacement_path);
                }
                ("evidence", StructuralVerifyPoint::EvidenceOwned) => {
                    replace_owned_path(&evidence_path, &replacement_path);
                }
                _ => {}
            },
        )
        .unwrap();
        assert_eq!(receipt, baseline_receipt);
    }
    fixture.assert_exclusive_reacquire();

    let generate_drift = ManagedFixture::new("generate-final-drift");
    let error = diagnostic(generate_with_hook(
        &generate_drift.root,
        &generate_drift.proposal_path,
        |point| {
            if matches!(point, StructuralGeneratePoint::ArtifactsRendered) {
                OpenOptions::new()
                    .append(true)
                    .open(generate_drift.root.join(".semaprax-workspace/ACTIVE"))
                    .unwrap()
                    .write_all(b"x")
                    .unwrap();
            }
        },
    ));
    assert_eq!(error.code, "SPX-G153");
    generate_drift.assert_exclusive_reacquire();

    let verify_drift = ManagedFixture::new("verify-final-drift");
    let artifacts =
        generate_with_hook(&verify_drift.root, &verify_drift.proposal_path, |_| {}).unwrap();
    let evidence_path = verify_drift.root.join("evidence.json");
    std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
    let error = diagnostic(verify_with_hook(
        &verify_drift.root,
        &verify_drift.proposal_path,
        &evidence_path,
        |point| {
            if matches!(point, StructuralVerifyPoint::ReceiptRendered) {
                OpenOptions::new()
                    .append(true)
                    .open(verify_drift.root.join(".semaprax-workspace/ACTIVE"))
                    .unwrap()
                    .write_all(b"x")
                    .unwrap();
            }
        },
    ));
    assert_eq!(error.code, "SPX-G153");
    verify_drift.assert_exclusive_reacquire();
}

#[test]
fn evidence_format_confusion_and_exact_replay_mutations_fail_closed() {
    let fixture = ManagedFixture::new("evidence-hostile");
    let artifacts = generate_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
    let evidence = artifacts.evidence();
    let schema_prefix = concat!(
        "{\"schema\":\"semaprax.workspace-semantic-structural-change-evidence.v1\",",
        "\"workspace_manifest_schema\":\"semaprax.workspace-semantic-manifest.v1\""
    );
    let reordered_prefix = concat!(
        "{\"workspace_manifest_schema\":\"semaprax.workspace-semantic-manifest.v1\",",
        "\"schema\":\"semaprax.workspace-semantic-structural-change-evidence.v1\""
    );
    let mut missing = evidence.to_owned();
    missing = missing.replace("\"entry_module\":\"structural.entry\",", "");
    let extra = evidence.replacen("{\"schema\":", "{\"extra\":0,\"schema\":", 1);
    let duplicate = evidence.replacen("{\"schema\":", "{\"schema\":\"duplicate\",\"schema\":", 1);
    let reordered = evidence.replacen(schema_prefix, reordered_prefix, 1);
    let wrong_type = evidence.replace(
        "\"entry_module\":\"structural.entry\"",
        "\"entry_module\":0",
    );
    let no_lf = evidence.trim_end_matches('\n').to_owned();
    let crlf = format!("{}\r\n", evidence.trim_end_matches('\n'));
    let bom = format!("\u{feff}{evidence}");
    let two_lines = format!("{evidence}\n");
    let proposal_ref = object_after(evidence, "\"proposal\":");
    let missing_proposal_ref_field = evidence.replacen(
        proposal_ref,
        &remove_nonfirst_scalar_field(proposal_ref, "bytes"),
        1,
    );
    let extra_proposal_ref_field = evidence.replacen(
        proposal_ref,
        &proposal_ref.replacen('{', "{\"extra\":0,", 1),
        1,
    );
    let wrong_proposal_ref_type = evidence.replacen(
        proposal_ref,
        &replace_scalar_field(proposal_ref, "bytes", "\"invalid\""),
        1,
    );
    let graph_ref = object_after(evidence, "\"base_workspace_graph\":");
    let missing_graph_digest = evidence.replacen(
        graph_ref,
        &remove_nonfirst_scalar_field(graph_ref, "digest"),
        1,
    );
    let path_row = object_after(evidence, "\"paths\":[");
    let missing_path_peer = evidence.replacen(
        path_row,
        &remove_nonfirst_scalar_field(path_row, "peer_path"),
        1,
    );
    let limits = object_after(evidence, "\"limits\":");
    let missing_limit = evidence.replacen(
        limits,
        &remove_nonfirst_scalar_field(limits, "max_operations"),
        1,
    );
    let budget = object_after(evidence, "\"budget\":");
    let wrong_budget_type = evidence.replacen(
        budget,
        &replace_scalar_field(budget, "used_operations", "\"four\""),
        1,
    );
    let wrong_nonclaim_type =
        evidence.replacen("\"not_signature_or_authenticated_provenance\"", "0", 1);
    let evidence_path = fixture.root.join("evidence.json");
    let mut format_hostiles = vec![
        missing,
        extra,
        duplicate,
        reordered,
        wrong_type,
        no_lf,
        crlf,
        bom,
        two_lines,
        "[[[[[[[[[[]]]]]]]]]\n".to_owned(),
        missing_proposal_ref_field,
        extra_proposal_ref_field,
        wrong_proposal_ref_type,
        missing_graph_digest,
        missing_path_peer,
        missing_limit,
        wrong_budget_type,
        wrong_nonclaim_type,
    ];
    for (marker, first, second) in [
        ("\"proposal\":", "schema", "digest"),
        ("\"base_workspace_graph\":", "schema", "digest"),
        ("\"paths\":[", "path", "change"),
        ("\"limits\":", "max_managed_files", "max_operations"),
        (
            "\"budget\":",
            "used_base_managed_files",
            "used_candidate_managed_files",
        ),
    ] {
        format_hostiles.extend(nested_shape_mutations(evidence, marker, first, second));
    }
    for hostile in format_hostiles {
        assert_eq!(
            diagnostic(verification::parse_evidence(&hostile)).code,
            "SPX-G193"
        );
        std::fs::write(&evidence_path, hostile).unwrap();
        assert_eq!(
            read_only_failure(&fixture, || verify_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |_| {},
            ))
            .code,
            "SPX-G193"
        );
    }

    std::fs::write(&evidence_path, evidence).unwrap();
    let receipt = verify_with_hook(
        &fixture.root,
        &fixture.proposal_path,
        &evidence_path,
        |_| {},
    )
    .unwrap();
    assert_eq!(
        diagnostic(verification::parse_evidence(&receipt)).code,
        "SPX-G193"
    );
    std::fs::write(&evidence_path, &receipt).unwrap();
    let malformed_proposal = fixture.root.join("receipt-confusion-proposal.json");
    std::fs::write(&malformed_proposal, "{}\n").unwrap();
    assert_eq!(
        read_only_failure(&fixture, || verify_with_hook(
            &fixture.root,
            &malformed_proposal,
            &evidence_path,
            |_| {},
        ))
        .code,
        "SPX-G193"
    );

    let path_row = object_after(evidence, "\"paths\":[");
    let proposal_ref = object_after(evidence, "\"proposal\":");
    let graph_ref = object_after(evidence, "\"base_workspace_graph\":");
    let limits = object_after(evidence, "\"limits\":");
    let budget = object_after(evidence, "\"budget\":");
    let replay_mutations = [
        evidence.replace(
            "\"entry_module\":\"structural.entry\"",
            "\"entry_module\":\"structural.entri\"",
        ),
        evidence.replacen(
            proposal_ref,
            &replace_scalar_field(
                proposal_ref,
                "digest",
                &format!("\"sha256:{}\"", "0".repeat(64)),
            ),
            1,
        ),
        evidence.replacen(
            graph_ref,
            &replace_scalar_field(
                graph_ref,
                "digest",
                &format!("\"sha256:{}\"", "0".repeat(64)),
            ),
            1,
        ),
        evidence.replacen(
            path_row,
            &replace_scalar_field(path_row, "change", "\"mutated\""),
            1,
        ),
        evidence.replacen(
            limits,
            &replace_scalar_field(limits, "max_managed_files", "15"),
            1,
        ),
        evidence.replacen(
            budget,
            &replace_scalar_field(budget, "used_operations", "3"),
            1,
        ),
        evidence.replacen(
            "\"not_signature_or_authenticated_provenance\"",
            "\"mutated_nonclaim\"",
            1,
        ),
    ];
    for mutated in replay_mutations {
        if let Err(diagnostics) = verification::parse_evidence(&mutated) {
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, "SPX-G195");
        }
        std::fs::write(&evidence_path, mutated).unwrap();
        let error = read_only_failure(&fixture, || {
            verify_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |_| {},
            )
        });
        assert_eq!(error.code, "SPX-G195");
        assert_eq!(
                error.message,
                "Semantic Workspace Structural Change Evidence does not exactly replay the authenticated proposal and candidate"
            );
    }
}

#[test]
fn structural_apply_process_child() {
    if std::env::var_os("SEMAPRAX_STRUCTURAL_APPLY_CHILD").is_none() {
        return;
    }
    let root = PathBuf::from(std::env::var_os("SEMAPRAX_STRUCTURAL_APPLY_ROOT").unwrap());
    let proposal = PathBuf::from(std::env::var_os("SEMAPRAX_STRUCTURAL_APPLY_PROPOSAL").unwrap());
    let evidence = PathBuf::from(std::env::var_os("SEMAPRAX_STRUCTURAL_APPLY_EVIDENCE").unwrap());
    let boundary = std::env::var("SEMAPRAX_STRUCTURAL_APPLY_BOUNDARY").unwrap();
    let ready = PathBuf::from(std::env::var_os("SEMAPRAX_STRUCTURAL_APPLY_READY").unwrap());
    apply_authenticated_with_hook(&root, &proposal, &evidence, |point, _, _, _| {
        let selected = matches!(
            (boundary.as_str(), point),
            (
                "pre",
                StructuralApplyPoint::Workspace(
                    workspace::SemanticChangeApplyPoint::BeforeActiveReplace
                )
            ) | (
                "post",
                StructuralApplyPoint::Workspace(
                    workspace::SemanticChangeApplyPoint::AfterActiveReplace
                )
            )
        );
        if selected {
            let mut marker = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&ready)?;
            marker.write_all(b"ready\n")?;
            marker.sync_all()?;
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn structural_apply_killed_process_boundaries_preserve_exact_old_or_new() {
    for boundary in ["pre", "post"] {
        let (fixture, evidence_path) = application_fixture(&format!("process-kill-{boundary}"));
        let old_revision = workspace_graph::snapshot(&fixture.root, "structural.entry")
            .unwrap()
            .workspace_revision()
            .to_owned();
        let evidence = std::fs::read_to_string(&evidence_path).unwrap();
        let candidate_revision = serde_json::from_str::<Value>(&evidence).unwrap()
            ["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        let raw_before = fixture.raw_inventory();

        let (mut child, ready) = spawn_structural_apply_process(&fixture, &evidence_path, boundary);
        assert_eq!(std::fs::read(&ready).unwrap(), b"ready\n");
        let held_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(fixture.root.join(".semaprax-workspace/LOCK"))
            .unwrap();
        assert!(FileExt::try_lock_exclusive(&held_lock).is_err());
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        std::fs::remove_file(&ready).unwrap();

        fixture.assert_exclusive_reacquire();
        let current = workspace_graph::snapshot(&fixture.root, "structural.entry").unwrap();
        assert_eq!(
            current.workspace_revision(),
            if boundary == "pre" {
                &old_revision
            } else {
                &candidate_revision
            }
        );
        let mut expected_generations = [old_revision.as_str(), candidate_revision.as_str()]
            .map(|revision| revision.strip_prefix("sha256:").unwrap().to_owned())
            .to_vec();
        expected_generations.sort();
        let generations_path = fixture.root.join(".semaprax-workspace/generations");
        assert_eq!(directory_names(&generations_path), expected_generations);
        for generation in &expected_generations {
            let metadata = std::fs::symlink_metadata(generations_path.join(generation)).unwrap();
            assert!(metadata.is_dir());
            assert!(!metadata.file_type().is_symlink());
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt as _;
                assert_eq!(metadata.file_attributes() & 0x400, 0);
            }
        }
        let staging_path = fixture.root.join(".semaprax-workspace/staging");
        let staging_names = directory_names(&staging_path);
        if boundary == "pre" {
            assert_eq!(staging_names, ["0"]);
            let metadata = std::fs::symlink_metadata(staging_path.join("0")).unwrap();
            assert!(metadata.is_file());
            assert!(!metadata.file_type().is_symlink());
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt as _;
                assert_eq!(metadata.file_attributes() & 0x400, 0);
            }
        } else {
            assert!(staging_names.is_empty());
        }
        assert_eq!(fixture.raw_inventory(), raw_before);
    }
}

#[test]
fn structural_apply_publishes_exact_candidate_once_without_raw_writes() {
    let (fixture, evidence_path) = application_fixture("apply-success");
    let raw_before = fixture.raw_inventory();
    let active_before = std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
    let points = std::cell::RefCell::new(Vec::new());
    let candidate_path = std::cell::RefCell::new(None::<PathBuf>);
    let receipt = apply_authenticated_with_hook(
        &fixture.root,
        &fixture.proposal_path,
        &evidence_path,
        |point, _, _, candidate| {
            points.borrow_mut().push(apply_point_name(point));
            if let Some(candidate) = candidate {
                *candidate_path.borrow_mut() = Some(candidate.to_owned());
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        *points.borrow(),
        [
            "proposal_owned",
            "evidence_owned",
            "after_replay",
            "receipt_rendered",
            "generation_after_slot_create",
            "generation_after_manifest_write",
            "generation_after_files_write",
            "generation_before_stage_validation",
            "generation_before_publish",
            "generation_destination_checked",
            "generation_after_publish",
            "after_candidate_prepared",
            "after_active_staged",
            "before_first_final_check",
            "before_second_final_check",
            "before_active_replace",
            "after_active_replace",
        ]
    );
    let receipt_value: Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(
        receipt_value["schema"],
        "semaprax.workspace-semantic-structural-change-evidence-application.v1"
    );
    assert_eq!(receipt_value["result"], "applied");
    assert_eq!(
        raw_sha(&receipt),
        "sha256:34a106c08d475f4d326e6bb8fd49a269c4f4fcf1ab6871023f8752fab30fc03d"
    );
    assert_eq!(fixture.raw_inventory(), raw_before);
    assert_ne!(
        std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
        active_before
    );
    assert!(candidate_path.borrow().as_ref().unwrap().is_dir());
    assert_eq!(
        fixture.authenticated_paths_and_storage(),
        (
            vec![
                "b/created.spx".to_owned(),
                "c/provider.spx".to_owned(),
                "m/consumer.spx".to_owned(),
                "z/entry.spx".to_owned(),
            ],
            2,
            0,
        )
    );
    fixture.assert_exclusive_reacquire();
}

#[test]
fn every_structural_apply_hook_maps_pre_and_post_pivot_failures_exactly() {
    for target in [
        "proposal_owned",
        "evidence_owned",
        "after_replay",
        "receipt_rendered",
        "generation_after_slot_create",
        "generation_after_manifest_write",
        "generation_after_files_write",
        "generation_before_stage_validation",
        "generation_before_publish",
        "generation_destination_checked",
        "generation_after_publish",
        "after_candidate_prepared",
        "after_active_staged",
        "before_first_final_check",
        "before_second_final_check",
        "before_active_replace",
        "after_active_replace",
    ] {
        let (fixture, evidence_path) = application_fixture(target);
        let active_before = std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
        let reached = std::cell::Cell::new(false);
        let error = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, _, _, _| {
                if apply_point_name(point) == target {
                    reached.set(true);
                    return Err(std::io::Error::other("injected boundary failure"));
                }
                Ok(())
            },
        ));
        assert!(reached.get(), "hook was not reached: {target}");
        assert_eq!(
            error.code,
            if target == "after_active_replace" {
                "SPX-I212"
            } else {
                "SPX-I211"
            },
            "unexpected diagnostic at {target}: {error:?}"
        );
        let active_after = std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
        if target == "after_active_replace" {
            assert_ne!(active_after, active_before);
        } else {
            assert_eq!(active_after, active_before);
        }
        fixture.assert_exclusive_reacquire();
    }
}

#[test]
fn published_candidate_residue_requires_new_evidence_then_reuses_exact_path() {
    let (fixture, evidence_path) = application_fixture("candidate-residue");
    let active_before = std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
    let first_candidate = std::cell::RefCell::new(None::<PathBuf>);
    let error = diagnostic(apply_authenticated_with_hook(
        &fixture.root,
        &fixture.proposal_path,
        &evidence_path,
        |point, _, _, candidate| {
            if matches!(
                point,
                StructuralApplyPoint::Workspace(
                    workspace::SemanticChangeApplyPoint::AfterCandidatePrepared
                )
            ) {
                *first_candidate.borrow_mut() = candidate.map(Path::to_owned);
                return Err(std::io::Error::other("stop after candidate publication"));
            }
            Ok(())
        },
    ));
    assert_eq!(error.code, "SPX-I211");
    let first_candidate = first_candidate.into_inner().unwrap();
    let candidate_inventory = {
        let fixture_inventory = fixture.inventory();
        fixture_inventory
            .into_iter()
            .filter(|(path, _, _)| {
                first_candidate
                    .strip_prefix(&fixture.root)
                    .is_ok_and(|prefix| path.starts_with(prefix.to_string_lossy().as_ref()))
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
        active_before
    );
    assert_eq!(fixture.authenticated_paths_and_storage().1, 2);

    let stale = diagnostic(apply_authenticated_with_hook(
        &fixture.root,
        &fixture.proposal_path,
        &evidence_path,
        |_, _, _, _| Ok(()),
    ));
    assert_eq!(stale.code, "SPX-G195");
    let regenerated = generate_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
    std::fs::write(&evidence_path, regenerated.evidence()).unwrap();
    let reused_candidate = std::cell::RefCell::new(None::<PathBuf>);
    apply_authenticated_with_hook(
        &fixture.root,
        &fixture.proposal_path,
        &evidence_path,
        |point, _, _, candidate| {
            if matches!(
                point,
                StructuralApplyPoint::Workspace(
                    workspace::SemanticChangeApplyPoint::AfterCandidatePrepared
                )
            ) {
                *reused_candidate.borrow_mut() = candidate.map(Path::to_owned);
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(reused_candidate.into_inner().unwrap(), first_candidate);
    assert_eq!(fixture.authenticated_paths_and_storage().1, 2);
    let after_inventory = fixture.inventory();
    for fact in candidate_inventory {
        assert!(after_inventory.contains(&fact));
    }
    fixture.assert_exclusive_reacquire();
}

#[test]
fn structural_final_rechecks_detect_identity_and_post_pivot_candidate_drift() {
    let (identity_fixture, identity_evidence) = application_fixture("identity-drift");
    let active_path = identity_fixture.root.join(".semaprax-workspace/ACTIVE");
    let active_before = std::fs::read(&active_path).unwrap();
    let error = diagnostic(apply_authenticated_with_hook(
        &identity_fixture.root,
        &identity_fixture.proposal_path,
        &identity_evidence,
        |point, active, _, _| {
            if matches!(
                point,
                StructuralApplyPoint::Workspace(
                    workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck
                )
            ) {
                let replacement = active.with_extension("replacement");
                std::fs::write(&replacement, std::fs::read(active).unwrap())?;
                std::fs::remove_file(active)?;
                std::fs::rename(replacement, active)?;
            }
            Ok(())
        },
    ));
    assert_eq!(error.code, "SPX-G153");
    assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
    identity_fixture.assert_exclusive_reacquire();

    let (post_fixture, post_evidence) = application_fixture("post-pivot-drift");
    let error = diagnostic(apply_authenticated_with_hook(
        &post_fixture.root,
        &post_fixture.proposal_path,
        &post_evidence,
        |point, _, _, candidate| {
            if matches!(
                point,
                StructuralApplyPoint::Workspace(
                    workspace::SemanticChangeApplyPoint::AfterActiveReplace
                )
            ) {
                let candidate = candidate.unwrap();
                OpenOptions::new()
                    .append(true)
                    .open(candidate.join("files/z/entry.spx"))?
                    .write_all(b"x")?;
            }
            Ok(())
        },
    ));
    assert_eq!(error.code, "SPX-I212");
    post_fixture.assert_exclusive_reacquire();
}

#[test]
fn structural_apply_replay_failure_is_zero_write_and_destination_races_never_clobber() {
    let (fixture, evidence_path) = application_fixture("apply-replay-zero-write");
    let evidence = std::fs::read_to_string(&evidence_path).unwrap().replace(
        "\"entry_module\":\"structural.entry\"",
        "\"entry_module\":\"structural.entri\"",
    );
    verification::parse_evidence(&evidence).unwrap();
    std::fs::write(&evidence_path, evidence).unwrap();
    let before = fixture.inventory();
    let error = diagnostic(apply_authenticated_with_hook(
        &fixture.root,
        &fixture.proposal_path,
        &evidence_path,
        |_, _, _, _| Ok(()),
    ));
    assert_eq!(error.code, "SPX-G195");
    assert_eq!(fixture.inventory(), before);
    fixture.assert_exclusive_reacquire();

    for kind in ["file", "directory"] {
        let (fixture, evidence_path) = application_fixture(&format!("destination-{kind}"));
        let active_before = std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
        let foreign = std::cell::RefCell::new(None::<PathBuf>);
        let error = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, _, _, candidate| {
                if matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::Generation(
                            workspace::GenerationPoint::DestinationChecked
                        )
                    )
                ) {
                    let candidate = candidate.unwrap();
                    if kind == "file" {
                        std::fs::write(candidate, "foreign structural generation\n")?;
                    } else {
                        std::fs::create_dir(candidate)?;
                    }
                    *foreign.borrow_mut() = Some(candidate.to_owned());
                }
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-I211");
        let foreign = foreign.into_inner().unwrap();
        assert_eq!(foreign.is_file(), kind == "file");
        assert_eq!(foreign.is_dir(), kind == "directory");
        assert_eq!(
            std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
            active_before
        );
        fixture.assert_exclusive_reacquire();
    }
}

#[test]
fn structural_generation_rechecks_reject_same_byte_manifest_and_source_substitution() {
    for (label, point, relative) in [
        (
            "manifest",
            workspace::GenerationPoint::AfterManifestWrite,
            "manifest.json",
        ),
        (
            "source",
            workspace::GenerationPoint::AfterFilesWrite,
            "files/z/entry.spx",
        ),
    ] {
        let (fixture, evidence_path) = application_fixture(&format!("generation-{label}"));
        let active_before = std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
        let substituted = std::cell::Cell::new(false);
        let error = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |current, _, staged, _| {
                if matches!(
                    current,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::Generation(observed)
                    ) if observed == point
                ) {
                    let path = staged.unwrap().join(relative);
                    let bytes = std::fs::read(&path)?;
                    std::fs::remove_file(&path)?;
                    std::fs::write(path, bytes)?;
                    substituted.set(true);
                }
                Ok(())
            },
        ));
        assert!(substituted.get());
        assert_eq!(error.code, "SPX-G153");
        assert_eq!(
            std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
            active_before
        );
        fixture.assert_exclusive_reacquire();
    }
}

#[cfg(unix)]
#[test]
fn structural_generation_rejects_staged_symlink_and_hardlink_aliases() {
    use std::os::unix::fs::symlink;

    for kind in ["symlink", "hardlink"] {
        let (fixture, evidence_path) = application_fixture(&format!("alias-{kind}"));
        let active_before = std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
        let error = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, _, staged, _| {
                if matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::Generation(
                            workspace::GenerationPoint::AfterFilesWrite
                        )
                    )
                ) {
                    let staged = staged.unwrap();
                    let target = staged.join("files/z/entry.spx");
                    std::fs::remove_file(&target)?;
                    if kind == "symlink" {
                        symlink(&fixture.proposal_path, target)?;
                    } else {
                        std::fs::hard_link(staged.join("files/m/consumer.spx"), target)?;
                    }
                }
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-G153");
        assert_eq!(
            std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
            active_before
        );
        fixture.assert_exclusive_reacquire();
    }
}

#[cfg(unix)]
#[test]
fn structural_candidate_destination_aliases_preserve_foreign_targets() {
    use std::os::unix::fs::symlink;

    for kind in ["symlink", "hardlink"] {
        let (fixture, evidence_path) = application_fixture(&format!("destination-{kind}"));
        let active_path = fixture.root.join(".semaprax-workspace/ACTIVE");
        let active_before = std::fs::read(&active_path).unwrap();
        let foreign = fixture.root.join(format!("foreign-{kind}-target"));
        if kind == "symlink" {
            std::fs::create_dir(&foreign).unwrap();
            std::fs::write(foreign.join("sentinel.txt"), b"foreign-directory-target\n").unwrap();
        } else {
            std::fs::write(&foreign, b"foreign-file-target\n").unwrap();
        }
        let alias = std::cell::RefCell::new(None::<PathBuf>);
        let error = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, _, _, candidate| {
                if matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::Generation(
                            workspace::GenerationPoint::DestinationChecked
                        )
                    )
                ) {
                    let destination = candidate.unwrap();
                    if kind == "symlink" {
                        symlink(&foreign, destination)?;
                    } else {
                        std::fs::hard_link(&foreign, destination)?;
                    }
                    *alias.borrow_mut() = Some(destination.to_owned());
                }
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-I211");
        assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
        let alias = alias.into_inner().unwrap();
        if kind == "symlink" {
            assert!(std::fs::symlink_metadata(&alias)
                .unwrap()
                .file_type()
                .is_symlink());
            std::fs::remove_file(alias).unwrap();
            assert_eq!(
                std::fs::read(foreign.join("sentinel.txt")).unwrap(),
                b"foreign-directory-target\n"
            );
        } else {
            assert!(alias.is_file());
            assert_eq!(std::fs::read(&alias).unwrap(), b"foreign-file-target\n");
            std::fs::remove_file(alias).unwrap();
            assert_eq!(std::fs::read(&foreign).unwrap(), b"foreign-file-target\n");
        }
        fixture.assert_exclusive_reacquire();
    }
}

#[cfg(unix)]
#[test]
fn structural_apply_rejects_permission_drift_without_pivot() {
    use std::os::unix::fs::PermissionsExt;

    for case in ["lock", "active", "candidate"] {
        let (fixture, evidence_path) = application_fixture(&format!("permission-{case}"));
        let control = fixture.root.join(".semaprax-workspace");
        let active_path = control.join("ACTIVE");
        let active_before = std::fs::read(&active_path).unwrap();
        let changed = std::cell::RefCell::new(None::<(PathBuf, std::fs::Permissions)>);
        let error = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, active, _, candidate| {
                if !matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck
                    )
                ) {
                    return Ok(());
                }
                let path = match case {
                    "lock" => control.join("LOCK"),
                    "active" => active.to_owned(),
                    "candidate" => candidate.unwrap().join("manifest.json"),
                    _ => unreachable!(),
                };
                let original = std::fs::metadata(&path)?.permissions();
                let mut altered = original.clone();
                altered.set_mode(altered.mode() ^ 0o100);
                std::fs::set_permissions(&path, altered)?;
                *changed.borrow_mut() = Some((path, original));
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-G153");
        assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
        let (path, permissions) = changed.into_inner().unwrap();
        std::fs::set_permissions(path, permissions).unwrap();
        fixture.assert_exclusive_reacquire();
    }
}

#[test]
fn cooperative_reader_observes_only_locked_old_then_complete_new_structural_state() {
    let (fixture, evidence_path) = application_fixture("cooperative-reader");
    let artifacts = generate_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
    let expected_revision = serde_json::from_str::<Value>(artifacts.evidence()).unwrap()
        ["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let active_path = fixture.root.join(".semaprax-workspace/ACTIVE");
    let active_before = std::fs::read(&active_path).unwrap();
    let (arrived_tx, arrived_rx) = std::sync::mpsc::sync_channel(0);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
    std::thread::scope(|scope| {
        let root = fixture.root.as_path();
        let proposal_path = fixture.proposal_path.as_path();
        let evidence_path = evidence_path.as_path();
        let writer = scope.spawn(move || {
            apply_authenticated_with_hook(root, proposal_path, evidence_path, |point, _, _, _| {
                if matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::BeforeActiveReplace
                    )
                ) {
                    arrived_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                }
                Ok(())
            })
        });
        arrived_rx.recv().unwrap();
        let diagnostics = match workspace_graph::snapshot(&fixture.root, "structural.entry") {
            Ok(_) => panic!("reader must not observe an in-progress structural generation"),
            Err(diagnostics) => diagnostics,
        };
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-I210");
        assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
        release_tx.send(()).unwrap();
        writer.join().unwrap().unwrap();
    });
    assert_eq!(
        workspace_graph::snapshot(&fixture.root, "structural.entry")
            .unwrap()
            .workspace_revision(),
        expected_revision
    );
    fixture.assert_exclusive_reacquire();
}

#[cfg(windows)]
#[test]
fn structural_apply_rejects_windows_junction_and_casefold_destination_without_clobber() {
    use std::process::Command;

    let (junction_fixture, junction_evidence) = application_fixture("windows-junction");
    let active_path = junction_fixture.root.join(".semaprax-workspace/ACTIVE");
    let active_before = std::fs::read(&active_path).unwrap();
    let foreign = junction_fixture.root.join("foreign-junction-target");
    std::fs::create_dir(&foreign).unwrap();
    let sentinel = foreign.join("sentinel.txt");
    std::fs::write(&sentinel, b"structural-foreign-junction\n").unwrap();
    let junction = std::cell::RefCell::new(None::<PathBuf>);
    let error = diagnostic(apply_authenticated_with_hook(
        &junction_fixture.root,
        &junction_fixture.proposal_path,
        &junction_evidence,
        |point, _, _, candidate| {
            if matches!(
                point,
                StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                    workspace::GenerationPoint::BeforeGenerationPublish
                ))
            ) {
                let destination = candidate.unwrap();
                let status = Command::new("cmd")
                    .args(["/C", "mklink", "/J"])
                    .arg(destination)
                    .arg(&foreign)
                    .status()?;
                assert!(status.success(), "mklink /J failed");
                *junction.borrow_mut() = Some(destination.to_owned());
            }
            Ok(())
        },
    ));
    assert_eq!(error.code, "SPX-I211");
    assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
    let junction = junction.into_inner().unwrap();
    {
        use std::os::windows::fs::MetadataExt as _;
        assert!(
            std::fs::symlink_metadata(&junction)
                .unwrap()
                .file_attributes()
                & 0x400
                != 0
        );
    }
    std::fs::remove_dir(junction).unwrap();
    assert_eq!(
        std::fs::read(&sentinel).unwrap(),
        b"structural-foreign-junction\n"
    );
    junction_fixture.assert_exclusive_reacquire();

    let (case_fixture, case_evidence) = application_fixture("windows-casefold");
    let active_path = case_fixture.root.join(".semaprax-workspace/ACTIVE");
    let active_before = std::fs::read(&active_path).unwrap();
    let alias = std::cell::RefCell::new(None::<PathBuf>);
    let error = diagnostic(apply_authenticated_with_hook(
        &case_fixture.root,
        &case_fixture.proposal_path,
        &case_evidence,
        |point, _, _, candidate| {
            if matches!(
                point,
                StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                    workspace::GenerationPoint::DestinationChecked
                ))
            ) {
                let candidate = candidate.unwrap();
                let name = candidate.file_name().unwrap().to_string_lossy();
                let upper = name.to_ascii_uppercase();
                assert_ne!(upper, name);
                let path = candidate.with_file_name(upper);
                std::fs::create_dir(&path)?;
                *alias.borrow_mut() = Some(path);
            }
            Ok(())
        },
    ));
    assert_eq!(error.code, "SPX-I211");
    assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
    assert!(alias.into_inner().unwrap().is_dir());
    case_fixture.assert_exclusive_reacquire();
}

#[cfg(windows)]
#[test]
fn structural_apply_rejects_windows_readonly_permission_drift() {
    for case in ["lock", "active", "candidate"] {
        let (fixture, evidence_path) = application_fixture(&format!("windows-readonly-{case}"));
        let control = fixture.root.join(".semaprax-workspace");
        let active_path = control.join("ACTIVE");
        let active_before = std::fs::read(&active_path).unwrap();
        let changed = std::cell::RefCell::new(None::<(PathBuf, std::fs::Permissions)>);
        let error = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, active, _, candidate| {
                if !matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck
                    )
                ) {
                    return Ok(());
                }
                let path = match case {
                    "lock" => control.join("LOCK"),
                    "active" => active.to_owned(),
                    "candidate" => candidate.unwrap().join("manifest.json"),
                    _ => unreachable!(),
                };
                let original = std::fs::metadata(&path)?.permissions();
                let mut altered = original.clone();
                altered.set_readonly(!altered.readonly());
                std::fs::set_permissions(&path, altered)?;
                *changed.borrow_mut() = Some((path, original));
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-G153");
        assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
        let (path, permissions) = changed.into_inner().unwrap();
        std::fs::set_permissions(path, permissions).unwrap();
        fixture.assert_exclusive_reacquire();
    }
}

#[cfg(windows)]
#[test]
fn structural_apply_rejects_windows_same_byte_file_index_substitution() {
    let (fixture, evidence_path) = application_fixture("windows-file-index");
    let active_path = fixture.root.join(".semaprax-workspace/ACTIVE");
    let active_before = std::fs::read(&active_path).unwrap();
    let identities = std::cell::RefCell::new(None::<(u64, u64)>);
    let error = diagnostic(apply_authenticated_with_hook(
        &fixture.root,
        &fixture.proposal_path,
        &evidence_path,
        |point, _, _, candidate| {
            if !matches!(
                point,
                StructuralApplyPoint::Workspace(
                    workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck
                )
            ) {
                return Ok(());
            }
            let path = candidate.unwrap().join("files/z/entry.spx");
            let before = winapi_util::Handle::from_path_any(&path)
                .and_then(winapi_util::file::information)?
                .file_index();
            let bytes = std::fs::read(&path)?;
            std::fs::remove_file(&path)?;
            std::fs::write(&path, bytes)?;
            let after = winapi_util::Handle::from_path_any(&path)
                .and_then(winapi_util::file::information)?
                .file_index();
            assert_ne!(before, after);
            *identities.borrow_mut() = Some((before, after));
            Ok(())
        },
    ));
    assert_eq!(error.code, "SPX-G153");
    assert!(identities.into_inner().is_some());
    assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
    fixture.assert_exclusive_reacquire();
}
