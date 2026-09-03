//! Authored, unrun ownership signature regressions; no local gate was executed.
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-own-signature-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/frame-payload-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/frame.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        let path = root.join("src/frame.spx");
        let mut source = std::fs::read_to_string(&path).unwrap();
        source.push_str(
            r#"
@id("frame.own-select") fn own_select(left: own Bytes, right: own Bytes, flag: i64) -> Bytes {
    if flag == 0 { left } else { right }
}
@id("frame.own-call") fn own_call(input: borrow Slice<u8>) -> Bytes {
    own_select(bytes_copy(input), bytes_copy(input), 4 / 2)
}
@id("frame.owner-view") fn owner_view(input: own Bytes, offset: i64) -> usize {
    byte_len(bytes_as_slice(input)) + if offset == 0 { 0usize } else { 1usize }
}
@id("frame.owner-view-call") fn owner_view_call(input: borrow Slice<u8>) -> usize {
    owner_view(bytes_copy(input), 8 / 4)
}
@id("frame.owner-return") fn owner_return(input: own Bytes) -> Bytes { input }
@id("frame.owner-duplicate-view") fn owner_duplicate_view(input: own Bytes) -> usize {
    byte_len(bytes_as_slice(input)) + byte_len(bytes_as_slice(input))
}
@id("frame.owner-contract") fn owner_contract(input: own Bytes) -> usize
requires byte_len(bytes_as_slice(input)) > 0usize
{ byte_len(bytes_as_slice(input)) }
"#,
        );
        let program = semaprax::parse(&source, &path).unwrap();
        std::fs::write(&path, semaprax::format::canonical(&program)).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
}

#[test]
fn own_bytes_replacement_derives_one_caller_view_after_left_to_right_staging_and_replays() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let change = SemanticChange::new(
        root.revision().project_revision(),
        &json!({
            "kind":"change_function_signature", "target":"frame.owner-view", "parameters":[
                {"name":"view","borrow_slice_from_owner":"input"}, {"from":"offset"}
            ]
        }),
    )
    .unwrap();
    let evolved = root.apply(root.candidate_digest(), &change).unwrap();
    let source = evolved
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/frame.spx")
        .unwrap()
        .source();
    assert!(source.contains("fn owner_view(view: borrow Slice<u8>, offset: i64) -> usize"));
    assert!(source.contains("byte_len(view)"));
    let stage_owner = source
        .find("let spx_sig_stage_0 = bytes_copy(input)")
        .unwrap();
    let stage_offset = source.find("let spx_sig_stage_1 = 8 / 4").unwrap();
    let derive_view = source
        .find("let spx_sig_stage_2 = bytes_as_slice(spx_sig_stage_0)")
        .unwrap();
    let migrated_call = source
        .find("owner_view(spx_sig_stage_2, spx_sig_stage_1)")
        .unwrap();
    assert!(
        stage_owner < stage_offset && stage_offset < derive_view && derive_view < migrated_call
    );
    let replayed = ProjectCandidate::replay(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        &[change],
        evolved.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), evolved.to_json());
}

#[test]
fn owner_to_view_rejects_transfer_duplicate_conversion_additive_alias_and_open_mapping() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    for (target, parameters, diagnostic) in [
        (
            "frame.owner-return",
            json!([{"name":"view","borrow_slice_from_owner":"input"}]),
            "SPX-G469",
        ),
        (
            "frame.owner-duplicate-view",
            json!([{"name":"view","borrow_slice_from_owner":"input"}]),
            "SPX-G469",
        ),
        (
            "frame.owner-contract",
            json!([{"name":"view","borrow_slice_from_owner":"input"}]),
            "SPX-G469",
        ),
        (
            "frame.owner-view",
            json!([
                {"from":"input"},
                {"name":"view","borrow_slice_from_owner":"input"},
                {"from":"offset"}
            ]),
            "SPX-G469",
        ),
        (
            "frame.owner-view",
            json!([
                {"name":"view","borrow_slice_from_owner":"input","type":"Slice<u8>"},
                {"from":"offset"}
            ]),
            "SPX-G225",
        ),
    ] {
        let change = SemanticChange::new(
            root.revision().project_revision(),
            &json!({
                "kind":"change_function_signature", "target":target, "parameters":parameters
            }),
        )
        .unwrap();
        let errors = root
            .apply(root.candidate_digest(), &change)
            .err()
            .expect("unsupported owner-to-view migration admitted");
        assert!(
            errors.iter().any(|error| error.code == diagnostic),
            "{errors:?}"
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn bytes_reorder_and_rename_replay_through_full_project_without_source_writes() {
    let fixture = Fixture::new();
    let source_before = std::fs::read(fixture.0.join("src/frame.spx")).unwrap();
    let root = fixture.candidate();
    let change = SemanticChange::new(
        root.revision().project_revision(),
        &json!({
            "kind":"change_function_signature", "target":"frame.own-select", "parameters":[
                {"from":"right","name":"second"}, {"from":"flag"}, {"from":"left","name":"first"}
            ]
        }),
    )
    .unwrap();
    let evolved = root.apply(root.candidate_digest(), &change).unwrap();
    let source = evolved
        .revision()
        .sources()
        .iter()
        .find(|s| s.path() == "src/frame.spx")
        .unwrap()
        .source();
    assert!(source.contains("fn own_select(second: own Bytes, flag: i64, first: own Bytes)"));
    assert!(source.contains("if flag == 0 { first } else { second }"));
    assert!(source.contains("let spx_sig_stage_0 = bytes_copy(input); let spx_sig_stage_1 = bytes_copy(input); let spx_sig_stage_2 = 4 / 2; own_select(spx_sig_stage_1, spx_sig_stage_2, spx_sig_stage_0)"));
    let replay = ProjectCandidate::replay(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        &[change],
        evolved.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.candidate_digest(), evolved.candidate_digest());
    assert_eq!(
        std::fs::read(fixture.0.join("src/frame.spx")).unwrap(),
        source_before
    );
}

#[test]
fn owned_parameter_cannot_be_dropped_and_duplicate_transfer_is_rejected() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let before = root.to_json().to_owned();
    for (parameters, diagnostic) in [
        (json!([{"from":"left"},{"from":"flag"}]), "SPX-G260"),
        (
            json!([{"from":"left"},{"from":"right"},{"from":"right"},{"from":"flag"}]),
            "SPX-G225",
        ),
    ] {
        let change = SemanticChange::new(root.revision().project_revision(), &json!({
            "kind":"change_function_signature","target":"frame.own-select","parameters":parameters
        })).unwrap();
        let errors = match root.apply(root.candidate_digest(), &change) {
            Ok(_) => panic!("invalid owning migration admitted"),
            Err(errors) => errors,
        };
        assert!(
            errors.iter().any(|error| error.code == diagnostic),
            "{errors:?}"
        );
        assert_eq!(root.to_json(), before);
    }
}
