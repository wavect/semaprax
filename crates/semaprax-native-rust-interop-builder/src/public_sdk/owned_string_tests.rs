use super::owner_sdk::prepare_owned_string_native;
use std::{fs, process::Command};

const SOURCE: &str = r#"module text.fixture;
@id("text.host") interface Host permits { } {
 @id("text.new") import rust fn text_new(pattern:i64) -> string from "fixture_string::make" effects { } failure infallible;
 @id("text.consume") import rust fn text_consume(text:own string,input:i64) -> bool from "fixture_string::consume" effects { } failure infallible;
}
@id("text.make") fn make(pattern:i64,input:i64,divisor:i64) -> string {
 let spare = text_new(99);
 let second = text_new(88);
 let text = text_new(pattern);
 let checked = input / divisor;
 text
}
@id("text.forward") fn forward(text:string) -> string {text}
@id("text.run") fn run(pattern:i64,input:i64,divisor:i64) -> i64 {
 let text = forward(make(pattern,input,divisor));
 if text_consume(text,input) {1} else {0}
}
@id("text.main") fn main() -> i64 {0}
"#;

// A real allocator observes ownership of string's actual allocation. Tracking
// uses fixed atomic storage so the allocator callback never allocates/reenters.
const FIXTURE: &str = r#"
use std::sync::atomic::{AtomicI64,AtomicUsize};
static TRACE:[AtomicI64;128]=[const {AtomicI64::new(0)};128];
static TRACE_LEN:AtomicUsize=AtomicUsize::new(0);
static CREATED_POINTER:AtomicUsize=AtomicUsize::new(0);
static CONSUMED_POINTER:AtomicUsize=AtomicUsize::new(0);
struct Counted;
unsafe impl std::alloc::GlobalAlloc for Counted {
 unsafe fn alloc(&self,l:std::alloc::Layout)->*mut u8 {unsafe{std::alloc::System.alloc(l)}}
 unsafe fn dealloc(&self,p:*mut u8,l:std::alloc::Layout) {
  // Only the fixture's fully initialized Strings allocate these exact lengths.
  // Their first four ASCII bytes identify the original value and every copy.
  if matches!(l.size(),4093|4096|4097) {
   let bytes=unsafe{std::slice::from_raw_parts(p,4)};
   let id=bytes.iter().fold(0i64,|id,b|id*10+i64::from(*b-b'0'));
   let at=TRACE_LEN.fetch_add(1,Ordering::SeqCst);
   if at<128 {TRACE[at].store(id,Ordering::SeqCst);}
  }
  unsafe{std::alloc::System.dealloc(p,l)}
 }
}
#[global_allocator]static ALLOCATOR:Counted=Counted;
mod fixture_string {
 use super::*;
 pub fn make(pattern:i64)->String {
  assert_ne!(pattern,-999,"constructor panic");
  let length=if pattern>=4096 {pattern as usize} else {4093};
  let mut bytes=vec![b' ';length];
  bytes[..4].copy_from_slice(format!("{pattern:04}").as_bytes());
  bytes[length-2..].copy_from_slice("λ".as_bytes());
  let text=String::from_utf8(bytes).unwrap();
  CREATED_POINTER.store(text.as_ptr() as usize,Ordering::SeqCst);
  text
 }
 pub fn consume(text:String,input:i64)->bool {
  CONSUMED_POINTER.store(text.as_ptr() as usize,Ordering::SeqCst);
  assert_ne!(input,-777,"consumer panic");
  text.ends_with('λ') && text[..4].parse::<i64>().unwrap()==input
 }
}
fn trace(expected:&[i64]) {
 let n=TRACE_LEN.load(Ordering::SeqCst);assert_eq!(n,expected.len());
 for (i,v) in expected.iter().enumerate(){assert_eq!(TRACE[i].load(Ordering::SeqCst),*v);}
}
fn invoke(pattern:i64,input:i64,divisor:i64,status:i32,value:i64,drops:&[i64]) {
 TRACE_LEN.store(0,Ordering::SeqCst);
 let context=spx_owner_context_new();assert_ne!(context,0);
 let mut out=1234567;
 assert_eq!(unsafe{spx_owner_entry(context,pattern,input,divisor,&mut out)},status);
 assert_eq!(out,value);trace(drops);
 assert_eq!(spx_owner_context_close(context),0);
}
fn main(){
 assert_eq!(spx_owner_call(7,7,1),Ok(1));
 // String place reads copy. Source owners drop in the canonical plan's order.
 invoke(7,7,1,0,1,&[7,88,99,7,7,7]);
 invoke(7,8,1,0,0,&[7,88,99,7,7,7]);
 invoke(7,7,0,8,1234567,&[7,88,99]);
 invoke(-999,7,1,2,1234567,&[88,99]);
 invoke(7,-777,1,2,1234567,&[7,88,99,7,7,7]);
 invoke(4097,7,1,4,1234567,&[4097,88,99]);
 if cfg!(injected_reserve){invoke(101,7,1,4,1234567,&[88,99]);}
 if cfg!(injected_admission){invoke(102,7,1,4,1234567,&[102,88,99]);}
 TRACE_LEN.store(0,Ordering::SeqCst);
 let context=spx_owner_context_new();let other=spx_owner_context_new();
 let mut wire=SpxOwner::default();assert_eq!(unsafe{spx_owner_new(context,4096,&mut wire)},0);
 assert_eq!(spx_owner_drop(other,wire),3);trace(&[]);
 assert_eq!(spx_owner_drop(context,wire),0);trace(&[4096]);
 assert_eq!(spx_owner_drop(context,wire),3);trace(&[4096]);
 TRACE_LEN.store(0,Ordering::SeqCst);
 assert_eq!(unsafe{spx_owner_new(context,7,&mut wire)},0);
 let mut result=255;assert_eq!(unsafe{spx_owner_consume(context,wire,7,&mut result)},0);
 assert_eq!(result,1);trace(&[7]);
 assert_eq!(CREATED_POINTER.load(Ordering::SeqCst),CONSUMED_POINTER.load(Ordering::SeqCst),"wire transfer copied String");
 assert_eq!(spx_owner_context_close(context),0);assert_eq!(spx_owner_context_close(other),0);
}
"#;

