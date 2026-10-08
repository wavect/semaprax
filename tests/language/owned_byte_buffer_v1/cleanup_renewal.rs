use super::*;

const SUCCESS: &str = r#"
module test.byte_cleanup_renewal;
@id("buffer.main") fn main()->i64 {
 let mut buffer=bytes_zeroed(2usize);
 let held=bytes_zeroed(1usize);
 let mut index=0usize;
 while index<2usize {
  if index==0usize {buffer=bytes_set(buffer,index,65u8);0}else{0}
  index=index+1usize;
  0
 }
 let value=match byte_get(bytes_as_slice(buffer),0usize){Option::Some{value}=>value,Option::None{}=>0u8,};
 if value==65u8 && byte_len(bytes_as_slice(held))==1usize {7}else{1}
}
"#;

const FAILURE: &str = r#"
module test.byte_cleanup_renewal_failure;
@id("buffer.offset") fn offset(value:usize)->usize{value+1usize}
@id("buffer.main") fn main()->i64 {
 let mut buffer=bytes_zeroed(1usize);
 let held=bytes_zeroed(1usize);
 let mut index=0usize;
 while index<1usize {
  if index==0usize {buffer=bytes_set(buffer,offset(0usize),65u8);0}else{0}
  index=index+1usize;
  0
 }
 if byte_len(bytes_as_slice(buffer))+byte_len(bytes_as_slice(held))==2usize {7}else{1}
}
"#;

const COMPOSED: &str = r#"
module test.byte_cleanup_renewal_composed;
@id("legacy.vec") fn vec_renewal()->i64 {
 let mut values=vec_with_capacity<i64>(1usize);let mut index=0;
 while index<1 {values=vec_push<i64>(values,index);index=index+1;0}
 if vec_len<i64>(values)==1usize {1}else{0}
}
@id("legacy.string") fn string_replacement()->i64 {
 let mut text="old";text="new";if string_len(text)==3 {3}else{0}
}
@id("buffer.main") fn main()->i64 {
 let mut values=vec_with_capacity<i64>(1usize);let mut index=0;
 while index<1 {values=vec_push<i64>(values,index);index=index+1;0}
 let mut text="old";text="new";
 let raw=[1u8,2u8,3u8,4u8,5u8,6u8];let source=array_as_slice(raw);
 let mut one=bytes_zeroed(1usize);one=bytes_set(one,0usize,1u8);
 let mut five=bytes_zeroed(5usize);five=bytes_set5(five,0usize,1u8,2u8,3u8,4u8,5u8);
 let mut tagged5=bytes_zeroed(5usize);tagged5=bytes_set1_or5_from_slice(tagged5,0usize,9u8,source,9223372036854775808usize);
 let mut tagged48=bytes_zeroed(48usize);tagged48=bytes_set1_or6_or48_from_slice(tagged48,0usize,9u8,source,13835058055282163712usize);
 let one_first=match byte_get(bytes_as_slice(one),0usize){Option::Some{value}=>value,Option::None{}=>0u8,};
 let five_last=match byte_get(bytes_as_slice(five),4usize){Option::Some{value}=>value,Option::None{}=>0u8,};
 let tagged5_last=match byte_get(bytes_as_slice(tagged5),4usize){Option::Some{value}=>value,Option::None{}=>0u8,};
 let tagged48_first=match byte_get(bytes_as_slice(tagged48),0usize){Option::Some{value}=>value,Option::None{}=>0u8,};
 let tagged48_source_last=match byte_get(bytes_as_slice(tagged48),5usize){Option::Some{value}=>value,Option::None{}=>0u8,};
 let tagged48_zero=match byte_get(bytes_as_slice(tagged48),47usize){Option::Some{value}=>value,Option::None{}=>1u8,};
 if vec_len<i64>(values)==1usize && string_len(text)==3 && byte_len(bytes_as_slice(one))+byte_len(bytes_as_slice(five))+byte_len(bytes_as_slice(tagged5))+byte_len(bytes_as_slice(tagged48))==59usize && one_first==1u8 && five_last==5u8 && tagged5_last==5u8 && tagged48_first==1u8 && tagged48_source_last==6u8 && tagged48_zero==0u8 {7}else{1}
}
"#;

