//! Physical C ingress into the bounded generated String owner table.
use super::*;
struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn owned_utf8_native_ingress_widths_allocation_and_hostile_controls() {
    let rustc = std::env::var("RUSTC").expect("configured rustc");
    let clang = std::env::var("CLANG").expect("configured clang");
    let root = std::env::temp_dir().join(format!("semaprax-utf8-ingress-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch(root.clone());
    let program = semaprax::check(SOURCE, "utf8-ingress.spx").unwrap();
    let generated = prepare_owned_string_native(&program, "text.run").unwrap();
    fs::write(root.join("owner.h"), &generated.header).unwrap();
    fs::write(
        root.join("owner.c"),
        format!(
            "{}\n{}",
            generated.c_source,
            include_str!("owned_string_input_probe.c")
        ),
    )
    .unwrap();
    let allocation=generated.rust_adapter.replace("value.try_reserve_exact(text.len()).map_err(|_| 4)?;","if !text.is_empty() { return Err(4); } value.try_reserve_exact(text.len()).map_err(|_| 4)?;");
    let unchecked = generated
        .rust_adapter
        .replace("Err(_) => return 3 };", "Err(_) => \"\" };");
    assert_ne!(allocation, generated.rust_adapter);
    assert_ne!(unchecked, generated.rust_adapter);
    for (label, opt, adapter, cfg, expected) in [
        ("o0", "-O0", &generated.rust_adapter, None, true),
        ("o2", "-O2", &generated.rust_adapter, None, true),
        (
            "allocation",
            "-O2",
            &allocation,
            Some("allocation_failure"),
            true,
        ),
        ("accept-invalid", "-O2", &unchecked, None, false),
    ] {
        let compiled = Command::new(&clang)
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", opt, "-c"])
            .arg(root.join("owner.c"))
            .arg("-o")
            .arg(root.join("owner.o"))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        fs::write(root.join("main.rs"), format!("{adapter}\n{FIXTURE}")).unwrap();
        let mut command = Command::new(&rustc);
        command.args(["--edition=2021", "-Dwarnings"]);
        if let Some(cfg) = cfg {
            command.args(["--cfg", cfg]);
        }
        let binary = root.join(label);
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
            "{label}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = Command::new(binary).output().unwrap();
        assert_eq!(
            ran.status.success(),
            expected,
            "{label}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        if expected && cfg.is_none() {
            assert_eq!(ran.stdout, b"native-utf8-ingress-ok\n");
        }
        if !expected {
            assert!(String::from_utf8_lossy(&ran.stderr).contains("native-utf8-ingress"));
        }
    }
}

const FIXTURE: &str = r#"
use std::alloc::{GlobalAlloc,Layout,System};
use std::sync::atomic::AtomicUsize;
static FREES:AtomicUsize=AtomicUsize::new(0);
static CONSTRUCTORS:AtomicUsize=AtomicUsize::new(0);
static CONSUMERS:AtomicUsize=AtomicUsize::new(0);
struct CountAlloc;
unsafe impl GlobalAlloc for CountAlloc {
 unsafe fn alloc(&self,l:Layout)->*mut u8{unsafe{System.alloc(l)}}
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){if l.size()==4096{FREES.fetch_add(1,Ordering::SeqCst);}unsafe{System.dealloc(p,l)}}
}
#[global_allocator]static ALLOCATOR:CountAlloc=CountAlloc;
unsafe extern "C"{fn spx_input_probe(context:u64)->i32;fn spx_input_source()->usize;}
mod fixture_string {
 use super::*;
 pub fn make(pattern:i64)->String{CONSTRUCTORS.fetch_add(1,Ordering::SeqCst);pattern.to_string()}
 pub fn consume(value:String,input:i64)->bool{
  CONSUMERS.fetch_add(1,Ordering::SeqCst);
  assert_ne!(value.as_ptr() as usize,unsafe{spx_input_source()},"Rust allocation must copy input");
  match input{0=>value.is_empty(),2=>value=="λ",4096=>value.len()==4096&&value.bytes().all(|b|b==b'x'),_=>false}
 }
}
fn unchanged(value:SpxOwner){assert_eq!((value.context,value.generation,value.slot),(u64::MAX,u64::MAX,u64::MAX));}
fn main(){
 let context=spx_owner_context_new();assert_ne!(context,0);
 let sentinel=SpxOwner{context:u64::MAX,generation:u64::MAX,slot:u64::MAX};
 if cfg!(allocation_failure){
  let mut out=sentinel;
  assert_eq!(unsafe{spx_owner_string_from_utf8(context,"λ".as_ptr(),2,&mut out)},4);unchanged(out);
  assert_eq!(FREES.load(Ordering::SeqCst),0);assert_eq!(spx_owner_context_close(context),0);return;
 }
 assert_eq!(unsafe{spx_input_probe(context)},0,"native-utf8-ingress");
 assert_eq!(CONSTRUCTORS.load(Ordering::SeqCst),0);assert_eq!(CONSUMERS.load(Ordering::SeqCst),3);assert_eq!(FREES.load(Ordering::SeqCst),1);
 // Deterministic bounded hostile bytes: compare closed UTF-8 admission to std,
 // reclaim every successful owner, and preserve the sentinel on every refusal.
 let mut seed=0x84351u32;
 for case in 0..512 {
  let mut bytes=[0u8;4];for byte in &mut bytes{seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);*byte=(seed>>24)as u8;}
  let len=case%5;let valid=std::str::from_utf8(&bytes[..len]).is_ok();let mut out=sentinel;
  let status=unsafe{spx_owner_string_from_utf8(context,bytes.as_ptr(),len as u64,&mut out)};
  assert_eq!(status,if valid{0}else{3});
  if valid{assert_eq!(spx_owner_drop(context,out),0);}else{unchanged(out);}
 }
 let mut retained=Vec::new();
 for _ in 0..32{let mut out=sentinel;assert_eq!(unsafe{spx_owner_string_from_utf8(context,b"x".as_ptr(),1,&mut out)},0);retained.push(out);}
 let mut refused=sentinel;assert_eq!(unsafe{spx_owner_string_from_utf8(context,b"x".as_ptr(),1,&mut refused)},4);unchanged(refused);
 assert_eq!(spx_owner_context_close(context),5);
 for owner in retained{assert_eq!(spx_owner_drop(context,owner),0);}
 assert_eq!(spx_owner_context_close(context),0);
 println!("native-utf8-ingress-ok");
}
"#;
