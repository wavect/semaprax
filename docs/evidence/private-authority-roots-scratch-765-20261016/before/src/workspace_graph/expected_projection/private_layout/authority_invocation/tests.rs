//! Saved-old roots and exact held backing, not a cached HIR signature verdict.
use super::{Diagnostic, Frame, Invocation, Lookup, Program, DEPTH};
use crate::workspace_graph::index_authored;
const PROVIDER: &str = r#"module private.provider;
@id("payload") record Payload { @id("payload.text") text:string, }
@id("root") variant Root {
 @id("root.ok") Ok { @id("root.payload") payload:Payload, },
 @id("root.no") No { @id("root.code") code:i64, @id("root.offset") offset:usize, @id("root.field") field:i64, },
}
@id("read") fn read(input:borrow Slice<u8>)->Root {Root::No {code:7,offset:0usize,field:0}}
"#;
const CALLER: &str = r#"module private.caller;
use type @id("root") from private.provider as Root;
use function @id("read") from private.provider as read;
@id("main") fn main()->i64 {let bytes=[0u8];let value=read(array_as_slice(bytes));match own value {Root::No {code,offset,field}=>code,Root::Ok {payload}=>0,}}
"#;
const EXTRA: &str = r#"module private.extra;
@id("extra.token")
resource Token {
    @id("extra.token.drop")
    drop import "extra.token.finalize";
}
@id("extra.host")
interface TokenHost
    permits { extra.token.release }
{
    @id("extra.token.finalize")
    import fn finalize(token: own Token) -> unit
        effects { extra.token.release }
        failure infallible
        consumes token always;
}
@id("extra.helper")
fn helper() -> i64
{
    0
}
"#;
const CLASSES: &str = r#"module private.classes;
@id("classes.counter")
class Counter {
    @id("classes.counter.value")
    value: i64,

    @id("classes.counter.get")
    fn get(self: Counter) -> i64
{
        self.value
    }
}
@id("classes.helper")
fn helper() -> i64 { 0 }
"#;
fn programs(classes: bool) -> Vec<Program> {
    [CALLER, PROVIDER, if classes { CLASSES } else { EXTRA }]
        .iter()
        .enumerate()
        .map(|(n, s)| crate::parse(s, format!("authority-{n}.spx")).unwrap())
        .collect()
}
#[test]
fn one_invocation_preserves_old_authority_universe_and_current_caller_checks() {
    for classes in [false, true] {
        let programs = programs(classes);
        let authored = index_authored(&programs).unwrap();
        let owner = Invocation::new(&programs, &authored);
        let (saved, _) = super::super::oracle::saved_roots(&programs[0], &programs).unwrap();
        let mut first_work = None;
        for _ in 0..8 {
            let private = owner
                .derive_bound(&programs[0], &programs, &authored)
                .unwrap();
            assert_eq!(private.roots, saved);
            for (actual, old) in private.roots.iter().zip(&saved) {
                assert!(std::ptr::eq(actual.0, old.0) && std::ptr::eq(actual.1, old.1));
            }
            assert!(private
                .contains("payload", Some("private.provider"))
                .unwrap());
            // The independent callback runs afresh for each current HIR query.
            let mut calls = 0;
            private
                .validate_signatures(|_, _| {
                    calls += 1;
                    true
                })
                .unwrap();
            assert_eq!(calls, saved.len());
            if let Some(first) = first_work {
                assert!(private.remaining.get() >= first)
            } else {
                first_work = Some(private.remaining.get())
            }
            assert!(owner.state.borrow().frames.is_empty());
        }
        let state = owner.state.borrow();
        let held = state.held.as_ref().unwrap();
        for absent in [
            "payload.text",
            "extra.host",
            "extra.token.finalize",
            "classes.counter.get",
        ] {
            assert!(
                held.get(absent).is_none(),
                "old top-level universe: {absent}"
            );
        }
        if classes {
            for ty in &programs[2].types {
                if let crate::ast::TypeDeclarationKind::Class { methods, .. } = &ty.kind {
                    for method in methods {
                        assert!(held.get(&method.stable_id).is_none());
                    }
                }
            }
        }
        drop(state);
        let foreign = programs[0].clone();
        assert!(owner.derive(&foreign).is_err());
        assert!(owner.state.borrow().failed);
        assert!(owner.state.borrow().frames.is_empty());
        assert!(owner.derive(&programs[0]).is_err());
    }
}
#[test]
fn foreign_same_cardinality_authority_and_map_binding_fail_stickily() {
    let programs = programs(false);
    let authored = index_authored(&programs).unwrap();
    for mutation in 0..4 {
        let mut wrong = index_authored(&programs).unwrap();
        let entry = wrong.get_mut("payload").unwrap();
        match mutation {
            0 => entry.path = "foreign.spx",
            1 => entry.module = "foreign",
            2 => entry.explicit = false,
            _ => entry.kind = crate::workspace_graph::AuthoredKind::Other,
        }
        assert_eq!(wrong.len(), authored.len());
        let owner = Invocation::new(&programs, &wrong);
        assert!(owner.derive(&programs[0]).is_err());
        assert!(owner.derive(&programs[0]).is_err());
        assert!(owner.state.borrow().failed);
        assert!(owner.state.borrow().frames.is_empty());
    }
    let owner = Invocation::new(&programs, &authored);
    let copied_map = index_authored(&programs).unwrap();
    assert!(owner
        .derive_bound(&programs[0], &programs, &copied_map)
        .is_err());
    assert!(owner.derive(&programs[0]).is_err());
}
#[test]
fn held_frames_are_funded_once_and_repeated_roots_keep_exact_short_and_drop_controls() {
    let programs = programs(false);
    let authored = index_authored(&programs).unwrap();
    let count = programs.iter().map(|p| p.types.len()).sum::<usize>();
    let frames = DEPTH * std::mem::size_of::<Frame<'_>>();
    let roots = count * std::mem::size_of::<(&str, &str)>();
    let run = || {
        let owner = Invocation::new(&programs, &authored);
        for _ in 0..16 {
            drop(owner.derive(&programs[0])?);
        }
        Ok::<_, Vec<Diagnostic>>(())
    };
    let ((answer, measured), overflowed, used) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            crate::test_allocation_observer::measure(run)
        });
    answer.unwrap();
    assert!(!overflowed && !measured.invalid);
    assert_eq!(used, frames + 16 * roots);
    assert_eq!(measured.allocated, used);
    assert_eq!(measured.live, 0);
    assert_eq!(measured.freed, measured.allocated);
    assert_eq!(measured.allocations, 17);
    assert_eq!(measured.reallocations, 0);
    let (exact, overflowed, paid) = crate::bounded_output::with_limit_usage(used, run);
    exact.unwrap();
    assert!(!overflowed);
    assert_eq!(paid, used);
    let (short, overflowed, _) = crate::bounded_output::with_limit_usage(used - 1, || {
        let owner = Invocation::new(&programs, &authored);
        for _ in 0..15 {
            drop(owner.derive(&programs[0])?);
        }
        assert!(owner.derive(&programs[0]).is_err());
        assert!(owner.state.borrow().frames.is_empty());
        assert!(owner.state.borrow().failed);
        assert!(owner.derive(&programs[0]).is_err());
        Ok::<_, Vec<Diagnostic>>(())
    });
    short.unwrap();
    assert!(overflowed);
}
