//! Exact selected borrowed chain walks, without a forwarded query Result.
//! The ordinary allocating query remains unchanged. The or-false form preserves
//! the old caller's intentional semantic-error suppression and adds no debit.
macro_rules! contains {($permit:ident,$function:expr,$block:expr,$region:expr,$selected:expr $(,)?)=>{{
let function:&crate::hir::ResolvedFunction=$function;let block:crate::cleanup_plan::BlockId=$block;let region:crate::cleanup_plan::CleanupRegionId=$region;let selected:bool=$selected;
'chain:{
if !selected {break 'chain crate::cleanup_plan::replay::lexical_cert::storage::chain_contains(function,block,region,false)?;}
let mut current=Some(function.cleanup_plan.blocks[block.0 as usize].region);let mut found=false;let mut remaining=function.cleanup_plan.regions.len();
while let Some(id)=current {
remaining=match remaining.checked_sub(1) {Some(value)=>value,None=>return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Boundary(crate::cleanup_plan::renewal_profile::boundary_refusal::BoundaryRefusal::CleanupRegionTreeContainsACycle)))};
let item=match function.cleanup_plan.regions.get(id.0 as usize) {Some(value)=>value,None=>return Err($permit.refuse(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Boundary(crate::cleanup_plan::renewal_profile::boundary_refusal::BoundaryRefusal::CleanupRegionChainReferencesAnUnknownRegion)))};
found|=id==region;current=item.parent;
}
found
}
}};}
macro_rules! contains_or_false {
    ($function:expr,$block:expr,$region:expr,$selected:expr $(,)?) => {{
        let function: &crate::hir::ResolvedFunction = $function;
        let block: crate::cleanup_plan::BlockId = $block;
        let region: crate::cleanup_plan::CleanupRegionId = $region;
        let selected: bool = $selected;
        'chain: {
            if !selected {
                break 'chain crate::cleanup_plan::replay::lexical_cert::storage::chain_contains(
                    function, block, region, false,
                )
                .unwrap_or(false);
            }
            let mut current = Some(function.cleanup_plan.blocks[block.0 as usize].region);
            let mut found = false;
            let mut remaining = function.cleanup_plan.regions.len();
            while let Some(id) = current {
                remaining = match remaining.checked_sub(1) {
                    Some(value) => value,
                    None => break 'chain false,
                };
                let item = match function.cleanup_plan.regions.get(id.0 as usize) {
                    Some(value) => value,
                    None => break 'chain false,
                };
                found |= id == region;
                current = item.parent;
            }
            found
        }
    }};
}
pub(in crate::cleanup_plan::replay) use contains;
pub(in crate::cleanup_plan::replay) use contains_or_false;

#[cfg(test)]
#[path = "chains_tests.rs"]
mod tests;
