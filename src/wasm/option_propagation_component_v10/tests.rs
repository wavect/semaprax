use std::path::Path;
use std::process::Command;

use super::*;

fn program() -> Program {
    crate::parse(
        SOURCE_V10,
        Path::new("component-option-propagation-v10.spx"),
    )
    .unwrap()
}

fn artifact() -> PrivateOptionPropagationCoreArtifactV10 {
    emit_private_option_propagation_core_v10(&program()).unwrap()
}

fn hex(value: [u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(64);
    for byte in value {
        write!(&mut encoded, "{byte:02x}").unwrap();
    }
    encoded
}

fn adversarial_core(kind: u8) -> Vec<u8> {
    let program = program();
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let ordered = validate_profile(&resolved).unwrap();
    let mut lowering =
        aggregate::lower_selected_functions(&resolved, &ordered, &DeclarationId::new(FUNCTION_ID))
            .unwrap();
    let mut body = vec![0x00];
    match kind {
        0 => {
            // Some(false) with a noncanonical physical bool payload.
            body.extend([0x20, 0x02, 0x41, 0x02, 0x36, 0x02, 0x04]);
            body.extend([0x20, 0x02, 0x41, 0x01, 0x36, 0x02, 0x00]);
            body.extend([0x41, 0x00, 0x0b]);
        }
        1 => {
            body.extend([0x20, 0x02, 0x41, 0x02, 0x36, 0x02, 0x00]);
            body.extend([0x41, 0x00, 0x0b]);
        }
        2 => body.extend([0x41, 0x63, 0x0b]),
        _ => unreachable!(),
    }
    lowering.bodies[0] = body;
    let source_revision = graph::revision(&program);
    let graph_json = graph::to_json(&program).unwrap();
    let layouts = VariantLayoutCache::build(&resolved, VariantTarget::Wasm32).unwrap();
    let i64_layout = layouts.layout(&option_type(ResolvedType::I64)).unwrap();
    let bool_layout = layouts.layout(&option_type(ResolvedType::Bool)).unwrap();
    let plan_json = crate::graph_cleanup::cleanup_plan_json(
        &function(&resolved, &DeclarationId::new(FUNCTION_ID))
            .unwrap()
            .cleanup_plan,
    );
    compose(
        lowering,
        ProfileRoots {
            source_revision: &source_revision,
            graph_digest: Sha256::digest(graph_json.as_bytes()).into(),
            prelude_digest: prelude::digest_v1(),
            option_i64_layout_digest: i64_layout.digest(),
            option_bool_layout_digest: bool_layout.digest(),
            plan_digest: plan_digest(&plan_json),
        },
        false,
    )
    .unwrap()
}

#[test]
fn deterministic_core_is_upstream_valid_and_v11_v3_bound() {
    let first = artifact();
    assert_eq!(first, artifact());
    let graph_json = graph::to_json(&program()).unwrap();
    assert!(graph_json.starts_with("{\"schema\":\"semaprax.graph.v11\","));
    let resolved = hir::resolve(&program()).unwrap();
    let selected = function(&resolved, &DeclarationId::new(FUNCTION_ID)).unwrap();
    assert_eq!(
        selected.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V3
    );
    assert_eq!(
        first.source_revision,
        "sha256:98b8fc892c183499153142d5bbdb4162e31bda95ef145d34dbb1ff57c9b8fc72"
    );
    assert_eq!(
        hex(first.graph_digest),
        "96083f90fab18c919a96cee48109e606e089159e109869a42bdf48831743d45d"
    );
    assert_eq!(
        hex(first.prelude_digest),
        "d37bad7e3911669bbf2c66b25c8b31d5c2e36eb181cc54fdc86c3a49a8fb9c5e"
    );
    assert_eq!(
        hex(first.option_i64_layout_digest),
        "79194fc88011ac060877e60293d0a4272429dd9e2d720674d0d54e804562deda"
    );
    assert_eq!(
        hex(first.option_bool_layout_digest),
        "dec126293ece7ec0e48d3d85ccdb494f7c7cfe4c3d4a9b1a61b50f6f862ff038"
    );
    assert_eq!(
        hex(first.plan_digest),
        "d07fa51fc6f192a43318140264fa0e5964933ed90bc065cc8c74708e258ff92f"
    );
    assert_eq!(
        hex(Sha256::digest(&first.bytes).into()),
        "16d1d34024e3fad920d8d00a61d7cb3bd010335ca382f23615b3b3da4143aaec"
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&first.bytes)
        .expect("upstream validator rejected option-propagation v10 core");
}

#[test]
fn exact_source_and_profile_mutations_reject() {
    for hostile in [
        SOURCE_V10.replacen(
            "component.option-propagation.evaluate",
            "component.evaluate",
            1,
        ),
        SOURCE_V10.replacen("let checked = input?;", "let other = input?;", 1),
        SOURCE_V10.replacen("checked + 1", "checked + 2", 1),
        SOURCE_V10.replacen("divisor != -99", "divisor != -98", 1),
        SOURCE_V10.replacen("divisor != 13", "divisor != 12", 1),
        SOURCE_V10.replacen("Option<bool>", "Option<i64>", 1),
    ] {
        match crate::parse(&hostile, Path::new("hostile-option-propagation-v10.spx")) {
            Ok(program) => assert!(emit_private_option_propagation_core_v10(&program).is_err()),
            Err(error) => assert!(!error.code.is_empty()),
        }
    }
    let mut resolved = hir::resolve(&program()).unwrap();
    resolved.functions[0].cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V2;
    assert!(validate_profile(&resolved).is_err());
}

#[test]
fn hostile_resolved_hir_identity_order_entrypoint_and_try_metadata_reject() {
    let mut wrong_entrypoint = hir::resolve(&program()).unwrap();
    wrong_entrypoint.entrypoint = DeclarationId::new(FUNCTION_ID);
    assert!(validate_profile(&wrong_entrypoint).is_err());

    let mut reordered = hir::resolve(&program()).unwrap();
    reordered.functions.swap(0, 1);
    assert!(validate_profile(&reordered).is_err());

    let mut wrong_try_metadata = hir::resolve(&program()).unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut wrong_try_metadata.functions[0].body.kind
    else {
        panic!("evaluate body block shape drifted");
    };
    let crate::hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("evaluate body shape drifted");
    };
    let ResolvedExprKind::TryOption {
        some_case,
        none_case,
        ..
    } = &mut value.kind
    else {
        panic!("evaluate postfix-option shape drifted");
    };
    *none_case = some_case.clone();
    assert!(validate_profile(&wrong_try_metadata).is_err());
}

