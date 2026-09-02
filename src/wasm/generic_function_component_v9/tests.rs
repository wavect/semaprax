use std::path::Path;
use std::process::Command;

use super::*;

fn artifact() -> PrivateGenericFunctionCoreArtifactV9 {
    let program = crate::parse(SOURCE_V9, Path::new("component-generic-function-v9.spx")).unwrap();
    emit_private_generic_function_core_v9(&program).unwrap()
}

#[derive(Clone, Copy)]
enum AdversarialOutcome {
    InvalidBool,
    InvalidTag,
    UnknownStatus,
}

fn adversarial_core(outcome: AdversarialOutcome) -> Vec<u8> {
    let program =
        crate::parse(SOURCE_V9, Path::new("generic-function-v9-adversarial.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let ordered = validate_profile(&resolved).unwrap();
    let mut lowering =
        aggregate::lower_selected_function_instances(&resolved, &ordered, &ordered[0]).unwrap();
    for shape in Shape::ALL {
        let mut body = vec![0x00];
        match outcome {
            AdversarialOutcome::InvalidBool => {
                body.extend([0x20, 0x02, 0x41, 0x02, 0x36, 0x02, 0x00]);
                body.extend([0x41, 0x00, 0x0b]);
            }
            AdversarialOutcome::InvalidTag => {
                body.push(0x41);
                write_i64(&mut body, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
                body.push(0x0b);
            }
            AdversarialOutcome::UnknownStatus => body.extend([0x41, 0x63, 0x0b]),
        }
        lowering.bodies[shape.index()] = body;
    }
    let graph_json = graph::to_json(&program).unwrap();
    compose(
        lowering,
        &graph::revision(&program),
        Sha256::digest(graph_json.as_bytes()).into(),
        plan_digest(),
    )
    .unwrap()
}

#[test]
fn deterministic_core_is_upstream_valid_and_graph_v14_bound() {
    let first = artifact();
    assert_eq!(first, artifact());
    assert_eq!(
        first.source_revision,
        "sha256:218085fb5ea1bcc090c04ac0acb3395912d0dad09027b9118d8817978b2fde0c"
    );
    assert_eq!(
        first.graph_digest,
        [
            0x62, 0x90, 0x7c, 0x4b, 0x95, 0x49, 0x5b, 0xb5, 0x73, 0xb2, 0xb3, 0x7d, 0xe9, 0xf0,
            0xb0, 0x8c, 0x7a, 0x82, 0x21, 0x89, 0x34, 0x15, 0x45, 0x21, 0xe8, 0xc0, 0xc8, 0x39,
            0x61, 0x58, 0xcc, 0x6e,
        ]
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&first.bytes)
        .expect("upstream validator rejected generic-function v9 core");
}

#[test]
fn source_and_exact_instance_mutations_reject() {
    for hostile in [
        SOURCE_V9.replacen("preserve<i64>", "preserve<bool>", 1),
        SOURCE_V9.replacen("ordered<i64, bool>", "ordered<bool, i64>", 1),
        SOURCE_V9.replacen("control != -99", "control != -98", 1),
        SOURCE_V9.replacen("control != 13", "control != 12", 1),
        SOURCE_V9.replacen("fn invert<T>", "fn invert<U>", 1),
        SOURCE_V9.replacen("!marker", "marker", 1),
        SOURCE_V9.replacen("fn ordered<T, U>", "fn ordered<U, T>", 1),
    ] {
        let parsed = crate::parse(&hostile, Path::new("hostile-generic-function-v9.spx"));
        match parsed {
            Ok(program) => assert!(emit_private_generic_function_core_v9(&program).is_err()),
            Err(error) => assert!(!error.code.is_empty()),
        }
    }
}

#[test]
fn hostile_resolved_profiles_reject_identity_and_call_confusion() {
    let program =
        crate::parse(SOURCE_V9, Path::new("generic-function-v9-hir-hostile.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();

    let mut hostile = resolved.clone();
    hostile.function_templates[2].type_parameters.swap(0, 1);
    assert!(validate_profile(&hostile).is_err());

    let mut hostile = resolved.clone();
    hostile.function_instances.swap(0, 1);
    assert!(validate_profile(&hostile).is_err());

    let mut hostile = resolved.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut hostile.functions[0].body.kind else {
        panic!("materialize shape drifted");
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("materialize shape drifted");
    };
    let ResolvedExprKind::Call { instance, .. } = &mut value.kind else {
        panic!("materialize call shape drifted");
    };
    *instance = Some(Shape::PreserveBool.instance());
    assert!(validate_profile(&hostile).is_err());

    let mut hostile = resolved;
    hostile.entrypoint = DeclarationId::new(MATERIALIZE_ID);
    assert!(validate_profile(&hostile).is_err());
}

#[test]
fn node_executes_all_instances_contracts_and_invalid_input_bools() {
    let artifact = artifact();
    let stem = format!("semaprax-generic-function-v9-{}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, artifact.bytes).unwrap();
    std::fs::write(
        &script_path,
        "import fs from 'node:fs';\nconst {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]));const m=new DataView(instance.exports.memory.buffer);const u=new Uint8Array(instance.exports.memory.buffer);const names=['cabi_preserve_i64_v9','cabi_invert_i64_v9','cabi_preserve_bool_v9','cabi_invert_bool_v9','cabi_ordered_i64_bool_v9','cabi_ordered_bool_i64_v9'];const f=names.map(n=>instance.exports[n]);const results=[160,224,288,352,416,480];for(let i=0;i<6;i++){for(const b of [0,1]){let p=f[i](b,0n);const expected=(i===1||i===3)?1-b:b;if(m.getUint8(p)!==0||m.getUint8(p+4)!==expected)throw Error(`semantic-${i}-${b}`)}}for(let i=0;i<6;i++){let p=f[i](1,-99n);if(m.getUint8(p)!==1||m.getUint32(p+12,true)!==1)throw Error('requires');p=f[i](1,13n);if(m.getUint8(p)!==1||m.getUint32(p+12,true)!==2)throw Error('ensures');u.fill(0x3c,results[i],results[i]+20);let trapped=false;try{f[i](2,0n)}catch{trapped=true}if(!trapped)throw Error('bool2');for(let j=0;j<20;j++)if(u[results[i]+j]!==0xa5)throw Error(`bool2-poison-${i}-${j}`)}console.log('generic-function-v9-core-ok');\n",
    )
    .unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .expect("Node is required by the existing Wasm gate");
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node generic-function v9 gate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn adversarial_status_and_output_bools_trap_before_any_publication() {
    let stem = format!(
        "semaprax-generic-function-v9-hostile-{}",
        std::process::id()
    );
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(
        &script_path,
        "import fs from 'node:fs';\nconst names=['cabi_preserve_i64_v9','cabi_invert_i64_v9','cabi_preserve_bool_v9','cabi_invert_bool_v9','cabi_ordered_i64_bool_v9','cabi_ordered_bool_i64_v9'];const results=[160,224,288,352,416,480];for(const path of process.argv.slice(2)){const {instance}=await WebAssembly.instantiate(fs.readFileSync(path));const u=new Uint8Array(instance.exports.memory.buffer);for(let i=0;i<6;i++){u.fill(0x3c,results[i],results[i]+20);let trapped=false;try{instance.exports[names[i]](1,0n)}catch{trapped=true}if(!trapped)throw Error(`hostile-no-trap-${path}-${i}`);for(let j=0;j<20;j++)if(u[results[i]+j]!==0xa5)throw Error(`hostile-published-${path}-${i}-${j}`)}}console.log('generic-function-v9-hostiles-ok');\n",
    )
    .unwrap();
    let mut paths = Vec::new();
    for (name, outcome) in [
        ("bool2", AdversarialOutcome::InvalidBool),
        ("tag", AdversarialOutcome::InvalidTag),
        ("unknown", AdversarialOutcome::UnknownStatus),
    ] {
        let path = std::env::temp_dir().join(format!("{stem}-{name}.wasm"));
        let bytes = adversarial_core(outcome);
        assert_ne!(bytes, artifact().bytes);
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&bytes)
            .expect("upstream validator rejected adversarial v9 core");
        std::fs::write(&path, bytes).unwrap();
        paths.push(path);
    }
    let output = Command::new("node")
        .arg(&script_path)
        .args(&paths)
        .output()
        .expect("Node is required by the existing Wasm gate");
    let _ = std::fs::remove_file(&script_path);
    for path in paths {
        let _ = std::fs::remove_file(path);
    }
    assert!(
        output.status.success(),
        "Node generic-function v9 hostile gate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
