//! Generic iterator helpers preserve the single owner across every backend.
use super::collections;

const HELPERS: &str = r#"
module test.generic_iterator_helpers;
@id("generic.iter.start") fn start<T>(values:own Vec<T>)->Iter<T>{vec_into_iter<T>(values)}
@id("generic.iter.advance") fn advance<T>(value:own Iter<T>)->IterStep<T>{iter_next<T>(value)}
@id("generic.iter.first") fn first<T>(values:own Vec<T>)->IterStep<T>{advance<T>(start<T>(values))}
@id("generic.iter.rebuild") fn rebuild<T>(value:own IterStep<T>)->IterStep<T>{
 match own value {
  IterStep::Done{}=>IterStep<T>::Done{},
  IterStep::Yield{item,rest}=>IterStep<T>::Yield{item:item,rest:rest},
 }
}
@id("generic.iter.project") fn project<T>(value:own IterStep<T>,fallback:T,callback:fn(T)->T)->T{
 match own value {
  IterStep::Done{}=>fallback,
  IterStep::Yield{item,rest}=>callback(item),
 }
}
"#;

fn source(fail: bool) -> String {
    let mut source = String::from(HELPERS);
    let mut calls = Vec::new();
    for (ty, item, fallback, captured) in [
        ("i64", "-17", "3", "29"),
        ("i32", "-17i32", "3i32", "29i32"),
        ("u8", "17u8", "3u8", "29u8"),
        ("usize", "17usize", "3usize", "29usize"),
        ("char", "'a'", "'q'", "'z'"),
        ("f32", "-1.25f32", "3.0f32", "2.5f32"),
        ("f64", "-1.25", "3.0", "2.5"),
        ("bool", "false", "false", "true"),
    ] {
        if fail {
            source.push_str(&format!(
                "@id(\"generic.iter.fail.{ty}\") fn fail_{ty}(value:{ty})->{ty} requires false {{value}}\n"
            ));
        }
        source.push_str(&format!(
            r#"
@id("generic.iter.run.{ty}") fn run_{ty}()->i64{{
 let captured={captured};
 let callback=fn(value:{ty})->{ty}{{if value=={item}{{captured}}else{{{fallback}}}}};
 let empty=project<{ty}>(rebuild<{ty}>(first<{ty}>(vec_with_capacity<{ty}>(0usize))),{fallback},callback);
 let exact=match own rebuild<{ty}>(advance<{ty}>(start<{ty}>(vec_push<{ty}>(vec_with_capacity<{ty}>(1usize),{item})))){{
  IterStep::Done{{}}=>false,
  IterStep::Yield{{item:seen,rest}}=>seen=={item},
 }};
 let projected=project<{ty}>(advance<{ty}>(start<{ty}>(vec_push<{ty}>(vec_with_capacity<{ty}>(1usize),{item}))),{fallback},callback);
 let exhausted=match own advance<{ty}>(start<{ty}>(vec_push<{ty}>(vec_with_capacity<{ty}>(1usize),{item}))){{
  IterStep::Done{{}}=>false,
  IterStep::Yield{{item:first,rest}}=>project<{ty}>(advance<{ty}>(rest),{fallback},callback)=={fallback},
 }};
 let early=start<{ty}>(vec_push<{ty}>(vec_with_capacity<{ty}>(1usize),{item}));
 {failure}
 if empty=={fallback} && exact && projected==captured && exhausted{{1}}else{{0}}
}}
"#,
            failure = if fail {
                format!("let failed=project<{ty}>(advance<{ty}>(start<{ty}>(vec_push<{ty}>(vec_with_capacity<{ty}>(1usize),{item}))),{fallback},fail_{ty});")
            } else {
                String::new()
            },
        ));
        calls.push(format!("run_{ty}()"));
    }
    source.push_str(&format!(
        "@id(\"app.main\") fn main()->i64{{{}}}",
        calls.join("+")
    ));
    source
}

#[test]
fn generic_iterator_helpers_all_scalars_callbacks_exhaustion_and_early_drop() {
    collections::run_source_value(&source(false), 8);
}

#[test]
fn generic_iterator_helper_contract_failure_settles_live_owners() {
    collections::run_source(&source(true), 1);
}

#[test]
fn generic_iterator_helpers_reject_owner_reuse_stably() {
    let source = format!(
        r#"{HELPERS}
@id("generic.iter.reuse") fn reuse<T>(value:own Iter<T>)->i64{{
 let first=advance<T>(value);
 let second=advance<T>(value);
 0
}}
@id("app.main") fn main()->i64{{reuse<i64>(start<i64>(vec_with_capacity<i64>(0usize)))}}
"#
    );
    let first = semaprax::check(&source, "generic-iterator-reuse.spx").unwrap_err();
    let second = semaprax::check(&source, "generic-iterator-reuse.spx").unwrap_err();
    assert_eq!(format!("{first:?}"), format!("{second:?}"));
    assert!(first.iter().any(|diagnostic| diagnostic.code == "SPX-O101"));
}
