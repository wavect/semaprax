//! Native C11 evidence for private scalar-snapshot closures.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.native_function_value_closures;
@id("closure.named") fn named(value:i64)->i64 { value + 1 }
@id("closure.make") fn make(offset:i64)->fn(i64)->i64 { fn(value:i64)->i64 { offset + value } }
@id("closure.choose") fn choose(flag:bool, offset:i64)->fn(i64)->i64 { if flag { named } else { fn(value:i64)->i64 { offset + value } } }
@id("closure.snapshot") fn snapshot()->i64 { let mut value=7; let callback=fn(input:i64)->i64 { value + input }; value=100; callback(5) }
@id("closure.all") fn all()->i64 {
 let a=7; let b=7i32; let c=7u8; let d=7usize; let e='x'; let f=7.0f32; let g=7.0f64; let h=true;
 let callback=fn()->i64 { if a==7 && b==7i32 && c==7u8 && d==7usize && e=='x' && f==7.0f32 && g==7.0f64 && h { 1 } else { 0 } };
 callback()
}
@id("closure.sixteen") fn sixteen()->i64 { let a=1;let b=2;let c=3;let d=4;let e=5;let f=6;let g=7;let h=8; let callback=fn(p:i64,q:i64,r:i64,s:i64,t:i64,u:i64,v:i64,w:i64)->i64 { a+b+c+d+e+f+g+h+p+q+r+s+t+u+v+w }; callback(1,2,3,4,5,6,7,8) }
@id("app.main") fn main()->i64 { let escaped=make(40); let picked=choose(false,20); let named_choice=choose(true,0); snapshot()+escaped(2)+picked(1)+named_choice(1)+all()+sixteen() }
"#;

fn clang_available() -> bool {
    let available = Command::new("clang").arg("--version").output().is_ok();
    assert!(
        available || std::env::var_os("SPX_REQUIRE_CLANG").is_none(),
        "SPX_REQUIRE_CLANG requires clang for native closure evidence"
    );
    available
}

fn path(suffix: &str) -> std::path::PathBuf {
    let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-native-closures-{}-{id}.{suffix}",
        std::process::id()
    ))
}

#[test]
fn native_closures_snapshot_escape_named_selection_and_all_scalar_cells_at_o0_o2() {
    let (program, _) = super::checked(SOURCE);
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    assert_eq!(generated, semaprax::codegen::emit_c(&program).unwrap());
    assert!(generated.contains("spx_closure_"));
    assert!(generated.contains("spx_closure_thunk_"));
    assert!(generated.contains("spx_reference_thunk_"));
    if clang_available() {
        let probe = r#"
int main(void) {
 struct spx_status_entry entries[UINT32_C(16)]; struct spx_context context={0};
 if(!spx_context_init(&context,UINT64_C(91),entries,UINT32_C(16),NULL,NULL,NULL)) return 1;
 for(unsigned run=0;run<2;run++){int64_t value=INT64_C(-1); if(spx_decl_6170702e6d61696e(&context,&value)!=SPX_STATUS_SUCCESS || value!=INT64_C(150)) return 2;} return 0;
}
"#;
        for optimization in ["-O0", "-O2"] {
            let source = path("c");
            let executable = path("native");
            std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
            let output = Command::new("clang")
                .args([
                    "-std=c11",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_NO_ENTRY_WRAPPER",
                ])
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{optimization}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                Command::new(&executable).status().unwrap().success(),
                "{optimization}"
            );
            let _ = std::fs::remove_file(source);
            let _ = std::fs::remove_file(executable);
        }
    }
    let node = Command::new("node").arg("--version").output().is_ok();
    assert!(
        node || std::env::var_os("SPX_REQUIRE_NODE").is_none(),
        "SPX_REQUIRE_NODE requires Node for Wasm closure evidence"
    );
    if node {
        let root = path("web");
        semaprax::wasm::build_web(&program, &root).unwrap();
        std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
        std::fs::write(
            root.join("probe.mjs"),
            r#"
import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'));
for(let run=0;run<2;run++){
  const value=instance.exports.semaprax_main();
  if(value!==150n) throw Error(`closure run ${run} returned ${value}`);
}
console.log('150');
"#,
        )
        .unwrap();
        let output = Command::new("node")
            .arg("probe.mjs")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "150");
        let _ = std::fs::remove_dir_all(root);
    }
}
