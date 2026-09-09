//! Owned iteration transfers payloads and the remaining window independently.
use semaprax::interpreter;
#[allow(clippy::duplicate_mod)]
#[path = "owned_vec_bytes_runtime/native.rs"]
mod native;
#[path = "owned_iterator_payloads/wasm.rs"]
mod wasm;

pub(crate) const ORDERED: &str = r#"module owned.iterator.runtime;
@id("iter.byte-is") fn byte_is(view:borrow Slice<u8>,index:usize,expected:u8)->bool {
 match byte_get(view,index) {Option::Some{value}=>value==expected,Option::None{}=>false,}
}
@id("iter.size") fn size(value:own Bytes)->i64 {
 let view=bytes_as_slice(value);
 if byte_len(view)==1usize && byte_is(view,0usize,255u8) {1}
 else {if byte_len(view)==2usize && byte_is(view,0usize,0u8) && byte_is(view,1usize,128u8) {2}else{0}}
}
@id("iter.relay") fn relay(value:own Iter<Bytes>)->Iter<Bytes>{value}
@id("iter.rebuild") fn rebuild(value:own IterStep<Bytes>)->IterStep<Bytes>{
 match own value {
  IterStep::Done{}=>IterStep<Bytes>::Done{},
  IterStep::Yield{item,rest}=>IterStep<Bytes>::Yield{item:item,rest:rest},
 }
}
@id("app.main") fn main()->i64 {
 let one=[255u8];
 let two=[0u8,128u8];
 let first=vec_push<Bytes>(vec_with_capacity<Bytes>(2usize),bytes_copy(array_as_slice(one)));
 let values=vec_push<Bytes>(first,bytes_copy(array_as_slice(two)));
 let iterator=relay(vec_into_iter<Bytes>(values));
 let step=rebuild(iter_next<Bytes>(iterator));
 match own step {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>{
   let first_size=size(item);
   let mut total=first_size;
   for own second in rest {total=total*10+size(second);0}
   total
  },
 }
}
"#;
pub(crate) const EARLY: &str = r#"module owned.iterator.early;
@id("app.main") fn main()->i64 {
 let first=vec_push<Bytes>(vec_with_capacity<Bytes>(2usize),bytes_zeroed(1usize));
 let values=vec_push<Bytes>(first,bytes_zeroed(2usize));
 let step=iter_next<Bytes>(vec_into_iter<Bytes>(values));
 match own step {IterStep::Done{}=>0,IterStep::Yield{item,rest}=>29,}
}
"#;
pub(crate) const LOCAL_DONE: &str = r#"module owned.iterator.done;
@id("app.main") fn main()->i64 {
 let step=IterStep<Bytes>::Done{};
 match own step {IterStep::Done{}=>29,IterStep::Yield{item,rest}=>0,}
}
"#;

pub(crate) const EMPTY: &str = r#"module owned.iterator.empty;
@id("iter.empty-size") fn size(value:own Bytes)->usize {
 let view=bytes_as_slice(value);byte_len(view)
}
@id("app.main") fn main()->i64 {
 let values=vec_push<Bytes>(vec_with_capacity<Bytes>(1usize),bytes_zeroed(0usize));
 let mut seen=0;
 for own item in vec_into_iter<Bytes>(values) {
  let length=size(item);
  if length==0usize {seen=seen+1;0}else{seen=seen+100;0}
 }
 for own item in vec_into_iter<Bytes>(vec_with_capacity<Bytes>(0usize)) {
  let length=size(item);seen=seen+100;0
 }
 seen
}
"#;

#[test]
fn owned_iterator_payloads_interpreter_and_native_transfer_and_settlement() {
    let failure = ORDERED.replace(
        "fn size(value:own Bytes)->i64 {",
        "fn size(value:own Bytes)->i64 ensures false {",
    );
    let cases = [
        (ORDERED, "", 0, 12),
        (EARLY, "", 0, 29),
        (LOCAL_DONE, "", 0, 29),
        (EMPTY, "", 0, 1),
        (&failure, "semaprax.contract.v1", 2, 0),
    ];
    let path = std::env::temp_dir().join(format!(
        "semaprax-owned-iterator-{}.spx",
        std::process::id()
    ));
    for (source, domain, code, value) in cases {
        std::fs::write(&path, source).unwrap();
        for _ in 0..4 {
            let outcome = interpreter::interpret(
                &path,
                "app.main",
                &[],
                &interpreter::InterpreterOptions::default(),
            )
            .unwrap();
            interpreter::verify_envelope(&outcome.envelope).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&outcome.envelope).unwrap();
            if code == 0 {
                assert!(outcome.returned);
                assert_eq!(envelope["payload"]["outcome"]["value"], value.to_string());
            } else {
                assert!(!outcome.returned);
                assert_eq!(
                    envelope["payload"]["outcome"]["status"]["domain_id"],
                    domain
                );
                assert_eq!(envelope["payload"]["outcome"]["status"]["code"], code);
            }
        }
        native::run_native(source, domain, code, value, "none");
    }
    native::run_native(ORDERED, "semaprax.vec.v1", 3, 0, "allocation");
    std::fs::remove_file(path).unwrap();
}
