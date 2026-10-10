//! Host-only physical fixtures; compiler-directed settlement is covered separately.

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn private_host_accounting_and_hostile_carriers() {
    // Exercise the final token without billions of invocations or a production
    // mutation hook. Only the initial counter and allocation observation differ.
    let mut token_arena = include_str!("arena.js").to_owned();
    for (original, replacement) in [
        ("function createArena(", "function tokenArena("),
        ("let nextToken=1,", "let nextToken=0x7fffffff,"),
        (
            "const bytes=new Bytes(length);",
            "tokenPayloadAllocations++;const bytes=new Bytes(length);",
        ),
    ] {
        assert_eq!(token_arena.matches(original).count(), 1);
        token_arena = token_arena.replacen(original, replacement, 1);
    }
    let source = [
        "import assert from 'node:assert/strict';\n",
        include_str!("input.js"),
        include_str!("arena.js"),
        &token_arena,
        include_str!("tests/arena.mjs"),
    ]
    .join("\n");
    let mut child = Command::new("node")
        .arg("--input-type=module")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the standalone Wasm host gate requires provisioned Node.js");
    child
        .stdin
        .take()
        .expect("piped Node stdin")
        .write_all(source.as_bytes())
        .expect("send bounded host fixture");
    let output = child.wait_with_output().expect("wait for host fixture");
    assert!(
        output.status.success(),
        "host fixture failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn literal_format_status_is_bound_once_and_does_not_reclassify_conversion_failure() {
    let source = r#"module format.runtime;
@id("render") fn render()->i64 {string_len(string_format("{}",1))}
@id("app.main") fn main()->i64 {0}
"#;
    let ast = crate::check(source, "format-runtime.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let closure = std::collections::BTreeSet::from([crate::hir::DeclarationId::new("render")]);
    for enabled in [false, true] {
        let runtime = super::render_toolkit("{}", "digest", 0, &program, &closure, enabled);
        assert_eq!(runtime.contains("status===34"), enabled);
        assert!(runtime.contains("status<=22?\"semaprax.convert.v1\""));
        assert!(runtime.contains("status<=22?status-20"));
        if enabled {
            assert!(runtime.contains("(status===11||status===34)!==(cause!==null)"));
            assert!(runtime.contains("else if(status===34)result=Object.freeze({kind:\"failure\",domain:\"semaprax.string-format.v1\",code:1})"));
        } else {
            assert!(runtime.contains("(status===11)!==(cause!==null)"));
            assert!(!runtime.contains("semaprax.string-format.v1"));
        }
    }
    let legacy = super::render("{}", "digest", 0, true);
    assert!(!legacy.contains("status===34"));
    assert!(legacy.contains("status===21)result=Object.freeze({kind:\"failure\",domain:\"semaprax.convert.v1\",code:1})"));
}

#[test]
fn borrowed_toolkit_selection_preserves_artifacts_and_excludes_unselected_hir() {
    use crate::hir::DeclarationId;
    use crate::string_ops::StringOp;
    use crate::wasm::aggregate::{map_collections, text_toolkit};
    use crate::wasm::internal_strings::{emit_text_toolkit_module, InternalStringOptions};
    use std::collections::BTreeSet;

    let baseline = r#"module toolkit.borrowed_selection;
@id("app.main") fn main()->i64 { string_len(string_trim(" text ")) }
"#;
    let additions = r#"
@id("unused.record") record Unused { @id("unused.record.value") value:i64, }
@id("unused.generic") fn generic<T>(value:T)->T { value }
@id("unused.instance") fn instance()->i64 { generic<i64>(9) }
@id("unused.count") fn count(value:borrow Map<i64,i64>)->i64 { i64_from_usize(map_len<i64,i64>(value)) }
@id("unused.map") fn map_count()->i64 { let value=map_new<i64,i64>(1usize); count(value) }
@id("unused.byte") fn first_byte()->i64 {
    let text="a"; let view=str_as_bytes(string_as_str(text));
    match byte_get(view,0usize) { Option::Some { value: first } => if first==97u8 { 1 } else { 0 }, Option::None {} => 0, }
}
@id("unused.slice") fn slice_count()->i64 { string_len(string_slice("abc",0,1)) }
"#;
    let base = crate::check(baseline, "toolkit-base.spx").unwrap();
    let extended = crate::check(&format!("{baseline}{additions}"), "toolkit-extended.spx").unwrap();
    let program = crate::hir::resolve(&extended).unwrap();
    assert!(!program.types.is_empty());
    assert!(!program.function_templates.is_empty());
    assert!(!program.function_instances.is_empty());
    let before = crate::cache_codec::encode(&program).unwrap();

    for (id, expected, byte_get, collections) in [
        ("app.main", vec![StringOp::Trim], false, false),
        ("unused.slice", vec![StringOp::Slice], false, false),
        ("unused.byte", vec![], true, false),
        ("unused.map", vec![], false, true),
    ] {
        let (_, closure) =
            super::super::admission::prepare_toolkit(&program, &[id.into()]).unwrap();
        let selected = program.functions.iter().filter(|f| closure.contains(&f.id));
        let actual = text_toolkit::selected_functions(selected.clone());
        assert_eq!(actual, (expected, byte_get));
        assert_eq!(
            map_collections::selected_functions_use(selected),
            collections
        );

        // Independent prior selection recipe: owned filtering is retained only
        // in this regression to prove exact import and runtime-byte equivalence.
        let mut filtered = program.clone();
        filtered.functions.retain(|f| closure.contains(&f.id));
        filtered.function_instances.clear();
        filtered.types.clear();
        assert_eq!(actual.0, text_toolkit::selected(&filtered));
        assert_eq!(actual.1, text_toolkit::uses_byte_get(&filtered));
        assert_eq!(collections, map_collections::uses(&filtered));
        let all = filtered
            .functions
            .iter()
            .map(|f| f.id.clone())
            .collect::<BTreeSet<DeclarationId>>();
        assert_eq!(
            super::render_toolkit("{}", "digest", 0, &program, &closure, false),
            super::render_toolkit("{}", "digest", 0, &filtered, &all, false),
        );
    }
    assert_eq!(crate::cache_codec::encode(&program).unwrap(), before);
    let old = emit_text_toolkit_module(
        &base,
        &["app.main".into()],
        InternalStringOptions::default(),
    )
    .unwrap();
    let new = emit_text_toolkit_module(
        &extended,
        &["app.main".into()],
        InternalStringOptions::default(),
    )
    .unwrap();
    assert_eq!(old.wasm_bytes(), new.wasm_bytes());
    assert_eq!(old.descriptor(), new.descriptor());
    assert_eq!(old.runtime_source(), new.runtime_source());
    assert!(new
        .runtime_source()
        .contains("\"compare\",\"spx_string_trim_v2\"]"));
    assert!(!new.runtime_source().contains("collections.settle()"));
}