#[test]
fn node_executes_some_none_contracts_sticky_arithmetic_skip_and_reentry() {
    let artifact = emit_test_profile(&program()).unwrap();
    let stem = format!("semaprax-option-propagation-v10-{}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, artifact.bytes).unwrap();
    let script = format!(
        "import fs from 'node:fs';\nconst {{instance}}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]));const v=new DataView(instance.exports.memory.buffer);const u=new Uint8Array(instance.exports.memory.buffer);const f=instance.exports['{canonical}'];const validate=instance.exports['{validate}'];const area={area};const poison=()=>u.fill(0xa5,area,area+20);const assertPoison=(l)=>{{for(let i=0;i<20;i++)if(u[area+i]!==0xa5)throw Error(l+'-poison-'+i)}};const ok=(tag,payload,divisor,expectedTag,expectedBool,l)=>{{const p=f(tag,BigInt(payload),BigInt(divisor));if(p!==area||v.getUint8(p)!==0||v.getUint8(p+4)!==expectedTag||(expectedTag===1&&v.getUint8(p+5)!==expectedBool))throw Error(l)}};ok(1,83,2,1,1,'some-true');ok(1,-5,2,1,0,'some-false');ok(0,0,0,0,0,'none-skips-div0');for(let i=5;i<20;i++)if(u[area+i]!==0xa5)throw Error('none-payload-'+i);for(let i=0;i<4096;i++)ok(i&1,7,2,i&1,1,'reentry');const err=(tag,payload,divisor,code,l)=>{{const p=f(tag,BigInt(payload),BigInt(divisor));if(v.getUint8(p)!==1||v.getUint32(p+12,true)!==code)throw Error(l)}};err(1,1,-99,1,'requires-some');err(0,0,-99,1,'requires-none');err(0,0,13,2,'none-ensures');err(1,1,0,4,'div0');err(1,9223372036854775807n,1,1,'overflow');v.setUint32(600,0,true);v.setUint32(604,0xa5a5a5a5,true);u.fill(0xa5,620,628);if(validate(600,620)!==0||v.getUint32(620,true)!==0)throw Error('none-validator');for(let i=624;i<628;i++)if(u[i]!==0xa5)throw Error('none-validator-payload');v.setUint32(600,1,true);v.setUint32(604,2,true);if(validate(600,620)!=={invalid})throw Error('bool2');v.setUint32(600,2,true);if(validate(600,620)!=={invalid})throw Error('tag2');poison();let trapped=false;try{{f(2,0n,1n)}}catch{{trapped=true}}if(!trapped)throw Error('input-tag2');assertPoison('input-tag2');console.log('option-propagation-v10-core-ok');\n",
        canonical = CANONICAL_EXPORT,
        validate = TEST_VALIDATE_EXPORT,
        area = RESULT_AREA,
        invalid = aggregate::STATUS_INTERNAL_INVALID_TAG,
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .expect("Node is required by the existing Wasm gate");
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node v10 core gate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn adversarial_output_bool_tag_and_unknown_status_trap_with_full_poison() {
    for kind in 0..3 {
        let bytes = adversarial_core(kind);
        let stem = format!("semaprax-option-v10-hostile-{}-{kind}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();
        let script = format!(
            "import fs from 'node:fs';const {{instance}}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]));const u=new Uint8Array(instance.exports.memory.buffer);const f=instance.exports['{canonical}'];let trapped=false;try{{f(1,5n,1n)}}catch{{trapped=true}}if(!trapped)throw Error('not-trapped');for(let i=0;i<20;i++)if(u[{area}+i]!==0xa5)throw Error('published-'+i);",
            canonical = CANONICAL_EXPORT,
            area = RESULT_AREA,
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "hostile outcome {kind} escaped: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
