use super::*;
use crate::hir::{self, DeclarationId, ResolvedExprKind, ResolvedType};
const SOURCE: &str = r#"module replay.update;
@id("row") record Row { @id("row.label") label:string,@id("row.left") left:i64,@id("row.right") right:i64 }
@id("update") fn update(rows:own Vec<Row>)->Vec<Row> { vec_update_field<Row>(rows,0usize,4,"left") }
@id("app.main") fn main()->i64 {0}
"#;
#[test]
fn coherent_same_type_field_substitution_fails_source_and_cache_replay() {
    let current_profile = crate::project::PROJECT_SEMANTIC_CACHE_COMPATIBILITY;
    assert_eq!(current_profile, "semaprax.project-checked-module-hir.v12");
    let manifest_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/calculator-project/semaprax.toml"
    ));
    let authored = ProjectManifest::parse(manifest_source)
        .unwrap()
        .to_canonical_toml();
    let context = format!(
        "{}\0{}\0{}\0{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        current_profile,
        authored
    );
    assert_eq!(
        super::super::manifest(&context)
            .unwrap()
            .to_canonical_toml(),
        authored
    );
    let legacy = context.replacen(
        current_profile,
        "semaprax.project-checked-module-hir.v11",
        1,
    );
    assert_ne!(legacy, context);
    let errors = super::super::manifest(&legacy).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-G255");
    assert_eq!(
        errors[0].message,
        "semantic snapshot compiler context is incompatible"
    );
    let ast = crate::check(SOURCE, "update.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let mut retained = hir::resolve(&ast).unwrap();
    replay_source(&canonical, &ast, &retained).unwrap();
    let element = ResolvedType::Nominal {
        declaration: DeclarationId::new("row"),
        arguments: vec![],
    };
    let other = crate::vec_ops::field_update::resolve(&retained.declarations, &element, "right")
        .unwrap()
        .identity()
        .unwrap();
    let position = retained
        .functions
        .iter()
        .position(|f| f.id.as_str() == "update")
        .unwrap();
    let function = &mut retained.functions[position];
    let body = if matches!(function.body.kind, ResolvedExprKind::Block { .. }) {
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            unreachable!()
        };
        &mut **tail
    } else {
        &mut function.body
    };
    let ResolvedExprKind::Call { callee, .. } = &mut body.kind else {
        panic!("call")
    };
    *callee = DeclarationId::new(other);
    // Rebuild every derived proof: refusing only stale cleanup metadata would
    // not demonstrate canonical source authority against a coherent substitution.
    retained.functions[position].loan_plan =
        crate::loan_plan::build_plan(&retained, &retained.functions[position]).unwrap();
    retained.functions[position].cleanup =
        crate::cleanup::build_inventory(&retained, &retained.functions[position]).unwrap();
    retained.functions[position].cleanup_plan =
        crate::cleanup_plan::build_plan(&retained, &retained.functions[position]).unwrap();
    hir::validate(&retained).unwrap();
    assert!(replay_source(&canonical, &ast, &retained).is_err());
    let wire = crate::cache_codec::encode(&retained).unwrap();
    let restored: hir::ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    assert!(replay_source(&canonical, &ast, &restored).is_err());
    let graph = crate::graph::to_json(&ast).unwrap();
    assert!(graph.contains(crate::vec_ops::field_update::PREFIX));
}
