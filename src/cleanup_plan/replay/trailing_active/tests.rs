//! Whole-producer, paid-caller and actual guarded Drop controls; authored, unrun.
use super::*;
#[path = "../../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod source_reader;
use crate::bounded_output::{self, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, watch_free_order, Metrics};

fn fixture() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(
        "module trailing.active; @id(\"main\") fn main()->i64{0}", "trailing-active.spx").unwrap()).unwrap()
}
fn seed(count: usize, shape: usize, work: &mut SkeletonWork<'_, '_>) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut paths = Vec::new();
    for index in 0..count {
        let mut path = empty_expr_path();
        match shape {
            0 => path.failed = index + 1 < count,
            1 => path.failed = true,
            2 => (),
            3 => path.failed = index != count / 2,
            4 => { path.residual = index % 2 == 0 && index + 1 < count; path.failed = index % 2 == 1 && index + 1 < count; }
            _ => unreachable!(),
        }
        path.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
        let expression = work.clone_owned(&work.function.body.id, "trailing fixture identity")?;
        work.push_observation(&mut path, SkeletonObservation::Status {
            source: StatusSourceId { expression, lane: StatusLane::OperationFailure }, success: index % 2 == 0,
        }, "trailing fixture observation")?;
        path.continuation = lexical_skeleton::Continuation::Advancing(path.observations.items[0].clone());
        work.push_expr_path(&mut paths, path, "trailing fixture path")?;
    }
    Ok(paths)
}
fn equal(actual: &[ExprSkeletonPath], expected: &[ExprSkeletonPath]) {
    assert_eq!(actual.len(), expected.len());
    for (a,b) in actual.iter().zip(expected) {
        assert_eq!(a.observations, b.observations);
        assert_eq!(a.owned_source, b.owned_source);
        assert_eq!(a.failed,b.failed);assert_eq!(a.residual,b.residual);
        assert_eq!(a.continuation,b.continuation);
    }
}
fn clean(metrics: Metrics, paid: usize) {
    assert!(!metrics.invalid);assert_eq!(metrics.live,0);
    assert_eq!(metrics.allocated,metrics.freed);assert!(metrics.allocated<=paid && metrics.highwater<=paid);
}
fn scan_expected(count: usize, fanout: usize, shape: usize, capacity: usize) -> bool {
    if count<8 || fanout<2 || shape==2 || (shape==3 && count/2==0) { return false; }
    let Some(required)=count.checked_sub(1).and_then(|n|n.checked_add(fanout)) else {return false};
    if required>MAX_REPLAY_PATHS || required>capacity {return false;}
    count.next_power_of_two()*size_of::<ExprSkeletonPath>() > QUERY+FRAME+count*(SCAN_ROW+CURSOR_ROW)
}
#[test]
fn complete_old_and_current_paths_work_materializations_and_actual_backing_cover_all_shapes() {
    let program=fixture();let function=&program.functions[0];let mut reached=false;
    let (((),metrics),refused,paid,first)=with_limit_usage_refusal(usize::MAX,||measure(|| {
        for count in [0,1,8,31,127,128,255,511] {for fanout in [0,1,2,3] {for shape in 0..5 {
            let mut old=ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut current=ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut old_work=SkeletonWork::new(&program,function,&mut old);
            let mut work=SkeletonWork::new(&program,function,&mut current);
            let old_input=seed(count,shape,&mut old_work).unwrap();
            let input=seed(count,shape,&mut work).unwrap();
            let old_suffixes=seed(fanout,2,&mut old_work).unwrap();let suffixes=seed(fanout,2,&mut work).unwrap();
            let pointer=input.as_ptr();let capacity=input.capacity();
            let scanned=false;let _=(pointer,capacity);
            let before=skeleton_materializations();
            let expected=before_sequence_skeleton_paths(old_input,&old_suffixes,&mut old_work).unwrap();
            let materialized=skeleton_materializations()-before;let before=skeleton_materializations();
            let actual=sequence_skeleton_paths(input,&suffixes,&mut work).unwrap();
            assert_eq!(skeleton_materializations()-before,materialized);
            equal(&actual,&expected);
            let selected=false;
            if selected {assert_eq!(actual.as_ptr(),pointer);assert_eq!(actual.capacity(),capacity);reached=true;}
            assert_eq!(work.budget.skeleton_spent,old_work.budget.skeleton_spent+if scanned {count}else{0});
            assert_eq!(old_work.budget.skeleton_remaining,work.budget.skeleton_remaining+if scanned {count}else{0});
            drop((actual,expected,suffixes,old_suffixes));drop((work,old_work));drop((current,old));
        }}}
    }));
    assert!(!reached,"test ActiveBuilder pays the closed selector then keeps the original fallback");
    assert!(!refused && first.is_none());clean(metrics,paid);
}
fn oracle_cold_run(program:&ResolvedProgram,old:bool,count:usize,shape:usize)->(OracleRun,usize){
    let function=&program.functions[0];
    // Independently obtain every expected output field before the paid observer.
    let expected={let mut budget=ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);let mut work=SkeletonWork::new(program,function,&mut budget);let input=seed(count,shape,&mut work).unwrap();let suffixes=seed(2,2,&mut work).unwrap();before_sequence_skeleton_paths(input,&suffixes,&mut work).unwrap()};
    let ((ok,receipt,seed_work,materialized),metrics)=measure(||{
        let scope=allocation::OracleScope::open_with_work(1<<28,1<<26).unwrap();let lease=scope.lease();let invocation=allocation::enter(program,function,lease).unwrap();let observation=SequenceObservation::begin(lease);
        let mut budget=ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS,lease);let mut work=SkeletonWork::with_index(program,function,&mut budget,None).unwrap();
        let input=seed(count,shape,&mut work).unwrap();let suffixes=seed(2,2,&mut work).unwrap();let seed_work=scope.receipt().work;let before=skeleton_materializations();
        let result=if old{before_sequence_skeleton_paths(input,&suffixes,&mut work)}else{sequence_skeleton_paths(input,&suffixes,&mut work)};
        let materialized=skeleton_materializations()-before;let ok=result.is_ok();equal(result.as_ref().unwrap(),&expected);drop(result);drop(suffixes);drop(work);drop(budget);assert_eq!(invocation.exit(),Ok(()));drop(observation);(ok,scope.finish(),seed_work,materialized)
    });
    assert!(!metrics.invalid&&metrics.live==0);assert_eq!(metrics.allocated,metrics.freed);assert!(metrics.allocated<=receipt.charged&&metrics.highwater<=receipt.charged);assert!(receipt.first_refusal.is_none()&&receipt.first_work_refusal.is_none()&&receipt.physical_failure.is_none());assert_eq!(receipt.started,1);assert_eq!(receipt.completed,1);
    (OracleRun{ok,receipt,seed_work,metrics},materialized)
}

