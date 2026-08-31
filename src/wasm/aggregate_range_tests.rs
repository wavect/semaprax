//! Byte ranges must retain scratch storage when matching selects plan v5/v6.
use super::{emit_profile, hex_identity, program_uses_byte_range};
use crate::cleanup_plan::{StatusProducer, CLEANUP_PLAN_SCHEMA_V4};
use crate::hir::{self, DeclarationId};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn ranges_under_record_and_variant_match_preserve_status_output_and_reentry() {
    for (kind, declaration, constructor, schema) in [
        (
            "record",
            "record Packet { @id(\"range.payload\") payload: Bytes, }",
            "Packet",
            crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V5,
        ),
        (
            "variant",
            "variant Packet { @id(\"range.data\") Data { @id(\"range.payload\") payload: Bytes, }, }",
            "Packet::Data",
            crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V6,
        ),
    ] {
        let source = format!(
            r#"module test.range_match;
@id("range.packet") {declaration}
@id("range.window") fn window(input:borrow Slice<u8>,start:usize,end:usize)->i64 {{
    let packet = {constructor} {{payload:bytes_copy(input)}};
    match own packet {{ {constructor} {{payload}} => {{
        let selected = byte_range(input,start,end);
        if byte_len(selected) == 2usize {{42}} else {{0}}
    }}, }}
}}
@id("app.main") fn main()->i64 {{0}}
"#
        );
        let program = crate::check(&source, "range-match.spx").unwrap();
        let canonical = crate::format::canonical(&program);
        assert_eq!(
            crate::format::canonical(&crate::parse(&canonical, "range-match.spx").unwrap()),
            canonical
        );
        let resolved = hir::resolve(&program).unwrap();
        let window = resolved.functions.iter().find(|f| f.id.as_str() == "range.window").unwrap();
        assert_eq!(window.cleanup_plan.schema, schema);
        assert!(resolved.functions.iter().all(|f| f.cleanup_plan.schema != CLEANUP_PLAN_SCHEMA_V4));
        assert!(program_uses_byte_range(&resolved));
        let bytes = emit_profile(&resolved, true, false).unwrap();
        assert_eq!(bytes, emit_profile(&resolved, true, false).unwrap());
        wasmparser::Validator::new().validate_all(&bytes).unwrap();

        let root = Fixture(std::env::temp_dir().join(format!("spx-range-match-{}-{kind}", std::process::id())));
        std::fs::create_dir(&root.0).unwrap();
        let symbol = hex_identity(&DeclarationId::new("range.window"));
        let generated = crate::codegen::emit_c(&program).unwrap();
        let probe = format!(r#"
#include <string.h>
int main(void) {{
    struct spx_status_entry entries[32];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, 77, entries, 32, NULL, NULL, NULL)) return 10;
    const uint64_t starts[] = {{0,1,3,0,0}};
    const uint8_t input[] = {{7,8}};
    const spx_slice_u8_v1 slice = {{input,2}};
    const uint64_t ends[] = {{2,1,1,3,2}};
    const uint32_t codes[] = {{0,0,1,2,0}};
    const int64_t values[] = {{42,0,0,0,42}};
    for (int round=0; round<3; ++round) {{
        for (int i=0; i<5; ++i) {{
            int64_t output = INT64_C(12345);
            spx_status_token token = spx_decl_{symbol}(&context, slice, starts[i], ends[i], &output);
            if (codes[i] == 0) {{
                if (token != SPX_STATUS_SUCCESS || output != values[i]) return 11;
            }} else {{
                const struct spx_normalized_status *status = spx_status_resolve(&context, token);
                if (status == NULL || strcmp(status->domain_id,"semaprax.byte-range.v1") || status->code != codes[i]) return 12;
                if (output != INT64_C(12345)) return 13;
            }}
        }}
    }}
    return 0;
}}
"#);
        let c_path = root.0.join("probe.c");
        std::fs::write(&c_path, format!("{generated}\n{probe}")).unwrap();
        for optimization in ["-O0", "-O2"] {
            let executable = root.0.join(format!("probe{optimization}{}", std::env::consts::EXE_SUFFIX));
            let compiled = Command::new("clang").args(["-std=c11",optimization,"-Wall","-Wextra","-Werror","-DSPX_NO_ENTRY_WRAPPER"]).arg(&c_path).arg("-o").arg(&executable).output().unwrap();
            assert!(compiled.status.success(), "{kind}/{optimization}: {}", String::from_utf8_lossy(&compiled.stderr));
            let output = Command::new(&executable).output().unwrap();
            assert!(output.status.success(), "{kind}/{optimization}: {output:?}");
        }

        let web = root.0.join("web");
        crate::wasm::build_web(&program, &web).unwrap();
        // Render the real byte host against the exact private test artifact.
        // The ordinary package stays untouched; this is not a public ABI claim.
        let digest = format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&bytes)));
        let runtime = crate::wasm::browser_runtime()
            .replace("__SEMAPRAX_OWNED_EXPORTS__", "Object.freeze({})")
            .replace("__SEMAPRAX_WASM_SHA256__", &digest);
        std::fs::write(web.join("test-runtime.mjs"), runtime).unwrap();
        std::fs::write(web.join("test.wasm"), bytes).unwrap();
        std::fs::write(web.join("probe.mjs"), format!(r#"
import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes}} from './test-runtime.mjs';
const {{instance}} = await instantiateBytes(await readFile('./test.wasm'),{{maxOwnedByteEntries:1}});
const memory = new DataView(instance.exports.__spx_test_memory.buffer);
const window = instance.exports.__spx_test_{symbol};
const output = 65536;
memory.setUint8(1024,7); memory.setUint8(1025,8);
const input = (1024n << 32n) | 2n;
for(let round=0;round<3;++round) {{
  for(const [start,end,code,value] of [[0n,2n,0,42n],[1n,1n,0,0n],[3n,1n,1,0n],[0n,3n,2,0n],[0n,2n,0,42n]]) {{
    memory.setBigInt64(output,12345n,true);
    if(window(input,start,end,output)!==code) throw Error('wrong range status');
    if(memory.getBigInt64(output,true)!==(code===0?value:12345n)) throw Error('wrong result or changed failed output');
  }}
}}
"#)).unwrap();
        let node = Command::new("node").arg("probe.mjs").current_dir(&web).output().unwrap();
        assert!(node.status.success(), "{kind}: {node:?}");

        let mut forged = resolved.clone();
        let function = forged.functions.iter_mut().find(|f| f.id.as_str() == "range.window").unwrap();
        let status = function.cleanup_plan.status_sources.iter_mut().find(|source| matches!(&source.producer, StatusProducer::PropagatedCall {callee} if callee.as_str() == crate::byte_ops::RANGE_ID)).unwrap();
        status.producer = StatusProducer::PropagatedCall {callee:DeclarationId::new("forged.range")};
        assert_eq!(crate::wasm::emit_resolved_module(&forged).unwrap_err().code, "SPX-H006");
    }
}