#[test]
fn conditional_two_owner_byte_renewal_preserves_history_and_backend_routes() {
    let program = parse(SUCCESS, "byte-cleanup-renewal.spx").unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert_eq!(
        format::canonical(&parse(&canonical, "byte-cleanup-renewal-roundtrip.spx").unwrap()),
        canonical
    );
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let plan = &main_function(&resolved).cleanup_plan;
    assert_eq!(plan.schema, semaprax::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17);
    assert_eq!(
        plan.blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .filter(|transition| matches!(transition, CleanupTransition::ReserveRenewal { .. }))
            .count(),
        1
    );
    assert_eq!(
        plan.blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .filter(|transition| matches!(transition, CleanupTransition::Renew { .. }))
            .count(),
        1
    );
    let [buffer, held] = initial_owner_storages(main_function(&resolved));
    let commit = plan
        .exits
        .iter()
        .find(|exit| {
            matches!(
                exit.continuation,
                semaprax::cleanup_plan::ExitContinuation::CommitResult { .. }
            )
        })
        .unwrap();
    assert_eq!(
        commit
            .finalize_in_order
            .iter()
            .map(|action| action.source.storage.clone())
            .collect::<Vec<_>>(),
        [held, buffer]
    );
    let interpreted = interpret(SUCCESS, "conditional-renewal-success");
    assert!(
        interpreted.contains("\"kind\":\"returned\"") && interpreted.contains("\"value\":\"7\"")
    );
    let graph = graph::to_json(&program).unwrap();
    assert_eq!(graph, graph::to_json(&program).unwrap());
    graph::verify_json(&program, &graph).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(document["schema"], "semaprax.graph.v70");
    assert_eq!(
        document["byte_buffer_renewal"]["schema"],
        "semaprax.byte-buffer-renewal.v1"
    );
    assert_eq!(
        document["byte_buffer_renewal"]["updates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        codegen::emit_c(&program).unwrap(),
        codegen::emit_c(&program).unwrap()
    );
    assert_eq!(
        wasm::emit_module(&program).unwrap(),
        wasm::emit_module(&program).unwrap()
    );
    run_wasm(&program, "success", Some(7), None, "renewed");
    run_native(&program, "success", Some("7"), None);
}

#[test]
fn byte_renewal_failure_settles_both_owners_and_composes_without_rewriting_v15_v16() {
    let failed = parse(FAILURE, "byte-cleanup-renewal-failure.spx").unwrap();
    let resolved = hir::resolve(&failed).unwrap();
    hir::validate(&resolved).unwrap();
    let plan = &main_function(&resolved).cleanup_plan;
    assert_eq!(plan.schema, "semaprax.cleanup-plan.v17");
    let [buffer, held] = initial_owner_storages(main_function(&resolved));
    let set_status = plan
        .status_sources
        .iter()
        .find(|source| {
            matches!(&source.producer,
            semaprax::cleanup_plan::StatusProducer::PropagatedCall { callee }
                if callee.as_str() == "core.bytes.set")
        })
        .unwrap();
    let failure = plan
        .exits
        .iter()
        .find(|exit| {
            matches!(&exit.continuation,
            semaprax::cleanup_plan::ExitContinuation::ReturnFailure { source }
                if source == &set_status.id)
        })
        .unwrap();
    // Failure precedes `renew`: the old buffer has moved from its named slot
    // into the call-argument epoch after `held`, so reverse finalization is
    // staged buffer then held. Success restores named history above.
    assert_eq!(failure.finalize_in_order.len(), 2);
    let staged = &failure.finalize_in_order[0].source;
    assert!(matches!(
        &staged.storage,
        StorageId::CallArgument {
            parameter_index: 0,
            ..
        }
    ));
    assert_eq!(failure.finalize_in_order[1].source.storage, held);
    assert!(plan.blocks.iter().flat_map(|block| &block.transitions).any(
        |transition| matches!(transition,
            CleanupTransition::Transfer { source, destination, .. }
                if source.storage == buffer && destination == staged)
    ));
    let interpreted = interpret(FAILURE, "conditional-renewal-failure");
    let envelope: serde_json::Value = serde_json::from_str(&interpreted).unwrap();
    assert_eq!(envelope["payload"]["outcome"]["kind"], "failed");
    assert_eq!(
        envelope["payload"]["outcome"]["status"]["domain_id"],
        "semaprax.byte-buffer.v1"
    );
    assert_eq!(envelope["payload"]["outcome"]["status"]["code"], 1);
    let wasm = wasm::emit_module(&failed).unwrap();
    assert!(wasm.starts_with(b"\0asm"));
    run_wasm(&failed, "failure", None, Some(16), "staged-first");
    run_native(&failed, "failure", None, Some(73));

    let composed = parse(COMPOSED, "byte-cleanup-renewal-composed.spx").unwrap();
    let resolved = hir::resolve(&composed).unwrap();
    hir::validate(&resolved).unwrap();
    for (id, schema) in [
        ("legacy.vec", "semaprax.cleanup-plan.v15"),
        ("legacy.string", "semaprax.cleanup-plan.v16"),
        ("buffer.main", "semaprax.cleanup-plan.v17"),
    ] {
        assert_eq!(
            resolved
                .functions
                .iter()
                .find(|function| function.id.as_str() == id)
                .unwrap()
                .cleanup_plan
                .schema,
            schema
        );
    }
    let byte_plan = &resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "buffer.main")
        .unwrap()
        .cleanup_plan;
    assert_eq!(
        byte_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .filter(|transition| matches!(transition, CleanupTransition::Renew { .. }))
            .count(),
        4
    );
    let graph = graph::to_json(&composed).unwrap();
    graph::verify_json(&composed, &graph).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(document["schema"], "semaprax.graph.v70");
    assert_eq!(
        document["byte_buffer_renewal"]["updates"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        document["vec_loop_renewal"]["schema"],
        "semaprax.vec-loop-renewal.v1"
    );
    assert_eq!(
        document["string_replacement"]["schema"],
        "semaprax.string-replacement.v1"
    );
}

fn initial_owner_storages(function: &ResolvedFunction) -> [StorageId; 2] {
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("main block")
    };
    [0, 1].map(|index| {
        let ResolvedStatement::Let { binding, .. } = &statements[index] else {
            panic!("owned binding")
        };
        StorageId::Value(binding.id.clone())
    })
}

