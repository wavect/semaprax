//! Ordinary functional Bytes writer. No new dependencies or generated-source authority.
use super::*;

pub(in crate::project::json_codec) fn source(
    root: &TypeDeclaration,
    profile: &str,
    namespace: &str,
) -> String {
    super::super::template::expand(
        include_str!("writer.spx"),
        &[
            ("__PREFIX__", &format!("json_{}_{}", root.name, namespace)),
            ("__ID__", &format!("{}.json.{profile}", root.stable_id)),
        ],
    )
}

/// Keys and punctuation are compile-time UTF-8 bytes, never temporary Strings.
pub(in crate::project::json_codec) fn literal_write(
    prefix: &str,
    output: &str,
    position: &str,
    value: &str,
    site: usize,
) -> String {
    if value.len() == 1 {
        return format!(
            "{output}=bytes_set({output},{position},{}u8);{position}={position}+1usize;\n",
            value.as_bytes()[0]
        );
    }
    let bytes = value
        .bytes()
        .map(|byte| format!("{byte}u8"))
        .collect::<Vec<_>>()
        .join(",");
    // A fixed Copy array is ordinary stack data, not an owned String/Bytes
    // payload. Its borrow remains inside this statement's checked call.
    // Discard the Copy block result with an ordinary immutable let. Bare
    // expression statements are closed and discard bindings use distinct sites.
    // Keep renewal an exact direct Call; a Block-valued RHS consumes the owner
    // without carrying that ordinary renewal authority.
    format!("let k{site}={{let k=[{bytes}];{output}={prefix}_writer_bytes(array_as_slice(k),{position},{output});0}};{position}={position}+{}usize;\n",value.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn functional_writer_is_ordinary_deterministic_and_minimum_safe() {
        let program = crate::parse(
            "module schema;@id(\"response.row\") record Row{@id(\"response.row.x\")x:i64,}@id(\"schema.main\") fn main()->i64{0}",
            "schema.spx",
        )
        .unwrap();
        let output = source(&program.types[0], "collection-response", "response");
        assert_eq!(
            output,
            source(&program.types[0], "collection-response", "response")
        );
        assert!(!output.contains("string_concat"));
        assert!(!output.contains("string_from_"));
        assert!(!output.contains("bytes_zeroed"));
        assert!(!output.contains("use "));
        assert!(output.contains("if value>0{0-value}else{value}"));
        assert!(output.contains("usize_from_i64(0-remaining%10)"));
        assert!(output.contains("position<=byte_len(bytes_as_slice(output)) &&"));
        assert!(output.contains("next=bytes_set(next,cursor,byte)"));
        let parsed = crate::parse(&format!("module schema;\n{output}"), "writer.spx").unwrap();
        assert_eq!(parsed.functions.len(), 7);
        assert!(parsed
            .functions
            .iter()
            .all(|function| function.effects.is_empty()));
        assert!(parsed.functions.iter().all(|function| function
            .stable_id
            .starts_with("response.row.json.collection-response.writer.")));
    }
    #[test]
    fn scoped_literal_writes_admit_exact_helpers_inside_and_outside_loops() {
        let schema = crate::parse(
            "module schema;@id(\"response.row\") record Row{@id(\"response.row.x\")x:i64,}@id(\"schema.main\") fn main()->i64{0}",
            "schema.spx",
        ).unwrap();
        let helpers = source(&schema.types[0], "collection-response", "response");
        let bytes_helper = helpers
            .split("@id(\"response.row.json.collection-response.writer.digit\")")
            .next()
            .unwrap();
        let first = literal_write("json_Row_response", "output", "cursor", "\"first\":", 0);
        let helper_first = literal_write("json_Row_response", "next", "cursor", "\"first\":", 0);
        let helper_second = literal_write("json_Row_response", "next", "cursor", ",\"second\":", 0);
        let text = format!(
            "module writer;\n{bytes_helper}\n
@id(\"writer.first\") fn first(position:usize,output:own Bytes)->Bytes{{
let mut next=output;let mut cursor=position;{helper_first}next}}
@id(\"writer.second\") fn second(position:usize,output:own Bytes)->Bytes{{
let mut next=output;let mut cursor=position;{helper_second}next}}
@id(\"writer.main\") fn main()->i64{{
let mut output=bytes_zeroed(128usize);let mut cursor=0usize;
{first}let mut at=0usize;
while at<2usize{{output=first(cursor,output);cursor=cursor+8usize;output=second(cursor,output);cursor=cursor+10usize;at=at+1usize;0}}
if cursor==44usize{{1}}else{{0}}}}\n"
        );
        // Actual object-render helpers contain their keys outside runtime loops;
        // row loops call these ordinary acyclic helpers. Inlining an array
        // literal into a loop remains closed by the existing language contract.
        let loop_literal = text.replace(
            "output=first(cursor,output);cursor=cursor+8usize;",
            &first.replace("let k0=", "let k1="),
        );
        assert!(crate::check(&loop_literal, "loop-array-writer.spx")
            .unwrap_err()
            .iter()
            .any(|e| e.code == "SPX-T252"));
        let checked = crate::check(&text, "scoped-writer.spx").unwrap();
        let canonical = crate::format::canonical(&checked);
        let checked_again = crate::check(&canonical, "scoped-writer-canonical.spx").unwrap();
        assert_eq!(canonical, crate::format::canonical(&checked_again));
        let helper = checked_again
            .functions
            .iter()
            .find(|f| f.name == "json_Row_response_writer_bytes")
            .unwrap();
        assert!(crate::loop_calls::byte_writer::source_target(
            &checked_again,
            helper
        ));
        let resolved = crate::hir::resolve(&checked_again).unwrap();
        crate::hir::validate(&resolved).unwrap();
        crate::cleanup_plan::validate_program(&resolved).unwrap();
        let main = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "writer.main")
            .unwrap();
        assert_eq!(
            main.cleanup_plan.schema,
            crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
        );
        assert_eq!(
            crate::loop_calls::byte_writer::checked_bindings(&resolved, main)
                .unwrap()
                .len(),
            3
        );
        let graph = crate::graph::to_json(&checked_again).unwrap();
        crate::graph::verify_json(&checked_again, &graph).unwrap();
        let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
        assert_eq!(document["schema"], "semaprax.graph.v76");
        assert_eq!(
            document["byte_helper_renewal"]["schema"],
            "semaprax.byte-helper-renewal.v1"
        );
        let encoded = crate::cache_codec::encode(&main.cleanup_plan).unwrap();
        let mut cached = resolved.clone();
        cached
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "writer.main")
            .unwrap()
            .cleanup_plan = crate::cache_codec::decode(&encoded).unwrap();
        crate::hir::validate(&cached).unwrap();
        assert_eq!(
            crate::cache_codec::encode(
                &cached
                    .functions
                    .iter()
                    .find(|f| f.id.as_str() == "writer.main")
                    .unwrap()
                    .cleanup_plan
            )
            .unwrap(),
            encoded
        );
        // Since 043faab51 (SCALAR-EXPRESSION-STATEMENTS-V1) an intermediate
        // `expr;` discards exactly one Copy scalar. This block's value is the
        // i64 `0`, so the bare spelling is no longer SPX-P106: the parser mints
        // an immutable unannotated ExpressionStatement let around the block.
        let bare = text.replacen("let k0={let k=", "{let k=", 1);
        let bare = crate::parse(&bare, "bare-block-writer.spx").unwrap();
        let bare_first = bare.functions.iter().find(|f| f.name == "first").unwrap();
        let crate::ast::ExprKind::Block { statements, .. } = &bare_first.body.kind else {
            panic!("writer helper body is a block")
        };
        assert!(statements.iter().any(|statement| matches!(
            statement,
            crate::ast::Statement::Let {
                syntax: crate::ast::LetSyntax::ExpressionStatement,
                mutable: false,
                declared: None,
                value,
                ..
            } if matches!(value.kind, crate::ast::ExprKind::Block { .. })
        )));
        // Arbitrary Block-valued ownership assignment still has no direct-call
        // authority; correcting the generator must not broaden that boundary.
        let old_rhs = first
            .replace("let k0={let k=", "output={let k=")
            .replace(
                ";output=json_Row_response_writer_bytes",
                ";json_Row_response_writer_bytes",
            )
            .replace(";0};", "};");
        let hostile = text.replacen(&first, &old_rhs, 1);
        let errors = crate::check(&hostile, "block-rhs-writer.spx").unwrap_err();
        assert!(errors.iter().any(|e| e.code == "SPX-U105"));
        assert!(errors.iter().any(|e| e.code == "SPX-O101"));
    }

    #[test]
    fn literal_keys_use_copy_arrays_without_generated_source_expansion() {
        let key = format!("\"{}\":", "k".repeat(64));
        let output = literal_write("json_Root_response", "next", "cursor", &key, 0);
        assert!(output.starts_with("let k0={let k=[34u8,107u8"));
        assert!(output.contains("writer_bytes(array_as_slice(k),cursor,next)"));
        assert!(output.contains("cursor=cursor+67usize"));
        assert_eq!(output.matches("bytes_set").count(), 0);
        assert!(!output.contains("string_"));
        assert!(output.len() < 512);
        assert_eq!(
            literal_write("unused", "next", "cursor", "}", 0),
            "next=bytes_set(next,cursor,125u8);cursor=cursor+1usize;\n"
        );
    }
}
