use std::path::Path;
use std::process::Command;

use super::*;

fn artifact() -> PrivateGenericRecordCoreArtifactV7 {
    let program = crate::parse(SOURCE_V7, Path::new("component-generic-record-v7.spx")).unwrap();
    emit_private_generic_record_core_v7(&program).unwrap()
}

#[derive(Clone, Copy)]
enum AdversarialOutcome {
    InvalidBool,
    InvalidTag,
    UnknownStatus,
}

fn adversarial_core(outcome: AdversarialOutcome) -> Vec<u8> {
    let program = crate::parse(SOURCE_V7, Path::new("adversarial-generic-record-v7.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let ordered = validate_profile(&resolved).unwrap();
    let mut lowering =
        aggregate::lower_selected_functions(&resolved, &ordered, &ordered[0]).unwrap();
    for shape in Shape::ALL {
        let mut body = vec![0x00];
        match outcome {
            AdversarialOutcome::InvalidBool => {
                let output_parameter = if matches!(shape, Shape::DuoI64Bool | Shape::DuoBoolI64) {
                    3
                } else {
                    1
                };
                body.push(0x20);
                write_u32(&mut body, output_parameter);
                body.extend([0x41, 0x02, 0x36, 0x02]);
                write_u32(&mut body, shape.bool_offset() as u32);
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
    let mut layout_digests = [[0_u8; 32]; 4];
    for shape in Shape::ALL {
        let layout =
            AggregateLayout::for_type(&resolved, AggregateTarget::Wasm32, &shape.ty()).unwrap();
        require_layout(&layout, shape).unwrap();
        layout_digests[shape.index()] = layout.digest();
    }
    let graph_json = graph::to_json(&program).unwrap();
    compose(
        lowering,
        &[0, 1, 2, 3],
        &graph::revision(&program),
        Sha256::digest(graph_json.as_bytes()).into(),
        plan_digest(&layout_digests),
        layout_digests,
    )
    .unwrap()
}

#[test]
fn deterministic_core_is_upstream_valid_and_exact_instance_bound() {
    let first = artifact();
    assert_eq!(first, artifact());
    assert_ne!(first.layout_digests[0], first.layout_digests[1]);
    assert_ne!(first.layout_digests[2], first.layout_digests[3]);
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&first.bytes)
        .expect("upstream validator rejected generic-record v7 core");
}

#[test]
fn exact_source_semantic_and_instance_mutations_reject() {
    for hostile in [
        SOURCE_V7.replacen("Duo<T, U>", "Duo<U, T>", 1),
        SOURCE_V7.replacen("Duo<i64, bool>", "Duo<bool, i64>", 1),
        SOURCE_V7.replacen("left + delta", "left - delta", 1),
        SOURCE_V7.replacen("right + delta", "right - delta", 1),
        SOURCE_V7.replacen("delta != -99", "delta != -98", 1),
        SOURCE_V7.replacen("divisor != 13", "divisor != 12", 1),
        SOURCE_V7.replacen("Phantom<i64>", "Phantom<bool>", 1),
        SOURCE_V7.replacen("!input.marker", "input.marker", 1),
    ] {
        let parsed = crate::parse(&hostile, Path::new("hostile-v7.spx"));
        match parsed {
            Ok(program) => assert!(emit_private_generic_record_core_v7(&program).is_err()),
            Err(error) => assert!(!error.code.is_empty()),
        }
    }
}

#[test]
fn node_executes_all_instances_status_order_poison_and_invalid_bools() {
    let artifact = artifact();
    let stem = format!("semaprax-generic-record-v7-{}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, artifact.bytes).unwrap();
    std::fs::write(
            &script_path,
            "import fs from 'node:fs';\nconst {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]));\nconst m=new DataView(instance.exports.memory.buffer);\nconst a=instance.exports.cabi_transform_i64_bool_v7;const b=instance.exports.cabi_transform_bool_i64_v7;const p64=instance.exports.cabi_preserve_phantom_i64_v7;const pb=instance.exports.cabi_invert_phantom_bool_v7;\nlet p=a(83n,1,1n,2n);if(m.getUint8(p)!==0||m.getBigInt64(p+8,true)!==42n||m.getUint8(p+16)!==1)throw Error('a');\np=b(0,83n,1n,2n);if(m.getUint8(p)!==0||m.getUint8(p+8)!==0||m.getBigInt64(p+16,true)!==42n)throw Error('b');\np=p64(1);if(m.getUint8(p)!==0||m.getUint8(p+4)!==1)throw Error('p64');p=pb(0);if(m.getUint8(p)!==0||m.getUint8(p+4)!==1)throw Error('pb');\np=a(9223372036854775807n,1,1n,0n);if(m.getUint8(p)!==1||m.getUint32(p+16,true)!==1)throw Error('sticky-a');p=b(1,9223372036854775807n,1n,0n);if(m.getUint8(p)!==1||m.getUint32(p+16,true)!==1)throw Error('sticky-b');\nfor(const [f,args] of [[a,[1n,1,1n,0n]],[b,[1,1n,1n,0n]]]){p=f(...args);if(m.getUint8(p)!==1||m.getUint32(p+16,true)!==4)throw Error('div0');}\nfor(const [f,args] of [[a,[1n,2,1n,2n]],[b,[2,1n,1n,2n]],[p64,[2]],[pb,[2]]]){let trapped=false;try{f(...args)}catch{trapped=true}if(!trapped)throw Error('bool2');}\nconsole.log('generic-record-v7-core-ok');\n",
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
        "Node generic-record v7 gate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn adversarial_selected_bodies_trap_before_publishing_poisoned_results() {
    let stem = format!("semaprax-generic-record-v7-hostile-{}", std::process::id());
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(
            &script_path,
            "import fs from 'node:fs';\nconst names=['cabi_transform_i64_bool_v7','cabi_transform_bool_i64_v7','cabi_preserve_phantom_i64_v7','cabi_invert_phantom_bool_v7'];const args=[[7n,1,1n,2n],[1,7n,1n,2n],[1],[0]];const results=[192,320,416,480];\nfor(const path of process.argv.slice(2)){const {instance}=await WebAssembly.instantiate(fs.readFileSync(path));const bytes=new Uint8Array(instance.exports.memory.buffer);for(let i=0;i<4;i++){bytes.fill(0x3c,results[i],results[i]+24);let trapped=false;try{instance.exports[names[i]](...args[i])}catch{trapped=true}if(!trapped)throw Error(`hostile did not trap ${path} ${names[i]}`);for(let j=0;j<24;j++)if(bytes[results[i]+j]!==0xa5)throw Error(`published byte ${path} ${names[i]} ${j}`)}}\nconsole.log('generic-record-v7-hostiles-ok');\n",
        )
        .unwrap();
    let mut wasm_paths = Vec::new();
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
            .expect("upstream validator rejected adversarial v7 core");
        std::fs::write(&path, bytes).unwrap();
        wasm_paths.push(path);
    }
    let output = Command::new("node")
        .arg(&script_path)
        .args(&wasm_paths)
        .output()
        .expect("Node is required by the existing Wasm gate");
    let _ = std::fs::remove_file(&script_path);
    for path in wasm_paths {
        let _ = std::fs::remove_file(path);
    }
    assert!(
        output.status.success(),
        "Node generic-record v7 hostile gate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