fn run(program: &ResolvedProgram, old: bool, count: usize, shape: usize, cap: usize, floor: usize) -> (bool,usize,usize,Metrics) {
    let function=&program.functions[0];
    let (((ok,spent),metrics),refused,paid,first)=with_limit_usage_refusal(cap,|| {
        assert!(bounded_output::set_active_floor(floor));
        let observed=measure(|| {
            let mut budget=ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let result=(|| {
                let mut work=SkeletonWork::with_index(program,function,&mut budget,None)?;
                let input=seed(count,shape,&mut work)?;let suffixes=seed(2,2,&mut work)?;
                let result=if old {before_sequence_skeleton_paths(input,&suffixes,&mut work)}else{sequence_skeleton_paths(input,&suffixes,&mut work)};
                drop(suffixes);result
            })();
            let ok=result.is_ok();drop(result);
            if !ok {
                let remaining=bounded_output::active_remaining();
                let retry=budget.reserve_bytes(function,0,"trailing sticky");assert!(retry.is_err());drop(retry);
                assert_eq!(bounded_output::active_remaining(),remaining);
            }
            let spent=budget.skeleton_spent;drop(budget);(ok,spent)
        });
        bounded_output::clear_active_floor();observed
    });
    assert_eq!(refused,!ok);assert_eq!(first.is_some(),!ok);clean(metrics,paid);(ok,paid,spent,metrics)
}
#[test]
fn warm_and_cold_whole_callers_keep_exact_short_zero_floor_sticky_and_complete_drop() {
    let program=fixture();
    for (count,shape) in [(511,0),(511,4),(31,0),(511,2),(511,3),(511,1)] {
        let before=run(&program,true,count,shape,usize::MAX,0);let current=run(&program,false,count,shape,usize::MAX,0);
        assert!(before.0&&current.0);
        assert_eq!(current.1,before.1+PROBE,"exact cfg(test) cold probe folded into the original donor; no heavy eligibility on ActiveBuilder");assert_eq!(current.2,before.2);assert_eq!(current.3,before.3);
        for (old,exact) in [(true,before.1),(false,current.1)] {
            assert!(run(&program,old,count,shape,exact,0).0);
            assert!(!run(&program,old,count,shape,exact-1,0).0);
            assert!(!run(&program,old,count,shape,exact,1).0);
            assert!(!run(&program,old,count,shape,0,0).0);
        }
    }
    // Actual Oracle cold invocations, not an ActiveBuilder stand-in.
    for (count,shape,heavy) in [(31,2,false),(7,0,false),(8,0,true)] {
        let (old,old_materialized)=oracle_cold_run(&program,true,count,shape);
        let ((current,current_materialized),frontiers)=with_frontiers(||oracle_cold_run(&program,false,count,shape));
        assert!(old.ok&&current.ok);assert_eq!(old_materialized,current_materialized);assert_eq!(old.receipt.work,current.receipt.work);assert_eq!(old.metrics,current.metrics);
        assert_eq!(current.receipt.charged,old.receipt.charged+PROBE+if heavy{ELIGIBILITY}else{0});
        assert!(frontiers[1]>0);assert_eq!(frontiers[3]>0,heavy);assert_eq!(frontiers[5],0,"cold shape never pays FRAME");
    }
}
#[test]
fn original_first_work_failure_precedes_optional_reuse_query_for_stopped_and_active() {
    let program=fixture();let function=&program.functions[0];
    for shape in [0,2] {
        let observed=|old| {
            let scope=allocation::OracleScope::open_with_work(1<<28,1<<26).unwrap();let lease=scope.lease();let invocation=allocation::enter(&program,function,lease).unwrap();
            let mut budget=ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS,lease);
            let mut work=SkeletonWork::new(&program,function,&mut budget);
            let input=seed(127,shape,&mut work).unwrap();let suffixes=seed(2,2,&mut work).unwrap();
            work.budget.skeleton_remaining=0;
            let before=skeleton_materializations();
            let error=if old {before_sequence_skeleton_paths(input,&suffixes,&mut work)}else{sequence_skeleton_paths(input,&suffixes,&mut work)}.err().unwrap();
            let delta=skeleton_materializations()-before;drop(suffixes);drop(work);drop(budget);assert_eq!(invocation.exit(),Ok(()));let receipt=scope.finish();assert!(receipt.first_refusal.is_none());(error,delta)
        };
        assert_eq!(observed(true),observed(false),"entire six-field diagnostic and materialization frontier");
    }
}
fn drop_paths() -> Vec<ExprSkeletonPath> {
    (0..5).map(|index| {let mut path=empty_expr_path();path.failed=true;
        path.observations.items=vec![std::rc::Rc::new(SkeletonObservation::Status {
            source:StatusSourceId {expression:ExpressionId::from_owned(format!("drop.{index}")),lane:StatusLane::OperationFailure},success:false})];path}).collect()
}
fn guarded_order(unwind: bool) -> [usize;32] {
    let paths=drop_paths();let pointers:Vec<_>=paths.iter().map(|p|p.observations.items.as_ptr()as usize).collect();
    let (_,order)=watch_free_order(&pointers,|| {
        let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let buffer=PrefixBuffer {paths,published:2,has_unprocessed:true};
            if unwind {std::panic::resume_unwind(Box::new("real guarded receiving unwind"))};drop(buffer);
        }));assert_eq!(result.is_err(),unwind);
    });assert!(!order.invalid);assert_eq!(order.count,5);order.order
}
#[test]
fn actual_path_tail_drops_before_published_prefix_on_refusal_and_unwind() {
    let ((), metrics) = measure(|| {
    let paths=drop_paths();let pointers:Vec<_>=paths.iter().map(|p|p.observations.items.as_ptr()as usize).collect();
    let (_,old)=watch_free_order(&pointers,|| {
        let mut pending=paths.into_iter();let published=vec![pending.next().unwrap(),pending.next().unwrap()];
        let current=pending.next().unwrap();drop(current);drop(pending);drop(published);
    });assert!(!old.invalid);assert_eq!(old.count,5);
    assert_eq!(&old.order[..5],&[2,3,4,0,1]);
    assert_eq!(guarded_order(false),old.order);assert_eq!(guarded_order(true),old.order);
    });
    assert!(!metrics.invalid); assert_eq!(metrics.live,0); assert_eq!(metrics.allocated,metrics.freed);
}
#[test]
fn entire_immediate_sequence_body_and_full_original_root_are_source_bound() {
    use sha2::{Digest,Sha256};
    let old=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a"));
    assert_eq!(old.len(),326143);
    assert_eq!(format!("{:x}",crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))),"392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a");
    let start=old.find("fn sequence_skeleton_paths(").unwrap();let end=old[start..].find("\nuse path_summary::has_active_paths;").unwrap()+start;
    let expected=old[start..end].replacen("fn sequence_skeleton_paths(","pub(super) fn before_sequence_skeleton_paths(",1);
    assert_eq!(include_str!("../trailing_active.rs").matches(&expected).count(),1);
    let physical=source_reader::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("e/392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a"),old.len(),"392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a",2*1024*1024).unwrap();
    assert_eq!(physical,old);
    // Exact whole original helper sources remain byte prefixes. Every added
    // typed bank is bound to actual current compiled/physical bytes; archive
    // channels and late fresh reads are retained independently.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (source, current, original, length, sha) in [
        ("src/vec_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/vec_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/7c161ddc20a7729a9b1451df9a65844c5ef3a3ee7d6e2852e3239ca6e9dd50e9")), 30453, "c880a46547d9385e12ea75f4577eef5e059fde9a189a17839951be22e278f5f2"),
        ("src/box_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/box_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/314a10e9a06e9e767d918539f810258bf84e98e8ca21673efba3d116065d7325")), 10543, "c1c21bfdfc4ae4db27239ee21426328f42b3404c42e5a4bca55bf59117f2df79"),
        ("src/stdin_stream_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/stdin_stream_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/5c216d1345d20face39f0e07ac9554c2d659b0a171fcf0cbb33dd60684961836")), 17778, "df37f338d309c71d599b4ada290b6fa1f681d018737344c6cb0fc7b9c8104343"),
        ("src/str_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/str_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/ba40008730318730664ca40e17df6c2a9f80ca2f31432145828ac9fc4a6ad8bf")), 6225, "f681546ee58598288095926f2c76a25a26665770258eb25dea221e5c38cbbf8a"),
        ("src/byte_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/byte_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/22e3f98ab80d320bed149c19de4ab1394f0dca7b55be3b7080f2745a50186ade")), 30884, "aac7715eed4a017553d09f3543aab57177e8400d9bffc45bfbe78e277fe42f9e"),
        ("src/host_io_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/host_io_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/cf414c6df8b7c6cdfad1770b27b0d0c384a58922bb296e7083e77e685bf2deed")), 4638, "56f1d29f043877d59cf70f84c7bf17b35e5ee27347383d75b4e9f8348498bf1e"),
        ("src/map_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/map_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/adf43789d79a542aa5b160665a65f10a9ea116490efbbf1f91f6202651dd1dcf")), 17442, "65bb7cb33edd6bed0e197243ba25b6cf6f7fb3f929cce6d6cdddee079990c4ba"),
        ("src/list_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/list_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/eaf364364d29ea595520b8390a195ddd947a9caab5969b5c86cf1cf6c1923ab2")), 12487, "a8e692ad890dacbfdb6b412f6cf277a9c22e09888675af9c816347c7444e86ef"),
        ("src/iterator_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/iterator_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/dc672b05a80a67c565e58d13e1c60ec2c3e9ad8a7b1d3828378c376f0a47c391")), 19175, "1b18820c66c5b47e18e6ed7e2c58d4319e5ab50f0fe2c2ea1ee153c16620fa69"),
        ("src/environment_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/environment_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/db1ec5b288810cef98c7a2d7f29b7da29172c32b4a3db5a4fb66e242a679d799")), 3495, "fc20ba1d398a69b13aae41a8057bc3953091d3b45b84702dfdca8c0cdc989af0"),
        ("src/filesystem_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/filesystem_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/a4c3dc9b7e2aa97a7bcf030e2b1da14970205b267b94a69350d2873365e97966")), 10702, "30c9508c40a210f5724a643562f48169f1e5a4278def0631c56c7692cc1041af"),
        ("src/network_io_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/network_io_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/7f189ffc6b9a4b30787a588f9af31c753fc6285456869f4cd88c1f256928e76b")), 20828, "b44fb31432a28a6e846fe0bf4b371159d4e750b6a70cd76695fb90dbcd09cd65"),
        ("src/command_io_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/command_io_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/91ea1c1d6e33c9ca68920e00509c502f2cee55118d9981b999b975871b8b6b07")), 44220, "31bee6ec1d18c786de7161c63f2fd85ddea221bf9ca366762d8a332452892239"),
        ("src/hir/ids/compiler.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/ids/compiler.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/89ff8b726bcbacb4474e73f7bb7a1c321da261666c4b039e98745d2fbcab15df")), 7672, "0e2cb9430e9a6ec9f73e8e1bd07d36fbec84b5299635f2883b26a835cc6796b5"),
        ("src/hir/ids.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/ids.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/1eceacba63ce7ce7cbeb99ebe1ecfa05b3bdc25b679c6f74fadaa0b5b3a0284e")), 24265, "5a7fb6366d3f448c6ab0f30dc99b24a007e3c2805db082f59396d8c0ca4ee0d8"),
    ] {
        // Acquire the actual shipping helper through all current channels
        // before entering the complete unchanged trailing-probe source epoch.
        let physical_current = current;
        let acquired = super::super::factored::moderate_tests::cold949_previous_input(source, current, length, sha);
        let current = acquired.as_str();
        assert_eq!(current.len(), length);
        assert_eq!(format!("{:x}",crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))),sha);
        assert_eq!(&current[..original.len()], original);
        let old_sha=format!("{:x}",crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes())));
        assert_eq!(super::super::factored::moderate_tests::cold949_previous_input(source,physical_current,length,sha),current);
        assert_eq!(source_reader::read(&root.join("e").join(old_sha),original.len(),&format!("{:x}",crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))),2*1024*1024).unwrap(),original);
        assert_eq!(super::super::factored::moderate_tests::cold949_previous_input(source,physical_current,length,sha),current);
    }
    let current=include_str!("../../replay.rs");
    let expected=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/ci-inputs-20261021/before/e688f57ac691a478b7c751ad69975d3ba447ffdc6ac8f71b018a5d438e8f3d0c.txt"));
    assert_eq!(crate::source_reflows::legacy_ci_expected_input("src/cleanup_plan/replay.rs","e688f57ac691a478b7c751ad69975d3ba447ffdc6ac8f71b018a5d438e8f3d0c",current).as_ref(),expected);
    for bad in [format!("{current} "),current[..current.len()-1].to_owned(),old.to_owned()] {
        assert!(std::panic::catch_unwind(|| { let entered=crate::source_reflows::legacy_ci_expected_input("src/cleanup_plan/replay.rs","e688f57ac691a478b7c751ad69975d3ba447ffdc6ac8f71b018a5d438e8f3d0c",&bad); assert_eq!(entered.as_ref(),expected,"unchanged complete old consumer refuses a foreign supplied stage"); }).is_err());
    }

}

