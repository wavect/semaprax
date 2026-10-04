//! Transactional scalar receiver source, graph, and backend parity.
use semaprax::{ast::Type, hir::ResolvedType};

#[test]
fn mutable_closures_keep_distinct_canonical_syntax_and_types() {
    let source = r#"module test.mutable_closures;
@id("mut.update") fn update(state:i64, value:i64)->i64 { state+value }
@id("mut.factory") fn factory(state: i64) -> FnMutI64(i64) -> i64 {
    mut fn(value: i64) -> i64 { update(state, value) }
}
@id("mut.main") fn main() -> i64 { 0 }
"#;
    let ast = semaprax::parse(source, "mutable.spx").unwrap();
    assert_eq!(ast.functions[1].return_type, Type::MutFunctionI64);
    let canonical = semaprax::format::canonical(&ast);
    assert!(canonical.contains("mut fn(value: i64) -> i64"));
    assert!(canonical.contains("FnMutI64(i64) -> i64"));
    assert_eq!(
        canonical,
        semaprax::format::canonical(&semaprax::parse(&canonical, "mutable.spx").unwrap())
    );
    assert!(!Type::MutFunctionI64.is_once_function());
    assert!(!ResolvedType::MutFunctionI64.is_once_function());
    assert!(ResolvedType::MutFunctionI64.is_mut_function());
    assert_ne!(
        ResolvedType::MutFunctionI64.identity_key(),
        ResolvedType::Function {
            parameters: vec![ResolvedType::I64],
            result: Box::new(ResolvedType::I64),
        }
        .identity_key()
    );
    let (checked, resolved) = super::checked(source);
    let facts = resolved
        .declarations
        .type_facts(&ResolvedType::MutFunctionI64)
        .unwrap();
    assert!(!facts.copy && !facts.needs_drop);
    assert!(Type::MutFunctionI64.is_uniquely_owned());
    assert!(ResolvedType::MutFunctionI64.is_uniquely_owned());
    let graph = semaprax::graph::to_json(&checked).unwrap();
    assert!(graph.contains("mutable_function"), "{graph}");
    semaprax::graph::verify_json(&checked, &graph).unwrap();
}

#[test]
fn mutable_closures_refuse_missing_state_update() {
    let source = r#"module test.mutable_closures;
@id("mut.main") fn main() -> i64 {
    let state = 1;
    let callback = mut fn(value: i64) -> i64 { state + value };
    callback(2)
}
"#;
    let errors = semaprax::check(source, "mutable.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-T308"),
        "{errors:?}"
    );
}

#[test]
fn mutable_closures_reject_every_noncanonical_fixed_signature() {
    for signature in [
        "FnMutI64() -> i64",
        "FnMutI64(bool) -> i64",
        "FnMutI64(i64, i64) -> i64",
        "FnMutI64(i64) -> bool",
    ] {
        let source = format!(
            "module test.mutable_closures; @id(\"mut.bad\") fn bad(value: {signature}) -> i64 {{ 0 }}"
        );
        let error = semaprax::parse(&source, "mutable.spx").unwrap_err();
        assert_eq!(error.code, "SPX-T308", "{signature}: {error:?}");
    }
}

#[test]
fn mutable_closures_reject_forged_retained_signature_independently() {
    let source =
        "module test.mutable_closures; @id(\"mut.bad\") fn bad(value: i64) -> i64 { value } @id(\"mut.main\") fn main() -> i64 { 0 }";
    let ast = semaprax::check(source, "mutable.spx").unwrap();
    let mut resolved = semaprax::hir::resolve(&ast).unwrap();
    resolved.functions[0].params[0].ty = ResolvedType::MutFunctionI64;
    let error = semaprax::hir::validate(&resolved).unwrap_err();
    assert_eq!(error.code, "SPX-H006", "{error:?}");
}

const SOURCE: &str = r#"module test.mutable;
@id("mut.update") fn update(state:i64, arg:i64)->i64 { state + arg }
@id("mut.make") fn make(state:i64)->FnMutI64(i64)->i64 ensures true {
    mut fn(arg:i64)->i64 { update(state,arg) }
}
@id("app.main") fn main()->i64 {
    let mut receiver = make(10);
    let first = receiver(2);
    let second = receiver(3);
    let third = receiver(1);
    let mut seed = 20;
    let mut independent = mut fn(arg:i64)->i64 { update(seed,arg) };
    seed = 100;
    let fourth = independent(2);
    first + second + third + fourth
}
"#;

