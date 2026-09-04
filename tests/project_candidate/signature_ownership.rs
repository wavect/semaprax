//! Authored ownership signature regressions over a self-contained project.
//!
//! The project is written here rather than copied from an example so the
//! ownership surface these tests need stays inside the published workspace
//! `max_builder_bytes` budget.
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
const MANIFEST: &str = r#"schema = "semaprax.project.v8"
name = "own-signature"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "frame_payload.app"
sources = ["src/app.spx", "src/frame.spx", "src/tests.spx"]
web_exports = ["frame.public"]
tests = ["frame_payload.tests"]
"#;
const APP: &str = r#"module frame_payload.app;
use function @id("frame.public") from frame_payload.frame as public_value;
@id("frame-payload.app.main") fn main() -> i64 { public_value(0) }
"#;
const TESTS: &str = r#"module frame_payload.tests;
use function @id("frame.public") from frame_payload.frame as public_value;
@id("frame-payload.tests.main") fn main() -> i64 { if public_value(0) == 0 { 0 } else { 1 } }
"#;
const FRAME: &str = r#"module frame_payload.frame;
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
@id("frame.owner-views") fn owner_views(left: own Bytes, flag: i64, right: own Bytes) -> usize {
    byte_len(bytes_as_slice(left)) + if flag == 0 { byte_len(bytes_as_slice(right)) } else { 1usize }
}
@id("frame.owner-views-call") fn owner_views_call(input: borrow Slice<u8>) -> usize {
    owner_views(bytes_copy(input), 9 / 3, bytes_copy(input))
}
@id("frame.owner-views-nine") fn owner_views_nine(
    p0: own Bytes, p1: own Bytes, p2: own Bytes, p3: own Bytes, p4: own Bytes,
    p5: own Bytes, p6: own Bytes, p7: own Bytes, p8: own Bytes
) -> usize {
    byte_len(bytes_as_slice(p0)) + byte_len(bytes_as_slice(p1))
        + byte_len(bytes_as_slice(p2)) + byte_len(bytes_as_slice(p3))
        + byte_len(bytes_as_slice(p4)) + byte_len(bytes_as_slice(p5))
        + byte_len(bytes_as_slice(p6)) + byte_len(bytes_as_slice(p7))
        + byte_len(bytes_as_slice(p8))
}
@id("frame.owner-return") fn owner_return(input: own Bytes) -> Bytes { input }
@id("frame.owner-duplicate-view") fn owner_duplicate_view(input: own Bytes) -> usize {
    byte_len(bytes_as_slice(input)) + byte_len(bytes_as_slice(input))
}
@id("frame.owner-contract") fn owner_contract(input: own Bytes) -> usize
requires byte_len(bytes_as_slice(input)) > 0usize
{ byte_len(bytes_as_slice(input)) }
@id("frame.mixed-owner-views") fn mixed_owner_views(input: own Bytes, text: string, flag: i64) -> usize {
    byte_len(bytes_as_slice(input))
        + if str_len_bytes(string_as_str(text)) == 0 { 0usize } else { 1usize }
        + if flag == 0 { 0usize } else { 1usize }
}
@id("frame.mixed-owner-views-call") fn mixed_owner_views_call(input: borrow Slice<u8>) -> usize {
    mixed_owner_views(bytes_copy(input), string_from_char('x'), 6 / 2)
}
@id("frame.string-owner-contract") fn string_owner_contract(text: string) -> i64
requires str_len_bytes(string_as_str(text)) > 0
{ str_len_bytes(string_as_str(text)) }
@id("frame.bytes-envelope") record BytesEnvelope {
    @id("frame.bytes-envelope.value") value: Bytes,
}
@id("frame.string-envelope") record StringEnvelope {
    @id("frame.string-envelope.value") value: string,
}
@id("frame.bytes-envelope-wide") record BytesEnvelopeWide {
    @id("frame.bytes-envelope-wide.value") value: Bytes,
    @id("frame.bytes-envelope-wide.flag") flag: bool,
}
@id("frame.make-bytes-envelope") fn make_bytes_envelope(input: borrow Slice<u8>) -> BytesEnvelope {
    BytesEnvelope { value: bytes_copy(input) }
}
@id("frame.make-string-envelope") fn make_string_envelope(value: char) -> StringEnvelope {
    StringEnvelope { value: string_from_char(value) }
}
@id("frame.make-bytes-envelope-wide") fn make_bytes_envelope_wide(input: borrow Slice<u8>) -> BytesEnvelopeWide {
    BytesEnvelopeWide { value: bytes_copy(input), flag: true }
}
@id("frame.make-owned-bytes") fn make_owned_bytes(input: borrow Slice<u8>) -> Bytes {
    bytes_copy(input)
}
@id("frame.forward-owned-bytes") fn forward_owned_bytes(input: borrow Slice<u8>) -> Bytes {
    make_owned_bytes(input)
}
@id("frame.make-owned-string") fn make_owned_string(value: char) -> string {
    string_from_char(value)
}
@id("frame.forward-owned-string") fn forward_owned_string(value: char) -> string {
    make_owned_string(value)
}
@id("frame.echo-owned-string") fn echo_owned_string(value: string) -> string {
    value
}
@id("frame.call-echo-owned-string") fn call_echo_owned_string(value: char) -> string {
    echo_owned_string(string_from_char(value))
}
@id("frame.public") fn public_value(value: i64) -> i64 { value }
"#;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-own-signature-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
        for (path, source) in [
            ("src/app.spx", APP),
            ("src/frame.spx", FRAME),
            ("src/tests.spx", TESTS),
        ] {
            let program = semaprax::parse(source, std::path::Path::new(path)).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
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
fn mixed_owned_bytes_and_string_views_stage_left_to_right_then_derive_in_mapping_order() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let change = SemanticChange::new(
        root.revision().project_revision(),
        &json!({
            "kind":"change_function_signature", "target":"frame.mixed-owner-views", "parameters":[
                {"name":"text_view","borrow_str_from_owner":"text"},
                {"from":"flag"},
                {"name":"input_view","borrow_slice_from_owner":"input"}
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
    assert!(source.contains(
        "fn mixed_owner_views(text_view: borrow str, flag: i64, input_view: borrow Slice<u8>)"
    ));
    assert!(source.contains("str_len_bytes(text_view)"));
    assert!(source.contains("byte_len(input_view)"));
    let stage_bytes = source
        .find("let spx_sig_stage_0 = bytes_copy(input)")
        .unwrap();
    let stage_string = source
        .find("let spx_sig_stage_1 = string_from_char('x')")
        .unwrap();
    let stage_flag = source.find("let spx_sig_stage_2 = 6 / 2").unwrap();
    let derive_string = source
        .find("let spx_sig_stage_3 = string_as_str(spx_sig_stage_1)")
        .unwrap();
    let derive_bytes = source
        .find("let spx_sig_stage_4 = bytes_as_slice(spx_sig_stage_0)")
        .unwrap();
    let migrated = source
        .find("mixed_owner_views(spx_sig_stage_3, spx_sig_stage_2, spx_sig_stage_4)")
        .unwrap();
    assert!(stage_bytes < stage_string && stage_string < stage_flag);
    assert!(stage_flag < derive_string && derive_string < derive_bytes && derive_bytes < migrated);
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
fn string_owner_view_rejects_contract_roots_and_mismatched_mapping_kinds() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    for (target, parameters) in [
        (
            "frame.string-owner-contract",
            json!([{"name":"view","borrow_str_from_owner":"text"}]),
        ),
        (
            "frame.mixed-owner-views",
            json!([
                {"name":"wrong","borrow_str_from_owner":"input"},
                {"from":"text"},
                {"from":"flag"}
            ]),
        ),
        (
            "frame.mixed-owner-views",
            json!([
                {"name":"input_view","borrow_slice_from_owner":"input"},
                {"name":"wrong","borrow_slice_from_owner":"text"},
                {"from":"flag"}
            ]),
        ),
    ] {
        let change = SemanticChange::new(
            root.revision().project_revision(),
            &json!({"kind":"change_function_signature","target":target,"parameters":parameters}),
        )
        .unwrap();
        let errors = root.apply(root.candidate_digest(), &change).err().unwrap();
        assert!(
            errors.iter().any(|error| error.code == "SPX-G469"),
            "{errors:?}"
        );
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
fn bounded_distinct_owner_views_derive_in_mapping_order_after_all_old_stages() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let change = SemanticChange::new(
        root.revision().project_revision(),
        &json!({
            "kind":"change_function_signature", "target":"frame.owner-views", "parameters":[
                {"name":"right_view","borrow_slice_from_owner":"right"},
                {"from":"flag"},
                {"name":"left_view","borrow_slice_from_owner":"left"}
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
    assert!(source.contains(
        "fn owner_views(right_view: borrow Slice<u8>, flag: i64, left_view: borrow Slice<u8>)"
    ));
    assert!(source.contains("byte_len(left_view)"));
    assert!(source.contains("byte_len(right_view)"));
    let stage_left = source
        .find("let spx_sig_stage_0 = bytes_copy(input)")
        .unwrap();
    let stage_flag = source.find("let spx_sig_stage_1 = 9 / 3").unwrap();
    let stage_right = source
        .rfind("let spx_sig_stage_2 = bytes_copy(input)")
        .unwrap();
    let derive_right = source
        .find("let spx_sig_stage_3 = bytes_as_slice(spx_sig_stage_2)")
        .unwrap();
    let derive_left = source
        .find("let spx_sig_stage_4 = bytes_as_slice(spx_sig_stage_0)")
        .unwrap();
    let migrated = source
        .find("owner_views(spx_sig_stage_3, spx_sig_stage_1, spx_sig_stage_4)")
        .unwrap();
    assert!(stage_left < stage_flag && stage_flag < stage_right);
    assert!(stage_right < derive_right && derive_right < derive_left && derive_left < migrated);
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
fn bounded_owner_view_set_rejects_duplicate_partial_mixed_and_more_than_eight() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    for (parameters, diagnostic) in [
        (
            json!([
                {"name":"first","borrow_slice_from_owner":"left"},
                {"from":"flag"},
                {"name":"again","borrow_slice_from_owner":"left"}
            ]),
            "SPX-G477",
        ),
        (
            json!([
                {"name":"aliased","borrow_slice_from_owner":"left"},
                {"from":"flag"},
                {"name":"aliased","borrow_slice_from_owner":"right"}
            ]),
            "SPX-G479",
        ),
        (
            json!([
                {"name":"left_view","borrow_slice_from_owner":"left"},
                {"from":"flag"}
            ]),
            "SPX-G260",
        ),
        (
            json!([
                {"name":"view","borrow_slice_from_owner":"flag"},
                {"from":"left"},
                {"from":"right"}
            ]),
            "SPX-G469",
        ),
    ] {
        let change = SemanticChange::new(root.revision().project_revision(), &json!({
            "kind":"change_function_signature","target":"frame.owner-views","parameters":parameters
        })).unwrap();
        let errors = root.apply(root.candidate_digest(), &change).err().unwrap();
        assert!(
            errors.iter().any(|error| error.code == diagnostic),
            "{errors:?}"
        );
    }
    let parameters = (0..9)
        .map(|index| {
            json!({
                "name":format!("view{index}"),
                "borrow_slice_from_owner":format!("p{index}")
            })
        })
        .collect::<Vec<_>>();
    let change = SemanticChange::new(
        root.revision().project_revision(),
        &json!({
            "kind":"change_function_signature",
            "target":"frame.owner-views-nine",
            "parameters":parameters
        }),
    )
    .unwrap();
    let errors = root.apply(root.candidate_digest(), &change).err().unwrap();
    assert!(
        errors.iter().any(|error| error.code == "SPX-G478"),
        "{errors:?}"
    );
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

#[test]
fn whole_owned_results_wrap_once_and_local_callers_move_the_exact_field() {
    let fixture = Fixture::new();
    for (target, record, field, provider, caller) in [
        (
            "frame.make-owned-bytes",
            "frame.bytes-envelope",
            "frame.bytes-envelope.value",
            "fn make_owned_bytes(input: borrow Slice<u8>) -> BytesEnvelope\n{\n    BytesEnvelope { value: { bytes_copy(input) } }\n}",
            "fn forward_owned_bytes(input: borrow Slice<u8>) -> Bytes\n{\n    ({ let spx_sig_stage_0 = input; make_owned_bytes(spx_sig_stage_0) }).value\n}",
        ),
        (
            "frame.make-owned-string",
            "frame.string-envelope",
            "frame.string-envelope.value",
            "fn make_owned_string(value: char) -> StringEnvelope\n{\n    StringEnvelope { value: { string_from_char(value) } }\n}",
            "fn forward_owned_string(value: char) -> string\n{\n    ({ let spx_sig_stage_0 = value; make_owned_string(spx_sig_stage_0) }).value\n}",
        ),
        (
            "frame.echo-owned-string",
            "frame.string-envelope",
            "frame.string-envelope.value",
            "fn echo_owned_string(value: string) -> StringEnvelope\n{\n    StringEnvelope { value: { value } }\n}",
            "fn call_echo_owned_string(value: char) -> string\n{\n    ({ let spx_sig_stage_0 = string_from_char(value); echo_owned_string(spx_sig_stage_0) }).value\n}",
        ),
    ] {
        let root = fixture.candidate();
        let parameters = if target.ends_with("bytes") {
            json!([{"from":"input"}])
        } else {
            json!([{"from":"value"}])
        };
        let change = SemanticChange::new(
            root.revision().project_revision(),
            &json!({"kind":"change_function_signature","target":target,"parameters":parameters,
                "wrap_return":{"record":record,"field":field}}),
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
        assert!(source.contains(provider), "{source}");
        assert!(source.contains(caller), "{source}");
        let replay = ProjectCandidate::replay(
            Arc::clone(root.base_revision()),
            root.base_revision().project_revision(),
            &[change],
            evolved.to_json().as_bytes(),
        )
        .unwrap();
        assert_eq!(replay.candidate_digest(), evolved.candidate_digest());
    }
}

#[test]
fn result_wrapper_shape_and_selector_fail_closed_without_mutating_the_candidate() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let before = root.to_json().to_owned();
    for (record, field, diagnostic) in [
        (
            "frame.bytes-envelope-wide",
            "frame.bytes-envelope-wide.value",
            "SPX-G494",
        ),
        (
            "frame.string-envelope",
            "frame.string-envelope.value",
            "SPX-G494",
        ),
        (
            "frame.bytes-envelope",
            "frame.string-envelope.value",
            "SPX-G494",
        ),
    ] {
        let change = SemanticChange::new(
            root.revision().project_revision(),
            &json!({"kind":"change_function_signature","target":"frame.make-owned-bytes",
                "parameters":[{"from":"input"}],"wrap_return":{"record":record,"field":field}}),
        )
        .unwrap();
        let errors = root
            .apply(root.candidate_digest(), &change)
            .err()
            .expect("invalid owning result wrapper admitted");
        assert!(
            errors.iter().any(|error| error.code == diagnostic),
            "{errors:?}"
        );
        assert_eq!(root.to_json(), before);
    }
}
