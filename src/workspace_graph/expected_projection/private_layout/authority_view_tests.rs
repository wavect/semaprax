use super::view::Lookup;
use super::{oracle, view, Frame, PrivateTypes, DEPTH};
use crate::ast::{ModuleUseKind, Program, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::workspace_graph::{index_authored, AuthoredDeclaration, AuthoredKind};
use std::collections::BTreeMap;

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
// Interface, interface import and resource lifecycle identities are held by
// `index_authored` but were never members of the old top-level map.
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

// Foreign declarations whose own identities equal current child entries.
const FOREIGN: &str = r#"module private.foreign;
@id("payload.text") record Forged { @id("forged.n") n:i64, }
@id("extra.host")
fn forged() -> i64
{
    0
}
"#;

fn parse(sources: &[&str]) -> Vec<Program> {
    sources
        .iter()
        .enumerate()
        .map(|(n, source)| crate::parse(source, format!("m{n}.spx")).unwrap())
        .collect()
}
fn codes(errors: &[Diagnostic]) -> Vec<(&str, &str)> {
    errors
        .iter()
        .map(|e| (e.code, e.message.as_ref()))
        .collect()
}
/// Runs only the borrowed proof for the current member caller, so each
/// hostile index reaches the guards after caller membership.
fn proof(
    programs: &[Program],
    index: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<bool, Vec<Diagnostic>> {
    let types = programs.iter().map(|p| p.types.len()).sum::<usize>();
    let indexed = types + programs.iter().map(|p| p.functions.len()).sum::<usize>();
    let mut work = super::WORK;
    view::TopLevel::prove(&programs[0], programs, index, types, indexed, &mut work)
        .map(|proved| proved.is_some())
}
fn refused(
    caller: &Program,
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> bool {
    PrivateTypes::derive(caller, programs, authored).is_err()
}
/// Saved-old and borrowed verdicts and exact root identities must agree.
/// Returns the borrowed path's additional charged proof work on success.
fn differential(
    caller: &Program,
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Option<usize> {
    let saved = oracle::saved_roots(caller, programs);
    let borrowed = PrivateTypes::derive(caller, programs, authored);
    match (saved, borrowed) {
        (Ok((roots, work)), Ok(private)) => {
            assert_eq!(roots.len(), private.roots.len());
            for ((m, id), (n, j)) in roots.iter().zip(&private.roots) {
                assert!(std::ptr::eq(*m, *n) && std::ptr::eq(*id, *j));
            }
            Some(work - private.remaining.get())
        }
        (Err(old), Err(new)) => {
            assert_eq!(codes(&old), codes(&new));
            None
        }
        (old, new) => panic!(
            "saved verdict {:?} diverged from borrowed {:?}",
            old.is_ok(),
            new.is_ok()
        ),
    }
}
// Independent recomputation of the borrowed proof's separately charged scans;
// `held` is the whole-index kind census, absent on the class-method fallback.
fn proof_work(programs: &[Program], census: bool, held: usize) -> usize {
    let types = programs.iter().map(|p| p.types.len()).sum::<usize>();
    let children = programs
        .iter()
        .flat_map(|p| &p.types)
        .map(|t| match &t.kind {
            TypeDeclarationKind::Variant { cases } => cases.len(),
            TypeDeclarationKind::Resource { lifecycles } => lifecycles.len(),
            _ => 0,
        })
        .sum::<usize>();
    let tops = programs
        .iter()
        .map(|p| p.interfaces.len() + p.protocols.len())
        .sum::<usize>();
    programs.len() + types + if census { children + tops + held } else { 0 }
}
fn with_import(programs: &[Program], id: &str, kind: ModuleUseKind, module: &str) -> Vec<Program> {
    let mut changed = programs.to_vec();
    let mut import = changed[0].module_uses[0].clone();
    import.persistent_id = id.into();
    import.kind = kind;
    import.target_module = module.into();
    // Appended after the admitted root, so the optional absence scan selects
    // inference before it reaches this import.
    changed[0].module_uses.push(import);
    changed
}

#[test]
fn borrowed_view_matches_saved_old_roots_and_verdicts_and_charges_only_its_proof() {
    let programs = parse(&[CALLER, PROVIDER, EXTRA]);
    let authored = index_authored(&programs).unwrap();
    let indexed = programs
        .iter()
        .map(|p| p.functions.len() + p.types.len())
        .sum::<usize>();
    assert!(
        authored.len() > indexed,
        "the held index is wider than the old map"
    );
    let (extra, views, legacy) =
        view::observe_legacy_calculated(|| differential(&programs[0], &programs, &authored));
    assert_eq!(extra, Some(proof_work(&programs, true, authored.len())));
    assert_eq!(views, 1);
    let node =
        crate::cleanup_plan::replay_btree_node_upper::<&str, AuthoredDeclaration<'_>>().unwrap();
    assert_eq!(legacy, indexed * node);
    let private = PrivateTypes::derive(&programs[0], &programs, &authored).unwrap();
    assert!(private
        .contains("payload", Some("private.provider"))
        .unwrap());
    assert!(!private.contains("root", Some("private.provider")).unwrap());
    // Callers without type or function imports never reach the proof.
    for caller in &programs[1..] {
        assert_eq!(differential(caller, &programs, &authored), Some(0));
    }
    for alias in [
        "use type @id(\"root\") from private.provider as Outcome;",
        "use type @id(\"payload\") from private.provider as Body;",
    ] {
        let source = CALLER.replacen(
            "use type @id(\"root\") from private.provider as Root;",
            alias,
            1,
        );
        let source = if alias.contains("Outcome") {
            source.replace("Root::", "Outcome::")
        } else {
            source.replacen(
                "module private.caller;",
                "module private.caller;\nuse type @id(\"root\") from private.provider as Root;",
                1,
            )
        };
        let aliased = parse(&[&source, PROVIDER, EXTRA]);
        let index = index_authored(&aliased).unwrap();
        differential(&aliased[0], &aliased, &index);
    }
    // Field, case, case-field, lifecycle, interface and interface-import
    // identities are present in the held index but absent from the old map;
    // a function member imported as a type is refused by both.
    for (id, kind, module) in [
        ("payload.text", ModuleUseKind::Type, "private.provider"),
        ("root.ok", ModuleUseKind::Function, "private.provider"),
        ("root.payload", ModuleUseKind::Type, "private.provider"),
        ("extra.token.drop", ModuleUseKind::Function, "private.extra"),
        ("extra.host", ModuleUseKind::Type, "private.extra"),
        (
            "extra.token.finalize",
            ModuleUseKind::Function,
            "private.extra",
        ),
        ("extra.helper", ModuleUseKind::Type, "private.extra"),
    ] {
        let hostile = with_import(&programs, id, kind, module);
        let index = index_authored(&hostile).unwrap();
        assert!(index.contains_key(id), "{id}");
        assert_eq!(differential(&hostile[0], &hostile, &index), None, "{id}");
    }
}

#[test]
fn class_methods_keep_the_saved_owned_map_with_identical_roots() {
    let programs = parse(&[CALLER, PROVIDER, CLASSES]);
    let authored = index_authored(&programs).unwrap();
    assert_eq!(
        authored.get("classes.counter.get").map(|d| d.kind),
        Some(AuthoredKind::Function)
    );
    let (extra, views, _) =
        view::observe_legacy_calculated(|| differential(&programs[0], &programs, &authored));
    assert_eq!(extra, Some(proof_work(&programs, false, authored.len())));
    assert_eq!(views, 0, "a held class method selects the legacy map");
    let hostile = with_import(
        &programs,
        "classes.counter.get",
        ModuleUseKind::Function,
        "private.classes",
    );
    let index = index_authored(&hostile).unwrap();
    assert_eq!(differential(&hostile[0], &hostile, &index), None);
}

#[test]
fn stale_partial_wider_forged_duplicate_and_foreign_bindings_refuse() {
    let programs = parse(&[CALLER, PROVIDER, EXTRA]);
    let authored = index_authored(&programs).unwrap();
    assert!(!refused(&programs[0], &programs, &authored));
    // Equal text, IDs and paths in a copied source set are not current pointers.
    let copied = programs.clone();
    let stale = index_authored(&copied).unwrap();
    assert!(oracle::saved_roots(&programs[0], &programs).is_ok());
    assert!(refused(&programs[0], &programs, &stale));
    // An index of a wider or narrower source slice is not this slice's authority.
    assert!(oracle::saved_roots(&programs[0], &programs[..2]).is_ok());
    assert!(refused(&programs[0], &programs[..2], &authored));
    let narrow = index_authored(&programs[..2]).unwrap();
    assert!(refused(&programs[0], &programs, &narrow));
    // The caller must be a current member of the bound slice.
    let detached = programs[0].clone();
    assert!(refused(&detached, &programs, &authored));
    // A same-size forgery cannot move or relabel a member entry.
    let mut forged = index_authored(&programs).unwrap();
    forged.get_mut("payload").unwrap().path = programs[0].path.as_str();
    assert!(refused(&programs[0], &programs, &forged));
    let mut forged = index_authored(&programs).unwrap();
    forged.get_mut("read").unwrap().explicit = false;
    assert!(refused(&programs[0], &programs, &forged));
    // A duplicate top-level identity cannot map both members to themselves.
    let mut duplicate = programs.clone();
    duplicate[0].functions[0].stable_id = "payload".into();
    let mut forged = BTreeMap::new();
    for p in &duplicate {
        for f in &p.functions {
            forged.insert(
                f.stable_id.as_str(),
                AuthoredDeclaration {
                    path: &p.path,
                    module: &p.module,
                    explicit: f.explicit_id,
                    kind: AuthoredKind::Function,
                    function: Some(f),
                    ty: None,
                },
            );
        }
        for t in &p.types {
            forged.insert(
                t.stable_id.as_str(),
                AuthoredDeclaration {
                    path: &p.path,
                    module: &p.module,
                    explicit: t.explicit_id,
                    kind: AuthoredKind::Type,
                    function: None,
                    ty: Some(t),
                },
            );
        }
    }
    assert!(oracle::saved_roots(&duplicate[0], &duplicate).is_err());
    assert!(refused(&duplicate[0], &duplicate, &forged));
}

#[test]
fn same_cardinality_child_replacements_and_current_slice_entry_forgeries_refuse_in_the_proof() {
    let programs = parse(&[CALLER, PROVIDER, EXTRA]);
    let foreign = parse(&[FOREIGN]);
    // Declared before every forged index that borrows it as a key.
    let rekey = String::from("read");
    let authored = index_authored(&programs).unwrap();
    // Positive control: the genuine binding proves, and its inner lookup is
    // exactly the old top-level map.
    let types = programs.iter().map(|p| p.types.len()).sum::<usize>();
    let indexed = types + programs.iter().map(|p| p.functions.len()).sum::<usize>();
    let mut work = super::WORK;
    let proved = view::TopLevel::prove(
        &programs[0],
        &programs,
        &authored,
        types,
        indexed,
        &mut work,
    )
    .unwrap()
    .unwrap();
    for id in [
        "payload",
        "root",
        "read",
        "main",
        "extra.helper",
        "extra.token",
    ] {
        assert!(proved.get(id).is_some(), "{id}");
    }
    for id in [
        "payload.text",
        "root.ok",
        "root.payload",
        "extra.token.drop",
        "extra.host",
        "extra.token.finalize",
        "absent",
    ] {
        assert!(proved.get(id).is_none(), "{id}");
    }
    // A child entry replaced by a foreign type or function keyed by its own
    // identity and pointer keeps the total size; the kind census refuses it.
    for entry in [
        AuthoredDeclaration {
            path: &foreign[0].path,
            module: &foreign[0].module,
            explicit: true,
            kind: AuthoredKind::Type,
            function: None,
            ty: Some(&foreign[0].types[0]),
        },
        AuthoredDeclaration {
            path: &foreign[0].path,
            module: &foreign[0].module,
            explicit: true,
            kind: AuthoredKind::Function,
            function: Some(&foreign[0].functions[0]),
            ty: None,
        },
    ] {
        let key = entry.ty.map_or_else(
            || entry.function.unwrap().stable_id.as_str(),
            |ty| ty.stable_id.as_str(),
        );
        let mut forged = index_authored(&programs).unwrap();
        assert!(forged.remove(key).is_some(), "{key} is a held child entry");
        forged.insert(key, entry);
        assert_eq!(forged.len(), authored.len());
        assert!(proof(&programs, &forged).is_err(), "{key}");
        assert!(refused(&programs[0], &programs, &forged), "{key}");
    }
    // Current-slice member entries with a forged namespace, path, identity,
    // kind, declaration pointer or key pointer reach the guards after caller
    // membership and refuse there.
    for case in 0..9 {
        let mut forged = index_authored(&programs).unwrap();
        match case {
            0 => forged.get_mut("payload").unwrap().module = programs[0].module.as_str(),
            1 => forged.get_mut("read").unwrap().path = programs[2].path.as_str(),
            2 => forged.get_mut("root").unwrap().explicit = false,
            3 => forged.get_mut("read").unwrap().kind = AuthoredKind::Type,
            4 => forged.get_mut("read").unwrap().function = Some(&programs[0].functions[0]),
            5 => forged.get_mut("payload").unwrap().ty = Some(&programs[1].types[1]),
            6 => forged.get_mut("read").unwrap().ty = Some(&programs[1].types[0]),
            7 => {
                let entry = forged.remove("read").unwrap();
                forged.insert(rekey.as_str(), entry);
            }
            _ => {
                assert!(forged.remove("main").is_some());
            }
        }
        assert!(proof(&programs, &forged).is_err(), "case {case}");
        assert!(refused(&programs[0], &programs, &forged), "case {case}");
    }
}

#[test]
fn borrowed_authority_retains_only_frames_and_roots_with_exact_one_short_and_drop() {
    for (sources, borrowed) in [
        (&[CALLER, PROVIDER, EXTRA][..], true),
        (&[CALLER, PROVIDER, CLASSES][..], false),
    ] {
        let programs = parse(sources);
        let authored = index_authored(&programs).unwrap();
        let types = programs.iter().map(|p| p.types.len()).sum::<usize>();
        let indexed = types + programs.iter().map(|p| p.functions.len()).sum::<usize>();
        let node = crate::cleanup_plan::replay_btree_node_upper::<&str, AuthoredDeclaration<'_>>()
            .unwrap();
        let frames = DEPTH * std::mem::size_of::<Frame<'_>>();
        let roots = types * std::mem::size_of::<(&str, &str)>();
        let legacy = if borrowed { 0 } else { indexed * node };
        let run = || PrivateTypes::derive(&programs[0], &programs, &authored).map(drop);
        let (answer, overflow, used) = crate::bounded_output::with_limit_usage(usize::MAX, run);
        answer.unwrap();
        assert!(!overflow);
        assert_eq!(used, legacy + frames + roots);
        let (answer, overflow, consumed) = crate::bounded_output::with_limit_usage(used, run);
        answer.unwrap();
        assert!(!overflow);
        assert_eq!(consumed, used);
        let (answer, overflow, _) = crate::bounded_output::with_limit_usage(used - 1, run);
        assert!(answer.is_err());
        assert!(overflow);
        if !borrowed {
            continue;
        }
        let (private, observed) = crate::test_allocation_observer::measure(|| {
            PrivateTypes::derive(&programs[0], &programs, &authored).unwrap()
        });
        assert_eq!(observed.allocations, 2, "frame and root backing only");
        assert_eq!(observed.reallocations, 0);
        assert_eq!(observed.allocated, frames + roots);
        assert_eq!(observed.live, roots, "frames are released before return");
        assert!(!observed.invalid);
        // Lookups after derivation reuse the retained roots without backing.
        let (found, queried) = crate::test_allocation_observer::measure(|| {
            private.contains("payload", Some("private.provider"))
        });
        assert!(found.unwrap());
        assert_eq!(queried.allocations, 0);
        drop(private);
        let ((), dropped) = crate::test_allocation_observer::measure(|| {
            drop(PrivateTypes::derive(&programs[0], &programs, &authored).unwrap())
        });
        assert_eq!(dropped.allocated, frames + roots);
        assert_eq!(dropped.freed, dropped.allocated);
        assert_eq!(dropped.live, 0);
        let (saved, old) = crate::test_allocation_observer::measure(|| {
            oracle::saved_roots(&programs[0], &programs).map(drop)
        });
        saved.unwrap();
        assert!(old.allocations > observed.allocations, "saved map nodes");
        assert!(old.allocated > observed.allocated);
        assert_eq!(old.live, 0);
    }
}
