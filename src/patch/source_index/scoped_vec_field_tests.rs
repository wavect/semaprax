use super::*;

#[test]
fn direct_and_fused_static_selectors_have_exact_member_rename_spans() {
    let source = r#"module scoped.index;
@id("row") record Row {@id("row.title") title:string,@id("row.marker") marker:i64,}
@id("inspect") fn inspect(values:borrow Vec<Row>,index:usize)->i64 {
 let view=str_as_bytes(vec_field<Row>(values,index,"title"));
 vec_field<Row>(values,index,"marker")+i64_from_usize(byte_len(view))
}
@id("app.main") fn main()->i64 {0}
"#;
    let parsed = crate::check(source, "scoped-index.spx").unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    let tokens = crate::lexer::lex(source, "scoped-index.spx").unwrap();
    let index = SemanticSourceIndex::build(&parsed, &resolved, &tokens).unwrap();
    for (field, name) in [("row.title", "title"), ("row.marker", "marker")] {
        let sites = &index.members[&("row".to_owned(), field.to_owned())];
        assert_eq!(sites.len(), 2);
        assert!(
            sites
                .iter()
                .all(|site| source.get(site.span.start..site.span.end) == Some(name))
        );
        assert!(sites.iter().all(|site| site.shorthand_binding.is_none()));
    }
}
