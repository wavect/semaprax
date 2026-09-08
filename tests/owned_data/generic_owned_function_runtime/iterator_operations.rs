//! Ordered, consuming iterator operations across every input/output scalar pair.
use super::collections;

const EXAMPLE: &str = include_str!("../../../examples/iterator-operations.spx");
const SCALARS: [(&str, &str, &str); 8] = [
    ("i64", "-7", "19"),
    ("i32", "-7i32", "19i32"),
    ("u8", "7u8", "19u8"),
    ("usize", "7usize", "19usize"),
    ("char", "'a'", "'z'"),
    ("f32", "-1.5f32", "2.5f32"),
    ("f64", "-1.5", "2.5"),
    ("bool", "false", "true"),
];
fn helpers() -> &'static str {
    EXAMPLE.split("@id(\"app.main\")").next().unwrap()
}

fn pair_source(input: (&str, &str, &str)) -> String {
    let (t, first, second) = input;
    let mut source = helpers().to_owned();
    let mut calls = Vec::new();
    for (u, before, after) in SCALARS {
        source.push_str(&format!(r#"
@id("pair.{t}.{u}") fn run_{t}_{u}()->i64 {{
 let left={before};
 let right={after};
 let transform=fn(value:{t})->{u}{{if value=={first}{{left}}else{{right}}}};
 let input=vec_push<{t}>(vec_push<{t}>(vec_with_capacity<{t}>(2usize),{first}),{second});
 let mapped=map_via<{u},{t}>(vec_into_iter<{t}>(input),2usize,transform);
 let ordered=vec_len<{u}>(mapped)==2usize && vec_get<{u}>(mapped,0usize)=={before} && vec_get<{u}>(mapped,1usize)=={after};
 let keep=fn(value:{u})->bool{{value==right}};
 let filtered=filter<{u}>(vec_into_iter<{u}>(mapped),2usize,keep);
 let retained=vec_len<{u}>(filtered)==1usize && vec_get<{u}>(filtered,0usize)=={after};
 let dropped=fold<{u},{t}>(vec_into_iter<{u}>(filtered),{first},fn(acc:{t},value:{u})->{t}{{if value==right{{{second}}}else{{acc}}}});
 let empty=fold<{t},{u}>(vec_into_iter<{t}>(vec_with_capacity<{t}>(0usize)),{before},fn(acc:{u},value:{t})->{u}{{acc}});
 let input2=vec_push<{t}>(vec_push<{t}>(vec_with_capacity<{t}>(2usize),{first}),{second});
 let folded=fold<{t},{u}>(vec_into_iter<{t}>(input2),{before},fn(acc:{u},value:{t})->{u}{{if value=={first}{{left}}else{{if acc==left{{right}}else{{left}}}}}});
 if ordered && retained && dropped=={second} && empty=={before} && folded=={after} {{1}}else{{0}}
}}
"#));
        calls.push(format!("run_{t}_{u}()"));
    }
    source.push_str(&format!(
        "@id(\"app.main\") fn main()->i64{{{}}}",
        calls.join("+")
    ));
    source
}

#[test]
fn iterator_operations_map_filter_fold_all_64_scalar_pairs() {
    for input in SCALARS {
        collections::run_source_value(&pair_source(input), 8);
    }
}

#[test]
fn iterator_operations_example_executes_on_every_engine() {
    collections::run_source_value(EXAMPLE, 1);
}

#[test]
fn iterator_operations_callback_and_capacity_failure_settle_all_owners() {
    let callback_failure = format!(
        r#"{}
@id("guard") fn guard(value:i64)->bool requires value<2 {{true}}
@id("app.main") fn main()->i64{{
 let input=vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize),1),2);
 let output=map<i64,bool>(vec_into_iter<i64>(input),2usize,guard);
 0
}}
"#,
        helpers()
    );
    collections::run_source(&callback_failure, 1);
    let capacity_failure = format!(
        r#"{}
@id("app.main") fn main()->i64{{
 let input=vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize),1),2);
 let output=map<i64,bool>(vec_into_iter<i64>(input),1usize,fn(value:i64)->bool{{true}});
 0
}}
"#,
        helpers()
    );
    collections::run_source(&capacity_failure, 2);
}

#[test]
fn iterator_operations_conditional_filter_failure_settles_reserved_and_remainder_owners() {
    for (capacity, guard, status) in [(1, "true", 2), (2, "value<2", 1)] {
        let source = format!(
            r#"{}
@id("guard") fn guard(value:i64)->bool requires {guard} {{true}}
@id("app.main") fn main()->i64{{
 let input=vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize),1),2);
 let output=filter<i64>(vec_into_iter<i64>(input),{capacity}usize,guard);
 0
}}
"#,
            helpers()
        );
        collections::run_source(&source, status);
    }
}

#[test]
fn iterator_operations_renewal_rhs_callback_failure_does_not_revive_output() {
    let source = r#"module test.renewal_rhs;
@id("guard") fn guard(value:i64)->i64 requires value<2 {value}
@id("app.main") fn main()->i64{
 let input=vec_into_iter<i64>(vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize),1),2));
 let mut output=vec_with_capacity<i64>(2usize);
 for own item in input {
  if true {output=vec_push<i64>(output,guard(item));0}else{0}
 }
 0
}
"#;
    collections::run_source(source, 1);
}

#[test]
fn iterator_operations_output_allocation_refusal_settles_consumed_input() {
    let source = format!(
        r#"{}
@id("transform") fn transform(value:i64)->bool {{value==1}}
@id("app.main") fn main()->i64{{
 let input=vec_push<i64>(vec_with_capacity<i64>(1usize),1);
 let output=map<i64,bool>(vec_into_iter<i64>(input),1usize,transform);
 0
}}
"#,
        helpers()
    );
    collections::run_backend_output_allocation_refusal(&source);
}
