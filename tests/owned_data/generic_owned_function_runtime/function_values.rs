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

#[test]
fn captured_closures_cross_generic_map_filter_fold_with_the_shared_vec_runtime_host() {
    let source = String::from(ADAPTERS)
        + r#"
@id("adapter.run") fn run()->i64 {
 let map_offset=1;
 let mapped_callback=fn(value:i64)->i64 {value+map_offset};
 let filter_floor=2;
 let kept_callback=fn(value:i64)->bool {value>filter_floor};
 let fold_base=10;
 let folded_callback=fn(left:i64,right:i64)->i64 {left*fold_base+right};
 let first=vec_push<i64>(vec_with_capacity<i64>(3usize),1);
 let second=vec_push<i64>(first,3);
 let values=vec_push<i64>(second,2);
 fold<i64>(filter<i64>(map<i64>(values,mapped_callback),kept_callback),0,folded_callback)
}
@id("app.main") fn main()->i64 {run()}
"#;
    collections::run_source_value(&source, 43);
}

#[test]
fn function_values_generic_callbacks_fail_after_a_committed_output_push() {
    for (adapter, callback) in [("map", "guard_map"), ("filter", "guard_filter")] {
        let source = format!(
            r#"{ADAPTERS}
@id("adapter.guard-map") fn guard_map(value:i64)->i64 requires value<2 {{value}}
@id("adapter.guard-filter") fn guard_filter(value:i64)->bool requires value<2 {{true}}
@id("app.main") fn main()->i64 {{
 let first=vec_push<i64>(vec_with_capacity<i64>(2usize),1);
 let input=vec_push<i64>(first,2);
 let output={adapter}<i64>(input,{callback});
 if vec_len<i64>(output)==1usize {{1}} else {{0}}
}}
"#
        );
        collections::run_source(&source, 1);
    }
}

#[test]
fn function_values_generic_adapters_settle_input_after_output_capacity_limit() {
    for adapter in ["map_refusal", "filter_refusal"] {
        let source = format!(
            r#"{ADAPTERS}
@id("adapter.map-refusal") fn map_refusal<T>(values:own Vec<T>, callback:fn(T)->T)->Vec<T> {{
 let count=vec_len<T>(values);
 let capacity=8192usize+1usize;
 let output=vec_with_capacity<T>(capacity);
 output
}}
@id("adapter.filter-refusal") fn filter_refusal<T>(values:own Vec<T>, predicate:fn(T)->bool)->Vec<T> {{
 let count=vec_len<T>(values);
 let capacity=8192usize+1usize;
 let output=vec_with_capacity<T>(capacity);
 output
}}
@id("adapter.identity") fn identity(value:i64)->i64 {{value}}
@id("adapter.keep") fn keep(value:i64)->bool {{true}}
@id("app.main") fn main()->i64 {{
 let input=vec_push<i64>(vec_with_capacity<i64>(1usize),7);
 let output={adapter}<i64>(input,{callback});
 if vec_len<i64>(output)==1usize {{1}} else {{0}}
}}
"#,
            callback = if adapter == "map_refusal" {
                "identity"
            } else {
                "keep"
            },
        );
        collections::run_source(&source, 3);
    }
}

#[test]
fn function_values_generic_adapters_settle_input_after_backend_output_allocation_refusal() {
    for (adapter, callback) in [("map", "identity"), ("filter", "keep")] {
        let source = format!(
            r#"{ADAPTERS}
@id("adapter.identity") fn identity(value:i64)->i64 {{value}}
@id("adapter.keep") fn keep(value:i64)->bool {{true}}
@id("app.main") fn main()->i64 {{
 let input=vec_push<i64>(vec_with_capacity<i64>(1usize),7);
 let output={adapter}<i64>(input,{callback});
 if vec_len<i64>(output)==1usize {{1}} else {{0}}
}}
"#
        );
        collections::run_backend_output_allocation_refusal(&source);
    }
}
