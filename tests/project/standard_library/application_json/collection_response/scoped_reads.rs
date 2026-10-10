//! Derived response preflight/render must not request independent Row owners.
use super::*;

#[test]
fn collection_response_scoped_two_string_reads_succeed_with_clone_allocator_refused() {
    let expected = r#"{"stats":{"selected":1,"total":4,"live":true},"entries":[{"id":"é\u0000😀","server":"worker-😀","arrival":0,"start":0,"finish":4,"wait":4,"late":true}]}"#;
    let app = imports().to_owned()
        + &format!(
            r#"
@id("consumer.main") fn main()->i64{{
let nul=string_from_char(char_from_i64(0));let prefix=string_concat("é",nul);let id=string_concat(prefix,"😀");
let item=Item{{id,server:"worker-😀",arrival:0,start:0,finish:4,wait:4,late:true}};
let entries=vec_push<Item>(vec_with_capacity<Item>(1usize),item);
let report=Report{{stats:Metrics{{selected:1usize,total:4,live:true}},entries}};
let mut at=0usize;let mut valid=true;
while valid && at<64usize{{valid=encoded_len(report)=={}usize;at=at+1usize;valid && at<64usize}}
let short=encode(report,{}usize);
let short_ok=match own short{{Encoded::Refused{{required}}=>required=={}usize,Encoded::Encoded{{text}}=>false,}};
let full=encode(report,{}usize);let expected={};
let same=match own full{{Encoded::Refused{{required}}=>false,Encoded::Encoded{{text}}=>equal(string_as_str(text),string_as_str(expected)),}};
if valid && short_ok && same{{728}}else{{0}}
}}
"#,
            expected.len(),
            expected.len() - 1,
            expected.len(),
            expected.len(),
            spx_string(expected)
        );
    let root = install_schema(
        "response-scoped-reads",
        multi_string::MULTI_STRING_SCHEMA,
        &app,
        64,
    );
    let generated = std::fs::read_to_string(root.join("src/schema.spx")).unwrap();
    assert!(generated.contains("vec_field<Item>"));
    assert!(!generated.contains("vec_clone_at"));
    qualify(&root);
    // This is separate from the retained caller-owned clone failure witness.
    // Refusing every deep String clone must not affect either response pass;
    // the strict host still checks all payload and vector settlement each run.
    wasm_run(&root, 0, 728, "string-clone-allocation");
    std::fs::remove_dir_all(root).unwrap();
}
