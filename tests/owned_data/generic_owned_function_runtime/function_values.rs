//! Reusable generic callback adapters preserve vector owners across every engine.
use super::collections;

const ADAPTERS: &str = r#"
module test.function_values_collections;
@id("adapter.map") fn map<T>(values:own Vec<T>, callback:fn(T)->T)->Vec<T> {
 let count=vec_len<T>(values);
 let mut output=vec_with_capacity<T>(count);
 let mut index=0usize;
 while index<count {
  output=vec_push<T>(output,callback(vec_get<T>(values,index)));
  index=index+1usize;
  true
 }
 output
}
@id("adapter.filter") fn filter<T>(values:own Vec<T>, predicate:fn(T)->bool)->Vec<T> {
 let count=vec_len<T>(values);
 let mut output=vec_with_capacity<T>(count);
 let mut index=0usize;
 while index<count {
  let item=vec_get<T>(values,index);
  let kept=if predicate(item) {output=vec_push<T>(output,item);true}else{false};
  index=index+1usize;
  kept
 }
 output
}
@id("adapter.fold") fn fold<T>(values:own Vec<T>, initial:T, combine:fn(T,T)->T)->T {
 let count=vec_len<T>(values);
 let mut accumulator=initial;
 let mut index=0usize;
 while index<count {
  accumulator=combine(accumulator,vec_get<T>(values,index));
  index=index+1usize;
  true
 }
 accumulator
}
"#;

#[test]
fn function_values_generic_adapters_all_scalars_and_callback_failure_settle_owners() {
    for failure in [false, true] {
        let mut source = String::from(ADAPTERS);
        let mut calls = Vec::new();
        for (ty, value) in [
            ("i64", "7"),
            ("i32", "7i32"),
            ("u8", "7u8"),
            ("usize", "7usize"),
            ("char", "'x'"),
            ("f32", "1.5f32"),
            ("f64", "1.5"),
            ("bool", "true"),
        ] {
            source.push_str(&format!(
                r#"
@id("adapter.identity.{ty}") fn identity_{ty}(value:{ty})->{ty} requires {allowed} {{value}}
@id("adapter.keep.{ty}") fn keep_{ty}(value:{ty})->bool {{true}}
@id("adapter.last.{ty}") fn last_{ty}(before:{ty},value:{ty})->{ty} {{value}}
@id("adapter.run.{ty}") fn run_{ty}()->i64 {{
 let input=vec_push<{ty}>(vec_with_capacity<{ty}>(1usize),{value});
 let mapped=map<{ty}>(input,identity_{ty});
 let selected=filter<{ty}>(mapped,keep_{ty});
 let observed=fold<{ty}>(selected,{value},last_{ty});
 let empty=map<{ty}>(vec_with_capacity<{ty}>(0usize),identity_{ty});
 let unchanged=fold<{ty}>(empty,{value},last_{ty});
 if observed=={value} && unchanged=={value} {{1}} else {{0}}
}}
"#,
                allowed = !failure
            ));
            calls.push(format!("run_{ty}()"));
        }
        source.push_str(&format!(
            "@id(\"app.main\") fn main()->i64{{{}}}",
            calls.join("+")
        ));
        collections::run_source(&source, u32::from(failure));
    }
}

#[test]
fn function_values_generic_adapters_preserve_selection_and_fold_order() {
    let source = String::from(ADAPTERS)
        + r#"
@id("adapter.increment") fn increment(value:i64)->i64 {value+1}
@id("adapter.even") fn even(value:i64)->bool {value%2==0}
@id("adapter.append") fn append(prefix:i64,value:i64)->i64 {prefix*10+value}
@id("app.main") fn main()->i64 {
 let first=vec_push<i64>(vec_with_capacity<i64>(3usize),1);
 let second=vec_push<i64>(first,2);
 let input=vec_push<i64>(second,3);
 let mapped=map<i64>(input,increment);
 let selected=filter<i64>(mapped,even);
 let observed=fold<i64>(selected,0,append);
 let empty=filter<i64>(vec_with_capacity<i64>(0usize),even);
 let unchanged=fold<i64>(empty,7,append);
 if observed==24 && unchanged==7 {8}else{0}
}
"#;
    collections::run_source(&source, 0);
}