#[test]
fn mutable_closures_execute_same_receiver_and_independent_construction_on_all_backends() {
    use std::process::Command;
    let (program, _) = super::checked(SOURCE);
    let root = std::env::temp_dir().join(format!("spx-mut-parity-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("source.spx");
    std::fs::write(&path, semaprax::format::canonical(&program)).unwrap();
    let result =
        semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
    assert!(
        result.envelope.contains("\"value\":\"65\""),
        "{}",
        result.envelope
    );
    // A callback result remains unique while its factory postcondition runs.
    // Failure must retain the contract status, not try to copy the receiver.
    std::fs::write(&path, SOURCE.replace("ensures true", "ensures false")).unwrap();
    let failed =
        semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
    assert!(
        failed
            .envelope
            .contains("\"domain_id\":\"semaprax.contract.v1\",\"code\":2"),
        "{}",
        failed.envelope
    );
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    assert_eq!(generated, semaprax::codegen::emit_c(&program).unwrap());
    let probe = r#"
int main(void) {
 struct spx_status_entry entries[16]={{0}}; struct spx_context context={0};
 if(!spx_context_init(&context,91,entries,16,NULL,NULL,NULL)) return 1;
 int64_t out=-1;
 if(spx_decl_6170702e6d61696e(&context,&out)!=SPX_STATUS_SUCCESS || out!=65) return 2;
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
    std::fs::write(web.join("probe.mjs"), r#"
import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'));
for(let run=0;run<2;run++){const value=instance.exports.semaprax_main();if(value!==65n)throw Error(`mutable result ${value}`);}
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

#[test]
fn mutable_closures_refuse_receiver_and_update_profile_widening() {
    for source in [
        SOURCE.replace("let mut receiver", "let receiver"),
        SOURCE.replace("update(state,arg)", "update(arg,state)"),
        SOURCE.replace("update(state,arg)", "state = update(state,arg); state"),
        SOURCE.replace("update(state,arg)", "update(state+1,arg)"),
        SOURCE.replace(
            "let second = receiver(3);",
            "let mut copied = receiver; let second = copied(3);",
        ),
        SOURCE.replace(
            "let second = receiver(3);",
            "let second = receiver(receiver(3));",
        ),
        SOURCE.replace("fn make(state:i64)", "fn make<T>(state:i64)"),
        SOURCE.replace(
            "fn main()->i64 {",
            "fn main()->i64 requires { let mut r = make(0); r(1) == 1 } {",
        ),
    ] {
        let errors = semaprax::check(&source, "mutable-refusal.spx").unwrap_err();
        assert!(errors.iter().any(|e| e.code == "SPX-T308"), "{errors:?}");
    }
}

#[test]
fn mutable_closures_independently_reject_forged_body_and_receiver() {
    use semaprax::hir::{ResolvedExprKind, ResolvedStatement};
    let (_, baseline) = super::checked(SOURCE);
    for mutation in 0..5 {
        let mut program = baseline.clone();
        if mutation >= 3 {
            let main = program
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "app.main")
                .unwrap();
            let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
                panic!()
            };
            if mutation == 3 {
                let ResolvedStatement::Let { mutable, .. } = &mut statements[0] else {
                    panic!()
                };
                *mutable = false;
            } else {
                let ResolvedStatement::Let { binding, value, .. } = &mut statements[1] else {
                    panic!()
                };
                let ResolvedExprKind::Invoke { callable, .. } = &value.kind else {
                    panic!()
                };
                binding.ty = callable.ty.clone();
                binding.ownership = callable.ownership;
                value.ty = callable.ty.clone();
                value.ownership = callable.ownership;
                value.kind = callable.kind.clone();
            }
        } else {
            let factory = program
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "mut.make")
                .unwrap();
            let ResolvedExprKind::Block { tail, .. } = &mut factory.body.kind else {
                panic!()
            };
            let ResolvedExprKind::Closure {
                captures,
                parameters,
                body,
            } = &mut tail.kind
            else {
                panic!()
            };
            match mutation {
                0 => parameters[0].id = captures[0].binding.id.clone(),
                1 => captures[0].value.ty = ResolvedType::Bool,
                _ => {
                    let ResolvedExprKind::Block { tail, .. } = &mut body.kind else {
                        panic!()
                    };
                    let ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
                        panic!()
                    };
                    args.swap(0, 1);
                }
            }
        }
        assert_eq!(
            semaprax::hir::validate(&program).unwrap_err().code,
            "SPX-H006"
        );
    }
}
