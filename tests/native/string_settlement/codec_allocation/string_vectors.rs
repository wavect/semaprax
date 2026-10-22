//! OPT808: actual slot borrows, short-output preflight and fatal boundaries.
use super::*;
const INPUT_V2: &str = r#" {"rows":["same","same","\u00e9\u0000\ud83d\ude00","\"\\"],"ok":true,"config":{"retry":2,"label":"\u00e9\u0000"}} "#;
const OUTPUT_V2: &str = "{\"config\":{\"label\":\"é\\u0000\",\"retry\":2},\"rows\":[\"same\",\"same\",\"é\\u0000😀\",\"\\\"\\\\\"],\"ok\":true}";
fn app_v2(invalid: bool) -> String {
    let bytes = OUTPUT_V2
        .bytes()
        .map(|b| format!("{b}u8"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"module consumer.app;
use type @id("fault.report") from consumer.schema as Report;
use type @id("fault.report.json.nested.decode-result") from consumer.schema as Decoded;
use type @id("fault.report.json.nested-response-v2.encode-result") from consumer.schema as Encoded;
use function @id("fault.report.json.nested.decode") from consumer.schema as decode;
use function @id("fault.report.json.nested-response-v2.encode") from consumer.schema as encode;
use function @id("fault.report.json.nested-response-v2.encoded-len") from consumer.schema as encoded_len;
@id("fault.decision") fn decision(values:borrow Vec<string>)->bool{{
let count=vec_len<string>(values);if count==4usize{{let first=vec_str_at(values,0usize);let last=vec_str_at(values,count-1usize);str_len_bytes(first)==4usize && str_len_bytes(last)==2usize}}else{{false}}
}}
@id("fault.decode") fn decode_only(input:borrow Slice<u8>)->i64{{let outcome=decode(input,byte_len(input));match own outcome{{Decoded::Error{{code,offset,field}}=>0,Decoded::Ready{{value}}=>if value.config.retry==2usize && value.ok{{724}}else{{0}},}}}}
@id("fault.preflight") fn preflight(input:borrow Slice<u8>)->i64{{let outcome=decode(input,byte_len(input));match own outcome{{Decoded::Error{{code,offset,field}}=>0,Decoded::Ready{{value}}=>{{
let accepted=decision(value.rows);let required=encoded_len(value);let short=encode(value,required-1usize);
let refused=match own short{{Encoded::Refused{{required:n}}=>n==required,Encoded::Encoded{{text}}=>false,}};
if accepted && refused && required=={length}usize{{724}}else{{0}}
}},}}}}
@id("fault.compose") fn compose(input:borrow Slice<u8>)->i64{{let outcome=decode(input,byte_len(input));match own outcome{{Decoded::Error{{code,offset,field}}=>0,Decoded::Ready{{value}}=>{{
let accepted=decision(value.rows);let required=encoded_len(value);let encoded=encode(value,required);
match own encoded{{Encoded::Refused{{required:n}}=>if {invalid} && n==18446744073709551615usize {{724}}else{{0}},Encoded::Encoded{{text}}=>{{
let expected=[{bytes}];let expected_view=array_as_slice(expected);let actual=str_as_bytes(string_as_str(text));let mut at=0usize;let mut same=byte_len(actual)==byte_len(expected_view);
while same && at<byte_len(actual){{same=match byte_get(actual,at){{Option::None{{}}=>false,Option::Some{{value:a}}=>match byte_get(expected_view,at){{Option::None{{}}=>false,Option::Some{{value:b}}=>a==b,}},}};at=at+1usize;same && at<byte_len(actual)}}
if accepted && same && required=={length}usize{{724}}else{{0}}
}},}}
}},}}}}
@id("consumer.main") fn main()->i64{{let bytes=[0u8];let input=array_as_slice(bytes);let first=decode_only(input);let second=preflight(input);let third=compose(input);first+second+third}}
"#,
        length = if invalid { usize::MAX } else { OUTPUT_V2.len() },
        invalid = invalid
    )
}
// Diagnostic build only: all compiler owners unwind/drop before reporting.
// This labels the fixture bytes, not an authenticated repository revision.
#[cfg(feature = "unstable-workflow-profiling")]
fn observed_generated_v2(response: project::JsonCodecProfile, invalid: bool) -> String {
    eprintln!(
        "PLAN_GENERATED_PROJECT_SOURCE_EXPECTED checkpoints=initial-written,request-codec-written,response-codec-written,final-app-written; only reached checkpoints emit four actual byte hashes; absent checkpoints remain unobserved"
    );
    use semaprax::workflow_profile::plan_comparison::{Binding, Capture};
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(SCHEMA.replace("Vec<Row>", "Vec<string>").as_bytes());
    digest.update(include_bytes!("../bulk_utf8/semaprax.toml"));
    digest.update(INPUT_V2.as_bytes());
    digest.update(OUTPUT_V2.as_bytes());
    digest.update(b"request=NestedRequest/64/256; generated-v2-v1");
    digest.update(app_v2(invalid).as_bytes());
    digest.update(format!("{response:?}/{invalid}").as_bytes());
    let capture = match Capture::begin(Binding::caller_labelled(
        "native/string-vectors/generated-v2; revision=unknown (bind external round manifest); caller-labelled",
        digest.finalize().into(),
    )) {
        Ok(capture) => capture,
        Err(error) => {
            eprintln!("PLAN_COMPARISON_DIAGNOSTIC_REFUSED {error:?}");
            return generated_v2(response, invalid);
        }
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        generated_v2(response, invalid)
    }));
    match capture.finish() {
        Ok(report) => {
            use semaprax::workflow_profile::plan_comparison::Site;
            eprintln!(
        "PLAN_COMPARISON_DIAGNOSTIC build={:?} generation={} P={} N={} STEP={} LOAD={} EMPTY={} TAIL={} left_head={} right_head={} left_exhaustion={} right_exhaustion={} terminal={} started={} ended={} success={} unwind={} counts_complete={} detail_complete={} overflow={}",
        report.build, report.generation,
        report.counts.at(Site::PairPrepared), report.counts.at(Site::Round),
        report.counts.at(Site::StepAdmitted), report.counts.at(Site::LoadAdmitted),
        report.counts.at(Site::EmptySegment), report.counts.at(Site::TailAdmitted),
        report.counts.at(Site::LeftHead), report.counts.at(Site::RightHead),
        report.counts.at(Site::LeftExhausted), report.counts.at(Site::RightExhausted),
        report.counts.at(Site::TerminalCompared), report.counts.started, report.counts.ended,
        report.counts.succeeded, report.counts.unwound,
        report.counts_complete, report.detail_complete, report.overflow,
    );
            eprintln!("PLAN_COMPARISON_DIAGNOSTIC_DETAILS {report:?}");
        }
        Err(error) => eprintln!("PLAN_COMPARISON_DIAGNOSTIC_INCOMPLETE {error:?}"),
    }
    match result {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
// Host-fixture diagnostics only: hash actual written bytes before each compiler
// admission. Unreached checkpoints have no equality claim. This does not emit
// compiler events, change generated source, or enter the generated-C allocator.
#[cfg(feature = "unstable-workflow-profiling")]
fn generated_project_fingerprints(
    root: &std::path::Path,
    checkpoint: &'static str,
    response: project::JsonCodecProfile,
    invalid: bool,
) {
    use sha2::{Digest, Sha256};
    for relative_path in [
        "semaprax.toml",
        "src/schema.spx",
        "src/app.spx",
        "src/tests.spx",
    ] {
        let bytes = fs::read(root.join(relative_path)).expect("written native fixture source");
        let sha256 = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        eprintln!(
            "PLAN_GENERATED_PROJECT_SOURCE {}",
            serde_json::json!({
                "checkpoint": checkpoint, "response_profile": format!("{response:?}"),
                "invalid": invalid, "relative_path": relative_path,
                "byte_length": bytes.len(), "sha256": sha256,
                "scope": "reached-written-source-only; absent stages unobserved"
            })
        );
    }
}
fn generated_v2(response: project::JsonCodecProfile, invalid: bool) -> String {
    let root = std::env::temp_dir().join(format!(
        "native-codec-allocation-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("semaprax.toml"),
        include_str!("../bulk_utf8/semaprax.toml"),
    )
    .unwrap();
    fs::write(
        root.join("src/schema.spx"),
        canonical(&SCHEMA.replace("Vec<Row>", "Vec<string>")),
    )
    .unwrap();
    for (path, source) in [
        (
            "src/app.spx",
            "module consumer.app;@id(\"consumer.main\") fn main()->i64{0}",
        ),
        (
            "src/tests.spx",
            "module consumer.tests;@id(\"consumer.tests\") fn main()->i64{0}",
        ),
    ] {
        fs::write(root.join(path), canonical(source)).unwrap();
    }
    #[cfg(feature = "unstable-workflow-profiling")]
    generated_project_fingerprints(&root, "initial-written", response, invalid);
    for policy in [
        project::JsonCodecProfile::NestedRequest {
            max_string_bytes: 64,
            max_array_items: 256,
        },
        response,
    ] {
        let source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            let source = project::derive_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "fault.report",
                policy,
            )?;
            project::verify_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "fault.report",
                &source,
                policy,
            )?;
            assert_eq!(source, canonical(&source));
            Ok(source)
        })
        .unwrap();
        fs::write(root.join("src/schema.spx"), source).unwrap();
        #[cfg(feature = "unstable-workflow-profiling")]
        generated_project_fingerprints(
            &root,
            if matches!(&policy, project::JsonCodecProfile::NestedRequest { .. }) {
                "request-codec-written"
            } else {
                "response-codec-written"
            },
            response,
            invalid,
        );
    }
    fs::write(root.join("src/app.spx"), canonical(&app_v2(invalid))).unwrap();
    #[cfg(feature = "unstable-workflow-profiling")]
    generated_project_fingerprints(&root, "final-app-written", response, invalid);
    let result = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        let first = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        let second = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        assert_eq!(first, second);
        // main reaches both roots, so entry linking preserves both symbols.
        for id in ["fault.decode", "fault.compose"] {
            assert!(first.contains(&symbol(id)));
        }
        Ok(first)
    })
    .unwrap();
    fs::remove_dir_all(root).unwrap();
    result
}
// Probe-only forwarding: count the exact emitted descriptor operation while the
// original total-copy observer still sees every production memcpy.
fn observe_slot_copies(generated: String) -> String {
    let backend =
        include_str!("../../../../src/codegen/native_emit/expression/vec_ops/string_slot.rs");
    for exact in [
        "layout.kinds.as_slice() != [9] || layout.offsets.as_slice() != [0]",
        "spx_leaf_read_field(spx_ctx, &({}), &{}, {}, UINT32_C(0), &{field})",
        "memcpy(&{text}, {field}, sizeof({text}));",
    ] {
        assert!(
            backend.contains(exact),
            "String-slot metadata ownership premise changed: {exact}"
        );
    }
    let needle = "memcpy(&spx_string_slot_text_";
    let count = generated.matches(needle).count();
    assert!(
        count >= 4,
        "decision first/last plus length/render slot reads"
    );
    for line in generated.lines().filter(|line| line.contains(needle)) {
        let suffix = line
            .split(needle)
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap();
        assert!(suffix.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(line.trim(), format!("memcpy(&spx_string_slot_text_{suffix}, spx_string_slot_{suffix}, sizeof(spx_string_slot_text_{suffix}));"));
        assert!(generated.contains(&format!("&spx_string_slot_{suffix});")));
    }
    let watched = generated.replace(needle, "fixture_slot_memcpy(&spx_string_slot_text_");
    assert_eq!(
        watched.replace("fixture_slot_memcpy(&spx_string_slot_text_", needle),
        generated
    );
    watched
}
fn expected_wire_bytes() -> String {
    OUTPUT_V2
        .bytes()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
#[test]
fn primitive_string_codec_reads_and_short_refusal_allocate_nothing_then_copy_final_string_once() {
    #[cfg(not(feature = "unstable-workflow-profiling"))]
    let generated = observe_slot_copies(generated_v2(
        project::JsonCodecProfile::NestedResponseV2 {
            max_string_bytes: 64,
            max_array_items: 256,
        },
        false,
    ));
    #[cfg(feature = "unstable-workflow-profiling")]
    let generated = observe_slot_copies(observed_generated_v2(
        project::JsonCodecProfile::NestedResponseV2 {
            max_string_bytes: 64,
            max_array_items: 256,
        },
        false,
    ));
    let raw = INPUT_V2
        .bytes()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let allocator = include_str!("allocator.c");
    let copies = include_str!("../bulk_utf8/copies.c");
    let classified_copies = include_str!("string_slot_copies.c");
    let expected_bytes = expected_wire_bytes();
    let physical = format!(
        r#"
static void physical_checks(void) {{
fixture_copy_classification_control();
fixture_payload_watch(fixture_expected_wire,sizeof fixture_expected_wire);
fixture_copy_reset();size_t decoded=baseline(false),decode_copies=fixture_copy_calls,decode_bytes=fixture_copy_bytes;
REQUIRE(fixture_slot_copy_calls==0 && fixture_other_copy_calls==decode_copies && fixture_other_copy_bytes==decode_bytes && fixture_payload_copy_calls==0);
struct spx_status_entry entries[16];struct spx_context context={{0}};context_init(&context,entries);
fault_attempts=0;fault_selected=0;fixture_copy_reset();int64_t result=INT64_MIN;
REQUIRE(FAULT_PREFLIGHT(&context,input_view(),&result)==SPX_STATUS_SUCCESS && result==724);
REQUIRE(fault_attempts==decoded && fixture_other_copy_calls==decode_copies && fixture_other_copy_bytes==decode_bytes);
REQUIRE(fixture_slot_copy_calls>0 && fixture_slot_copy_bytes==fixture_slot_copy_calls*sizeof(const char *));
REQUIRE(fixture_copy_calls==decode_copies+fixture_slot_copy_calls && fixture_copy_bytes==decode_bytes+fixture_slot_copy_bytes);
REQUIRE(fixture_payload_copy_calls==0);size_t preflight_slots=fixture_slot_copy_calls;
REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
REQUIRE(context.call_depth==0 && context.borrowed_str_depth==0);
fixture_copy_reset();size_t composed=baseline(true);
REQUIRE(composed==decoded+2); /* one actual Bytes reservation, one final String */
REQUIRE(fixture_other_copy_calls==decode_copies+1 && fixture_other_copy_bytes==decode_bytes+{});
REQUIRE(fixture_slot_copy_calls>preflight_slots && fixture_slot_copy_bytes==fixture_slot_copy_calls*sizeof(const char *));
REQUIRE(fixture_copy_calls==fixture_other_copy_calls+fixture_slot_copy_calls && fixture_copy_bytes==fixture_other_copy_bytes+fixture_slot_copy_bytes);
REQUIRE(fixture_payload_copy_calls==1 && fixture_payload_copy_bytes==sizeof fixture_expected_wire);
}}
"#,
        OUTPUT_V2.len()
    );
    let probe = include_str!("probe.c")
        .replace(
            "static void child(",
            &format!("{physical}\nstatic void child("),
        )
        .replace(
            "size_t decoded=baseline(false),composed=baseline(true);",
            "physical_checks();size_t decoded=baseline(false),composed=baseline(true);",
        );
    let source = format!(
        r#"#define _GNU_SOURCE
{STDIO}
#define FIXTURE_TRACK_CALLOC
{OBSERVER}
{copies}
static const uint8_t fixture_expected_wire[]={{{expected_bytes}}};
{classified_copies}
#undef malloc
#undef calloc
{allocator}
#define malloc fault_malloc
#define calloc fault_calloc
#define main fixture_generated_main
{generated}
#undef main
#undef malloc
#undef calloc
#undef free
#undef memcpy
#define FAULT_DECODE {}
#define FAULT_COMPOSE {}
#define FAULT_PREFLIGHT {}
static uint8_t fault_input[]={{{raw}}};
{probe}
"#,
        symbol("fault.decode"),
        symbol("fault.compose"),
        symbol("fault.preflight")
    );
    // Same complete fault injection receiver, including fatal/nonrecoverable policy.
    compile_and_run("primitive-string-codec-allocation", &source, false);
}

#[test]
fn primitive_string_codec_actual_byte_and_item_plus_one_preflight_never_reserve_output() {
    for (label, policy) in [
        (
            "bytes",
            project::JsonCodecProfile::NestedResponseV2 {
                max_string_bytes: 6,
                max_array_items: 256,
            },
        ),
        (
            "items",
            project::JsonCodecProfile::NestedResponseV2 {
                max_string_bytes: 64,
                max_array_items: 3,
            },
        ),
    ] {
        #[cfg(not(feature = "unstable-workflow-profiling"))]
        let generated = observe_slot_copies(generated_v2(policy, true));
        #[cfg(feature = "unstable-workflow-profiling")]
        let generated = observe_slot_copies(observed_generated_v2(policy, true));
        let raw = INPUT_V2
            .bytes()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let allocator = include_str!("allocator.c");
        let copies = include_str!("../bulk_utf8/copies.c");
        let classified_copies = include_str!("string_slot_copies.c");
        let expected_bytes = expected_wire_bytes();
        let prefix = include_str!("probe.c")
            .split_once("static void child(")
            .unwrap()
            .0;
        let source = format!(
            r#"#define _GNU_SOURCE
{STDIO}
#define FIXTURE_TRACK_CALLOC
{OBSERVER}
{copies}
static const uint8_t fixture_expected_wire[]={{{expected_bytes}}};
{classified_copies}
#undef malloc
#undef calloc
{allocator}
#define malloc fault_malloc
#define calloc fault_calloc
#define main fixture_generated_main
{generated}
#undef main
#undef malloc
#undef calloc
#undef free
#undef memcpy
#define FAULT_DECODE {}
#define FAULT_COMPOSE {}
#define FAULT_PREFLIGHT {}
static uint8_t fault_input[]={{{raw}}};
{prefix}
int main(void){{
REQUIRE(fixture_binary_stdout());
fault_borrowed_input=mmap(NULL,sizeof fault_input,PROT_READ|PROT_WRITE,MAP_SHARED|MAP_ANONYMOUS,-1,0);REQUIRE(fault_borrowed_input!=MAP_FAILED);
memcpy(fault_borrowed_input,fault_input,sizeof fault_input);REQUIRE(mprotect(fault_borrowed_input,sizeof fault_input,PROT_READ)==0);
fixture_copy_classification_control();fixture_payload_watch(fixture_expected_wire,sizeof fixture_expected_wire);
fixture_copy_reset();size_t decoded=baseline(false),calls=fixture_copy_calls,bytes=fixture_copy_bytes;
REQUIRE(fixture_slot_copy_calls==0 && fixture_other_copy_calls==calls && fixture_other_copy_bytes==bytes && fixture_payload_copy_calls==0);
struct spx_status_entry entries[16];struct spx_context context={{0}};context_init(&context,entries);
fault_attempts=0;fault_selected=0;fixture_copy_reset();int64_t result=INT64_MIN;
REQUIRE(FAULT_PREFLIGHT(&context,input_view(),&result)==SPX_STATUS_SUCCESS && result==724);
REQUIRE(fault_attempts==decoded && fixture_other_copy_calls==calls && fixture_other_copy_bytes==bytes);
REQUIRE(fixture_slot_copy_calls>0 && fixture_slot_copy_bytes==fixture_slot_copy_calls*sizeof(const char *));
REQUIRE(fixture_copy_calls==calls+fixture_slot_copy_calls && fixture_copy_bytes==bytes+fixture_slot_copy_bytes && fixture_payload_copy_calls==0);
REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees && context.call_depth==0 && context.borrowed_str_depth==0);
fixture_copy_reset();REQUIRE(baseline(true)==decoded && fixture_other_copy_calls==calls && fixture_other_copy_bytes==bytes);
REQUIRE(fixture_slot_copy_calls>0 && fixture_slot_copy_bytes==fixture_slot_copy_calls*sizeof(const char *));
REQUIRE(fixture_copy_calls==calls+fixture_slot_copy_calls && fixture_copy_bytes==bytes+fixture_slot_copy_bytes && fixture_payload_copy_calls==0);
REQUIRE(!memcmp(fault_borrowed_input,fault_input,sizeof fault_input));REQUIRE(munmap(fault_borrowed_input,sizeof fault_input)==0);
(void)puts("native-ordinary-strings-settled");return 0;
}}
"#,
            symbol("fault.decode"),
            symbol("fault.compose"),
            symbol("fault.preflight")
        );
        compile_and_run(
            &format!("primitive-string-codec-{label}-refusal"),
            &source,
            false,
        );
    }
}

// Exact public-workspace host for production-library paid attribution. This
// does not assert admission when the unchanged 64MiB builder refuses.
#[cfg(feature = "unstable-workflow-profiling")]
#[test]
fn catalog_normalizer_public_workspace_reports_actual_paid_validation_phases() {
    use semaprax::workflow_profile::plan_comparison::{
        Binding, Build, Capture, PaidPhase, PhaseOrigin, PhasePayer,
    };
    use sha2::{Digest, Sha256};
    const MANIFEST: &str =
        include_str!("../../../../examples/catalog-normalizer-project/semaprax.toml");
    const SOURCES: &[(&str, &str)] = &[
        (
            "src/app.spx",
            include_str!("../../../../examples/catalog-normalizer-project/src/app.spx"),
        ),
        (
            "src/batch.spx",
            include_str!("../../../../examples/catalog-normalizer-project/src/batch.spx"),
        ),
        (
            "src/enrichment.spx",
            include_str!("../../../../examples/catalog-normalizer-project/src/enrichment.spx"),
        ),
        (
            "src/limits.spx",
            include_str!("../../../../examples/catalog-normalizer-project/src/limits.spx"),
        ),
        (
            "src/record.spx",
            include_str!("../../../../examples/catalog-normalizer-project/src/record.spx"),
        ),
        (
            "src/tests.spx",
            include_str!("../../../../examples/catalog-normalizer-project/src/tests.spx"),
        ),
        (
            "dependencies/std.data.json.dec/0.1.0/dec.spx",
            include_str!("../../../../std/data-json-dec/src/dec.spx"),
        ),
        (
            "dependencies/std.data.json.doc/0.1.0/doc.spx",
            include_str!("../../../../std/data-json-doc/src/doc.spx"),
        ),
        (
            "dependencies/std.io/0.1.0/io.spx",
            include_str!("../../../../std/io/src/io.spx"),
        ),
        (
            "dependencies/std.data.ndjson/0.1.0/ndjson.spx",
            include_str!("../../../../std/data-ndjson/src/ndjson.spx"),
        ),
        (
            "dependencies/std.io.lines/0.1.0/lines.spx",
            include_str!("../../../../std/io-lines/src/lines.spx"),
        ),
        (
            "dependencies/std.data.json.utf8/0.1.0/utf8.spx",
            include_str!("../../../../std/data-json-utf8/src/utf8.spx"),
        ),
    ];
    let manifest = project::ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(manifest.sources().len(), 6);
    for (path, _) in &SOURCES[..6] {
        assert!(manifest.sources().iter().any(|p| p.as_str() == *path));
    }
    assert_eq!(SOURCES.len(), 12);
    let root = std::env::temp_dir().join(format!(
        "native-normalizer-paid-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _remove = Remove(root.clone());
    fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
    let mut ordered = SOURCES.to_vec();
    ordered.sort_by(|left, right| left.0.cmp(right.0));
    let mut digest = Sha256::new();
    digest.update((MANIFEST.len() as u64).to_le_bytes());
    digest.update(MANIFEST.as_bytes());
    let mut files = Vec::new();
    for (path, source) in ordered {
        let destination = root.join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(&destination, source).unwrap();
        let actual = fs::read(&destination).unwrap();
        assert_eq!(actual, source.as_bytes());
        digest.update((path.len() as u64).to_le_bytes());
        digest.update(path.as_bytes());
        digest.update((actual.len() as u64).to_le_bytes());
        digest.update(&actual);
        eprintln!(
            "NORMALIZER_PAID_INPUT path={path} bytes={} sha256={:x}",
            actual.len(),
            semaprax::digest_hex::LowerHex(Sha256::digest(&actual))
        );
        files.push(serde_json::json!({"path":path}));
    }
    let paths = root.join("paths.json");
    fs::write(
        &paths,
        format!(
            "{{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":{}}}\n",
            serde_json::to_string(&files).unwrap()
        ),
    )
    .unwrap();
    let capture = Capture::begin(Binding::caller_labelled("catalog-normalizer exact12/public-semantic-workspace/default64MiB; revision externally bound", digest.finalize().into())).unwrap();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        semaprax::semantic_workspace::initialize(&root, &paths)
    }));
    // The public route has returned or unwound and all compiler owners dropped.
    let report = capture.finish().unwrap();
    eprintln!(
        "NORMALIZER_PAID_PHASES default_builder_cap=67108864 inclusive_not_additive=true report={report:?}"
    );
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    };
    assert_eq!(report.build, Build::InstrumentedProductionAlgorithm);
    let exclusive = &report.resolved_core;
    assert_eq!(exclusive.algorithm, semaprax::workflow_profile::plan_comparison::CoreDebitAlgorithm::InstrumentedProductionAlgorithm);
    assert!(
        exclusive.complete && !exclusive.overflow && !exclusive.lost && !exclusive.invalid,
        "{exclusive:?}"
    );
    assert!(exclusive.entered_attempts > 0);
    assert_eq!(
        exclusive.final_attempt_ordinal,
        Some(exclusive.entered_attempts - 1)
    );
    for attempt in &exclusive.attempts[..exclusive.stored_attempts] {
        assert!(attempt.complete && !attempt.invalid, "{attempt:?}");
        assert_eq!(attempt.stage_sum(), Some(attempt.consumed));
        assert_eq!(attempt.operation_sum(), Some(attempt.consumed));
    }
    eprintln!(
        "NORMALIZER_EXCLUSIVE_CORE_DEBITS reservations_not_allocations=true attempts_nonadditive=true final_attempt_explicit=true report={exclusive:?}"
    );
    assert!(report.paid_phases.balanced && !report.paid_phases.overflow);
    for phase in [
        PaidPhase::CrosscheckEnumerated,
        PaidPhase::CrosscheckFactored,
    ] {
        for payer in [
            PhasePayer::ActiveBuilder,
            PhasePayer::Unobserved,
            PhasePayer::Oracle,
            PhasePayer::Selected,
            PhasePayer::Unknown,
        ] {
            assert_eq!(
                report
                    .paid_phases
                    .row(phase, payer, PhaseOrigin::TestCrosscheck)
                    .entered,
                0,
                "production library has no cfgtest crosscheck"
            );
        }
    }
    let validation = report.paid_phases.row(
        PaidPhase::CleanupValidation,
        PhasePayer::ActiveBuilder,
        PhaseOrigin::Shipping,
    );
    assert!(
        validation.entered > 0 && validation.ended == validation.entered,
        "{report:?}"
    );
    assert!(
        validation.known_paid > 0 && validation.unknown == 0,
        "{report:?}"
    );
    match result {
        Ok(revision) => assert!(!revision.is_empty()),
        Err(errors) => {
            eprintln!("NORMALIZER_PAID_REAL_REFUSAL {errors:?}");
            assert!(
                !errors.is_empty() && errors.iter().all(|e| e.code == "SPX-G171"),
                "{errors:?}"
            );
        }
    }
}
