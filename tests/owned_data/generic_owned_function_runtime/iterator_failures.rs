//! Iterator commit/publication failures retain one owner and one selected status.
use super::*;
const SOURCE: &str = r#"module test.iterator_failure;
@id("iter.handoff") fn handoff(value:own Iter<i64>,allowed:bool)->Iter<i64> ensures allowed {value}
@id("app.main") fn main()->i64{
 let values=vec_push<i64>(vec_with_capacity<i64>(1usize),7);
 let iterator=handoff(vec_into_iter<i64>(values),true);
 match own iter_next<i64>(iterator){IterStep::Done{}=>0,IterStep::Yield{item,rest}=>item,}
}"#;
#[test]
fn owning_iterators_private_return_postcondition_settles_provisional_owner() {
    super::collections::run_source_value(SOURCE, 7);
    super::collections::run_source_postcondition_failure(
        &SOURCE.replace("(values),true)", "(values),false)"),
    );
}
#[test]
fn owning_iterators_native_read_failure_preserves_carrier_and_output() {
    let generated =
        codegen::emit_c(&semaprax::check(SOURCE, "iterator-native-failure.spx").unwrap()).unwrap();
    let case = format!(
        "spx_case_{}",
        "core.iter-step.yield"
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let rest = format!(
        "spx_field_{}",
        "core.iter-step.yield.rest"
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let probe = format!(
        r#"
int main(void) {{
 struct spx_status_entry entries[32]; struct spx_context context={{0}};
 if(!spx_context_init(&context,17,entries,32,NULL,NULL,NULL))return 1;
 for(uint32_t attempt=0;attempt<4;++attempt){{
  spx_vec_v1 empty={{0}},values={{0}};
  if(spx_vec_with_capacity(&context,1,1,&empty)!=SPX_STATUS_SUCCESS)return 2;
  if(spx_vec_push(&context,1,&empty,7,&values)!=SPX_STATUS_SUCCESS)return 3;
  spx_iter_v1 iterator=spx_iter_from_vec(&context,&values,1);
  uint32_t authority=iterator.vec.authority; uint64_t generation=iterator.vec.generation;
  iterator.cursor=2; spx_iter_step_v1 output; memset(&output,0xa5,sizeof output);
  uint32_t before=context.status_arena.length;
  spx_status_token status=spx_iter_next(&context,&iterator,1,&output);
  if(status==SPX_STATUS_SUCCESS)return 4;
  const struct spx_normalized_status *entry=spx_status_resolve(&context,status);
  if(entry==NULL||strcmp(entry->domain_id,"semaprax.vec.v1")!=0||entry->code!=2)return 5;
  if(context.status_arena.length!=before+1||iterator.cursor!=2||iterator.vec.authority!=authority||iterator.vec.generation!=generation)return 6;
  for(size_t byte=0;byte<sizeof output;++byte)if(((const unsigned char*)&output)[byte]!=0xa5)return 7;
  iterator.cursor=0;
  if(spx_iter_next(&context,&iterator,1,&output)!=SPX_STATUS_SUCCESS||output.spx_tag!=1)return 8;
  if(iterator.vec.authority!=0)return 9;
  spx_iter_drop(&context,&output.spx_payload.{case}.{rest});
  for(uint32_t i=0;i<SPX_VEC_AUTHORITY_CAPACITY;++i)if(context.vec_authority[i].live)return 10;
 }}
 return 0;
}}
"#
    );
    for optimization in ["-O0", "-O2"] {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "semaprax-iterator-read-{}-{serial}",
            std::process::id()
        ));
        let c = base.with_extension("c");
        let exe = base.with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&c, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&c)
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = Command::new(&exe).output().unwrap();
        assert!(
            ran.status.success(),
            "{:?}: {}",
            ran.status,
            String::from_utf8_lossy(&ran.stderr)
        );
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(exe);
    }
}