#[test]
fn owned_string_source_graph_and_move_checks() {
    let p = semaprax::check(SOURCE, "native-string.spx").unwrap();
    let canonical = semaprax::format::canonical(&p);
    let round = semaprax::check(&canonical, "native-string.spx").unwrap();
    let graph = semaprax::graph::to_json(&p).unwrap();
    assert_eq!(semaprax::graph::to_json(&round).unwrap(), graph);
    assert!(graph.contains("semaprax.graph.v57"));
    assert!(graph.contains("core.string.drop"));
    let moved = SOURCE.replace(
        "-> string {text}",
        "-> string {let used = text_consume(text,7); text}",
    );
    let errors = semaprax::check(&moved, "moved-string.spx").unwrap_err();
    assert!(errors.iter().any(|e| e.code == "SPX-O101"), "{errors:?}");
    let generated = prepare_owned_string_native(&p, "text.run").unwrap();
    assert_eq!(
        generated.c_source,
        prepare_owned_string_native(&round, "text.run")
            .unwrap()
            .c_source
    );
    assert!(prepare_owned_string_native(&p, "text.make").is_err());
    let mut selected = p.clone();
    for import in &mut selected.interfaces[0].imports {
        import.index_selected = true;
        let signature = if import.name == "text_new" {
            "fn make(pattern: i64) -> alloc::string::String"
        } else {
            "fn consume(text: alloc::string::String, input: i64) -> bool"
        };
        assert!(
            semaprax::native_rust_binding::bind_selected_owner_signature(
                import,
                &selected.types,
                signature,
                &format!("sha256:{}", "a".repeat(64)),
                "none",
            )
            .unwrap()
        );
    }
    assert_eq!(
        prepare_owned_string_native(&selected, "text.run")
            .unwrap()
            .c_source,
        generated.c_source
    );
    let error = semaprax::native_rust_binding::bind_selected_owner_signature(
        &mut selected.interfaces[0].imports[0],
        &selected.types,
        "fn make(pattern: u64) -> alloc::string::String",
        &format!("sha256:{}", "a".repeat(64)),
        "none",
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-B145");
}

#[test]
fn owned_string_generated_execution_and_allocation_controls() {
    let rustc = std::env::var("RUSTC").expect("absolute rustc");
    let clang = std::env::var("CLANG").expect("absolute clang");
    let p = semaprax::check(SOURCE, "native-string.spx").unwrap();
    let generated = prepare_owned_string_native(&p, "text.run").unwrap();
    let root = std::env::temp_dir().join(format!("semaprax-owned-string-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("owner.h"), &generated.header).unwrap();
    // Inject refusal at the actual fallible bridge-reservation boundary.
    let reserve = generated.rust_adapter.replace(
        "context.slots.try_reserve(1).map_err(|_|4)?;",
        "if arg==101 {return Err(4);} context.slots.try_reserve(1).map_err(|_|4)?;",
    );
    assert_ne!(reserve, generated.rust_adapter);
    let admission = generated.rust_adapter.replacen(
        "value.len()>4096 || value.capacity()>4096",
        "value.len()>4096 || value.capacity()>4096 || arg==102",
        1,
    );
    assert_ne!(admission, generated.rust_adapter);
    let aborted = generated.rust_adapter.replace(
        "value.try_reserve_exact(source.len())",
        "{ if source.starts_with(\"0007\") {std::process::abort();} value.try_reserve_exact(source.len()) }",
    );
    assert_ne!(aborted, generated.rust_adapter);
    let missing=generated.c_source.replace("spx_owner_drop(context,f->owners[","spx_control_skip_drop(context,f->owners[")
  .replace("#include <limits.h>","#include <limits.h>\nint32_t spx_control_skip_drop(uint64_t c,spx_owner h){(void)c;(void)h;return 0;}");
    for (name, opt, c, rust, cfg, success) in [
        (
            "o0",
            "-O0",
            &generated.c_source,
            &generated.rust_adapter,
            None,
            true,
        ),
        (
            "o2",
            "-O2",
            &generated.c_source,
            &generated.rust_adapter,
            None,
            true,
        ),
        (
            "reserve",
            "-O2",
            &generated.c_source,
            &reserve,
            Some("injected_reserve"),
            true,
        ),
        (
            "admission",
            "-O2",
            &generated.c_source,
            &admission,
            Some("injected_admission"),
            true,
        ),
        (
            "clone-allocation-abort",
            "-O2",
            &generated.c_source,
            &aborted,
            None,
            false,
        ),
        (
            "missing-drop",
            "-O2",
            &missing,
            &generated.rust_adapter,
            None,
            false,
        ),
    ] {
        fs::write(root.join("owner.c"), c).unwrap();
        let compile = Command::new(&clang)
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", opt, "-c"])
            .arg(root.join("owner.c"))
            .arg("-o")
            .arg(root.join("owner.o"))
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        fs::write(root.join("main.rs"), format!("{rust}\n{FIXTURE}")).unwrap();
        let mut command = Command::new(&rustc);
        command.args(["--edition=2021", "-Dwarnings"]);
        if let Some(cfg) = cfg {
            command.args(["--cfg", cfg]);
        }
        let binary = root.join(name);
        let compile = command
            .arg(root.join("main.rs"))
            .arg("-C")
            .arg(format!("link-arg={}", root.join("owner.o").display()))
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = Command::new(binary).output().unwrap();
        #[cfg(unix)]
        if name == "clone-allocation-abort" {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(
                run.status.signal(),
                Some(6),
                "clone allocation failure must abort"
            );
        }

        assert_eq!(
            run.status.success(),
            success,
            "{name}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    fs::remove_dir_all(root).unwrap();
}