fn run_wasm(
    program: &semaprax::ast::Program,
    label: &str,
    value: Option<i64>,
    status: Option<u32>,
    drop_order: &str,
) {
    if !command_available("node") {
        return;
    }
    let path = std::env::temp_dir().join(format!(
        "semaprax-byte-cleanup-renewal-{label}-{}.wasm",
        std::process::id()
    ));
    std::fs::write(&path, wasm::emit_module(program).unwrap()).unwrap();
    let script = r#"
const fs=require('fs');
const bytes=fs.readFileSync(process.argv[1]);
const expectedValue=process.argv[2]==='none'?null:BigInt(process.argv[2]);
const expectedStatus=process.argv[3]==='none'?null:Number(process.argv[3]);
const dropOrder=process.argv[4];
let instance,next=1,allocations=[],drops=[];
const entries=new Map();
const decode=carrier=>{const word=BigInt.asUintN(64,carrier),length=Number(word&0xffffffffn),root=Number((word>>32n)&0xffffffffn),token=root&0x7fffffff;if((root&0x80000000)===0||token===0)throw Error('invalid owned Bytes carrier');return{word,length,token}};
const read=carrier=>{const decoded=decode(carrier),value=entries.get(decoded.token);if(!(value instanceof Uint8Array)||value.length!==decoded.length)throw Error('stale owned Bytes carrier');return{decoded,value}};
const allocate=value=>{const bytes=new Uint8Array(value),token=next++;entries.set(token,bytes);allocations.push(token);return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(bytes.length))};
const setBytes=(carrier,index,values)=>{const {decoded,value}=read(carrier);if(typeof index!=='bigint'||index<0n||index>BigInt(value.length)||BigInt(value.length)-index<BigInt(values.length)||!values.every(byte=>Number.isInteger(byte)&&byte>=0&&byte<=255))throw Error('owned byte interval');value.set(values,Number(index));return BigInt.asIntN(64,decoded.word)};
const unexpected=name=>()=>{throw Error(`unexpected ${name}`)};
const env={
  spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
  spx_contract_fail:selector=>{throw Object.assign(Error(`status:${selector}`),{selector:Number(selector)})},
  spx_bytes_copy:carrier=>allocate(read(carrier).value),
  spx_bytes_zeroed:length=>{if(typeof length!=='bigint'||length<0n||length>131072n)throw Error('owned byte capacity');return allocate(new Uint8Array(Number(length)))},
  spx_bytes_set:(carrier,index,byte)=>setBytes(carrier,index,[byte]),
  spx_bytes_set5:unexpected('spx_bytes_set5'),
  spx_bytes_set1_or5:unexpected('spx_bytes_set1_or5'),
  spx_bytes_set1_or6_or48:unexpected('spx_bytes_set1_or6_or48'),
  spx_bytes_get:(carrier,index)=>{const value=read(carrier).value,at=Number(index);return typeof index==='bigint'&&index>=0n&&index<BigInt(value.length)?value[at]:-1},
  spx_bytes_as_slice:carrier=>{read(carrier);return carrier},
  spx_bytes_drop:carrier=>{const {decoded}=read(carrier);if(!entries.delete(decoded.token))throw Error('double owned Bytes drop');drops.push(decoded.token)},
};
(async()=>{
  ({instance}=await WebAssembly.instantiate(bytes,{env}));
  for(let round=0;round<4;round++){
    allocations=[];drops=[];let actualValue=null,actualStatus=null;
    try{actualValue=instance.exports.semaprax_main()}catch(error){if(!Object.hasOwn(error,'selector'))throw error;actualStatus=error.selector}
    if(actualValue!==expectedValue||actualStatus!==expectedStatus)throw Error(`outcome:${actualValue}:${actualStatus}`);
    if(allocations.length!==2||drops.length!==2)throw Error(`cleanup-count:${allocations}:${drops}`);
    const expectedDrops=dropOrder==='renewed'?[allocations[1],allocations[0]]:dropOrder==='staged-first'?allocations:null;
    if(expectedDrops===null||drops[0]!==expectedDrops[0]||drops[1]!==expectedDrops[1])throw Error(`cleanup-order:${dropOrder}:${allocations}:${drops}`);
    if(entries.size!==0)throw Error(`unsettled:${entries.size}`);
  }
})().catch(error=>{console.error(error);process.exit(2)});
"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&path)
        .arg(value.map_or_else(|| "none".to_owned(), |value| value.to_string()))
        .arg(status.map_or_else(|| "none".to_owned(), |status| status.to_string()))
        .arg(drop_order)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(path);
    assert!(
        output.status.success(),
        "Core Wasm {label} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_native(
    program: &semaprax::ast::Program,
    label: &str,
    stdout: Option<&str>,
    code: Option<i32>,
) {
    if !command_available("clang") {
        return;
    }
    let path = std::env::temp_dir().join(format!(
        "semaprax-byte-cleanup-renewal-{label}-{}.native{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    codegen::build(program, &path).unwrap();
    let output = Command::new(&path).output().unwrap();
    let _ = std::fs::remove_file(path);
    assert_eq!(output.status.code(), code.or(Some(0)));
    if let Some(stdout) = stdout {
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), stdout);
    } else {
        assert!(output.stdout.is_empty());
    }
}
