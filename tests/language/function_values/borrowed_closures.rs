//! Synchronous borrowed captures keep the source parameter loan in its frame.
use semaprax::hir::{OwnershipMode, ResolvedExprKind, ResolvedStatement, ResolvedType};

const SOURCE: &str = r#"module test.borrowed_closures;
@id("borrow.packet") record Packet { @id("borrow.value") value:i64, }
@id("borrow.compute") fn compute(view:borrow str,arg:i64)->i64 {
    let bytes=str_as_bytes(view);
    match byte_get(bytes,0usize) {
        Option::Some { value:first } => if first==104u8 { str_len_bytes(view)+arg } else { -999 },
        Option::None {} => -999,
    }
}
@id("borrow.run") fn run(view:borrow str,arg:i64)->i64 {
    let callback=fn(value:i64)->i64 { compute(view,value) };
    callback(arg)+callback(0)
}
@id("app.main") fn main()->i64 {
    let text="hé";
    let view=string_as_str(text);
    let packet=Packet { value:run(view,36) };
    packet.value
}
"#;

#[test]
fn borrowed_closures_preserve_parameter_capture_in_canonical_graph() {
    let (ast, hir) = super::checked(SOURCE);
    let run = hir
        .functions
        .iter()
        .find(|f| f.id.as_str() == "borrow.run")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &run.body.kind else {
        panic!()
    };
    let ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!()
    };
    let ResolvedExprKind::Closure { captures, .. } = &value.kind else {
        panic!()
    };
    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].binding.ty, ResolvedType::Str);
    assert_eq!(captures[0].binding.ownership, OwnershipMode::Borrow);
    assert!(
        matches!(&captures[0].value.kind,ResolvedExprKind::Place(p) if p.root==run.params[0].id)
    );
    let graph = semaprax::graph::to_json(&ast).unwrap();
    semaprax::graph::verify_json(&ast, &graph).unwrap();
}

#[test]
fn borrowed_closures_refuse_escape_alias_and_nonparameter_capture() {
    for source in [
        SOURCE.replace(
            "callback(arg)+callback(0)",
            "let copied=callback; copied(arg)",
        ),
        SOURCE.replace("let callback=", "let mut callback="),
        SOURCE.replace("callback(arg)+callback(0)", "callback"),
        SOURCE.replace("fn run(view:borrow str,arg:i64)->i64 {", "fn run(view:borrow str,arg:i64)->i64 requires { let c=fn(value:i64)->i64 { compute(view,value) }; c(arg)>0 } {"),
        SOURCE.replace("compute(view,value)", "compute(view,value+1)"),
        SOURCE.replace(
            "fn run(view:borrow str,arg:i64)->i64",
            "fn run<T>(view:borrow str,arg:i64)->i64",
        ),
        SOURCE
            .replace(
                "let callback=",
                "let shadow=string_as_str(\"local\"); let callback=",
            )
            .replace("compute(view,value)", "compute(shadow,value)"),
    ] {
        let errors = semaprax::check(&source, "borrowed-refusal.spx").unwrap_err();
        assert!(errors.iter().any(|e| e.code == "SPX-T288"), "{errors:?}");
    }
}

#[test]
fn borrowed_closures_preserve_the_callers_owner_loan() {
    let source = SOURCE.replace(
        "let view=string_as_str(text);",
        "let view=string_as_str(text); let moved=text;",
    );
    let errors = semaprax::check(&source, "borrowed-owner-move.spx").unwrap_err();
    assert!(errors.iter().any(|e| e.code == "SPX-T265"), "{errors:?}");
}

#[test]
fn borrowed_closures_reject_forged_capture_mode_mutability_and_root() {
    let (_, baseline) = super::checked(SOURCE);
    for mutation in 0..3 {
        let mut hir = baseline.clone();
        let run = hir
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "borrow.run")
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &mut run.body.kind else {
            panic!()
        };
        let ResolvedStatement::Let { mutable, value, .. } = &mut statements[0] else {
            panic!()
        };
        let ResolvedExprKind::Closure { captures, .. } = &mut value.kind else {
            panic!()
        };
        match mutation {
            0 => captures[0].binding.ownership = OwnershipMode::Value,
            1 => *mutable = true,
            _ => {
                let ResolvedExprKind::Place(p) = &mut captures[0].value.kind else {
                    panic!()
                };
                p.root = run.params[1].id.clone();
            }
        }
        assert!(semaprax::hir::validate(&hir).is_err());
    }
}

#[test]
fn borrowed_closures_execute_on_interpreter_native_and_core_wasm() {
    use std::process::Command;
    let (program, _) = super::checked(SOURCE);
    let root = std::env::temp_dir().join(format!("spx-borrow-closure-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("source.spx");
    std::fs::write(&path, SOURCE).unwrap();
    let result =
        semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
    assert!(
        result.envelope.contains("\"value\":\"42\""),
        "{}",
        result.envelope
    );
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let probe = r#"
int main(void) {
 struct spx_status_entry entries[16]={{0}}; struct spx_context context={0};
 if(!spx_context_init(&context,91,entries,16,NULL,NULL,NULL)) return 1;
 int64_t out=-1;
 if(spx_decl_6170702e6d61696e(&context,&out)!=SPX_STATUS_SUCCESS || out!=42) return 2;
 return 0;
}
"#;
    std::fs::write(root.join("module.c"), format!("{generated}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let output = Command::new("clang")
            .current_dir(&root)
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
                "module.c",
                "-o",
                "native",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(Command::new(root.join("native"))
            .status()
            .unwrap()
            .success());
    }
    let web = root.join("web");
    semaprax::wasm::build_web(&program, &web).unwrap();
    std::fs::write(web.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(web.join("probe.mjs"),r#"
import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'));
for(let run=0;run<2;run++){const value=instance.exports.semaprax_main();if(value!==42n)throw Error(`borrowed result ${value}`);}
"#).unwrap();
    let output = Command::new("node")
        .current_dir(&web)
        .arg("probe.mjs")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}
