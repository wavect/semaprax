use super::*;
use std::{fs, process::Command};

#[test]
fn demanded_serde_wire_rejects_invalid_values_drops_partial_owners_and_reports_copies() {
    let source = r#"module ri07.wire;
@id("ri07.wire.record") record Projected {
 @id("ri07.wire.label") label: string,
 @id("ri07.wire.bytes") payload: string,
 @id("ri07.wire.enabled") enabled: bool,
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = semaprax::check(source, "serde-wire.spx").unwrap();
    let resolved = semaprax::hir::resolve(&program).unwrap();
    let generated = prepare_serde_record_projection(&resolved, "ri07.wire.record").unwrap();
    let root = std::env::temp_dir().join(format!("semaprax-ri07-wire-{}", std::process::id()));
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Temp(root.clone());
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname=\"ri07-wire-control\"\nversion=\"0.0.1\"\nedition=\"2021\"\n[dependencies]\nserde={version=\"=1.0.229\",features=[\"derive\"]}\nserde_json=\"=1.0.151\"\n").unwrap();
    let consumer = r#"
use std::alloc::{GlobalAlloc,Layout,System};
use std::sync::atomic::{AtomicUsize,Ordering::SeqCst};
struct Counting;
static FIRST:AtomicUsize=AtomicUsize::new(0);static SECOND:AtomicUsize=AtomicUsize::new(0);
static FIRST_DROPS:AtomicUsize=AtomicUsize::new(0);static SECOND_DROPS:AtomicUsize=AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
 unsafe fn alloc(&self,l:Layout)->*mut u8{unsafe{System.alloc(l)}}
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){if p as usize==FIRST.load(SeqCst){FIRST_DROPS.fetch_add(1,SeqCst);}if p as usize==SECOND.load(SeqCst){SECOND_DROPS.fetch_add(1,SeqCst);}unsafe{System.dealloc(p,l)}}
}
#[global_allocator]static ALLOC:Counting=Counting;
fn wire(label:Vec<u8>,enabled:u8)->SpxMirrorri07wirerecordWire{
 let payload=vec![91;257];FIRST.store(label.as_ptr() as usize,SeqCst);SECOND.store(payload.as_ptr() as usize,SeqCst);FIRST_DROPS.store(0,SeqCst);SECOND_DROPS.store(0,SeqCst);
 SpxMirrorri07wirerecordWire{label,payload,enabled}
}
fn counts(first:usize,second:usize){assert_eq!(FIRST_DROPS.load(SeqCst),first);assert_eq!(SECOND_DROPS.load(SeqCst),second);}
fn main(){
 let value=wire(vec![b'a';129],1);let pointer=value.label.as_ptr();let record=Projected::try_from(value).unwrap();counts(0,0);assert_eq!(record.label.as_ptr(),pointer);drop(record);counts(1,1);
 let value=wire(vec![b'a';129],2);let mut published:Option<Projected>=None;let error=Projected::try_from(value);if let Ok(record)=error{published=Some(record);}assert!(published.is_none(),"partial-owner-published");counts(1,1);
 let value=wire(vec![255;129],1);assert_eq!(Projected::try_from(value).unwrap_err(),SpxMirrorri07wirerecordConversionError::InvalidUtf8("label"));counts(1,1);
 FIRST.store(0,SeqCst);SECOND.store(0,SeqCst);
 let record=Projected{label:String::from("lambda λ"),payload:String::from("abc"),enabled:true};
 let began=std::time::Instant::now();let mut copied=0usize;
 for _ in 0..1000 {let (wire,n)=record.to_owned_wire();copied+=n;assert_eq!(Projected::try_from(wire).unwrap(),record);}
 assert_eq!(copied,(record.label.len()+record.payload.len())*1000);
 println!("ri07-wire-conversion iterations=1000 explicit_payload_copy_bytes={copied} elapsed_ns={}",began.elapsed().as_nanos());
}
"#;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let run = |source: String| {
        fs::write(root.join("src/main.rs"), source).unwrap();
        Command::new(&cargo)
            .args(["run", "--offline", "--quiet", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .output()
            .unwrap()
    };
    let success = run(format!("{}\n{consumer}", generated.rust_source));
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    let stdout = String::from_utf8(success.stdout).unwrap();
    assert!(
        stdout.contains("explicit_payload_copy_bytes=12000"),
        "{stdout}"
    );
    eprint!("{stdout}");
    let needle = "_=>return Err(SpxMirrorri07wirerecordConversionError::InvalidBool(\"enabled\"))";
    assert!(generated.rust_source.contains(needle));
    let mutant = generated.rust_source.replace(needle, "_=>true");
    let negative = run(format!("{mutant}\n{consumer}"));
    assert!(!negative.status.success());
    assert!(String::from_utf8_lossy(&negative.stderr).contains("partial-owner-published"));
}