struct OracleRun {
    ok: bool,
    receipt: allocation::OracleReceipt,
    seed_work: usize,
    metrics: Metrics,
}
// A real completed direct sequence-caller observation, using the same account
// protocol as factored/oracle_hook::Run. Invocation authority alone never starts
// this observation. The independent whole-program50 control remains unchanged.
struct SequenceObservation(allocation::OracleLease);
impl SequenceObservation {
    fn begin(lease: allocation::OracleLease) -> Self { lease.start().unwrap(); Self(lease) }
}
impl Drop for SequenceObservation {
    fn drop(&mut self) { self.0.complete().unwrap(); }
}
fn oracle_run(program: &ResolvedProgram, old: bool, bytes: usize, units: usize) -> OracleRun {
    use allocation::{enter,OracleScope,OracleRefusal};
    let function=&program.functions[0];
    let ((ok,receipt,seed_work),metrics)=measure(|| {
        let scope=OracleScope::open_with_work(bytes,units).unwrap();let lease=scope.lease();
        let invocation=enter(program,function,lease).unwrap();let observation=SequenceObservation::begin(lease);
        let mut budget=ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS,lease);
        let mut seed_work=0;
        let result=(|| {
            let mut work=SkeletonWork::with_index(program,function,&mut budget,None)?;
            let input=seed(warm_count(),0,&mut work)?;let suffixes=seed(2,2,&mut work)?;
            seed_work=scope.receipt().work;
            let result=if old {before_sequence_skeleton_paths(input,&suffixes,&mut work)}else{sequence_skeleton_paths(input,&suffixes,&mut work)};
            drop(suffixes);result
        })();
        let ok=result.is_ok();drop(result);
        if !ok {
            let first=scope.receipt();
            assert_eq!(lease.charge(0),Err(OracleRefusal::Sticky));
            assert!(budget.reserve_bytes(function,0,"oracle trailing sticky").is_err());
            let after=scope.receipt();
            assert_eq!(after.charged,first.charged);assert_eq!(after.work,first.work);
            assert_eq!(after.first_refusal,first.first_refusal);assert_eq!(after.first_work_refusal,first.first_work_refusal);
            assert_eq!(after.physical_failure,first.physical_failure);
        }
        drop(budget);assert_eq!(invocation.exit(),Ok(()));drop(observation);(ok,scope.finish(),seed_work)
    });
    assert!(!metrics.invalid&&metrics.live==0);assert_eq!(metrics.allocated,metrics.freed);
    assert!(metrics.allocated<=receipt.charged&&metrics.highwater<=receipt.charged);
    assert_eq!(receipt.started,1);assert_eq!(receipt.completed,1);
    OracleRun {ok,receipt,seed_work,metrics}
}
#[test]
fn actual_oracle_payer_keeps_cumulative_bytes_work_exact_short_and_sticky_receipts() {
    let program=fixture();
    let old=oracle_run(&program,true,1<<28,1<<26);let current=oracle_run(&program,false,1<<28,1<<26);
    assert!(old.ok&&current.ok);
    assert_eq!(current.receipt.work,old.receipt.work+warm_count(),"all old work plus the paid complete scan");
    assert!(current.metrics.allocated<old.metrics.allocated,"real Oracle backing removed");
    // Cold metadata may cost more than removed backing; report actual totals
    // honestly instead of asserting a fabricated old/new allowance or refund.
    eprintln!("TRAILING_ORACLE old={} current={} oldheap={} currentheap={}",old.receipt.charged,current.receipt.charged,old.metrics.allocated,current.metrics.allocated);
    for old in [true,false] {
        let full=oracle_run(&program,old,1<<28,1<<26);
        assert!(oracle_run(&program,old,full.receipt.charged,full.receipt.work).ok);
        assert!(!oracle_run(&program,old,full.receipt.charged-1,full.receipt.work).ok);
        assert!(!oracle_run(&program,old,full.receipt.charged,full.receipt.work-1).ok);
        assert!(!oracle_run(&program,old,0,1<<26).ok);
        let first=oracle_run(&program,old,1<<28,full.seed_work);
        assert!(!first.ok);assert!(first.receipt.first_work_refusal.is_some());
    }
}
#[test]
fn expired_oracle_receiver_cannot_touch_fresh_foreign_account_or_replace_old_diagnostic() {
    use allocation::{enter,OracleScope};
    let program=fixture();let foreign_program=fixture();let function=&program.functions[0];
    let expected=replay_error(function,"ordinary builder already failed");
    for old in [true,false] {
        let scope=OracleScope::open_with_work(1<<26,1<<26).unwrap();let lease=scope.lease();
        let invocation=enter(&program,function,lease).unwrap();
        let mut budget=ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS,lease);
        let mut work=SkeletonWork::new(&program,function,&mut budget);
        let input=seed(warm_count(),0,&mut work).unwrap();let suffixes=seed(2,2,&mut work).unwrap();
        assert_eq!(invocation.exit(),Ok(()));scope.finish();
        let fresh=OracleScope::open_with_work(1<<26,1<<26).unwrap();
        let foreign=enter(&foreign_program,&foreign_program.functions[0],fresh.lease()).unwrap();let before=fresh.receipt();
        let result=if old {before_sequence_skeleton_paths(input,&suffixes,&mut work)}else{sequence_skeleton_paths(input,&suffixes,&mut work)};
        assert_eq!(result.as_ref().err(),Some(&expected),"entire original raw diagnostic");
        assert_eq!(fresh.receipt(),before,"no foreign charge attempts or work");
        drop(result);drop(suffixes);drop(work);drop(budget);
        assert_eq!(foreign.exit(),Ok(()));assert_eq!(fresh.finish(),before);
    }
}

