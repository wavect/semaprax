//! Consuming traversal for the exact two-`Bytes` plus one-Copy record profile.

use super::{interpret_once, native, source, wasm, DECLARATION};

const MANUAL_AND_LOOP: &str = r#"
@id("app.consume") fn consume(value:own Line)->i64 {
 match own value { Line {id,label,quantity} => {
  let id_len=byte_len(bytes_as_slice(id));
  let label_len=byte_len(bytes_as_slice(label));
  if id_len==1usize && label_len==2usize {quantity}else{0}
 }, }
}
@id("app.main") fn main()->i64 {
 let values0=vec_with_capacity<Line>(3usize);
 let values1=vec_push<Line>(values0,Line{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:11});
 let values2=vec_push<Line>(values1,Line{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:22});
 let values=vec_push<Line>(values2,Line{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:33});
 let step=iter_next<Line>(vec_into_iter<Line>(values));
 match own step {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>{
   let first=consume(item);
   let mut tail=0;
   for own remaining in rest {tail=tail*100+consume(remaining);0}
   if first==11 && tail==2233 {29}else{0}
  },
 }
}
"#;

const EMPTY_TRAVERSAL: &str = r#"
@id("app.main") fn main()->i64 {
 let manual=match own iter_next<Line>(vec_into_iter<Line>(vec_with_capacity<Line>(0usize))) {
  IterStep::Done{}=>1,
  IterStep::Yield{item,rest}=>0,
 };
 let mut count=0usize;
 for own item in vec_into_iter<Line>(vec_with_capacity<Line>(0usize)) {count=count+1usize;0}
 if manual==1 && count==0usize {29}else{0}
}
"#;

#[test]
fn manual_next_and_for_own_execute_and_settle_on_all_backends() {
    let program = source(MANUAL_AND_LOOP);
    let path =
        std::env::temp_dir().join(format!("owned-record-iterator-{}.spx", std::process::id()));
    semaprax::check(&program, &path).expect("record traversal must be source-admitted");
    std::fs::write(&path, &program).unwrap();
    let interpreted = semaprax::interpreter::interpret(
        &path,
        "app.main",
        &[],
        &semaprax::interpreter::InterpreterOptions::default(),
    )
    .expect("record traversal must execute in the interpreter");
    let envelope: serde_json::Value = serde_json::from_str(&interpreted.envelope).unwrap();
    assert!(interpreted.returned);
    assert_eq!(envelope["payload"]["outcome"]["value"], "29");
    std::fs::remove_file(path).unwrap();

    native::run_native(&program, "", 0, 29, "none");
    wasm::run_wasm(&program, 0, 29, 6, &[11, 22, 33], "none");
}

#[test]
fn failure_after_the_first_detached_item_settles_the_unvisited_suffix() {
    let program = source(&MANUAL_AND_LOOP.replace(
        "fn consume(value:own Line)->i64 {",
        "fn consume(value:own Line)->i64 ensures false {",
    ));
    assert_eq!(
        interpret_once("iterator-helper-failure", &program),
        Err(("semaprax.contract.v1".to_owned(), 2))
    );
    native::run_native(&program, "semaprax.contract.v1", 2, 0, "none");
    wasm::run_wasm(&program, 10, 0, 6, &[11, 22, 33], "none");
}

#[test]
fn empty_manual_and_loop_traversals_execute_zero_iterations() {
    let program = source(EMPTY_TRAVERSAL);
    assert_eq!(interpret_once("iterator-empty", &program), Ok(29));
    native::run_native(&program, "", 0, 29, "none");
    wasm::run_wasm(&program, 0, 29, 0, &[], "none");
}

#[test]
fn stale_iterator_and_double_item_consumption_are_source_errors() {
    for body in [
        r#"@id("app.main") fn main()->i64 {
 let values=vec_with_capacity<Line>(0usize);
 let iterator=vec_into_iter<Line>(values);
 let stale=vec_len<Line>(values);
 0
}"#,
        r#"@id("app.main") fn main()->i64 {
 let iterator=vec_into_iter<Line>(vec_with_capacity<Line>(0usize));
 let first=iter_next<Line>(iterator);
 let stale=iter_next<Line>(iterator);
 0
}"#,
        r#"@id("app.consume") fn consume(value:own Line)->i64 {0}
