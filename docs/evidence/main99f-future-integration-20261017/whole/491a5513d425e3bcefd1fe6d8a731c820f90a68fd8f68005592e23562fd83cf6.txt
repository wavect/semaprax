//! OPT808: v2 composes real primitive String-slot borrows; v1 remains its own receiver.
use super::*;

const V2: project::JsonCodecProfile = project::JsonCodecProfile::NestedResponseV2 {
    max_string_bytes: 64,
    max_array_items: 256,
};
fn schema() -> String {
    SCHEMA.replace("Vec<Row>", "Vec<string>")
}
fn imports_v2() -> String {
    imports().replace(
        "r.root.json.nested-response.",
        "r.root.json.nested-response-v2.",
    )
}
fn install_v2(
    label: &str,
    schema: &str,
    app: &str,
    policy: project::JsonCodecProfile,
    request: bool,
) -> std::path::PathBuf {
    let root = fixture(label, schema);
    if request {
        let decoder =
            project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
                project::derive_json_codec_source_with_profile(
                    &snapshot.retain_revision(),
                    "src/schema.spx",
                    "r.root",
                    project::JsonCodecProfile::NestedRequest {
                        max_string_bytes: 64,
                        max_array_items: 256,
                    },
                )
            })
            .unwrap();
        std::fs::write(root.join("src/schema.spx"), decoder).unwrap();
    }
    let generated = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let source = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "r.root",
            policy,
        )?;
        assert_eq!(
            source,
            project::derive_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "r.root",
                policy
            )?
        );
        assert_eq!(canonical(&source), source);
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "r.root",
            &source,
            policy,
        )?;
        assert!(source.contains("vec_str_at(values, at)"));
        assert!(!source.contains("vec_clone_at"));
        let project::JsonCodecProfile::NestedResponseV2 {
            max_string_bytes,
            max_array_items,
        } = policy
        else {
            panic!("v2 policy")
        };
        for changed in [
            source.replace("vec_str_at(values, at)", "vec_clone_at<string>(values, at)"),
            source.replace(
                ".json.nested-response-v2.encode",
                ".json.nested-response.encode",
            ),
            source.replace(
                &format!("count <= {max_array_items}usize"),
                &format!("count <= {}usize", max_array_items + 1),
            ),
        ] {
            assert_ne!(changed, source);
            assert_eq!(
                project::verify_json_codec_source_with_profile(
                    &revision,
                    "src/schema.spx",
                    "r.root",
                    &changed,
                    policy
                )
                .unwrap_err()[0]
                    .code,
                "SPX-J180"
            );
        }
        for stale in [
            project::JsonCodecProfile::NestedResponse {
                max_string_bytes,
                max_array_items,
            },
            project::JsonCodecProfile::NestedResponseV2 {
                max_string_bytes: if max_string_bytes == 64 {
                    63
                } else {
                    max_string_bytes + 1
                },
                max_array_items,
            },
            project::JsonCodecProfile::NestedResponseV2 {
                max_string_bytes,
                max_array_items: if max_array_items == 256 {
                    255
                } else {
                    max_array_items + 1
                },
            },
        ] {
            assert_eq!(
                project::verify_json_codec_source_with_profile(
                    &revision,
                    "src/schema.spx",
                    "r.root",
                    &source,
                    stale
                )
                .unwrap_err()[0]
                    .code,
                "SPX-J180"
            );
        }
        Ok(source)
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), generated).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(app)).unwrap();
    root
}
fn qualify_v2(root: &std::path::Path) {
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        let revision = snapshot.retain_revision();
        let graph = revision.semantic_graph();
        assert!(graph.contains("r.root.json.nested-response-v2.encode"));
        let options = project::ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000)
            .map_err(|error| vec![error])?;
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(724)
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        assert_eq!(
            c,
            codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?
        );
        for optimization in ["-O0", "-O2"] {
            super::super::super::compile_and_run_c(&c, root, optimization, "724");
        }
        let wasm =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        std::fs::write(root.join("app.wasm"), wasm).unwrap();
        Ok(())
    })
    .unwrap();
    let host = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/owned_data/owned_leaf_vec/host.js");
    let output = Command::new("node")
        .arg(host)
        .arg(root.join("app.wasm"))
        .args(["0", "724", "none"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn text(value: &str) -> String {
    if value.is_empty() {
        "\"\"".to_owned()
    } else {
        format!(
            "{{let raw={};string_from_utf8(array_as_slice(raw))}}",
            array(value.as_bytes())
        )
    }
}
fn expected(label: &str, words: &[String]) -> String {
    // Independent JSON serializer owns values/escapes; generator code is not reused.
    format!(
        "{{\"config\":{{\"label\":{},\"retry\":0}},\"rows\":{},\"ok\":true}}",
        serde_json::to_string(label).unwrap(),
        serde_json::to_string(words).unwrap()
    )
}
#[test]
fn primitive_string_request_decision_response_keeps_duplicate_unicode_and_boundary_slots() {
    let cases = [
        Vec::new(),
        vec![String::new()],
        vec!["same".into(), "same".into()],
        vec!["é\0😀".into(), "\"\\\n\t".into()],
        vec!["a".repeat(64), "\0".repeat(64)],
        (0..256)
            .map(|at| {
                if at == 0 {
                    "first".to_owned()
                } else if at == 255 {
                    "last".to_owned()
                } else {
                    "same".to_owned()
                }
            })
            .collect(),
    ];
    for (ordinal, words) in cases.iter().enumerate() {
        let wire = format!(
            " {{\"ok\":true,\"rows\":{},\"config\":{{\"retry\":0,\"label\":\"\"}}}} ",
            serde_json::to_string(words).unwrap()
        );
        let expected = expected("", words);
        let first = words.first().map_or("", String::as_str);
        let last = words.last().map_or("", String::as_str);
        let decision = format!(
            r#"
@id("consumer.decision") fn decision(values:borrow Vec<string>)->bool {{
if vec_len<string>(values)==0usize {{true}}else{{
let first=vec_str_at(values,0usize);let last=vec_str_at(values,vec_len<string>(values)-1usize);
let expected_first={};let expected_last={};
equal(first,string_as_str(expected_first)) && equal(last,string_as_str(expected_last))
}}
}}
"#,
            text(first),
            text(last)
        );
        let app = imports_v2().replace(
            "@id(\"consumer.equal\")",
            r#"use type @id("r.root.json.nested.decode-result") from consumer.schema as Decoded;
use function @id("r.root.json.nested.decode") from consumer.schema as decode;
@id("consumer.equal")"#,
        ) + &decision
            + &format!(
                r#"
@id("consumer.main") fn main()->i64{{
let outcome={{let input={};decode(array_as_slice(input),{}usize)}};
match own outcome{{Decoded::Error{{code,offset,field}}=>0,Decoded::Ready{{value}}=>{{
let accepted=decision(value.rows);let required=encoded_len(value);
let short=encode(value,required-1usize);
let refused=match own short{{Encoded::Refused{{required:n}}=>n==required,Encoded::Encoded{{text:t}}=>false,}};
let full=encode(value,required);let expected_raw={};let expected=string_from_utf8(array_as_slice(expected_raw));
let same=match own full{{Encoded::Refused{{required:n}}=>false,Encoded::Encoded{{text:t}}=>equal(string_as_str(t),string_as_str(expected)),}};
if accepted && refused && same && required=={}usize{{724}}else{{0}}
}},}}
}}
"#,
                array(wire.as_bytes()),
                wire.len(),
                array(expected.as_bytes()),
                expected.len()
            );
        let root = install_v2(
            &format!("primitive-string-roundtrip-{ordinal}"),
            &schema(),
            &app,
            V2,
            true,
        );
        qualify_v2(&root);
        std::fs::remove_dir_all(root).unwrap();
    }
}
fn build_words(name: &str, words: &[String]) -> String {
    let mut output = format!("@id(\"consumer.{name}\") fn {name}()->Vec<string>{{let mut words=vec_with_capacity<string>({}usize);",words.len());
    for word in words {
        output += &format!("words=vec_push<string>(words,{});", text(word));
    }
    output + "words}\n"
}
#[test]
fn primitive_string_response_actual_item_and_byte_plus_one_refuse_before_publication() {
    for (ordinal, words) in [
        vec!["a".repeat(64)],
        vec!["a".repeat(65)],
        vec![String::new(); 8],
        vec![String::new(); 9],
        vec!["é".repeat(32)],
        vec!["é".repeat(32) + "a"],
        vec![String::new(); 256],
        vec![String::new(); 257],
    ]
    .iter()
    .enumerate()
    {
        let valid = ordinal % 2 == 0;
        let app = imports_v2()
            + &build_words("words", words)
            + &format!(
                r#"
@id("consumer.main") fn main()->i64{{
let value=Report{{config:Config{{label:"",retry:0usize}},rows:words(),ok:true}};
let required=encoded_len(value);let outcome=encode(value,131072usize);
let valid=match own outcome{{Encoded::Encoded{{text:t}}=>required<18446744073709551615usize,Encoded::Refused{{required:n}}=>false,}};
if valid=={} && (valid || required==18446744073709551615usize){{724}}else{{0}}
}}
"#,
                valid
            );
        let policy = project::JsonCodecProfile::NestedResponseV2 {
            max_string_bytes: 64,
            max_array_items: if ordinal >= 6 { 256 } else { 8 },
        };
        let root = install_v2(
            &format!("primitive-string-limits-{ordinal}"),
            &schema(),
            &app,
            policy,
            false,
        );
        qualify_v2(&root);
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn primitive_string_response_two_nested_paths_preserve_exact_and_one_short_bytes() {
    let schema = SCHEMA
        .replace("retry:usize", "retry:Vec<string>")
        .replace("rows:Vec<Row>", "rows:Config");
    let left = vec!["same".to_owned(), "same".to_owned(), "é\0".to_owned()];
    let right = vec![String::new(), "\"\\😀".to_owned()];
    let expected=format!("{{\"config\":{{\"label\":\"left\",\"retry\":{}}},\"rows\":{{\"label\":\"right\",\"retry\":{}}},\"ok\":true}}",serde_json::to_string(&left).unwrap(),serde_json::to_string(&right).unwrap());
    let app = imports_v2()
        + &build_words("left", &left)
        + &build_words("right", &right)
        + &format!(
            r#"
@id("consumer.main") fn main()->i64{{
let value=Report{{config:Config{{label:"left",retry:left()}},rows:Config{{label:"right",retry:right()}},ok:true}};
let required=encoded_len(value);let short=encode(value,required-1usize);
let refused=match own short{{Encoded::Refused{{required:n}}=>n==required,Encoded::Encoded{{text:t}}=>false,}};
let full=encode(value,required);let expected_raw={};let expected=string_from_utf8(array_as_slice(expected_raw));
let same=match own full{{Encoded::Refused{{required:n}}=>false,Encoded::Encoded{{text:t}}=>equal(string_as_str(t),string_as_str(expected)),}};
if refused && same && required=={}usize{{724}}else{{0}}
}}
"#,
            array(expected.as_bytes()),
            expected.len()
        );
    let root = install_v2("primitive-string-two-paths", &schema, &app, V2, false);
    qualify_v2(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn primitive_string_response_maximum_escaping_exact_output_cap_and_plus_one() {
    let schema = SCHEMA
        .replace("retry:usize", "retry:Vec<string>")
        .replace("rows:Vec<Row>", "rows:Config");
    let left = vec!["\0".repeat(64); 256];
    let wire = |right: &[String]| {
        format!("{{\"config\":{{\"label\":\"\",\"retry\":{}}},\"rows\":{{\"label\":\"\",\"retry\":{}}},\"ok\":true}}",serde_json::to_string(&left).unwrap(),serde_json::to_string(right).unwrap())
    };
    let base = wire(&[]).len();
    let row_len = serde_json::to_string(&"\0".repeat(64)).unwrap().len();
    let mut selected = None;
    'search: for count in 0..255 {
        for controls in 0..=64 {
            for plain in 1..=64 - controls {
                let tail = "\0".repeat(controls) + &"a".repeat(plain);
                if base + count * (row_len + 1) + serde_json::to_string(&tail).unwrap().len()
                    == 131072
                {
                    let mut right = vec!["\0".repeat(64); count];
                    right.push(tail);
                    selected = Some(right);
                    break 'search;
                }
            }
        }
    }
    let right = selected.expect("exact independent JSON output-cap witness");
    let exact = wire(&right);
    assert_eq!(exact.len(), 131072);
    let mut over = right.clone();
    let last = over.last_mut().unwrap();
    let at = last.find('a').unwrap();
    last.replace_range(at..at + 1, "\"");
    assert_eq!(wire(&over).len(), 131073);
    for (ordinal, right) in [&right, &over].into_iter().enumerate() {
        // Ordinary loops build 256 bounded inputs; no giant source-array/cap exception.
        let full = array(&[0u8; 64]);
        let tail = right.last().unwrap();
        let build = format!(
            r#"
@id("consumer.full") fn full(count:usize)->Vec<string>{{
let mut values=vec_with_capacity<string>(count);let raw={full};let mut at=0usize;
while at<count{{values=vec_push<string>(values,string_from_utf8(array_as_slice(raw)));at=at+1usize;at<count}}values
}}
@id("consumer.right") fn right()->Vec<string>{{
let mut values=full({}usize);values=vec_reserve_owned<string>(values,{}usize);
let raw={};values=vec_push<string>(values,string_from_utf8(array_as_slice(raw)));values
}}
"#,
            right.len() - 1,
            right.len(),
            array(tail.as_bytes())
        );
        let check = if ordinal == 0 {
            let row = serde_json::to_string(&"\0".repeat(64)).unwrap();
            let tail = serde_json::to_string(tail).unwrap();
            format!(
                r#"
@id("consumer.at") fn at(actual:borrow str,expected:borrow str,start:usize)->bool{{
let actual_bytes=str_as_bytes(actual);let expected_bytes=str_as_bytes(expected);let mut index=0usize;let mut same=true;
while same && index<byte_len(expected_bytes){{
same=match byte_get(actual_bytes,start+index){{Option::Some{{value:a}}=>match byte_get(expected_bytes,index){{Option::Some{{value:b}}=>a==b,Option::None{{}}=>false,}},Option::None{{}}=>false,}};
index=index+1usize;same && index<byte_len(expected_bytes)
}}same
}}
@id("consumer.wire") fn wire(actual:borrow str)->bool{{
let prefix={};let middle={};let suffix={};let row={};let comma=",";let tail={};
let mut cursor=byte_len(str_as_bytes(string_as_str(prefix)));let mut same=at(actual,string_as_str(prefix),0usize);let mut index=0usize;
while index<256usize{{let _ = if index>0usize{{same=same && at(actual,string_as_str(comma),cursor);cursor=cursor+1usize;true}}else{{true}};
same=same && at(actual,string_as_str(row),cursor);cursor=cursor+{}usize;index=index+1usize;index<256usize}}
same=same && at(actual,string_as_str(middle),cursor);cursor=cursor+{}usize;index=0usize;
while index<{}usize{{let _ = if index>0usize{{same=same && at(actual,string_as_str(comma),cursor);cursor=cursor+1usize;true}}else{{true}};
same=same && at(actual,string_as_str(row),cursor);cursor=cursor+{}usize;index=index+1usize;index<{}usize}}
let _ = if index>0usize{{same=same && at(actual,string_as_str(comma),cursor);cursor=cursor+1usize;true}}else{{true}};
same=same && at(actual,string_as_str(tail),cursor);cursor=cursor+{}usize;
same && at(actual,string_as_str(suffix),cursor) && cursor+{}usize==byte_len(str_as_bytes(actual))
}}
"#,
                quote("{\"config\":{\"label\":\"\",\"retry\":["),
                quote("]},\"rows\":{\"label\":\"\",\"retry\":["),
                quote("]},\"ok\":true}"),
                quote(&row),
                quote(&tail),
                row.len(),
                "]},\"rows\":{\"label\":\"\",\"retry\":[".len(),
                right.len() - 1,
                row.len(),
                right.len() - 1,
                tail.len(),
                "]},\"ok\":true}".len()
            )
        } else {
            String::new()
        };
        let verdict = if ordinal == 0 {
            "Encoded::Encoded{text:t}=>wire(string_as_str(t)),Encoded::Refused{required:n}=>false,"
        } else {
            "Encoded::Encoded{text:t}=>false,Encoded::Refused{required:n}=>n==18446744073709551615usize,"
        };
        let app = imports_v2()
            + &build
            + &check
            + &format!(
                r#"
@id("consumer.main") fn main()->i64{{
let value=Report{{config:Config{{label:"",retry:full(256usize)}},rows:Config{{label:"",retry:right()}},ok:true}};
let required=encoded_len(value);let output=encode(value,131072usize);
let same=match own output{{{verdict}}};
if same && required=={}usize{{724}}else{{0}}
}}
"#,
                if ordinal == 0 {
                    "131072"
                } else {
                    "18446744073709551615"
                }
            );
        let root = install_v2(
            &format!("primitive-string-output-cap-{ordinal}"),
            &schema,
            &app,
            V2,
            false,
        );
        qualify_v2(&root);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn primitive_string_response_plan_receipts_and_live_revision_reject_forged_versions() {
    let source = schema();
    let root = fixture("primitive-string-plan-replay", &source);
    let (bundle, entries) =
        project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            let entries = vec![project::JsonCodecPlanEntry {
                source_path: "src/schema.spx".into(),
                record_id: "r.root".into(),
                profile: V2,
            }];
            let bundle = project::derive_json_codec_plan(&revision, &entries)?;
            project::verify_json_codec_plan(&revision, &entries, &bundle)?;
            assert_eq!(
                bundle,
                project::derive_json_codec_plan(&revision, &entries)?
            );
            assert_eq!(bundle.entries[0].profile, "bounded-nested-response.v2");
            assert!(bundle.entries[0]
                .generated_declarations
                .iter()
                .all(|id| id.starts_with("r.root.json.nested-response-v2.")));
            let mut stale = bundle.clone();
            stale.entries[0].profile = "bounded-nested-response.v1".into();
            assert_eq!(
                project::verify_json_codec_plan(&revision, &entries, &stale).unwrap_err()[0].code,
                "SPX-J180"
            );
            for change in ["borrowed-read", "identity", "bounds"] {
                let mut forged = bundle.clone();
                let receiver = forged
                    .candidate_sources
                    .iter_mut()
                    .find(|s| s.path == "src/schema.spx")
                    .unwrap();
                receiver.source = match change {
                    "borrowed-read" => receiver
                        .source
                        .replace("vec_str_at(values, at)", "vec_str_at(value.rows, at)"),
                    "identity" => receiver.source.replace(
                        ".json.nested-response-v2.encode",
                        ".json.nested-response.encode",
                    ),
                    _ => receiver
                        .source
                        .replace("count <= 256usize", "count <= 257usize"),
                };
                assert_eq!(
                    project::verify_json_codec_plan(&revision, &entries, &forged).unwrap_err()[0]
                        .code,
                    "SPX-J180"
                );
            }
            assert_eq!(
                std::fs::read_to_string(root.join("src/schema.spx")).unwrap(),
                canonical(&source)
            );
            Ok((bundle, entries))
        })
        .unwrap();
    let changed = canonical(&source.replace("retry:usize", "retry:i64"));
    std::fs::write(root.join("src/schema.spx"), &changed).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert_eq!(
            project::verify_json_codec_plan(&snapshot.retain_revision(), &entries, &bundle)
                .unwrap_err()[0]
                .code,
            "SPX-J180"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("src/schema.spx")).unwrap(),
            changed
        );
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

// Primitive String fixture custody.
#[test]
fn primitive_string_fixture_owners_keep_all_original_values_cases_and_assertions() {
    use sha2::{Digest, Sha256};
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/string-fixture-owners-v1/string_vectors.before.rs.txt"
    ));
    let expected = [
        0x50u8, 0xdf, 0xbf, 0x22, 0x3f, 0x42, 0xbd, 0x4f, 0x64, 0x8b, 0x5a, 0xb9, 0x62, 0x84, 0x9d,
        0x2f, 0xa3, 0xda, 0xe8, 0x6b, 0x53, 0x19, 0xf3, 0x61, 0x59, 0x4c, 0x70, 0xf0, 0xa2, 0x9f,
        0x2f, 0xaf,
    ];
    assert!(Sha256::digest(original.as_bytes())
        .iter()
        .copied()
        .eq(expected));
    let mut restored = include_str!("string_vectors.rs")
        .split_once("\n// Primitive String fixture custody.")
        .unwrap()
        .0
        .to_owned();
    let after = "\"{{let raw={};string_from_utf8(array_as_slice(raw))}}\"";
    let before = "\"string_from_utf8(array_as_slice({}))\"";
    assert_eq!(restored.matches(after).count(), 1);
    restored = restored.replace(after, before);
    let after = "let full=encode(value,required);let expected_raw={};let expected=string_from_utf8(array_as_slice(expected_raw));";
    let before =
        "let full=encode(value,required);let expected=string_from_utf8(array_as_slice({}));";
    assert_eq!(restored.matches(after).count(), 2);
    restored = restored.replace(after, before);
    let after =
        "let raw={};values=vec_push<string>(values,string_from_utf8(array_as_slice(raw)));values";
    let before = "values=vec_push<string>(values,string_from_utf8(array_as_slice({})));values";
    assert_eq!(restored.matches(after).count(), 1);
    restored = restored.replace(after, before);
    let after = "let required=encoded_len(value);let outcome=encode(value,131072usize);\nlet valid=match own outcome";
    let before = "let required=encoded_len(value);let result=encode(value,131072usize);\nlet valid=match own result";
    assert_eq!(restored.matches(after).count(), 1);
    restored = restored.replace(after, before);
    assert_eq!(restored, original);
}

#[test]
fn primitive_string_fixture_named_byte_owner_passes_and_temporary_keeps_t266() {
    for value in ["a", "é\0😀", "\"\\\n\t"] {
        let source = format!("module fixture_string; @id(\"main\") fn main()->i64 {{let value={}; let bytes=str_as_bytes(string_as_str(value)); if byte_len(bytes)=={}usize {{724}}else{{0}}}}", text(value), value.len());
        let checked = semaprax::check(&source, "fixture-string.spx").unwrap();
        let canonical = semaprax::format::canonical(&checked);
        let checked = semaprax::check(&canonical, "fixture-string.spx").unwrap();
        assert_eq!(semaprax::format::canonical(&checked), canonical);
        semaprax::hir::resolve(&checked).unwrap();
    }
    let source = "module fixture_temporary; @id(\"main\") fn main()->i64 {let value=string_from_utf8(array_as_slice([65u8])); 724}";
    assert!(semaprax::check(source, "fixture-temporary.spx")
        .unwrap_err()
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T266"));
}

#[test]
fn primitive_string_fixture_reserved_result_keeps_s109_and_t201() {
    let source = "module fixture_reserved; @id(\"main\") fn main()->i64 {let result=724; result}";
    let diagnostics = semaprax::check(source, "fixture-reserved.spx").unwrap_err();
    for code in ["SPX-S109", "SPX-T201"] {
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == code));
    }
}
