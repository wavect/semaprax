use crate::public_sdk::owner_sdk::prepare_owned_container_native;
use std::{fs, path::PathBuf, process::Command};

const SOURCE: &str = r#"module container.fixture;
@id("container.host") interface Host permits { } {
 @id("container.new") import rust fn make_value(pattern:i64) -> @TYPE@ from "fixture::make" effects { } failure infallible;
 @id("container.consume") import rust fn consume_value(value:own @TYPE@,input:i64) -> bool from "fixture::consume" effects { } failure infallible;
}
@id("container.make") fn make(pattern:i64,input:i64,divisor:i64) -> @TYPE@ {
 let spare=make_value(99);
 let second=make_value(88);
 let value=make_value(pattern);
 let checked=input/divisor;
 value
}
@id("container.forward") fn forward(value:own @TYPE@) -> @TYPE@ {value}
@id("container.run") fn run(pattern:i64,input:i64,divisor:i64) -> i64 {
 let value=forward(make(pattern,input,divisor));
 if consume_value(value,input) {1} else {0}
}
@id("container.run_late") fn run_late(pattern:i64,input:i64,divisor:i64) -> i64 {
 let value=forward(make(pattern,input,1));
 if consume_value(value,input/divisor) {1} else {0}
}
@id("container.main") fn main() -> i64 {0}
"#;

