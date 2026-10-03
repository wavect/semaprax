//! Rust-only carrier validation and the physical C boundary have separate gates.
//! The native sanitizer gate does not claim to instrument Rust or run Miri.
use super::{prepare_owned_container_native, Scratch, FIXTURE, SOURCE};
use std::{fs, process::Command};

#[test]
fn owned_container_safety_allocation_transfer_and_width_controls() {
    let rustc = std::env::var("RUSTC").expect("absolute rustc");
    let root =
        std::env::temp_dir().join(format!("semaprax-container-safety-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch(root.clone());
    for (ty, option) in [("Option<string>", true), ("Result<string, i64>", false)] {
        let checked = semaprax::check(&SOURCE.replace("@TYPE@", ty), "safety.spx").unwrap();
        let generated = prepare_owned_container_native(&checked, "container.run").unwrap();
        // This unit has only Rust definitions. Miri can exercise this side of
        // the ownership boundary without pretending to execute generated C.
        let runtime = generated
            .rust_adapter
            .split_once("unsafe extern \"C\"{fn spx_container_entry")
            .expect("scalar entry boundary")
            .0;
        let fixture = RUST_ONLY
            .replace("@MAKE@", if option { "Some(value)" } else { "Ok(value)" })
            .replace(
                "@CONSUME@",
                if option {
                    "value.is_some()"
                } else {
                    "value.is_ok()"
                },
            );
        fs::write(root.join("carrier.rs"), format!("{runtime}\n{fixture}")).unwrap();
        let compiled = Command::new(&rustc)
            .args(["--edition=2021", "-Dwarnings"])
            .arg(root.join("carrier.rs"))
            .arg("-o")
            .arg(root.join("carrier"))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{ty}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = Command::new(root.join("carrier")).output().unwrap();
        assert!(
            ran.status.success(),
            "{ty}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        for wrong in ["i32", "u32", "u64", "isize", "usize"] {
            fs::write(root.join("width.rs"), format!("{runtime}\nmod fixture {{pub fn make(_:{wrong})->super::NativeValue{{panic!()}} pub fn consume(_:super::NativeValue,_:i64)->bool{{false}}}}" )).unwrap();
            let compiled = Command::new(&rustc)
                .args(["--edition=2021", "--crate-type=lib", "--emit=metadata"])
                .arg(root.join("width.rs"))
                .arg("-o")
                .arg(root.join("width.rmeta"))
                .output()
                .unwrap();
            assert!(
                !compiled.status.success(),
                "accepted incompatible actual width {wrong}"
            );
            assert!(
                String::from_utf8_lossy(&compiled.stderr).contains("E0308"),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
        }
    }
}

#[test]
fn owned_container_safety_native_address_undefined() {
    let rustc = std::env::var("RUSTC").expect("absolute rustc");
    let clang = std::env::var("CLANG").expect("absolute clang");
    let runtime_name = if cfg!(target_os = "macos") {
        "libclang_rt.asan_osx_dynamic.dylib".to_owned()
    } else {
        format!("libclang_rt.asan-{}.so", std::env::consts::ARCH)
    };
    let found = Command::new(&clang)
        .arg(format!("--print-file-name={runtime_name}"))
        .output()
        .unwrap();
    assert!(
        found.status.success(),
        "cannot discover configured Clang sanitizer runtime"
    );
    let runtime = std::path::PathBuf::from(String::from_utf8(found.stdout).unwrap().trim());
    assert!(
        runtime.is_absolute() && runtime.is_file(),
        "configured Clang sanitizer runtime unavailable: {}",
        runtime.display()
    );
    // rustc links with -nodefaultlibs, so Clang does not add its runtime.
    // This exact compiler's ASan runtime also supplies UBSan handlers.
    let sanitizer_link = [
        format!("link-arg={}", runtime.display()),
        format!(
            "link-arg=-Wl,-rpath,{}",
            runtime.parent().unwrap().display()
        ),
    ];
    let root = std::env::temp_dir().join(format!(
        "semaprax-container-sanitizer-{}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch(root.clone());
    for (ty, option) in [("Option<string>", true), ("Result<string, i64>", false)] {
        let checked = semaprax::check(&SOURCE.replace("@TYPE@", ty), "sanitizer.spx").unwrap();
        let generated = prepare_owned_container_native(&checked, "container.run").unwrap();
        fs::write(root.join("owner.h"), &generated.header).unwrap();
        fs::write(root.join("owner.c"), &generated.c_source).unwrap();
        let compiled = Command::new(&clang)
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-O1",
                "-g",
                "-fno-omit-frame-pointer",
                "-fsanitize=address,undefined",
                "-fno-sanitize-recover=all",
                "-c",
            ])
            .arg(root.join("owner.c"))
            .arg("-o")
            .arg(root.join("owner.o"))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "sanitizer compiler unavailable or failed: {}",
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
        fs::write(
            root.join("main.rs"),
            format!("{}\n{fixture}", generated.rust_adapter),
        )
        .unwrap();
        let compiled = Command::new(&rustc)
            .args(["--edition=2021", "-Dwarnings", "-C"])
            .arg(format!("linker={clang}"))
            .arg("-C")
            .arg("link-arg=-fsanitize=address,undefined")
            .args(sanitizer_link.iter().flat_map(|arg| ["-C", arg.as_str()]))
            .arg("-C")
            .arg(format!("link-arg={}", root.join("owner.o").display()))
            .arg(root.join("main.rs"))
            .arg("-o")
            .arg(root.join("sanitized"))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "sanitizer runtime unavailable or link failed: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = Command::new(root.join("sanitized"))
            .env("ASAN_OPTIONS", "halt_on_error=1:detect_leaks=0")
            .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1")
            .output()
            .unwrap();
        assert!(
            ran.status.success(),
            "{ty}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        if option {
            // Prove this invocation actually loaded an active ASan runtime.
            // This deliberate fault exists only in the isolated child fixture.
            fs::write(root.join("probe.c"), "#include <stdlib.h>\nvoid spx_asan_probe(void){volatile unsigned char *p=malloc(1);free((void*)p);(void)*p;}\n").unwrap();
            let probe = Command::new(&clang)
                .args(["-O1", "-g", "-fsanitize=address,undefined", "-c"])
                .arg(root.join("probe.c"))
                .arg("-o")
                .arg(root.join("probe.o"))
                .output()
                .unwrap();
            assert!(
                probe.status.success(),
                "{}",
                String::from_utf8_lossy(&probe.stderr)
            );
            fs::write(
                root.join("probe.rs"),
                "unsafe extern \"C\" {fn spx_asan_probe();} fn main(){unsafe{spx_asan_probe();}}\n",
            )
            .unwrap();
            let probe = Command::new(&rustc)
                .args(["--edition=2021", "-C"])
                .arg(format!("linker={clang}"))
                .arg("-C")
                .arg("link-arg=-fsanitize=address,undefined")
                .args(sanitizer_link.iter().flat_map(|arg| ["-C", arg.as_str()]))
                .arg("-C")
                .arg(format!("link-arg={}", root.join("probe.o").display()))
                .arg(root.join("probe.rs"))
                .arg("-o")
                .arg(root.join("probe"))
                .output()
                .unwrap();
            assert!(
                probe.status.success(),
                "{}",
                String::from_utf8_lossy(&probe.stderr)
            );
            let probe = Command::new(root.join("probe"))
                .env("ASAN_OPTIONS", "halt_on_error=1:detect_leaks=0")
                .output()
                .unwrap();
            assert!(
                !probe.status.success(),
                "ASan negative control did not fail"
            );
            assert!(
                String::from_utf8_lossy(&probe.stderr).contains("heap-use-after-free"),
                "{}",
                String::from_utf8_lossy(&probe.stderr)
            );
        }
    }
}

const RUST_ONLY: &str = r#"
use std::sync::atomic::{AtomicBool,AtomicUsize};
static FAIL_NEXT:AtomicBool=AtomicBool::new(false);
static REFUSED:AtomicUsize=AtomicUsize::new(0);
static CALLS:AtomicUsize=AtomicUsize::new(0);
static DROPS:AtomicUsize=AtomicUsize::new(0);
struct FaultAllocator;
unsafe impl std::alloc::GlobalAlloc for FaultAllocator {
 unsafe fn alloc(&self,layout:std::alloc::Layout)->*mut u8 {
  if FAIL_NEXT.swap(false,Ordering::SeqCst){REFUSED.fetch_add(1,Ordering::SeqCst);std::ptr::null_mut()}
  else{unsafe{std::alloc::System.alloc(layout)}}
 }
 unsafe fn dealloc(&self,p:*mut u8,layout:std::alloc::Layout){if layout.size()==4093{DROPS.fetch_add(1,Ordering::SeqCst);}unsafe{std::alloc::System.dealloc(p,layout)}}
}
#[global_allocator] static ALLOCATOR:FaultAllocator=FaultAllocator;
mod fixture {
 use super::*;
 pub fn make(_:i64)->NativeValue{CALLS.fetch_add(1,Ordering::SeqCst);let value=String::from_utf8(vec![b'x';4093]).unwrap();@MAKE@}
 pub fn consume(value:NativeValue,_:i64)->bool{CALLS.fetch_add(1,Ordering::SeqCst);@CONSUME@}
}
fn snapshot(v:SpxContainer)->(u8,[u8;7],i64,u64,u64,u64){(v.tag,v.reserved,v.error,v.payload.context,v.payload.generation,v.payload.slot)}
fn main(){
 FAIL_NEXT.store(true,Ordering::SeqCst);assert_eq!(spx_container_context_new(),0);assert_eq!(REFUSED.load(Ordering::SeqCst),1);assert_eq!(CALLS.load(Ordering::SeqCst),0);
 let context=spx_container_context_new();assert_ne!(context,0);
 let mut wire=SpxContainer{tag:9,reserved:[0xab;7],error:i64::MIN,payload:SpxOwner{context:u64::MAX,generation:u64::MAX,slot:u64::MAX}};
 let poison=snapshot(wire);
 FAIL_NEXT.store(true,Ordering::SeqCst);assert_eq!(unsafe{spx_container_new(context,1,&mut wire)},4);assert_eq!(snapshot(wire),poison);assert_eq!(CALLS.load(Ordering::SeqCst),0);assert_eq!(REFUSED.load(Ordering::SeqCst),2);
 assert_eq!(unsafe{spx_container_new(context,1,std::ptr::null_mut())},3);
 let mut aligned=[0u64;16];let misaligned=unsafe{aligned.as_mut_ptr().cast::<u8>().add(1).cast::<SpxContainer>()};assert_eq!(unsafe{spx_container_new(context,1,misaligned)},3);assert_eq!(CALLS.load(Ordering::SeqCst),0);
 let mut owners=[SpxContainer::default();32];for owner in &mut owners {assert_eq!(unsafe{spx_container_new(context,1,owner)},0);}
 assert_eq!(CALLS.load(Ordering::SeqCst),32);assert_eq!(unsafe{spx_container_new(context,1,&mut wire)},4);assert_eq!(CALLS.load(Ordering::SeqCst),32);assert_eq!(snapshot(wire),poison);assert_eq!(DROPS.load(Ordering::SeqCst),0);
 let valid=owners[7];let calls=CALLS.load(Ordering::SeqCst);let mut byte=0xa5;
 // Every byte of reserved/tag and every token component is initialized before
 // mutation. Arbitrary Rust enum layouts or allocation addresses are never read.
 for n in 0..256u64 {
  let invalid=[SpxContainer{tag:2+(n%254)as u8,..valid},SpxContainer{reserved:[(n%255+1)as u8;7],..valid},SpxContainer{error:(n+1)as i64,..valid},SpxContainer{payload:SpxOwner{context:0,..valid.payload},..valid},SpxContainer{payload:SpxOwner{generation:valid.payload.generation+n+1,..valid.payload},..valid},SpxContainer{payload:SpxOwner{slot:32+n,..valid.payload},..valid}];
  for bad in invalid {assert_eq!(spx_container_validate(context,bad),3);assert_eq!(unsafe{spx_container_consume(context,bad,1,&mut byte)},3);assert_eq!(spx_container_drop(context,bad),3);assert_eq!(byte,0xa5);}
 }
 assert_eq!(CALLS.load(Ordering::SeqCst),calls);assert_eq!(DROPS.load(Ordering::SeqCst),0);
 assert_eq!(spx_container_drop(context,valid),0);assert_eq!(DROPS.load(Ordering::SeqCst),1);
 assert_eq!(unsafe{spx_container_new(context,1,&mut wire)},0);assert_eq!(wire.payload.slot,valid.payload.slot);assert_ne!(wire.payload.generation,valid.payload.generation);assert_eq!(spx_container_drop(context,valid),3);
 assert_eq!(unsafe{spx_container_consume(context,wire,1,&mut byte)},0);assert_eq!(byte,1);assert_eq!(DROPS.load(Ordering::SeqCst),2);assert_eq!(unsafe{spx_container_consume(context,wire,1,&mut byte)},3);
 assert_eq!(spx_container_context_close(context),5);assert_eq!(DROPS.load(Ordering::SeqCst),2);assert_eq!(spx_container_validate(context,owners[0]),0);for(index,owner)in owners.iter().enumerate(){if index!=7{assert_eq!(spx_container_drop(context,*owner),0);}}assert_eq!(spx_container_context_close(context),0);assert_eq!(DROPS.load(Ordering::SeqCst),33);assert_eq!(spx_container_validate(context,owners[0]),3);assert_eq!(spx_container_context_close(context),3);
}
"#;
