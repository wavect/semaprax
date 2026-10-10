//! Same-source success and error outcomes carrying an owned-leaf collection.
use semaprax::{codegen, interpreter, wasm};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"module app.outcome;
@id("app.Entry") record Entry {
 @id("app.Entry.label") label:string,
 @id("app.Entry.rank") rank:i64,
}
@id("app.Outcome") variant Outcome {
 @id("app.Outcome.success") Success { @id("app.Outcome.entries") entries:Vec<Entry>, },
 @id("app.Outcome.error") Error {
  @id("app.Outcome.code") code:i64,
  @id("app.Outcome.offset") offset:usize,
  @id("app.Outcome.field") field:i64,
 },
}
@id("app.decode") fn decode(success:bool)->Outcome {
 if success {
  let mut entries=vec_with_capacity<Entry>(2usize);
  entries=vec_push<Entry>(entries,Entry{label:"beta",rank:20});
  entries=vec_push<Entry>(entries,Entry{label:"alpha",rank:10});
  Outcome::Success{entries:entries}
 } else { Outcome::Error{code:17,offset:9usize,field:2} }
}
@id("app.forward") fn forward(value:own Outcome)->Outcome {value}
@id("app.inspect") fn inspect(value:borrow Outcome)->i64 {
 match borrow value {
  Outcome::Success{entries} => i64_from_usize(vec_len<Entry>(entries)),
  Outcome::Error{code,offset,field} => code+i64_from_usize(offset)+field,
 }
}
@id("app.finish") fn finish(value:own Outcome)->i64 {
 match own value {
  Outcome::Success{entries} => {
   let ordered=vec_sort_owned<Entry>(entries);
   let first=vec_clone_at<Entry>(ordered,0usize);
   let first_ok=match own first {
    Entry{label,rank} => if label=="alpha" && rank==10 {1}else{0},
   };
   let replaced=vec_replace<Entry>(ordered,0usize,Entry{label:"gamma",rank:30});
   let mut total=0;
   for own entry in vec_into_iter<Entry>(replaced) {
    match own entry { Entry{label,rank} => {total=total+rank;0}, }
   }
   if first_ok==1 {total}else{0}
  },
  Outcome::Error{code,offset,field} => code+i64_from_usize(offset)+field,
 }
}
@id("app.main") fn main()->i64 {
 let success=decode(true);
 let success_size=inspect(success);
 let success_value=finish(forward(success));
 let error=decode(false);
 let error_value=inspect(error);
 let settled_error=finish(forward(error));
 if success_size==2 && success_value==50 && error_value==28 && settled_error==28 {42}else{1}
}
"#;

