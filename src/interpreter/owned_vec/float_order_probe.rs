//! Internal representation probe, not a public nonfinite-literal/ABI extension.
use crate::{codegen, hir, interpreter::*, wasm};
use std::{collections::BTreeMap, process::Command};

const BITS32: [u64; 12] = [
    0x7fc00002, 0xff800001, 0x80000000, 0x7f800001, 0xffc00002, 0x3f800000, 0x7f800000, 0xffc00001,
    0x00000000, 0xff800000, 0xbf800000, 0x7fc00001,
];
const BITS64: [u64; 12] = [
    0x7ff8000000000002,
    0xfff0000000000001,
    0x8000000000000000,
    0x7ff0000000000001,
    0xfff8000000000002,
    0x3ff0000000000000,
    0x7ff0000000000000,
    0xfff8000000000001,
    0x0000000000000000,
    0xfff0000000000000,
    0xbff0000000000000,
    0x7ff8000000000001,
];

fn source(narrow: bool) -> String {
    let ty = if narrow { "f32" } else { "f64" };
    let parameters = (0..12)
        .map(|i| format!("p{i}:{ty}"))
        .collect::<Vec<_>>()
        .join(",");
    let pushes = (0..12)
        .map(|i| format!("v=vec_push<R>(v,R{{key:p{i},id:{i}}});"))
        .collect::<String>();
    let arguments = (0..12)
        .map(|i| format!("{}.25{ty}", 1000 + i))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"module record.float_probe;
@id("r") record R {{ @id("r.key") key:{ty}, @id("r.id") id:i64, }}
@id("probe") fn probe({parameters})->i64 {{
 let mut v=vec_with_capacity<R>(12usize); {pushes}
 v=vec_sort<R>(v); let mut i=0usize; let mut digest=0;
 while i<12usize {{ let r=vec_get<R>(v,i); digest=digest*13+r.id; i=i+1usize; 0 }}
 digest
}}
@id("app.main") fn main()->i64 {{ probe({arguments}) }}
"#
    )
}

// Only the test artifact's finite argument constants are rewritten. Source and
// independent HIR verification still reject nonfinite literals. Exact bits are
// injected before Wasm execution, avoiding JS Number's NaN canonicalization.
fn inject_argument_bits(bytes: &mut [u8], narrow: bool, bits: &[u64]) {
    let mut patches = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut reader = body.get_operators_reader().unwrap();
            while !reader.eof() {
                let offset = reader.original_position();
                let value = match reader.read().unwrap() {
                    wasmparser::Operator::F32Const { value } if narrow => {
                        f32::from_bits(value.bits()) as f64
                    }
                    wasmparser::Operator::F64Const { value } if !narrow => {
                        f64::from_bits(value.bits())
                    }
                    _ => continue,
                };
                for (i, bits) in bits.iter().enumerate() {
                    if value == 1000.25 + i as f64 {
                        patches.push((offset + 1, *bits));
                    }
                }
            }
        }
    }
    assert_eq!(
        patches.len(),
        bits.len(),
        "each private argument must have one constant"
    );
    for (offset, bits) in patches {
        let count = if narrow { 4 } else { 8 };
        bytes[offset..offset + count].copy_from_slice(&bits.to_le_bytes()[..count]);
    }
    wasmparser::Validator::new().validate_all(bytes).unwrap();
}

#[test]
fn copy_record_float_total_order_preserves_nan_payloads_at_internal_boundaries() {
    for narrow in [true, false] {
        let bits = if narrow { &BITS32 } else { &BITS64 };
        let mut ordered = (0..bits.len()).collect::<Vec<_>>();
        ordered.sort_by(|&a, &b| {
            if narrow {
                f32::from_bits(bits[a] as u32).total_cmp(&f32::from_bits(bits[b] as u32))
            } else {
                f64::from_bits(bits[a]).total_cmp(&f64::from_bits(bits[b]))
            }
        });
        assert_eq!(ordered, [4, 7, 1, 9, 10, 2, 8, 5, 6, 3, 11, 0]);
        let expected = ordered.iter().fold(0i64, |n, &id| n * 13 + id as i64);
        let ast = crate::check(&source(narrow), "float-probe.spx").unwrap();
        let program = hir::resolve(&ast).unwrap();
        hir::validate(&program).unwrap();
        let functions = program
            .functions
            .iter()
            .map(|f| (f.id.as_str(), f))
            .collect::<BTreeMap<_, _>>();
        let arguments = bits
            .iter()
            .map(|&bits| {
                (
                    String::new(),
                    if narrow {
                        ArgumentValue::Float32(f32::from_bits(bits as u32))
                    } else {
                        ArgumentValue::Float64(f64::from_bits(bits))
                    },
                )
            })
            .collect::<Vec<_>>();
        for _ in 0..3 {
            let (result, _, _) = function_values::evaluate_resolved_entry(
                functions["probe"],
                &arguments,
                &functions,
                &program,
                1_000_000,
                false,
            );
            assert!(
                matches!(result, Ok(Value::Int(value)) if value == expected),
                "{result:?}"
            );
        }
        let root = std::env::temp_dir().join(format!(
            "spx-record-float-probe-{}-{narrow}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let c = root.join("probe.c");
        let (cty, ity) = if narrow {
            ("float", "uint32_t")
        } else {
            ("double", "uint64_t")
        };
        let args = bits
            .iter()
            .map(|n| format!("from_bits(({ity})UINT64_C({n}))"))
            .collect::<Vec<_>>()
            .join(",");
        let native = codegen::emit_c(&ast).unwrap();
        std::fs::write(&c, format!(r#"{native}
static {cty} from_bits({ity} bits){{{cty} value;memcpy(&value,&bits,sizeof(value));return value;}}
int main(void){{struct spx_status_entry entries[32];struct spx_context c={{0}};
 if(!spx_context_init(&c,UINT64_C(17),entries,32,NULL,NULL,NULL))return 1;
 for(unsigned n=0;n<3;n++){{int64_t result=0;
  if(spx_decl_70726f6265(&c,{args},&result)!=SPX_STATUS_SUCCESS||result!=INT64_C({expected})||c.status_arena.length)return 2;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;j++)if(c.vec_authority[j].live)return 3;
 }}return 0;}}
"#)).unwrap();
        for opt in ["-O0", "-O2"] {
            let binary = root.join(format!("probe{opt}"));
            let output = Command::new("clang")
                .args([
                    "-std=c11",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_NO_ENTRY_WRAPPER",
                    opt,
                ])
                .arg(&c)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(Command::new(binary).status().unwrap().success());
        }
        let mut module = wasm::emit_resolved_module(&program).unwrap();
        inject_argument_bits(&mut module, narrow, bits);
        let file = root.join("probe.wasm");
        std::fs::write(&file, module).unwrap();
        let output = Command::new("node")
            .arg("-e")
            .arg(include_str!(
                "../../../tests/owned_data/copy_record_vec/host.js"
            ))
            .arg(file)
            .arg("0")
            .arg(expected.to_string())
            .arg("0")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
