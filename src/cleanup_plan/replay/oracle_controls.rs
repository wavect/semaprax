//! Test-only invocation and diagnostic controls. Live owner opt-in is deferred.
use super::allocation::{authority, enter, InvocationRefusal, OracleRefusal, OracleScope};

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "module auth; @id(\"main\") fn main()->i64 {0}";

    fn program() -> crate::hir::ResolvedProgram {
        crate::hir::resolve(&crate::check(SOURCE, "oracle-auth.spx").unwrap()).unwrap()
    }

    #[test]
    fn invocation_authority_is_exact_scoped_and_fails_closed() {
        let program = program();
        let function = &program.functions[0];
        let other = self::program();
        assert_eq!(
            authority(&program, function),
            Ok(None),
            "missing is ordinary"
        );
        let scope = OracleScope::open(1 << 12).unwrap();
        let lease = scope.lease();
        let held = enter(&program, function, lease).unwrap();
        assert_eq!(authority(&program, function), Ok(Some(lease)));
        assert_eq!(
            enter(&program, function, lease).err(),
            Some(InvocationRefusal::Nested)
        );
        // Stale subjects: another function value and another program value,
        // equal in content but not the live pointers.
        let clone = function.clone();
        assert_eq!(
            authority(&program, &clone),
            Err(InvocationRefusal::StaleSubject)
        );
        // Preserve the original attempted foreign construction under authority
        // as a negative control, without admitting a replacement subject.
        let checked = crate::check(SOURCE, "oracle-auth.spx").unwrap();
        let diagnostics = crate::hir::resolve(&checked).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-H006");
        assert_eq!(
            diagnostics[0].message,
            "oracle invocation authority refused a type probe"
        );
        assert_eq!(authority(&program, function), Ok(Some(lease)));
        assert_eq!(
            authority(&other, &other.functions[0]),
            Err(InvocationRefusal::StaleSubject)
        );
        // An expired lease while entered fails closed.
        drop(scope);
        assert!(matches!(
            authority(&program, function),
            Err(InvocationRefusal::Expired(OracleRefusal::Missing))
        ));
        assert_eq!(held.exit(), Ok(()));
        assert_eq!(authority(&program, function), Ok(None));
        // A stale lease can never enter.
        assert!(matches!(
            enter(&program, function, lease).err(),
            Some(InvocationRefusal::Expired(_))
        ));
    }

    #[test]
    fn invocation_is_restored_on_unwind_and_exit_is_balanced() {
        let program = program();
        let function = &program.functions[0];
        let scope = OracleScope::open(1 << 12).unwrap();
        let lease = scope.lease();
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = enter(&program, function, lease).unwrap();
            panic!("unwind through the invocation");
        }));
        assert!(unwound.is_err());
        assert_eq!(authority(&program, function), Ok(None));
        let held = enter(&program, function, lease).unwrap();
        assert_eq!(held.exit(), Ok(()));
        assert_eq!(authority(&program, function), Ok(None));
    }

    #[test]
    fn replay_error_bridge_routes_by_current_invocation_and_keeps_original_bytes() {
        use super::super::{replay_error, replay_error_old};
        let program = program();
        let function = &program.functions[0];
        let old = replay_error_old(function, format!("too many {}s", "path"));
        // No invocation: every message form keeps the original bytes.
        assert_eq!(
            replay_error(function, format_args!("too many {}s", "path")),
            old
        );
        assert_eq!(replay_error(function, format!("too many {}s", "path")), old);
        assert_eq!(replay_error(function, "x"), replay_error_old(function, "x"));
        let scope = OracleScope::open(old.message.len()).unwrap();
        let lease = scope.lease();
        let held = enter(&program, function, lease).unwrap();
        // Formatted arguments are prepaid on the account, exactly.
        assert_eq!(
            replay_error(function, format_args!("too many {}s", "path")),
            old
        );
        assert_eq!(scope.receipt().charged, old.message.len());
        assert!(scope.receipt().physical_failure.is_none());
        // A pre-allocated owned String cannot be prepaid: sticky physical failure.
        let _ = replay_error(function, "x".to_owned());
        assert!(scope.receipt().physical_failure.is_some());
        assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
        // Another function value is a stale subject, never an ACTIVE fallback.
        let clone = function.clone();
        let stale = replay_error(&clone, "x");
        assert_ne!(stale, replay_error_old(&clone, "x"));
        assert_eq!(held.exit(), Ok(()));
    }

    /// Complete ordinary receiver (`replay_error_old` over an owned `format!`)
    /// against the funded receiver for one message: all six fields, exact
    /// account, one byte short, sticky retry and the zero-byte account. Only the
    /// diagnostic receiver is claimed; this says nothing about the whole pair.
    macro_rules! assert_complete_diagnostic_controls {
        ($function:expr, $($message:tt)+) => {{
            use super::super::allocation::{replay_error_funded, ReplayFailure};
            let function = $function;
            let old = super::super::replay_error_old(function, format!($($message)+));
            let bytes = old.message.len();
            let scope = OracleScope::open(bytes).unwrap();
            let ((), observed) = crate::test_allocation_observer::measure(|| {
                let funded = replay_error_funded(scope.lease(), function, format_args!($($message)+))
                    .unwrap();
                assert_eq!(funded, old);
                drop(funded);
            });
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0, "complete diagnostic dropped inside observation");
            assert!(observed.allocated <= scope.receipt().charged);
            assert_eq!(scope.receipt().charged, bytes);
            drop(scope);
            let scope = OracleScope::open(bytes).unwrap();
            let funded = replay_error_funded(scope.lease(), function, format_args!($($message)+))
                .unwrap();
            assert_eq!(funded.code, old.code);
            assert_eq!(funded.severity, old.severity);
            assert_eq!(funded.message, old.message);
            assert_eq!(funded.path, old.path);
            assert_eq!(funded.span, old.span);
            assert_eq!(funded.help, old.help);
            assert_eq!(funded, old);
            assert_eq!(scope.receipt().charged, bytes);
            assert!(scope.receipt().physical_failure.is_none());
            assert!(matches!(
                replay_error_funded(scope.lease(), function, format_args!("x")),
                Err(ReplayFailure::Account(OracleRefusal::Exhausted { .. }))
            ));
            assert!(matches!(
                replay_error_funded(scope.lease(), function, format_args!("x")),
                Err(ReplayFailure::Account(OracleRefusal::Sticky))
            ));
            drop(scope);
            let short = OracleScope::open(bytes - 1).unwrap();
            assert!(matches!(
                replay_error_funded(short.lease(), function, format_args!($($message)+)),
                Err(ReplayFailure::Account(OracleRefusal::Exhausted { .. }))
            ));
            assert_eq!(short.receipt().charged, 0);
            drop(short);
            let zero = OracleScope::open(0).unwrap();
            assert!(matches!(
                replay_error_funded(zero.lease(), function, format_args!($($message)+)),
                Err(ReplayFailure::Account(_))
            ));
            assert_eq!(zero.receipt().charged, 0);
        }};
    }

    #[test]
    fn dynamic_and_static_replay_messages_match_the_complete_ordinary_diagnostic() {
        let program = program();
        let function = &program.functions[0];
        let id = &function.body.id;
        let index = 7usize;
        let storage = crate::cleanup_plan::StorageId::Temporary(id.clone());
        assert_complete_diagnostic_controls!(function, "status source for `{id}` is repeated");
        assert_complete_diagnostic_controls!(
            function,
            "cleanup slot at index {index} has non-contiguous identity or index"
        );
        assert_complete_diagnostic_controls!(
            function,
            "call `{id}` atomically consumes dead argument flag {}",
            index
        );
        assert_complete_diagnostic_controls!(function, "cleanup storage `{storage:?}` is repeated");
        assert_complete_diagnostic_controls!(
            function,
            "cleanup join at block {} has incompatible control states",
            index
        );
        assert_complete_diagnostic_controls!(
            function,
            "resource cleanup slot has generic arguments"
        );
    }

    #[test]
    fn replay_diagnostic_matches_the_original_bytes_and_is_paid_before_construction() {
        let program = program();
        let function = &program.functions[0];
        let old = super::super::replay_error(function, format!("too many {}s", "path"));
        let scope = OracleScope::open(old.message.len()).unwrap();
        let lease = scope.lease();
        let funded = super::super::allocation::replay_error_funded(
            lease,
            function,
            format_args!("too many {}s", "path"),
        )
        .unwrap();
        assert_eq!(funded, old);
        assert_eq!(scope.receipt().charged, old.message.len());
        // The account is exactly spent; the retry is sticky and allocates nothing.
        let retry =
            super::super::allocation::replay_error_funded(lease, function, format_args!("x"));
        assert!(matches!(
            retry,
            Err(super::super::allocation::ReplayFailure::Account(
                OracleRefusal::Exhausted { .. }
            ))
        ));
        let sticky =
            super::super::allocation::replay_error_funded(lease, function, format_args!("x"));
        assert!(matches!(
            sticky,
            Err(super::super::allocation::ReplayFailure::Account(
                OracleRefusal::Sticky
            ))
        ));
        // One byte short refuses with the exact first receipt.
        drop(scope);
        let scope = OracleScope::open(old.message.len() - 1).unwrap();
        let refused = super::super::allocation::replay_error_funded(
            scope.lease(),
            function,
            format_args!("too many {}s", "path"),
        );
        assert!(matches!(
            refused,
            Err(super::super::allocation::ReplayFailure::Account(
                OracleRefusal::Exhausted { .. }
            ))
        ));
        assert_eq!(scope.receipt().charged, 0);
    }
}