const SOURCE_TWO_VEC_AND_STRING_ONLY: &str = r#"module app.outcome.two;
@id("app.Entry") record Entry {
 @id("app.Entry.label") label:string,
 @id("app.Entry.rank") rank:i64,
}
@id("app.Outcome") variant Outcome {
 @id("app.Outcome.success") Success {
  @id("app.Outcome.words") words:Vec<string>,
  @id("app.Outcome.entries") entries:Vec<Entry>,
 },
 @id("app.Outcome.error") Error {
  @id("app.Outcome.code") code:i64,
  @id("app.Outcome.offset") offset:usize,
  @id("app.Outcome.field") field:i64,
 },
}
@id("app.decode") fn decode(success:bool)->Outcome {
 if success {
  let mut words=vec_with_capacity<string>(2usize);
  words=vec_push<string>(words,"zeta");
  words=vec_push<string>(words,"beta");
  let mut entries=vec_with_capacity<Entry>(2usize);
  entries=vec_push<Entry>(entries,Entry{label:"beta",rank:20});
  entries=vec_push<Entry>(entries,Entry{label:"alpha",rank:10});
  Outcome::Success{words:words,entries:entries}
 } else { Outcome::Error{code:17,offset:9usize,field:2} }
}
@id("app.forward") fn forward(value:own Outcome)->Outcome {value}
@id("app.inspect") fn inspect(value:borrow Outcome)->i64 {
 match borrow value {
  Outcome::Success{words,entries} => i64_from_usize(vec_len<string>(words)+vec_len<Entry>(entries)),
  Outcome::Error{code,offset,field} => code+i64_from_usize(offset)+field,
 }
}
@id("app.finish") fn finish(value:own Outcome)->i64 {
 match own value {
  Outcome::Success{words,entries} => {
   let ordered_words=vec_sort_owned<string>(words);
   let first_word=vec_clone_at<string>(ordered_words,0usize);
   let word_ok=first_word=="beta";
   let replaced_words=vec_replace<string>(ordered_words,0usize,"alpha");
   let mut word_bytes=0;
   for own word in vec_into_iter<string>(replaced_words) {
    word_bytes=word_bytes+string_len(word); 0
   }

   let ordered_entries=vec_sort_owned<Entry>(entries);
   let first_entry=vec_clone_at<Entry>(ordered_entries,0usize);
   let entry_ok=match own first_entry {
    Entry{label,rank} => if label=="alpha" && rank==10 {1}else{0},
   };
   let replaced_entries=vec_replace<Entry>(ordered_entries,0usize,Entry{label:"gamma",rank:30});
   let mut total_rank=0;
   for own entry in vec_into_iter<Entry>(replaced_entries) {
    match own entry { Entry{label,rank} => {total_rank=total_rank+rank;0}, }
   }
   if word_ok && entry_ok==1 {total_rank+word_bytes}else{0}
  },
  Outcome::Error{code,offset,field} => code+i64_from_usize(offset)+field,
 }
}

@id("app.WordOutcome") variant WordOutcome {
 @id("app.WordOutcome.success") Success { @id("app.WordOutcome.words") words:Vec<string>, },
 @id("app.WordOutcome.error") Error {
  @id("app.WordOutcome.code") code:i64,
  @id("app.WordOutcome.offset") offset:usize,
  @id("app.WordOutcome.field") field:i64,
 },
}
@id("app.decode_words") fn decode_words(success:bool)->WordOutcome {
 if success {
  let mut words=vec_with_capacity<string>(2usize);
  words=vec_push<string>(words,"moon");
  words=vec_push<string>(words,"sun");
  WordOutcome::Success{words:words}
 } else { WordOutcome::Error{code:17,offset:9usize,field:2} }
}
@id("app.forward_words") fn forward_words(value:own WordOutcome)->WordOutcome {value}
@id("app.inspect_words") fn inspect_words(value:borrow WordOutcome)->i64 {
 match borrow value {
  WordOutcome::Success{words} => i64_from_usize(vec_len<string>(words)),
  WordOutcome::Error{code,offset,field} => code+i64_from_usize(offset)+field,
 }
}
@id("app.finish_words") fn finish_words(value:own WordOutcome)->i64 {
 match own value {
  WordOutcome::Success{words} => {
   let ordered=vec_sort_owned<string>(words);
   let first=vec_clone_at<string>(ordered,0usize);
   let first_ok=first=="moon";
   let mut total=0;
   for own word in vec_into_iter<string>(ordered) {total=total+string_len(word);0}
   if first_ok {total}else{0}
  },
  WordOutcome::Error{code,offset,field} => code+i64_from_usize(offset)+field,
 }
}
@id("app.main") fn main()->i64 {
 let combined=decode(true);
 let combined_size=inspect(combined);
 let combined_value=finish(forward(combined));
 let combined_error=decode(false);
 let combined_error_value=finish(forward(combined_error));
 let words=decode_words(true);
 let words_size=inspect_words(words);
 let words_value=finish_words(forward_words(words));
 let words_error=decode_words(false);
 let words_error_value=finish_words(forward_words(words_error));
 if combined_size==4 && combined_value==59 && combined_error_value==28
    && words_size==2 && words_value==7 && words_error_value==28 {42}else{1}
}
"#;