#[test]
fn owned_container_source_graph_move_and_closed_type_checks() {
    for ty in ["Option<string>", "Result<string, i64>"] {
        let ordinary = format!(
            r#"module ordinary; @id("ordinary.forward") fn forward(value:own {ty})->{ty} {{value}} @id("ordinary.main") fn main()->i64 {{0}}"#
        );
        let errors = semaprax::check(&ordinary, "ordinary.spx").unwrap_err();
        assert!(errors.iter().any(|e| e.code == "SPX-T223"), "{errors:?}");
        let source = SOURCE.replace("@TYPE@", ty);
        let checked = semaprax::check(&source, "container.spx").unwrap();
        let canonical = semaprax::format::canonical(&checked);
        let round = semaprax::check(&canonical, "container.spx").unwrap();
        let graph = semaprax::graph::to_json(&checked).unwrap();
        assert_eq!(graph, semaprax::graph::to_json(&round).unwrap());
        for expected in [
            "semaprax.graph.v58",
            "initialize_variant",
            "transfer_variant",
            "core.string.drop",
        ] {
            assert!(graph.contains(expected), "missing {expected}");
        }
        let generated = prepare_owned_container_native(&checked, "container.run").unwrap();
        assert_eq!(
            generated.c_source,
            prepare_owned_container_native(&round, "container.run")
                .unwrap()
                .c_source
        );
        assert!(prepare_owned_container_native(&checked, "container.make").is_err());
        let moved = source.replace("{value}", "{let used=consume_value(value,7);value}");
        let errors = semaprax::check(&moved, "moved-container.spx").unwrap_err();
        assert!(errors.iter().any(|e| e.code == "SPX-O101"), "{errors:?}");
    }
    for ty in [
        "Option<Option<string>>",
        "Result<string, string>",
        "Option<bytes>",
    ] {
        let errors =
            semaprax::check(&SOURCE.replace("@TYPE@", ty), "closed-container.spx").unwrap_err();
        assert!(errors.iter().any(|e| e.code == "SPX-P106"), "{errors:?}");
    }
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn owned_container_physical_some_none_domain_error_and_failures() {
    let rustc = std::env::var("RUSTC").expect("absolute rustc");
    let clang = std::env::var("CLANG").expect("absolute clang");
    let root =
        std::env::temp_dir().join(format!("semaprax-owned-container-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch(root.clone());
    for (ty, option) in [("Option<string>", true), ("Result<string, i64>", false)] {
        let checked = semaprax::check(&SOURCE.replace("@TYPE@", ty), "container.spx").unwrap();
        let generated = prepare_owned_container_native(&checked, "container.run").unwrap();
        let late = prepare_owned_container_native(&checked, "container.run_late").unwrap();
        fs::write(root.join("owner.h"), &generated.header).unwrap();
        let reserve = generated.rust_adapter.replace(
            "let produced=constructor(arg);",
            "if arg==101{return Err(4);} let produced=constructor(arg);",
        );
        let admission = generated.rust_adapter.replace(
            "value.len()>4096 || value.capacity()>4096",
            "value.len()>4096 || value.capacity()>4096 || arg==102",
        );
        let cleanup=generated.rust_adapter.replace("match caught(|| drop(value)) { Ok(()) => 0, Err(status) => status }","let injected=value.starts_with(\"0088\"); match caught(|| drop(value)) { Ok(()) => if injected {2}else{0}, Err(status) => status }");
        let missing=generated.c_source.replace("spx_container_drop(context,f->owners[","spx_control_skip_drop(context,f->owners[")
            .replace("#include <limits.h>","#include <limits.h>\nint32_t spx_control_skip_drop(uint64_t c,spx_container h){(void)c;(void)h;return 0;}");
        let collapsed = if option {
            generated
                .rust_adapter
                .replace("None=>(0,0,None)", "None=>return Err(9)")
        } else {
            generated
                .rust_adapter
                .replace("Err(error)=>(1,error,None)", "Err(_error)=>return Err(9)")
        };
        for mutation in [&reserve, &admission, &cleanup, &collapsed] {
            assert_ne!(mutation, &generated.rust_adapter);
        }
        for (label, opt, c, rust, cfg, success) in [
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
                "late-argument",
                "-O2",
                &late.c_source,
                &generated.rust_adapter,
                Some("late_argument"),
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
                "cleanup",
                "-O2",
                &generated.c_source,
                &cleanup,
                Some("injected_cleanup"),
                true,
            ),
            (
                "missing-drop",
                "-O2",
                &missing,
                &generated.rust_adapter,
                None,
                false,
            ),
            (
                "domain-collapsed",
                "-O2",
                &generated.c_source,
                &collapsed,
                None,
                false,
            ),
        ] {
            fs::write(root.join("owner.c"), c).unwrap();
            let compiled = Command::new(&clang)
                .args(["-std=c11", "-Wall", "-Wextra", "-Werror", opt, "-c"])
                .arg(root.join("owner.c"))
                .arg("-o")
                .arg(root.join("owner.o"))
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{ty}/{label} C: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let fixture = FIXTURE
                .replace("@OPTION@", if option { "true" } else { "false" })
                .replace(
                    "@MAKE@",
                    if option {
                        "if pattern<0{return None;}Some(text(pattern))"
                    } else {
                        "if pattern<0{return Err(pattern);}Ok(text(pattern))"
                    },
                )
                .replace(
                    "@CONSUME@",
                    if option {
                        "match value{Some(value)=>check(value,input),None=>input==-1}"
                    } else {
                        "match value{Ok(value)=>check(value,input),Err(error)=>error==input}"
                    },
                );
            fs::write(root.join("main.rs"), format!("{rust}\n{fixture}")).unwrap();
            let binary = root.join(label);
            let mut command = Command::new(&rustc);
            command.args(["--edition=2021", "-Dwarnings"]);
            if let Some(cfg) = cfg {
                command.args(["--cfg", cfg]);
            }
            let compiled = command
                .arg(root.join("main.rs"))
                .arg("-C")
                .arg(format!("link-arg={}", root.join("owner.o").display()))
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{ty}/{label} Rust: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let run = Command::new(binary).output().unwrap();
            assert_eq!(
                run.status.success(),
                success,
                "{ty}/{label}: {}",
                String::from_utf8_lossy(&run.stderr)
            );
            if !success {
                assert!(String::from_utf8_lossy(&run.stderr).contains("assertion"));
            }
        }
        fs::write(root.join("wrong_width.rs"), format!("{}\nmod fixture {{pub fn make(_:u64)->super::NativeValue{{panic!()}} pub fn consume(_:super::NativeValue,_:i64)->bool{{false}}}}",generated.rust_adapter)).unwrap();
        let wrong = Command::new(&rustc)
            .args(["--edition=2021", "--crate-type=lib", "--emit=metadata"])
            .arg(root.join("wrong_width.rs"))
            .arg("-o")
            .arg(root.join("wrong_width.rmeta"))
            .output()
            .unwrap();
        assert!(!wrong.status.success());
        assert!(
            String::from_utf8_lossy(&wrong.stderr).contains("E0308"),
            "{}",
            String::from_utf8_lossy(&wrong.stderr)
        );
    }
}

const FIXTURE: &str = r#"
use std::sync::atomic::{AtomicI64,AtomicUsize};
static TRACE:[AtomicI64;128]=[const{AtomicI64::new(0)};128];
static TRACE_LEN:AtomicUsize=AtomicUsize::new(0);
static CALLS:AtomicUsize=AtomicUsize::new(0);
static CREATED:AtomicUsize=AtomicUsize::new(0);
static CONSUMED:AtomicUsize=AtomicUsize::new(0);
struct Counted;
unsafe impl std::alloc::GlobalAlloc for Counted{
 unsafe fn alloc(&self,l:std::alloc::Layout)->*mut u8{unsafe{std::alloc::System.alloc(l)}}
 unsafe fn dealloc(&self,p:*mut u8,l:std::alloc::Layout){
  // Only fixture Strings allocate these lengths; their four initial bytes are
  // initialized ASCII IDs. Tracking itself never allocates or reenters.
  if matches!(l.size(),4093|4096|4097){let b=unsafe{std::slice::from_raw_parts(p,4)};let id=b.iter().fold(0i64,|v,b|v*10+i64::from(*b-b'0'));let i=TRACE_LEN.fetch_add(1,Ordering::SeqCst);if i<128{TRACE[i].store(id,Ordering::SeqCst);}}
  unsafe{std::alloc::System.dealloc(p,l)}
 }
}
#[global_allocator]static ALLOCATOR:Counted=Counted;
mod fixture{
 use super::*;
 fn text(id:i64)->String{if id==0{return String::new();}let len=if id>=4096{id as usize}else{4093};let mut bytes=vec![b' ';len];bytes[..4].copy_from_slice(format!("{id:04}").as_bytes());bytes[len-2..].copy_from_slice("λ".as_bytes());let text=String::from_utf8(bytes).unwrap();CREATED.store(text.as_ptr() as usize,Ordering::SeqCst);text}
 pub fn make(pattern:i64)->NativeValue{assert_ne!(pattern,-999,"constructor panic");@MAKE@}
 fn check(value:String,input:i64)->bool{CONSUMED.store(value.as_ptr() as usize,Ordering::SeqCst);assert_ne!(input,-777,"consumer panic");if value.is_empty(){input==0}else{value.ends_with('λ')&&value[..4].parse::<i64>().unwrap()==input}}
 pub fn consume(value:NativeValue,input:i64)->bool{CALLS.fetch_add(1,Ordering::SeqCst);@CONSUME@}
}
fn trace(expected:&[i64]){assert_eq!(TRACE_LEN.load(Ordering::SeqCst),expected.len());for(i,v)in expected.iter().enumerate(){assert_eq!(TRACE[i].load(Ordering::SeqCst),*v);}}
fn invoke(pattern:i64,input:i64,divisor:i64,status:i32,value:i64,drops:&[i64]){
 TRACE_LEN.store(0,Ordering::SeqCst);let context=spx_container_context_new();assert_ne!(context,0);let mut out=1234567;
 assert_eq!(unsafe{spx_container_entry(context,pattern,input,divisor,&mut out)},status);assert_eq!(out,value);trace(drops);assert_eq!(spx_container_context_close(context),0);
}
fn main(){
 if cfg!(injected_cleanup){invoke(7,7,1,2,1234567,&[88,99,7]);invoke(7,7,0,8,1234567,&[7,88,99]);return;}
 assert_eq!(spx_container_call(7,7,1),Ok(1));
 invoke(7,7,1,0,1,&[88,99,7]);invoke(7,8,1,0,0,&[88,99,7]);
 invoke(0,0,1,0,1,&[88,99]);
 let domain=if @OPTION@{-1}else{-73};invoke(domain,domain,1,0,1,&[88,99]);invoke(domain,3,1,0,0,&[88,99]);if !@OPTION@ {invoke(i64::MIN,i64::MIN,1,0,1,&[88,99]);}
 invoke(7,7,0,8,1234567,if cfg!(late_argument){&[88,99,7]}else{&[7,88,99]});invoke(-999,7,1,2,1234567,&[88,99]);invoke(7,-777,1,2,1234567,&[88,99,7]);
 invoke(4097,7,1,4,1234567,&[4097,88,99]);
 if cfg!(injected_reserve){invoke(101,7,1,4,1234567,&[88,99]);}
 if cfg!(injected_admission){invoke(102,7,1,4,1234567,&[102,88,99]);}
 TRACE_LEN.store(0,Ordering::SeqCst);let context=spx_container_context_new();let other=spx_container_context_new();let mut wire=SpxContainer::default();
 assert_eq!(unsafe{spx_container_new(context,4096,&mut wire)},0);assert_eq!(spx_container_drop(other,wire),3);trace(&[]);assert_eq!(spx_container_drop(context,wire),0);trace(&[4096]);assert_eq!(spx_container_drop(context,wire),3);
 TRACE_LEN.store(0,Ordering::SeqCst);assert_eq!(unsafe{spx_container_new(context,7,&mut wire)},0);let calls=CALLS.load(Ordering::SeqCst);let mut out=255;
 for bad in [SpxContainer{tag:9,..wire},SpxContainer{error:1,..wire},SpxContainer{reserved:[1;7],..wire},SpxContainer{tag:1-ACTIVE_TAG,error:0,..wire},SpxContainer{payload:SpxOwner{slot:u64::MAX,..wire.payload},..wire}]{assert_eq!(unsafe{spx_container_consume(context,bad,7,&mut out)},3);assert_eq!(out,255);assert_eq!(CALLS.load(Ordering::SeqCst),calls);trace(&[]);}
 assert_eq!(unsafe{spx_container_consume(context,wire,7,&mut out)},0);assert_eq!(out,1);trace(&[7]);assert_eq!(CREATED.load(Ordering::SeqCst),CONSUMED.load(Ordering::SeqCst));
 assert_eq!(unsafe{spx_container_consume(context,wire,7,&mut out)},3);trace(&[7]);
 assert_eq!(unsafe{spx_container_new(context,domain,&mut wire)},0);assert_eq!(wire.tag,1-ACTIVE_TAG);assert!(zero_owner(wire.payload));assert_eq!(unsafe{spx_container_consume(context,wire,domain,&mut out)},0);assert_eq!(out,1);trace(&[7]);
 if !@OPTION@{for error in [i64::MIN,i64::MAX]{let extreme=SpxContainer{error,..wire};assert_eq!(unsafe{spx_container_consume(context,extreme,error,&mut out)},0);assert_eq!(out,1);}}
 assert_eq!(spx_container_context_close(context),0);assert_eq!(spx_container_validate(context,wire),3);assert_eq!(spx_container_context_close(other),0);
}
"#;
