use super::*;

pub(super) const MULTI_STRING_SCHEMA: &str = r#"module consumer.schema;
@id("response.item") record Item {
 @id("response.item.id") id:string,
 @id("response.item.server") server:string,
 @id("response.item.arrival") arrival:i64,
 @id("response.item.start") start:i64,
 @id("response.item.finish") finish:i64,
 @id("response.item.wait") wait:i64,
 @id("response.item.late") late:bool,
}
@id("response.metrics") record Metrics {
 @id("response.metrics.selected") selected:usize,
 @id("response.metrics.total") total:i64,
 @id("response.metrics.live") live:bool,
}
@id("response.report") record Report {
 @id("response.report.metrics") stats:Metrics,
 @id("response.report.items") entries:Vec<Item>,
}
@id("consumer.schema.anchor") fn anchor()->i64{0}
"#;

#[test]
fn collection_response_two_strings_round_trips_utf8_nul_and_exact_output_on_three_backends() {
    let expected = r#"{"stats":{"selected":1,"total":4,"live":true},"entries":[{"id":"é\u0000😀","server":"worker-😀","arrival":0,"start":0,"finish":4,"wait":4,"late":true}]}"#;
    let app = imports().to_owned()
        + &format!(
            r#"
@id("consumer.main") fn main()->i64{{
let nul=string_from_char(char_from_i64(0));let patient_prefix=string_concat("é",nul);let patient_id=string_concat(patient_prefix,"😀");let server=string_concat("worker-","😀");
let row=Item{{id:patient_id,server,arrival:0,start:0,finish:4,wait:4,late:true}};
let entries=vec_push<Item>(vec_with_capacity<Item>(1usize),row);
let report=Report{{stats:Metrics{{selected:1usize,total:4,live:true}},entries}};
let required=encoded_len(report);let short=encode(report,{}usize);
let short_ok=match own short{{Encoded::Refused{{required:count}}=>count=={}usize,Encoded::Encoded{{text:outcome_text_1}}=>false,}};
let full=encode(report,{}usize);let expected={};
let same=match own full{{Encoded::Refused{{required:outcome_required_2}}=>false,Encoded::Encoded{{text:outcome_text_3}}=>equal(string_as_str(outcome_text_3),string_as_str(expected)),}};
if required=={}usize && short_ok && same{{728}}else{{0}}
}}
"#,
            expected.len() - 1,
            expected.len(),
            expected.len(),
            spx_string(expected),
            expected.len()
        );
    let root = install_schema("response-multi-string", MULTI_STRING_SCHEMA, &app, 64);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn collection_response_refuses_an_overbound_second_string_before_emitting_output() {
    let app = imports().to_owned()
        + r#"
@id("consumer.main") fn main()->i64{
let row=Item{id:"a",server:"xy",arrival:0,start:0,finish:1,wait:0,late:false};
let entries=vec_push<Item>(vec_with_capacity<Item>(1usize),row);
let report=Report{stats:Metrics{selected:1usize,total:1,live:true},entries};
let required=encoded_len(report);
let outcome=encode(report,131072usize);
let refused=match own outcome{Encoded::Refused{required:actual}=>actual==18446744073709551615usize,Encoded::Encoded{text:outcome_text_4}=>false,};
if required==18446744073709551615usize && refused{728}else{0}
}
"#;
    let root = install_schema("response-second-string-bound", MULTI_STRING_SCHEMA, &app, 1);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}