@id("app.main") fn main()->i64 {
 let values=vec_push<Line>(vec_with_capacity<Line>(1usize),Line{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:7});
 match own iter_next<Line>(vec_into_iter<Line>(values)) {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>{let first=consume(item);let second=consume(item);first+second},
 }
}"#,
    ] {
        let diagnostics = semaprax::check(&source(body), "owned-record-iterator-move.spx")
            .expect_err("a consumed owner must not be reusable");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-O101"),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn retained_hir_rejects_a_reminted_record_iterator_declaration() {
    let parsed = semaprax::check(&source(MANUAL_AND_LOOP), "owned-record-iterator-id.spx")
        .expect("record traversal source checks");
    let mut resolved = semaprax::hir::resolve(&parsed).expect("record traversal resolves");
    let item = resolved
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "app.catalog.line")
        .expect("record iterator item declaration");
    item.id = semaprax::hir::DeclarationId::new("foreign.catalog.line");
    assert_eq!(
        semaprax::hir::validate(&resolved).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn wasm_rejects_hostile_record_step_frames_before_owner_commit() {
    let program = source(MANUAL_AND_LOOP);
    for refusal in [
        "record-nowrite-next",
        "record-invalid-tag",
        "record-borrowed-item",
        "record-aliased-items",
        "record-stale-rest",
        "record-stale-handle",
    ] {
        wasm::run_wasm(&program, u32::MAX, 0, 6, &[11, 22, 33], refusal);
    }
}

#[test]
fn wasm_rejects_noncanonical_record_scalar_carriers_before_owner_commit() {
    for (ty, literal, bits, refusal) in [
        ("bool", "true", 1, "record-invalid-bool"),
        ("u8", "255u8", 255, "record-invalid-u8"),
        ("i32", "-1i32", -1, "record-noncanonical-i32"),
        (
            "f32",
            "1.5f32",
            1.5f32.to_bits().into(),
            "record-noncanonical-f32",
        ),
        ("char", "'k'", 107, "record-invalid-char"),
    ] {
        let declaration = DECLARATION.replace("quantity: i64", &format!("quantity: {ty}"));
        let body = format!(
            r#"@id("app.consume") fn consume(value:own Line)->i64 {{
 match own value {{Line{{id,label,quantity}}=>if byte_len(bytes_as_slice(id))==1usize && byte_len(bytes_as_slice(label))==2usize{{29}}else{{0}},}}
}}
@id("app.main") fn main()->i64 {{
 let values=vec_push<Line>(vec_with_capacity<Line>(1usize),Line{{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:{literal}}});
 match own iter_next<Line>(vec_into_iter<Line>(values)) {{
  IterStep::Done{{}}=>0,
  IterStep::Yield{{item,rest}}=>consume(item),
 }}
}}
"#
        );
        wasm::run_wasm(
            &format!("{declaration}{body}"),
            u32::MAX,
            0,
            2,
            &[bits],
            refusal,
        );
    }
}

#[test]
fn record_iterator_and_box_import_blocks_compose_in_core_wasm() {
    let program = source(
        r#"
@id("app.consume") fn consume(value:own Line)->i64 {
 match own value { Line {id,label,quantity} => quantity, }
}
@id("app.main") fn main()->i64 {
 let boxed=box_new<i64>(5);
 let unboxed=box_into_inner<i64>(boxed);
 let values=vec_push<Line>(vec_with_capacity<Line>(1usize),Line{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:11});
 match own iter_next<Line>(vec_into_iter<Line>(values)) {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>consume(item)+unboxed,
 }
}
"#,
    );
    wasm::run_wasm(&program, 0, 16, 2, &[11], "none");
}

#[test]
fn every_copy_scalar_shape_is_admitted_for_manual_next_and_for_own() {
    for (ty, literal, bits) in [
        ("i64", "-9223372036854775808", i64::MIN),
        ("i32", "-2147483648i32", i64::from(i32::MIN)),
        ("u8", "255u8", 255),
        ("usize", "65537usize", 65537),
        ("char", "'𝄞'", i64::from(u32::from('𝄞'))),
        ("f32", "-1.5f32", i64::from((-1.5f32).to_bits())),
        (
            "f64",
            "-1.5",
            i64::from_ne_bytes((-1.5f64).to_bits().to_ne_bytes()),
        ),
        ("bool", "false", 0),
    ] {
        let declaration = DECLARATION.replace("quantity: i64", &format!("quantity: {ty}"));
        let body = format!(
            r#"@id("app.observe") fn observe(value:own Line)->i64 {{
 match own value {{Line{{id,label,quantity}}=>if byte_len(bytes_as_slice(id))==1usize && byte_len(bytes_as_slice(label))==2usize && quantity=={literal}{{29}}else{{0}},}}
}}
@id("app.main") fn main()->i64 {{
 let first=vec_push<Line>(vec_with_capacity<Line>(1usize),Line{{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:{literal}}});
 let manual=match own iter_next<Line>(vec_into_iter<Line>(first)) {{IterStep::Done{{}}=>0,IterStep::Yield{{item,rest}}=>observe(item),}};
 let loop_values=vec_push<Line>(vec_with_capacity<Line>(1usize),Line{{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:{literal}}});
 let mut loop_total=0;
 for own item in vec_into_iter<Line>(loop_values) {{loop_total=loop_total+observe(item);0}}
 if manual==29 && loop_total==29 {{29}}else{{0}}
}}
"#
        );
        let program = format!("{declaration}{body}");
        let checked = semaprax::check(&program, "owned-record-iterator-scalars.spx")
            .unwrap_or_else(|diagnostics| panic!("{ty}: {diagnostics:?}"));
        let resolved = semaprax::hir::resolve(&checked)
            .unwrap_or_else(|diagnostics| panic!("{ty}: {diagnostics:?}"));
        semaprax::hir::validate(&resolved).unwrap_or_else(|error| panic!("{ty}: {error:?}"));
        assert!(resolved.functions.iter().any(|function| {
            function.id.as_str() == "app.main"
                && function.cleanup_plan.schema == semaprax::cleanup_plan::CLEANUP_PLAN_SCHEMA_V13
        }));
        assert_eq!(
            interpret_once(&format!("iterator-{ty}"), &program),
            Ok(29),
            "{ty} interpreter traversal"
        );
        native::run_native(&program, "", 0, 29, "none");
        wasm::run_wasm(&program, 0, 29, 4, &[bits, bits], "none");
    }
}