#[test]
fn owned_collection_outcome_return_match_executes_on_interpreter_native_and_strict_wasm() {
    for (label, source) in [
        ("one-vector", SOURCE),
        (
            "two-vectors-and-string-only",
            SOURCE_TWO_VEC_AND_STRING_ONLY,
        ),
    ] {
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "owned-collection-outcome-{}-{label}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source_path = root.join("outcome.spx");
        let ast = semaprax::check(source, &source_path).expect("source admission");

        std::fs::write(&source_path, source).unwrap();
        for _ in 0..2 {
            let result = interpreter::interpret(
                &source_path,
                "app.main",
                &[],
                &interpreter::InterpreterOptions::default(),
            )
            .expect("interpreter execution");
            interpreter::verify_envelope(&result.envelope).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert!(result.returned, "{}", result.envelope);
            assert_eq!(envelope["payload"]["outcome"]["value"], "42");
        }

        run_native(&ast, &root);
        run_strict_wasm(&ast, &root);
        std::fs::remove_dir_all(root).unwrap();
    }
}

pub(super) fn run_native(ast: &semaprax::ast::Program, root: &std::path::Path) {
    run_native_generated(codegen::emit_c(ast).expect("native C emission"), root);
}

pub(super) fn run_native_generated(generated: String, root: &std::path::Path) {
    let tracked = generated
        .replace("malloc(", "outcome_malloc(")
        .replace("calloc(", "outcome_calloc(")
        .replace("realloc(", "outcome_realloc(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC outcome_realloc",
        )
        .replace("free(", "outcome_free(");
    let probe = r#"
int main(void) {
 struct spx_status_entry entries[32]; struct spx_context context={0};
 if(!spx_context_init(&context,UINT64_C(17),entries,32,NULL,NULL,NULL))return 1;
 for(unsigned i=0;i<3;++i){
  int64_t value=INT64_C(123456789);
  uint32_t before=context.status_arena.length;
  spx_status_token status=spx_decl_6170702e6d61696e(&context,&value);
  if(status!=SPX_STATUS_SUCCESS||value!=INT64_C(42)||context.status_arena.length!=before)return 2;
  if(outcome_allocations)return 3;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;++j)
   if(context.vec_authority[j].live)return 4;
 }
 return 0;
}
"#;
    let allocator = r#"#include <stdint.h>
#include <stdlib.h>
static uint64_t outcome_allocations=0;
static void *outcome_malloc(size_t n){void*p=malloc(n);if(p)++outcome_allocations;return p;}
static void *outcome_calloc(size_t n,size_t s){void*p=calloc(n,s);if(p)++outcome_allocations;return p;}
static void *outcome_realloc(void*p,size_t n){void*r=realloc(p,n);if(r&&!p)++outcome_allocations;return r;}
static void outcome_free(void*p){if(p){if(!outcome_allocations)abort();--outcome_allocations;free(p);}}"#;
    let c_path = root.join("outcome.c");
    std::fs::write(&c_path, format!("{allocator}\n{tracked}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("native-{optimization}"));
        let output = Command::new("clang")
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
                optimization,
            ])
            .arg(&c_path)
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("Clang must compile the C11 probe");
        assert!(
            output.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(&binary)
            .output()
            .expect("native outcome probe must execute");
        assert!(
            output.status.success(),
            "{optimization}: {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

pub(super) fn run_strict_wasm(ast: &semaprax::ast::Program, root: &std::path::Path) {
    let bytes = wasm::emit_module(ast).expect("Core Wasm emission");
    wasmparser::Validator::new()
        .validate_all(&bytes)
        .expect("strict Core Wasm validation");
    let module = root.join("outcome.wasm");
    std::fs::write(&module, bytes).unwrap();
    let host = include_str!("owned_leaf_vec/host.js");
    let output = Command::new("node")
        .arg("-e")
        .arg(host)
        .arg(&module)
        .arg("0")
        .arg("42")
        .arg("none")
        .output()
        .expect("Node must execute the strict private host");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
