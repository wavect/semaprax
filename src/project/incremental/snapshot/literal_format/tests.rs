//! Source authority and canonical transfer replay for additive HIR tag 33.
use super::*;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedProgram};

const SOURCE: &str = r#"module format.cache;
@id("format.render") fn render(a:string,b:string)->string {string_format("{}:{}",a,b)}
@id("format.main") fn main()->i64 {string_len(render("left","right"))}
"#;

fn fixture() -> (String, Program, ResolvedProgram) {
    let ast = crate::check(SOURCE, "src/app.spx").unwrap();
    let source = crate::format::canonical(&ast);
    let ast = crate::check(&source, "src/app.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    (source, ast, program)
}
fn operation(program: &mut ResolvedProgram) -> &mut ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "format.render")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("block")
    };
    assert!(matches!(tail.kind, ResolvedExprKind::LiteralFormat { .. }));
    tail
}
fn round_trip(program: &ResolvedProgram) -> ResolvedProgram {
    let bytes = crate::cache_codec::encode(program).unwrap();
    let decoded = crate::cache_codec::decode(&bytes).unwrap();
    assert_eq!(crate::cache_codec::encode(&decoded).unwrap(), bytes);
    decoded
}
#[test]
fn literal_format_cache_tag_and_raw_template_are_bound_to_canonical_source() {
    let (source, ast, program) = fixture();
    let mut decoded = round_trip(&program);
    let wire = crate::cache_codec::encode(&operation(&mut decoded).kind).unwrap();
    assert_eq!(&wire[..2], &33u16.to_le_bytes());
    replay_source(&source, &ast, &decoded).unwrap();
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    let ResolvedExprKind::LiteralFormat { template, .. } = &mut operation(&mut decoded).kind else {
        unreachable!()
    };
    *template = "{}|{}".into();
    // This is a valid different HIR program, but has no authority for old source.
    crate::hir::validate(&decoded).unwrap();
    let wire = crate::cache_codec::encode(&decoded).unwrap();
    assert_eq!(
        replay_source(&source, &ast, &decoded).unwrap_err()[0].code,
        "SPX-G255"
    );
    assert_eq!(crate::cache_codec::encode(&decoded).unwrap(), wire);
    let changed = crate::check(&source.replace("{}:{}", "{}|{}"), "src/app.spx").unwrap();
    assert!(crate::graph::verify_json(&changed, &graph).is_err());
    let mut extra = program.clone();
    extra.functions.push(program.functions[0].clone());
    assert_eq!(
        replay_source(&source, &ast, &extra).unwrap_err()[0].code,
        "SPX-G255"
    );
}
#[test]
fn literal_format_cache_shape_and_owned_group_commit_are_independently_replayed() {
    let (_, _, program) = fixture();
    for mutation in 0..6 {
        let mut forged = round_trip(&program);
        if mutation < 2 {
            let ResolvedExprKind::LiteralFormat { template, args } =
                &mut operation(&mut forged).kind
            else {
                unreachable!()
            };
            if mutation == 0 {
                *template = "{}".into();
            } else {
                args[0].ownership = OwnershipMode::Borrow;
            }
        } else {
            let function = forged
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "format.render")
                .unwrap();
            let arguments =
                function
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .flat_map(|b| &mut b.transitions)
                    .find_map(|t| match t {
                        crate::cleanup_plan::CleanupTransition::CallCommit {
                            arguments, ..
                        } if arguments.len() == 2 => Some(arguments),
                        _ => None,
                    })
                    .unwrap();
            match mutation {
                2 => arguments.swap(0, 1),
                3 => {
                    arguments.remove(0);
                }
                4 => arguments[1] = arguments[0].clone(),
                5 => arguments[0].parameter_index = 1,
                _ => unreachable!(),
            }
        }
        let bytes = crate::cache_codec::encode(&forged).unwrap();
        assert!(
            crate::hir::validate(&forged).is_err(),
            "mutation {mutation}"
        );
        if mutation >= 2 {
            assert!(crate::cleanup_plan::validate_program(&forged).is_err());
        }
        assert_eq!(crate::cache_codec::encode(&forged).unwrap(), bytes);
    }
}

fn snapshot() -> Vec<u8> {
    let manifest = ProjectManifest::parse(
        r#"schema = "semaprax.manifest.v1"

[package]
name = "format-cache"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "format.cache"
sources = ["src/app.spx", "src/provider.spx", "src/tests.spx"]
tests = ["format.tests"]

[exports]
web = []
"#,
    )
    .unwrap();
    let source=SOURCE.replace("module format.cache;", "module format.cache; use function @id(\"format.imported\") from format.provider as provided;")
        .replace("string_len(render(\"left\",\"right\"))", "string_len(string_format(\"{}{}\",render(\"left\",\"right\"),provided()))");
    let source = crate::format::canonical(&crate::parse(&source, "src/app.spx").unwrap());
    let tests = crate::check(
        "module format.tests; @id(\"tests.main\") fn main()->i64 {0}",
        "src/tests.spx",
    )
    .unwrap();
    let provider = crate::check(
        "module format.provider; @id(\"format.imported\") fn provided()->i64 {7}",
        "src/provider.spx",
    )
    .unwrap();
    let sources = [
        ProjectFrontendSource::new("src/app.spx", &source).unwrap(),
        ProjectFrontendSource::new("src/provider.spx", &crate::format::canonical(&provider))
            .unwrap(),
        ProjectFrontendSource::new("src/tests.spx", &crate::format::canonical(&tests)).unwrap(),
    ];
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    cache.build(&manifest, &sources).unwrap();
    super::super::encode_snapshot(&cache).unwrap()
}
#[test]
fn literal_format_snapshot_rejects_template_source_and_synthetic_drift() {
    let bytes = snapshot();
    let clean = super::super::decode_snapshot(&bytes).unwrap();
    assert!(clean.restored_work().is_some());
    for mutation in 0..3 {
        let mut snapshot: super::super::Snapshot = crate::cache_codec::decode(&bytes).unwrap();
        let entry = snapshot
            .entries
            .iter_mut()
            .find(|e| e.path == "src/app.spx")
            .unwrap();
        if mutation == 0 {
            let ResolvedExprKind::LiteralFormat { template, .. } =
                &mut operation(&mut entry.resolved).kind
            else {
                unreachable!()
            };
            *template = "{}|{}".into();
        } else if mutation == 1 {
            entry.source = entry.source.replace("{}:{}", "{}|{}");
        } else {
            // Consistently forged AST + HIR must still lose against unchanged
            // canonical authored source during ordinary Project reconstruction.
            let function = entry
                .synthetic
                .functions
                .iter_mut()
                .find(|f| f.stable_id == "format.render")
                .unwrap();
            let crate::ast::ExprKind::Block { tail, .. } = &mut function.body.kind else {
                panic!("block")
            };
            let crate::ast::ExprKind::Call { args, .. } = &mut tail.kind else {
                panic!("format call")
            };
            args[0].kind = crate::ast::ExprKind::String("{}|{}".into());
            entry.resolved = crate::hir::resolve(&entry.synthetic).unwrap();
        }
        let hostile = crate::cache_codec::encode(&snapshot).unwrap();
        assert!(
            super::super::decode_snapshot(&hostile).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(crate::cache_codec::encode(&snapshot).unwrap(), hostile);
    }
}
