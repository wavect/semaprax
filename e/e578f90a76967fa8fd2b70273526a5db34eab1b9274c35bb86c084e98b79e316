// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_1 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[cfg(test)]
        fn $frame_owned_capacity(frame: &$Frame<'_>) -> usize {
            let ids = |values: &scope_checkpoint::IdSnapshot| values.retained_capacity();
            let scope = |scope: &scope_checkpoint::Scope| scope.retained_capacity();
            match frame {
                $Frame::InvokeNext { scope: value, .. } | $Frame::Enter { scope: value, .. } => {
                    scope(value)
                }
                $Frame::BinaryRight { baseline, .. } => baseline
                    .as_ref()
                    .map_or(0, |(outer_ids, value)| ids(outer_ids) + scope(value)),
                $Frame::IfThen {
                    outer, outer_ids, ..
                } => scope(outer) + ids(outer_ids),
                $Frame::IfElse {
                    then_scope,
                    outer,
                    outer_ids,
                    ..
                } => scope(then_scope) + scope(outer) + ids(outer_ids),
                $Frame::CallNext {
                    params,
                    return_type,
                    scope: value,
                    ..
                } => {
                    params.owned_capacity()
                        + call_signature::owned_capacity(return_type)
                        + scope(value)
                }
                $Frame::CallAfterArg {
                    params,
                    return_type,
                    ..
                } => params.owned_capacity() + call_signature::owned_capacity(return_type),
                $Frame::NativeNext {
                    result,
                    scope: value,
                    ..
                } => resolved_type_owned_capacity(result) + scope(value),
                $Frame::NativeAfterArg { result, .. } => resolved_type_owned_capacity(result),
                $Frame::BlockNext {
                    scope: value,
                    outer,
                    outer_ids,
                    ..
                }
                | $Frame::BlockAfterAssign {
                    scope: value,
                    outer,
                    outer_ids,
                    ..
                } => scope(value) + scope(outer) + ids(outer_ids),
                $Frame::BlockAfterLet {
                    outer, outer_ids, ..
                }
                | $Frame::BlockTail {
                    outer, outer_ids, ..
                } => scope(outer) + ids(outer_ids),
                $Frame::RecordNext {
                    seen, scope: value, ..
                }
                | $Frame::VariantNext {
                    seen, scope: value, ..
                }
                | $Frame::UpdateNext {
                    seen, scope: value, ..
                } => {
                    seen.len()
                        * (std::mem::size_of::<DeclarationId>()
                            + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + seen.iter().map(|id| id.as_str().len()).sum::<usize>()
                        + scope(value)
                }
                $Frame::RecordAfterField { seen, .. }
                | $Frame::VariantAfterField { seen, .. }
                | $Frame::UpdateAfterField { seen, .. } => {
                    seen.len()
                        * (std::mem::size_of::<DeclarationId>()
                            + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + seen.iter().map(|id| id.as_str().len()).sum::<usize>()
                }
                $Frame::RecordMatchArm {
                    outer, outer_ids, ..
                } => scope(outer) + ids(outer_ids),
                $Frame::VariantMatchNext {
                    outer,
                    outer_ids,
                    arm_scopes,
                    covered,
                    ..
                }
                | $Frame::VariantMatchAfterArm {
                    outer,
                    outer_ids,
                    arm_scopes,
                    covered,
                    ..
                } => {
                    scope(outer)
                        + ids(outer_ids)
                        + arm_scopes.capacity() * std::mem::size_of::<scope_checkpoint::Scope>()
                        + arm_scopes.iter().map(scope).sum::<usize>()
                        + covered.owned_capacity()
                }
                $Frame::ScalarMatchNext {
                    outer,
                    outer_ids,
                    arm_scopes,
                    ..
                }
                | $Frame::ScalarMatchAfterGuard {
                    outer,
                    outer_ids,
                    arm_scopes,
                    ..
                }
                | $Frame::ScalarMatchAfterArm {
                    outer,
                    outer_ids,
                    arm_scopes,
                    ..
                } => {
                    scope(outer)
                        + ids(outer_ids)
                        + arm_scopes.capacity() * std::mem::size_of::<scope_checkpoint::Scope>()
                        + arm_scopes.iter().map(scope).sum::<usize>()
                }
                _ => 0,
            }
        }
    };
}
