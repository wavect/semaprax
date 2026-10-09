//! Ordinary private owned results for application collection decoders.
use super::*;
const OUTCOME: &str = r#"
@id("span") record Span { @id("span.start") start:usize, @id("span.end") end:usize, }
@id("outcome") variant Outcome {
 @id("outcome.ok") Decoded { @id("outcome.servers") servers:Vec<Span>, @id("outcome.jobs") jobs:Vec<Job>, },
 @id("outcome.error") Error { @id("outcome.code") code:i64, @id("outcome.offset") offset:usize, @id("outcome.field") field:i64, },
}
@id("one") variant One {
 @id("one.ok") Decoded { @id("one.value") value:Job, },
 @id("one.error") Error { @id("one.code") code:i64, @id("one.offset") offset:usize, @id("one.field") field:i64, },
}
@id("decode_one") fn decode_one(id:i64)->One {
 One::Decoded{value:Job{priority:1,arrival:2,sequence:id,id:id,duration:3,worker:0}}
}
@id("make") fn make(ok:bool)->Outcome {
 if ok {
  let mut servers=vec_with_capacity<Span>(8usize);
  let mut jobs=vec_with_capacity<Job>(256usize);
  let mut i=0usize;
  while i<8usize {servers=vec_push<Span>(servers,Span{start:i,end:i+1usize}); i=i+1usize;0}
  let mut n=0usize;
  while n<256usize {
   let parsed=decode_one(i64_from_usize(n));
   let observed=match parsed {
    One::Decoded{value} => { jobs=vec_push<Job>(jobs,value);0 },
    One::Error{code,offset,field} => 0,
   };
   n=n+1usize;0
  }
  Outcome::Decoded{servers:servers,jobs:jobs}
 } else {Outcome::Error{code:17,offset:9usize,field:2}}
}
@id("forward") fn forward(value:own Outcome)->Outcome {value}
@id("inspect") fn inspect(value:borrow Outcome)->i64 {
 match borrow value {
  Outcome::Decoded{servers,jobs} => i64_from_usize(vec_len<Span>(servers)+vec_len<Job>(jobs)),
  Outcome::Error{code,offset,field} => code+i64_from_usize(offset)+field,
 }
}
@id("consume") fn consume(value:own Outcome)->i64 {
 match own value {
  Outcome::Decoded{servers,jobs} => {
   let sorted=vec_sort<Job>(jobs); let last=vec_get<Job>(sorted,255usize);
   let span=vec_get<Span>(servers,7usize); last.id+i64_from_usize(span.end)
  },
  Outcome::Error{code,offset,field} => code+i64_from_usize(offset)+field,
 }
}
"#;
fn text(body: &str) -> String {
    format!("{DECL}{OUTCOME}@id(\"app.main\") fn main()->i64 {{{body}}}")
}
#[test]
fn bounded_collection_outcomes_and_copy_record_loop_results_execute_and_settle() {
    runtime::run("outcome", &text("let ok=make(true); let before=inspect(ok); let consumed=consume(forward(ok)); let error=make(false); let observed=inspect(error); let rejected=consume(forward(error)); if before==264 && consumed==263 && observed==28 && rejected==28 {42}else{1}"),0,42,false);
    runtime::run(
        "outcome-unused",
        &text("let unused=make(true); 42"),
        0,
        42,
        false,
    );
    // A bounds failure after moving both fields must settle both live carriers.
    let failing = text("consume(make(true))").replace(
        "vec_get<Job>(sorted,255usize)",
        "vec_get<Job>(sorted,256usize)",
    );
    runtime::run("outcome-bounds", &failing, 2, 0, false);
    // Failure constructing the second owner releases the first staged owner.
    let failing = text("consume(make(true))").replace(
        "vec_with_capacity<Job>(256usize)",
        "vec_with_capacity<Job>(1366usize)",
    );
    runtime::run("outcome-allocation", &failing, 3, 0, false);
}
#[test]
fn bounded_collection_outcome_graph_and_hostile_ownership_are_authenticated() {
    let source = text("consume(make(true))");
    let ast = semaprax::check(&source, "outcome.spx").unwrap();
    let canonical = semaprax::format::canonical(&ast);
    let roundtrip = semaprax::check(&canonical, "outcome.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&roundtrip), canonical);
    let json = graph::to_json(&ast).unwrap();
    graph::verify_json(&ast, &json).unwrap();
    let mut resolved = hir::resolve(&ast).unwrap();
    let forward = resolved
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "forward")
        .unwrap();
    forward.params[0].ownership = hir::OwnershipMode::Value;
    assert!(hir::validate(&resolved).is_err());
    assert!(codegen::emit_hir_c(&resolved).is_err());
    assert!(semaprax::wasm::emit_resolved_module(&resolved).is_err());
    for hostile in [
        source.replace("servers:Vec<Span>", "servers:Vec<i64>"),
        source.replace("servers:Vec<Span>", "servers:Vec<Bytes>"),
        source.replace("end:usize", "end:string"),
        source.replace(
            "servers:Vec<Span>,",
            "servers:Vec<Span>, @id(\"outcome.third\") third:Vec<Span>,",
        ),
        source.replace("fn forward(value:own Outcome)", "fn forward(value:Outcome)"),
        source.replace(
            "consume(make(true))",
            "let value=make(true); let result=consume(value); inspect(value)",
        ),
    ] {
        assert!(semaprax::check(&hostile, "hostile-outcome.spx").is_err());
    }
}
