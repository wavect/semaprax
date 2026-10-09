//! OPT-723: named application records, ordering and settlement on every engine.
use semaprax::{codegen, graph, hir, interpreter};
use std::process::Command;
mod runtime;

const DECL: &str = r#"module copy.records;
@id("app.job") record Job {
 @id("app.job.priority") priority:i64,
 @id("app.job.arrival") arrival:i64,
 @id("app.job.sequence") sequence:i64,
 @id("app.job.id") id:i64,
 @id("app.job.duration") duration:i64,
 @id("app.job.worker") worker:i64,
}
"#;
const JOB: &str = "Job{priority:2,arrival:10,sequence:3,id:7,duration:20,worker:1}";
const APPLICATION: &str = r#"
@id("app.read") fn read(jobs:borrow Vec<Job>, index:usize)->Job { vec_get<Job>(jobs,index) }
@id("app.main") fn main()->i64 {
 let seed=Job{priority:2,arrival:10,sequence:3,id:7,duration:20,worker:1};
 let mut jobs=vec_with_capacity<Job>(2usize);
 jobs=vec_push<Job>(jobs,seed);
 jobs=vec_push<Job>(jobs,seed);
 jobs=vec_reserve_exact<Job>(jobs,2usize);
 let mut i=0usize;
 while i<2usize {
  jobs=vec_push<Job>(jobs,Job{priority:1,arrival:5,sequence:i64_from_usize(i),id:i64_from_usize(i),duration:4,worker:0});
  i=i+1usize;
  0
 }
 jobs=vec_sort<Job>(jobs);
 let first=read(jobs,0usize);
 let second=read(jobs,1usize);
 let mut sum=0;
 let mut n=0usize;
 while n<vec_len<Job>(jobs) { let job=read(jobs,n); sum=sum+job.id; n=n+1usize; 0 }
 let before=first.id==0 && second.id==1 && seed.id==7 && sum==15 && vec_capacity<Job>(jobs)==4usize;
 jobs=vec_set<Job>(jobs,2usize,Job{priority:0,arrival:0,sequence:0,id:42,duration:8,worker:2});
 jobs=vec_sort<Job>(jobs);
 let chosen=vec_get<Job>(jobs,0usize);
 jobs=vec_clear<Job>(jobs);
 jobs=vec_sort<Job>(jobs);
 if before && chosen.id==42 && vec_len<Job>(jobs)==0usize && vec_capacity<Job>(jobs)==4usize {42}else{1}
}
"#;
fn program(body: &str) -> String {
    format!("{DECL}@id(\"app.main\") fn main()->i64 {{ {body} }}")
}

#[test]
fn named_six_field_records_execute_and_settle_across_engines() {
    runtime::run("application", &format!("{DECL}{APPLICATION}"), 0, 42, false);
}

#[test]
fn scalar_field_total_orders_execute_across_engines() {
    let mut text = String::from("module copy.scalar_orders;");
    let mut body = String::new();
    let mut checks = Vec::new();
    for (i, (ty, values, first, last)) in [
        ("i64", "3,-7,3,0", "-7", "3"),
        ("i32", "3i32,-7i32,3i32,0i32", "-7i32", "3i32"),
        ("u8", "255u8,0u8,17u8,17u8", "0u8", "255u8"),
        (
            "usize",
            "99usize,0usize,17usize,17usize",
            "0usize",
            "99usize",
        ),
        ("char", "'🦀','a','é','a'", "'a'", "'🦀'"),
        ("f32", "3.5f32,-7.5f32,0.0f32,3.5f32", "-7.5f32", "3.5f32"),
        ("f64", "3.5,-7.5,0.0,3.5", "-7.5", "3.5"),
        ("bool", "true,false,true,false", "false", "true"),
    ]
    .into_iter()
    .enumerate()
    {
        text.push_str(&format!("@id(\"app.r{i}\") record R{i} {{ @id(\"app.r{i}.key\") key:{ty}, @id(\"app.r{i}.tie\") tie:i64, }}"));
        body.push_str(&format!("let mut v{i}=vec_with_capacity<R{i}>(4usize);"));
        for (j, value) in values.split(',').enumerate() {
            body.push_str(&format!(
                "v{i}=vec_push<R{i}>(v{i},R{i}{{key:{value},tie:{j}}});"
            ));
        }
        body.push_str(&format!("v{i}=vec_sort<R{i}>(v{i}); let a{i}=vec_get<R{i}>(v{i},0usize); let b{i}=vec_get<R{i}>(v{i},3usize);"));
        checks.push(format!("a{i}.key=={first} && b{i}.key=={last}"));
    }
    text.push_str(&format!(
        "@id(\"app.main\") fn main()->i64 {{ {body} if {} {{42}}else{{1}} }}",
        checks.join(" && ")
    ));
    runtime::run("scalar-orders", &text, 0, 42, false);
}

#[test]
fn capacity_bounds_and_failed_calls_settle_across_engines() {
    for (name,body,code) in [
        ("capacity-edge", "let n=1365usize; let v=vec_with_capacity<Job>(n); if vec_capacity<Job>(v)==n {42}else{1}".to_owned(),0),
        ("capacity-over", "let n=1366usize; let v=vec_with_capacity<Job>(n); 42".to_owned(),3),
        ("full",format!("let v=vec_with_capacity<Job>(0usize); let w=vec_push<Job>(v,{JOB}); 42"),1),
        ("get-bounds","let v=vec_with_capacity<Job>(0usize); let j=vec_get<Job>(v,18446744073709551615usize); j.id".to_owned(),2),
        ("set-bounds",format!("let v=vec_with_capacity<Job>(0usize); let w=vec_set<Job>(v,18446744073709551615usize,{JOB}); 42"),2),
        ("reserve-over",format!("let v=vec_with_capacity<Job>(1usize); let w=vec_push<Job>(v,{JOB}); let x=vec_reserve_exact<Job>(w,1365usize); 42"),3),
    ] { runtime::run(name,&program(&body),code,42,false); }
    runtime::run(
        "refused-allocation",
        &program("let v=vec_with_capacity<Job>(1usize); 42"),
        3,
        42,
        true,
    );
}

