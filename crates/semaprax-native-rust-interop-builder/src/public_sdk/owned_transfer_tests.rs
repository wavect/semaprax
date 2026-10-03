use super::*;
use semaprax::cleanup_plan::CleanupTransition;

const PAIR: &str = r#"module pair.fixture;
@id("pair.host") interface Host permits { } {
 @id("pair.new") import rust fn make_value(pattern:i64) -> @TYPE@ from "fixture::make" effects { } failure infallible;
 @id("pair.consume") import rust fn consume_value(value:own @TYPE@,input:i64) -> bool from "fixture::consume" effects { } failure infallible;
}
@id("pair.both") fn both(left:own @TYPE@,right:own @TYPE@,expected:i64)->bool {
 let left_ok=consume_value(left,expected);
 let right_ok=consume_value(right,8);
 if left_ok {right_ok} else {false}
}
@id("pair.run") fn run(first:i64,second:i64,divisor:i64)->i64 {
 if both(make_value(first),make_value(second),7/divisor) {1} else {0}
}
@id("pair.main") fn main()->i64 {0}
"#;

#[test]
fn owned_transfer_two_arguments_staging_atomicity_and_failures() {
    let rustc = std::env::var("RUSTC").expect("absolute rustc");
    let clang = std::env::var("CLANG").expect("absolute clang");
    let root = std::env::temp_dir().join(format!("semaprax-owner-pair-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch(root.clone());
    for (ty, option) in [("Option<string>", true), ("Result<string, i64>", false)] {
        let source = PAIR.replace("@TYPE@", ty);
        let checked = semaprax::check(&source, "pair.spx").unwrap();
        let canonical = semaprax::format::canonical(&checked);
        let round = semaprax::check(&canonical, "pair.spx").unwrap();
        assert_eq!(
            semaprax::graph::to_json(&checked).unwrap(),
            semaprax::graph::to_json(&round).unwrap()
        );
        let program = semaprax::hir::resolve(&checked).unwrap();
        let entry = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "pair.run")
            .unwrap();
        let arguments = entry
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|b| &b.transitions)
            .find_map(|t| match t {
                CleanupTransition::CallCommit { arguments, .. } if arguments.len() == 2 => {
                    Some(arguments)
                }
                _ => None,
            })
            .expect("checked helper must transfer two arguments together");
        let slots = arguments
            .iter()
            .map(|argument| {
                entry
                    .cleanup_plan
                    .slots
                    .iter()
                    .position(|s| s.storage == argument.source.storage)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_ne!(slots[0], slots[1]);
        let generated = prepare_owned_container_native(&checked, "pair.run").unwrap();
        let check = |slot| {
            format!("if(!f->live[{slot}]) return 7;\nif(spx_container_validate(context,f->owners[{slot}])) return 7;\n")
        };
        let first = check(slots[0]);
        let second = check(slots[1]);
        let commit = format!(
            "{first}{second}f->live[{}]=0;\nf->live[{}]=0;\n",
            slots[0], slots[1]
        );
        assert_eq!(
            generated.c_source.matches(&commit).count(),
            1,
            "one authenticated pair commit"
        );
        let refusal = |ordinal| {
            format!("if(spx_test_refuse()=={ordinal}){{spx_test_record(f->owners[{}],f->owners[{}],f->live[{}],f->live[{}]);return 7;}}\n",slots[0],slots[1],slots[0],slots[1])
        };
        let observed = format!(
            "{first}{}{second}{}f->live[{}]=0;\nf->live[{}]=0;\n",
            refusal(1),
            refusal(2),
            slots[0],
            slots[1]
        );
        let c = generated.c_source.replace(&commit, &observed);
        // This control violates the declared all-at-once boundary by clearing
        // the first source before validation of the second source completes.
        let non_atomic = c.replace(
            &observed,
            &format!(
                "{first}{}f->live[{}]=0;\n{second}{}f->live[{}]=0;\n",
                refusal(1),
                slots[0],
                refusal(2),
                slots[1]
            ),
        );
        assert_ne!(c, non_atomic);
        fs::write(root.join("owner.h"),format!("{}\nuint32_t spx_test_refuse(void);\nvoid spx_test_record(spx_container,spx_container,uint8_t,uint8_t);\n",generated.header)).unwrap();
        let reserve = generated.rust_adapter.replace("let produced=constructor(arg);","if (cfg!(reserve_first)&&arg==7)||(cfg!(reserve_second)&&arg==8){return Err(4);} let produced=constructor(arg);");
        assert_ne!(reserve, generated.rust_adapter);
        let fixture = format!("{}\n{MAIN}", FIXTURE.split_once("fn main(){").unwrap().0)
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
        for (label, opt, c_source, adapter, cfg, success) in [
            ("o0", "-O0", &c, &generated.rust_adapter, None, true),
            ("o2", "-O2", &c, &generated.rust_adapter, None, true),
            (
                "first-reserve",
                "-O2",
                &c,
                &reserve,
                Some("reserve_first"),
                true,
            ),
            (
                "second-reserve",
                "-O2",
                &c,
                &reserve,
                Some("reserve_second"),
                true,
            ),
            (
                "non-atomic",
                "-O2",
                &non_atomic,
                &generated.rust_adapter,
                None,
                false,
            ),
        ] {
            fs::write(root.join("owner.c"), c_source).unwrap();
            let output = Command::new(&clang)
                .args(["-std=c11", "-Wall", "-Wextra", "-Werror", opt, "-c"])
                .arg(root.join("owner.c"))
                .arg("-o")
                .arg(root.join("owner.o"))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{ty}/{label}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            fs::write(root.join("main.rs"), format!("{adapter}\n{fixture}")).unwrap();
            let mut command = Command::new(&rustc);
            command.args(["--edition=2021", "-Dwarnings"]);
            if let Some(cfg) = cfg {
                command.args(["--cfg", cfg]);
            }
            let executable = root.join(label);
            let output = command
                .arg(root.join("main.rs"))
                .arg("-C")
                .arg(format!("link-arg={}", root.join("owner.o").display()))
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{ty}/{label}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let output = Command::new(executable).output().unwrap();
            assert_eq!(
                output.status.success(),
                success,
                "{ty}/{label}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            if !success {
                assert!(String::from_utf8_lossy(&output.stderr).contains("atomic-live-bits"));
            }
        }
    }
}

const MAIN: &str = r#"
static REFUSE:AtomicUsize=AtomicUsize::new(0);
static LIVE_BITS:AtomicUsize=AtomicUsize::new(0);
static OBSERVED:std::sync::Mutex<Option<(SpxContainer,SpxContainer)>>=std::sync::Mutex::new(None);
#[no_mangle]pub extern "C" fn spx_test_refuse()->u32{REFUSE.load(Ordering::SeqCst) as u32}
#[no_mangle]pub extern "C" fn spx_test_record(first:SpxContainer,second:SpxContainer,a:u8,b:u8){
 LIVE_BITS.store(usize::from(a)+2*usize::from(b),Ordering::SeqCst);*OBSERVED.lock().unwrap()=Some((first,second));
}
fn main(){
 if cfg!(reserve_first){invoke(7,8,1,4,1234567,&[]);assert_eq!(CALLS.load(Ordering::SeqCst),0);return;}
 if cfg!(reserve_second){invoke(7,8,1,4,1234567,&[7]);assert_eq!(CALLS.load(Ordering::SeqCst),0);return;}
 invoke(7,8,1,0,1,&[7,8]);invoke(-1,-2,1,0,0,&[]);
 CALLS.store(0,Ordering::SeqCst);
 invoke(-999,8,1,2,1234567,&[]);invoke(7,-999,1,2,1234567,&[7]);
 invoke(7,4097,1,4,1234567,&[4097,7]);invoke(7,8,0,8,1234567,&[8,7]);
 assert_eq!(CALLS.load(Ordering::SeqCst),0);
 for ordinal in [1,2]{
  TRACE_LEN.store(0,Ordering::SeqCst);REFUSE.store(ordinal,Ordering::SeqCst);
  let context=spx_container_context_new();let mut output=1234567;
  assert_eq!(unsafe{spx_container_entry(context,7,8,1,&mut output)},7);assert_eq!(output,1234567);
  assert_eq!(CALLS.load(Ordering::SeqCst),0);trace(&[]);
  assert_eq!(LIVE_BITS.load(Ordering::SeqCst),3,"atomic-live-bits");
  // Defensive invariant rejection has no authenticated cleanup-plan edge yet.
  // The table retains both owners and refuses context closure. This observer
  // releases its captured test tokens explicitly; it is not Semaprax cleanup.
  assert_eq!(spx_container_context_close(context),5);
  let(first,second)=OBSERVED.lock().unwrap().take().unwrap();
  assert_eq!(spx_container_validate(context,first),0);assert_eq!(spx_container_validate(context,second),0);
  assert_eq!(spx_container_drop(context,first),0);assert_eq!(spx_container_drop(context,second),0);
  trace(&[7,8]);assert_eq!(spx_container_context_close(context),0);
 }
}
"#;
