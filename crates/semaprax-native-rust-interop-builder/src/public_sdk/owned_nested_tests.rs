//! A nested, drop-free domain error does not invent another owned cleanup guard.
use super::{prepare_owned_container_native, Scratch, FIXTURE, SOURCE};
use std::{fs, process::Command};
const TYPE: &str = "Result<string, Option<i64>>";

#[test]
fn owned_nested_source_graph_move_and_refusal() {
    let source = SOURCE.replace("@TYPE@", TYPE);
    let checked = semaprax::check(&source, "nested.spx").unwrap();
    let canonical = semaprax::format::canonical(&checked);
    let round = semaprax::check(&canonical, "nested.spx").unwrap();
    let graph = semaprax::graph::to_json(&checked).unwrap();
    assert_eq!(graph, semaprax::graph::to_json(&round).unwrap());
    for expected in [
        "semaprax.graph.v60",
        "initialize_variant",
        "transfer_variant",
        "core.string.drop",
    ] {
        assert!(graph.contains(expected), "missing {expected}");
    }
    let generated = prepare_owned_container_native(&checked, "container.run").unwrap();
    assert!(generated.header.contains("domain_tag,reserved[6]"));
    assert_eq!(
        generated.rust_adapter,
        prepare_owned_container_native(&round, "container.run")
            .unwrap()
            .rust_adapter
    );
    let moved = source.replace("{value}", "{let used=consume_value(value,7);value}");
    assert!(semaprax::check(&moved, "moved.spx")
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-O101"));
    for unsupported in [
        "Option<Result<string, i64>>",
        "Result<string, Option<string>>",
        "Result<string, Option<u8>>",
    ] {
        let errors =
            semaprax::check(&SOURCE.replace("@TYPE@", unsupported), "unsupported.spx").unwrap_err();
        assert!(errors.iter().any(|e| e.code == "SPX-P106"), "{errors:?}");
    }
    let ordinary = format!(
        r#"module ordinary; @id("ordinary.forward") fn forward(value:own {TYPE})->{TYPE} {{value}} @id("ordinary.main") fn main()->i64 {{0}}"#
    );
    assert!(semaprax::check(&ordinary, "ordinary.spx")
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-T223"));
}

#[test]
fn owned_nested_physical_error_tags_cleanup_and_negative_control() {
    let rustc = std::env::var("RUSTC").expect("absolute rustc");
    let clang = std::env::var("CLANG").expect("absolute clang");
    let root = std::env::temp_dir().join(format!("semaprax-owned-nested-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch(root.clone());
    let checked = semaprax::check(&SOURCE.replace("@TYPE@", TYPE), "nested.spx").unwrap();
    let generated = prepare_owned_container_native(&checked, "container.run").unwrap();
    let late = prepare_owned_container_native(&checked, "container.run_late").unwrap();
    let collapsed = generated.rust_adapter.replace(
        "Err(Some(error))=>(1,1,error,None)",
        "Err(Some(_error))=>(1,0,0,None)",
    );
    assert_ne!(collapsed, generated.rust_adapter);
    let fixture=FIXTURE.replace("@OPTION@","false")
        .replace("@MAKE@","if pattern==-1{return Err(None);}if pattern<0 || pattern==i64::MAX{return Err(Some(pattern));}Ok(text(pattern))")
        .replace("@CONSUME@","match value{Ok(value)=>check(value,input),Err(None)=>input==-1,Err(Some(error))=>error==input}")
        .replace("reserved:[1;7]","reserved:[1;6]")
        .replace("invoke(i64::MIN,i64::MIN,1,0,1,&[88,99]);", "invoke(i64::MIN,i64::MIN,1,0,1,&[88,99]);invoke(i64::MAX,i64::MAX,1,0,1,&[88,99]);")
        .replace("let domain=", "invoke(-1,-1,1,0,1,&[88,99]);invoke(-1,3,1,0,0,&[88,99]);let domain=")
        .replace("if !false{for error", "let before=CALLS.load(Ordering::SeqCst);let old=out;for bad in [SpxContainer{domain_tag:2,..wire},SpxContainer{domain_tag:0,error:1,..wire}]{assert_eq!(unsafe{spx_container_consume(context,bad,7,&mut out)},3);assert_eq!(out,old);assert_eq!(CALLS.load(Ordering::SeqCst),before);}let zero=SpxContainer{domain_tag:1,error:0,..wire};assert_eq!(unsafe{spx_container_consume(context,zero,0,&mut out)},0);assert_eq!(out,1);if !false{for error");
    assert!(fixture.contains("domain_tag:2"));
    fs::write(root.join("owner.h"), &generated.header).unwrap();
    for (label, opt, c, rust, is_late, success) in [
        (
            "o0",
            "-O0",
            &generated.c_source,
            &generated.rust_adapter,
            false,
            true,
        ),
        (
            "o2",
            "-O2",
            &generated.c_source,
            &generated.rust_adapter,
            false,
            true,
        ),
        (
            "late",
            "-O2",
            &late.c_source,
            &generated.rust_adapter,
            true,
            true,
        ),
        (
            "collapsed",
            "-O2",
            &generated.c_source,
            &collapsed,
            false,
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
            "{label}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        fs::write(root.join("main.rs"), format!("{rust}\n{fixture}")).unwrap();
        let mut command = Command::new(&rustc);
        command.args(["--edition=2021", "-Dwarnings"]);
        if is_late {
            command.args(["--cfg", "late_argument"]);
        }
        let compiled = command
            .arg(root.join("main.rs"))
            .arg("-C")
            .arg(format!("link-arg={}", root.join("owner.o").display()))
            .arg("-o")
            .arg(root.join(label))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = Command::new(root.join(label)).output().unwrap();
        assert_eq!(
            ran.status.success(),
            success,
            "{label}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        if !success {
            assert!(String::from_utf8_lossy(&ran.stderr).contains("assertion"));
        }
    }
}
