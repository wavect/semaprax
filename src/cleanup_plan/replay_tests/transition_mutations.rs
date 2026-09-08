use super::*;

#[test]
fn skeleton_replay_rejects_a_transition_location_substitution() {
    let program = program();
    let mut function = function(&program, "token.forward");
    let substitute = function.body.id.clone();
    let transition = function
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find(|transition| {
            matches!(
                transition,
                CleanupTransition::Initialize { at, .. }
                    | CleanupTransition::Transfer { at, .. }
                    if at != &substitute
            )
        })
        .expect("fixture must contain a transition at a non-body expression");
    match transition {
        CleanupTransition::Initialize { at, .. }
        | CleanupTransition::InitializeVariant { at, .. }
        | CleanupTransition::Transfer { at, .. }
        | CleanupTransition::TransferVariant { at, .. }
        | CleanupTransition::AuthenticateVariantCase { at, .. }
        | CleanupTransition::ReserveRenewal { at, .. }
        | CleanupTransition::Renew { at, .. } => *at = substitute,
        CleanupTransition::CallCommit { .. }
        | CleanupTransition::SelectFailure { .. }
        | CleanupTransition::StageCopyResult { .. } => {
            unreachable!()
        }
    }

    let diagnostic = validate_structure(&program, &function).unwrap_err();
    assert!(diagnostic
        .message
        .contains("decision or ownership-event sequence disagrees with typed HIR"));
}
