use std::path::Path;
use std::process::Command;

use super::*;

fn artifact() -> PrivateRecordPatternCoreArtifactV8 {
    let program = crate::parse(SOURCE_V8, Path::new("component-record-pattern-v8.spx")).unwrap();
    emit_private_record_pattern_core_v8(&program).unwrap()
}

#[derive(Clone, Copy)]
enum AdversarialOutcome {
    InvalidBool,
    InvalidTag,
    UnknownStatus,
}

fn adversarial_core(outcome: AdversarialOutcome) -> Vec<u8> {
    let program = crate::parse(SOURCE_V8, Path::new("record-pattern-v8-adversarial.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let ordered = validate_profile(&resolved).unwrap();
    let mut lowering =
        aggregate::lower_selected_functions(&resolved, &ordered, &ordered[0]).unwrap();
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
    let instances = [Shape::PreserveI64.ty(), Shape::PreserveBool.ty()];
    let mut layout_digests = [[0_u8; 32]; 2];
    for (index, instance) in instances.iter().enumerate() {
        let layout =
            AggregateLayout::for_type(&resolved, AggregateTarget::Wasm32, instance).unwrap();
        require_layout(&layout, instance).unwrap();
        layout_digests[index] = layout.digest();
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
fn deterministic_core_is_upstream_valid_and_graph_v13_bound() {
    let first = artifact();
    assert_eq!(first, artifact());
    assert_ne!(first.layout_digests[0], first.layout_digests[1]);
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&first.bytes)
        .expect("upstream validator rejected record-pattern v8 core");
}

#[test]
fn generic_and_pattern_semantic_mutations_reject() {
    for hostile in [
        SOURCE_V8.replacen("Phantom<T>", "Phantom<i64>", 1),
        SOURCE_V8.replacen("Phantom<i64>", "Phantom<bool>", 1),
        SOURCE_V8.replacen("control != -99", "control != -98", 1),
        SOURCE_V8.replacen("control != 13", "control != 12", 1),
        SOURCE_V8.replacen("Phantom { marker }", "_", 1),
        SOURCE_V8.replacen("=> marker", "=> !marker", 1),
        SOURCE_V8.replacen("fn preserve_phantom_i64", "fn preserve_phantom_i64<T>", 1),
    ] {
        let parsed = crate::parse(&hostile, Path::new("hostile-record-pattern-v8.spx"));
        match parsed {
            Ok(program) => assert!(emit_private_record_pattern_core_v8(&program).is_err()),
            Err(error) => assert!(!error.code.is_empty()),
        }
    }
}

#[test]
fn node_executes_all_patterns_contracts_and_invalid_input_bools() {
    let artifact = artifact();
    let stem = format!("semaprax-record-pattern-v8-{}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, artifact.bytes).unwrap();
    std::fs::write(
        &script_path,
        "import fs from 'node:fs';\nconst {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]));const m=new DataView(instance.exports.memory.buffer);const u=new Uint8Array(instance.exports.memory.buffer);const names=['cabi_preserve_pattern_phantom_i64_v8','cabi_invert_pattern_phantom_i64_v8','cabi_preserve_pattern_phantom_bool_v8','cabi_invert_pattern_phantom_bool_v8'];const f=names.map(n=>instance.exports[n]);const results=[160,224,288,352];for(let i=0;i<4;i++){for(const b of [0,1]){let p=f[i](b,0n);if(m.getUint8(p)!==0||m.getUint8(p+4)!==(i%2?1-b:b))throw Error('semantic')}}for(let i=0;i<4;i++){let p=f[i](1,-99n);if(m.getUint8(p)!==1||m.getUint32(p+12,true)!==1)throw Error('requires');p=f[i](1,13n);if(m.getUint8(p)!==1||m.getUint32(p+12,true)!==2)throw Error('ensures');u.fill(0x3c,results[i],results[i]+20);let trapped=false;try{f[i](2,0n)}catch{trapped=true}if(!trapped)throw Error('bool2');for(let j=0;j<20;j++)if(u[results[i]+j]!==0xa5)throw Error(`bool2-poison-${i}-${j}`)}console.log('record-pattern-v8-core-ok');\n",
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
        "Node record-pattern v8 gate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn adversarial_status_and_output_bools_trap_before_any_publication() {
    let stem = format!("semaprax-record-pattern-v8-hostile-{}", std::process::id());
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(
        &script_path,
        "import fs from 'node:fs';\nconst names=['cabi_preserve_pattern_phantom_i64_v8','cabi_invert_pattern_phantom_i64_v8','cabi_preserve_pattern_phantom_bool_v8','cabi_invert_pattern_phantom_bool_v8'];const results=[160,224,288,352];for(const path of process.argv.slice(2)){const {instance}=await WebAssembly.instantiate(fs.readFileSync(path));const u=new Uint8Array(instance.exports.memory.buffer);for(let i=0;i<4;i++){u.fill(0x3c,results[i],results[i]+20);let trapped=false;try{instance.exports[names[i]](1,0n)}catch{trapped=true}if(!trapped)throw Error(`hostile-no-trap-${path}-${i}`);for(let j=0;j<20;j++)if(u[results[i]+j]!==0xa5)throw Error(`hostile-published-${path}-${i}-${j}`)}}console.log('record-pattern-v8-hostiles-ok');\n",
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
            .expect("upstream validator rejected adversarial v8 core");
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
        "Node record-pattern v8 hostile gate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