fn warm_count() -> usize {
    (3..usize::BITS).map(|bits|(1usize<<bits)-1).find(|&n|n<MAX_REPLAY_PATHS && scan_expected(n,2,0,n.next_power_of_two())).expect("a bounded real-backing crossover must exist")
}
#[test]
fn actual_oracle_shape_bank_preserves_all_outputs_work_and_selected_backing() {
    let program=fixture();let function=&program.functions[0];let warm=warm_count();let mut reached=false;
    for count in [0,1,8,31,127,128,warm] {for fanout in [0,1,2,3] {for shape in 0..5 {
        let invoke=|old| {
            let scope=allocation::OracleScope::open_with_work(1<<28,1<<26).unwrap();let lease=scope.lease();let invocation=allocation::enter(&program,function,lease).unwrap();
            let mut budget=ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS,lease);let mut work=SkeletonWork::new(&program,function,&mut budget);
            let input=seed(count,shape,&mut work).unwrap();let suffixes=seed(fanout,2,&mut work).unwrap();let pointer=input.as_ptr();let capacity=input.capacity();
            let before=skeleton_materializations();let output=if old {before_sequence_skeleton_paths(input,&suffixes,&mut work)}else{sequence_skeleton_paths(input,&suffixes,&mut work)}.unwrap();
            let materialized=skeleton_materializations()-before;let spent=work.budget.skeleton_spent;drop(suffixes);drop(work);drop(budget);assert_eq!(invocation.exit(),Ok(()));let receipt=scope.finish();assert!(receipt.first_refusal.is_none()&&receipt.first_work_refusal.is_none()&&receipt.physical_failure.is_none());(output,pointer,capacity,spent,materialized,receipt)
        };
        let before=invoke(true);let actual=invoke(false);equal(&actual.0,&before.0);assert_eq!(actual.4,before.4);
        let scanned=scan_expected(count,fanout,shape,actual.2);assert_eq!(actual.3,before.3+if scanned {count}else{0});
        if scanned&&(shape==0||shape==4) {assert_eq!(actual.0.as_ptr(),actual.1);assert_eq!(actual.0.capacity(),actual.2);reached=true;}
    }}}
    assert!(reached);eprintln!("TRAILING_CROSSOVER count={warm}; original128fixture unchanged and unqualified");
}