#[test]
fn unsupported_shapes_and_owner_reuse_keep_stable_diagnostics() {
    for (decl,body,code) in [
        ("@id(\"app.r\") record R { @id(\"app.r.x\") x:Bytes, }", "let v=vec_with_capacity<R>(0usize); 0", "SPX-T281"),
        ("@id(\"app.inner\") record Inner { @id(\"app.inner.x\") x:i64, } @id(\"app.r\") record R { @id(\"app.r.x\") x:Inner, }", "let v=vec_with_capacity<R>(0usize); 0", "SPX-T281"),
        ("", "let v=vec_with_capacity<Job>(0usize); let w=vec_sort<Job>(v); let n=vec_len<Job>(v); 0", "SPX-O101"),
        ("", "let v=vec_with_capacity<Job>(0usize); let n=vec_get<Job>(v); 0", "SPX-T281"),
    ] {
        let text=format!("{DECL}{decl}@id(\"app.main\") fn main()->i64 {{ {body} }}");
        let ast=semaprax::parse(&text,"negative.spx").unwrap();
        let errors=semaprax::verify::verify(&ast);
        assert!(errors.iter().any(|error|error.code==code),"{text}: {errors:?}");
    }
    let fields = (0..9)
        .map(|i| format!("@id(\"app.r.f{i}\") f{i}:i64,"))
        .collect::<String>();
    let text=format!("module t; @id(\"app.r\") record R {{{fields}}} fn main()->i64 {{ let v=vec_with_capacity<R>(0usize); 0 }}");
    let ast = semaprax::parse(&text, "wide.spx").unwrap();
    assert!(semaprax::verify::verify(&ast)
        .iter()
        .any(|e| e.code == "SPX-T281"));
}

#[test]
fn canonical_graph_and_hostile_hir_preserve_nominal_identity() {
    let text=program(&format!("let v=vec_with_capacity<Job>(1usize); let w=vec_push<Job>(v,{JOB}); let j=vec_get<Job>(w,0usize); j.id"));
    let ast = semaprax::check(&text, "copy-record.spx").unwrap();
    let canonical = semaprax::format::canonical(&ast);
    let round = semaprax::parse(&canonical, "copy-record.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&round));
    let json = graph::to_json(&ast).unwrap();
    graph::verify_json(&ast, &json).unwrap();
    assert_eq!(json, graph::to_json(&round).unwrap());
    assert!(json.contains("app.job.priority") && json.contains("core.vec.get"));
    let mut resolved = hir::resolve(&ast).unwrap();
    let function = resolved
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("block")
    };
    let hir::ResolvedExprKind::Call { args, .. } = &mut statements[1].value_mut().kind else {
        panic!("push")
    };
    args[1].ownership = hir::OwnershipMode::Own;
    assert!(hir::validate(&resolved).is_err());
    assert!(codegen::emit_hir_c(&resolved).is_err());
}

#[test]
fn mixed_width_records_and_float_signed_zero_keep_their_bits() {
    let source = r#"module mixed.records;
@id("app.m") record Mixed {
 @id("app.m.a") a:i64, @id("app.m.b") b:i32,
 @id("app.m.c") c:u8, @id("app.m.d") d:usize,
 @id("app.m.e") e:char, @id("app.m.f") f:f32,
 @id("app.m.g") g:f64, @id("app.m.h") h:bool,
}
@id("app.z32") record Z32 { @id("app.z32.key") key:f32, @id("app.z32.id") id:i64, }
@id("app.z64") record Z64 { @id("app.z64.key") key:f64, @id("app.z64.id") id:i64, }
@id("app.main") fn main()->i64 {
 let m=Mixed{a:-9,b:-7i32,c:255u8,d:99usize,e:'🦀',f:-2.5f32,g:3.5,h:true};
 let v=vec_with_capacity<Mixed>(1usize); let w=vec_push<Mixed>(v,m); let x=vec_sort<Mixed>(w); let a=vec_get<Mixed>(x,0usize);
 let v32=vec_with_capacity<Z32>(2usize); let w32=vec_push<Z32>(v32,Z32{key:0.0f32,id:1}); let x32=vec_push<Z32>(w32,Z32{key:-0.0f32,id:2}); let y32=vec_sort<Z32>(x32); let a32=vec_get<Z32>(y32,0usize);
 let v64=vec_with_capacity<Z64>(2usize); let w64=vec_push<Z64>(v64,Z64{key:0.0,id:1}); let x64=vec_push<Z64>(w64,Z64{key:-0.0,id:2}); let y64=vec_sort<Z64>(x64); let a64=vec_get<Z64>(y64,0usize);
 if a.a==-9 && a.b==-7i32 && a.c==255u8 && a.d==99usize && a.e=='🦀' && a.f==-2.5f32 && a.g==3.5 && a.h && a32.id==2 && a64.id==2 {42}else{1}
}"#;
    runtime::run("mixed-width-zero", source, 0, 42, false);
}
