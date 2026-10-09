use semaprax::{codegen, hir};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug)]
pub(super) enum Failure {
    None,
    CloneLeaf(u32),
    Realloc,
}

pub(super) fn run(source: &str, code: u32, expected: i64, failure: Failure) {
    let ast = semaprax::check(source, "owned-leaf-native.spx").expect("source admission");
    let resolved = hir::resolve(&ast).expect("resolved admission");
    hir::validate(&resolved).expect("independent ownership replay");
    let generated = codegen::emit_c(&ast).expect("native owning projection");
    assert!(generated.contains("spx_leaf_clone"));
    // Inject failure only in the clone worker. Ordinary String/Bytes creation
    // keeps its existing policy; this checks the new fallible clone contract.
    // Both exact sites are asserted so moving the worker cannot reduce coverage.
    let string_site = "struct spx_string_v10 *copy = malloc(";
    let bytes_site = "copy.ptr = malloc(";
    assert_eq!(generated.matches(string_site).count(), 1);
    assert_eq!(generated.matches(bytes_site).count(), 1);
    let tracked = generated
        .replace(
            string_site,
            "struct spx_string_v10 *copy = fixture_clone_malloc(",
        )
        .replace(bytes_site, "copy.ptr = fixture_clone_malloc(");
    let clone_leaf = match failure {
        Failure::CloneLeaf(leaf) => leaf,
        _ => 0,
    };
    let refuse_realloc = u8::from(matches!(failure, Failure::Realloc));
    let probe = format!(
        r#"
#undef malloc
#undef calloc
#undef realloc
#undef free
int main(void) {{
 struct spx_status_entry entries[32]; struct spx_context context={{0}};
 if(!spx_context_init(&context,UINT64_C(17),entries,32,NULL,NULL,NULL))return 1;
 for(unsigned repeat=0;repeat<4;++repeat){{
  fixture_clone_attempts=0;fixture_refusals=0;
  fixture_clone_failure={clone_leaf};fixture_realloc_failure={refuse_realloc};
  int64_t result=INT64_C(123456789);uint32_t before=context.status_arena.length;
  spx_status_token status=spx_decl_6f776e65642e6c6561662e6d61696e(&context,&result);
  if({code}==0){{
   if(status!=SPX_STATUS_SUCCESS||result!=INT64_C({expected})||context.status_arena.length!=before)return 2;
  }}else{{
   const struct spx_normalized_status *entry=spx_status_resolve(&context,status);
   if(status==SPX_STATUS_SUCCESS||result!=INT64_C(123456789)||!entry||entry->code!={code}
      ||strcmp(entry->domain_id,"semaprax.vec.v1")||context.status_arena.length!=before+1)return 3;
  }}
  if(fixture_live)return 4;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;++j)if(context.vec_authority[j].live)return 5;
  if(fixture_refusals!=({clone_leaf}!=0||{refuse_realloc}!=0))return 6;
  if({clone_leaf}!=0&&fixture_clone_attempts!={clone_leaf})return 7;
 }}return 0;
}}
"#
    );
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-leaf-native-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let c = root.join("case.c");
    std::fs::write(
        &c,
        format!("{}\n{tracked}\n{probe}", include_str!("allocations.c")),
    )
    .unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!(
            "case{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let built = Command::new("clang")
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
            .arg(&binary)
            .output()
            .expect("clang is required by this native gate");
        assert!(
            built.status.success(),
            "{failure:?} {optimization}: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{failure:?} {optimization}: {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