#[test]
fn actual_query_and_frame_positive_frontiers_refuse_exact_request_before_scan_or_transfer() {
    let program=fixture();let (full,frontiers)=with_frontiers(||oracle_run(&program,false,1<<28,1<<26));assert!(full.ok);
    for index in 0..3 {let before=frontiers[index*2];let requested=frontiers[index*2+1];assert!(before>0&&requested>0);let (failed,seen)=with_frontiers(||oracle_run(&program,false,before+requested-1,1<<26));assert!(!failed.ok);
        let first=failed.receipt.first_refusal.unwrap();assert_eq!(first.requested,requested);assert_eq!(first.remaining,requested-1);assert_eq!(failed.receipt.charged,before);assert_eq!(seen[index*2+1],requested);
        assert_eq!(failed.receipt.work,failed.seed_work+1,"first original stopped work, no shape scan or transfer work");
        for later in index+1..3 {assert_eq!(seen[later*2+1],0,"earlier actual positive frontier refuses before later metadata");}
    }
}
#[test]
fn actual_selected_transfer_work_refusal_keeps_original_tail_then_published_drop_order() {
    // Only the immutable process-lifetime prelude is acquired outside this
    // invocation. The complete fixture, both producers and every input/output
    // owner are still constructed and dropped inside the original observation.
    let held_prelude = crate::prelude::declarations();
    let prelude_identity = (held_prelude.as_ptr(), held_prelude.len());
    let ((), metrics) = measure(|| {
    let program=fixture();let function=&program.functions[0];let n=warm_count();assert!(n>=8);
    let observe=|old| {
        let scope=allocation::OracleScope::open_with_work(1<<28,1<<26).unwrap();let lease=scope.lease();let invocation=allocation::enter(&program,function,lease).unwrap();let mut budget=ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS,lease);let mut work=SkeletonWork::new(&program,function,&mut budget);
        let input=seed(n,0,&mut work).unwrap();let suffixes=seed(2,2,&mut work).unwrap();let indices=[0,1,2,n-2,n-1];let pointers:Vec<_>=indices.iter().map(|&i|input[i].observations.items.as_ptr()as usize).collect();
        work.budget.skeleton_remaining=if old {2}else{n+2};
        let (result,order)=watch_free_order(&pointers,||if old {before_sequence_skeleton_paths(input,&suffixes,&mut work)}else{sequence_skeleton_paths(input,&suffixes,&mut work)});
        assert!(result.is_err());assert!(!order.invalid);assert_eq!(order.count,5);drop(suffixes);drop(work);drop(budget);assert_eq!(invocation.exit(),Ok(()));let receipt=scope.finish();assert!(receipt.first_refusal.is_none());(result.err().unwrap(),order.order)
    };
    let old=observe(true);let current=observe(false);assert_eq!(current.0,old.0,"whole original selected raw diagnostic");assert_eq!(current.1,old.1);assert_eq!(&current.1[..5],&[2,3,4,0,1]);
    });
    assert!(!metrics.invalid); assert_eq!(metrics.live,0); assert_eq!(metrics.allocated,metrics.freed);
    let after_prelude = crate::prelude::declarations();
    assert_eq!(prelude_identity, (after_prelude.as_ptr(), after_prelude.len()));
    assert!(std::ptr::eq(held_prelude, after_prelude));
}
