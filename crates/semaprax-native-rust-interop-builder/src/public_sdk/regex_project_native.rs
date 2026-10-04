//! Closed Regex Project lowering. The checked HIR supplies expressions and the
//! canonical cleanup CFG supplies every ownership transfer and finalizer edge.
use super::sdk_error;
use semaprax::cleanup_plan::*;
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::*;
use std::collections::BTreeMap;
use std::fmt::Write;

pub(super) struct Native {
    pub c: String,
    pub header: String,
    pub rust: String,
}
struct Emitter<'a> {
    function: &'a ResolvedFunction,
    expressions: Vec<&'a ResolvedExpr>,
    bindings: BTreeMap<ValueId, (&'a ResolvedExpr, bool)>,
    constructor: &'a DeclarationId,
    matcher: &'a DeclarationId,
    variant: DeclarationId,
    ok: DeclarationId,
    err: DeclarationId,
    payload: DeclarationId,
    lifecycle: DeclarationId,
}

pub(super) fn render(program: &ResolvedProgram, export: &str) -> Result<Native, Diagnostic> {
    semaprax::hir::validate(program)?;
    let imports = program
        .interfaces
        .iter()
        .flat_map(|i| &i.imports)
        .collect::<Vec<_>>();
    let constructor = imports
        .iter()
        .find(|i| i.rust_path.as_deref() == Some("regex_alias::Regex::new"))
        .ok_or_else(|| sdk_error("Regex Project constructor is absent"))?;
    let matcher = imports
        .iter()
        .find(|i| i.rust_path.as_deref() == Some("regex_alias::Regex::is_match"))
        .ok_or_else(|| sdk_error("Regex Project matcher is absent"))?;
    let ResolvedImportResultKind::OwnedResultResourceI64 { resource } = &constructor.result.kind
    else {
        return Err(sdk_error(
            "Regex Project constructor result is outside profile",
        ));
    };
    let declaration = program
        .types
        .iter()
        .find(|d| &d.id == resource)
        .ok_or_else(|| sdk_error("Regex Project resource is absent"))?;
    let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
        return Err(sdk_error("Regex Project owner is not a resource"));
    };
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == export)
        .ok_or_else(|| sdk_error("Regex Project export is absent"))?;
    if !function.params.is_empty()
        || function.return_type != ResolvedType::I64
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
        || !function.effects.is_empty()
        || function.cleanup_plan.slots.len() > 64
        || function.cleanup_plan.blocks.len() > 512
    {
        return Err(sdk_error(
            "Regex Project export requires bounded fn() -> i64 without contracts or effects",
        ));
    }
    let variant_ty = function
        .cleanup_plan
        .slots
        .iter()
        .find_map(|slot| {
            let ResolvedType::Nominal {
                declaration,
                arguments,
            } = &slot.ty
            else {
                return None;
            };
            (arguments
                == &[
                    ResolvedType::Nominal {
                        declaration: resource.clone(),
                        arguments: vec![],
                    },
                    ResolvedType::I64,
                ])
                .then_some(declaration.clone())
        })
        .ok_or_else(|| sdk_error("Regex Project body must retain its owned constructor Result"))?;
    let cases = program
        .declarations
        .variant_cases(&variant_ty)
        .ok_or_else(|| sdk_error("Regex Result cases absent"))?;
    let ok = cases
        .iter()
        .find(|c| c.name == "Ok" && c.fields.len() == 1)
        .ok_or_else(|| sdk_error("Regex Result Ok absent"))?;
    let err = cases
        .iter()
        .find(|c| c.name == "Err" && c.fields.len() == 1)
        .ok_or_else(|| sdk_error("Regex Result Err absent"))?;
    if cases.len() != 2 {
        return Err(sdk_error("Regex Result case inventory changed"));
    }
    let mut emitter = Emitter {
        function,
        expressions: vec![],
        bindings: BTreeMap::new(),
        constructor: &constructor.id,
        matcher: &matcher.id,
        variant: variant_ty,
        ok: ok.id.clone(),
        err: err.id.clone(),
        payload: ok.fields[0].id.clone(),
        lifecycle: drop.id.clone(),
    };
    emitter.collect(&function.body)?;
    for slot in &function.cleanup_plan.slots {
        match &slot.field_liveness_shape {
            semaprax::cleanup::FieldLivenessShape::Leaf { lifecycle, .. }
                if slot.ty == ResolvedType::String && lifecycle.as_str() == semaprax::cleanup::STRING_DROP_LIFECYCLE_ID => {},
            semaprax::cleanup::FieldLivenessShape::Variant {declaration,cases}
                if declaration==&emitter.variant && cases.len()==2 && cases.iter().all(|case| {
                    if case.case==emitter.ok { case.fields.len()==1 && case.fields[0].field==emitter.payload
                        && matches!(&case.fields[0].shape,semaprax::cleanup::FieldLivenessShape::Leaf {lifecycle,..} if lifecycle==&emitter.lifecycle) }
                    else {case.case==emitter.err && case.fields.iter().all(|f|f.shape==semaprax::cleanup::FieldLivenessShape::NoDrop)}
                })=>{},
            _=>return Err(sdk_error("Regex Project cleanup slot is outside the closed Result owner profile")),
        }
    }
    let header=String::from("#include <stdint.h>\n#include <stddef.h>\ntypedef struct {uint64_t context,generation,slot;} spx_owner;\ntypedef struct {uint8_t tag; uint8_t reserved[7]; int64_t error; spx_owner owner;} spx_result;\nint32_t spx_result_owner_new_utf8(uint64_t,const uint8_t*,uint64_t,spx_result*);\nint32_t spx_result_owner_is_match_utf8(uint64_t,spx_owner,const uint8_t*,uint64_t,uint8_t*);\nint32_t spx_result_owner_drop(uint64_t,spx_owner);\n");
    let mut rust = super::owner_borrowed_result::render_regex_result_owner_carrier(
        "regex_alias::Regex",
        "regex_alias::Regex::new",
        "regex_alias::Regex::is_match",
        "regex_alias::Error",
    )?;
    rust.push_str(r#"
unsafe extern "C" {
    fn spx_regex_project_entry(context:u64,out:*mut i64)->i32;
    fn spx_regex_project_borrow_pointer()->usize;
    fn spx_regex_project_borrow_length()->usize;
    fn spx_regex_project_string_constructions()->u64;
    fn spx_regex_project_live_strings()->u64;
}
/// Execute the exact checked Project export. The lexical context never escapes.
pub fn run()->Result<i64,i32> {
    let context=spx_result_owner_context_new(); if context==0{return Err(4)}
    let mut out=0; let status=unsafe{spx_regex_project_entry(context,&mut out)};
    let closed=spx_result_owner_context_close(context);
    if status!=0{Err(status)}else if closed!=0{Err(closed)}else{Ok(out)}
}
pub fn projected_borrow_matches_target()->bool {
    unsafe {spx_regex_project_borrow_pointer()==spx_result_owner_last_input_pointer()
        && spx_regex_project_borrow_length()==spx_result_owner_last_input_length()}
}
pub fn string_constructions()->u64 {unsafe{spx_regex_project_string_constructions()}}
pub fn live_string_count()->u64 {unsafe{spx_regex_project_live_strings()}}
pub fn live_owner_count()->usize {
    CONTEXTS.with(|contexts|contexts.borrow().iter().flat_map(|c|&c.slots).filter(|s|s.value.is_some()).count())
}
/// Repeats the authenticated scalar export without accepting any new foreign
/// input. `operations` is bounded so this measurement seam cannot retain an
/// unbounded amount of work under the caller's authority.
pub const MAX_BATCH_OPERATIONS:usize=4096;
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct BatchMetrics {
    pub operations:usize,
    pub checksum:i64,
    pub borrowed_input_bytes:u64,
    pub adapter_copy_events:usize,
    pub adapter_copied_bytes:u64,
    pub live_owner_count:usize,
    pub live_string_count:u64,
}
pub fn borrowed_input_length()->usize {unsafe{spx_regex_project_borrow_length()}}
pub fn run_batch(operations:usize)->Result<BatchMetrics,i32> {
    if operations==0||operations>MAX_BATCH_OPERATIONS{return Err(4)}
    let copies_before=spx_result_owner_adapter_copies();
    let bytes_before=spx_result_owner_adapter_copied_bytes();
    let mut checksum=0i64;
    let mut borrowed_input_bytes=0u64;
    for _ in 0..operations {
        checksum=checksum.checked_add(run()?).ok_or(4)?;
        if !projected_borrow_matches_target(){return Err(5)}
        borrowed_input_bytes=borrowed_input_bytes.checked_add(u64::try_from(borrowed_input_length()).map_err(|_|4)?).ok_or(4)?;
    }
    let adapter_copy_events=spx_result_owner_adapter_copies().checked_sub(copies_before).ok_or(5)?;
    let adapter_copied_bytes=spx_result_owner_adapter_copied_bytes().checked_sub(bytes_before).ok_or(5)?;
    let metrics=BatchMetrics{operations,checksum,borrowed_input_bytes,adapter_copy_events,adapter_copied_bytes,live_owner_count:live_owner_count(),live_string_count:live_string_count()};
    if metrics.adapter_copy_events!=0||metrics.adapter_copied_bytes!=0||metrics.live_owner_count!=0||metrics.live_string_count!=0{return Err(5)}
    Ok(metrics)
}
"#);
    Ok(Native {
        c: emitter.render()?,
        header,
        rust,
    })
}
impl<'a> Emitter<'a> {
    fn index(&self, id: &ExpressionId) -> Result<usize, Diagnostic> {
        self.expressions
            .iter()
            .position(|e| &e.id == id)
            .ok_or_else(|| sdk_error("Regex cleanup references absent expression"))
    }
    fn eval(&self, e: &ResolvedExpr) -> Result<String, Diagnostic> {
        Ok(format!("eval_{}(context,f)", self.index(&e.id)?))
    }
    fn collect(&mut self, e: &'a ResolvedExpr) -> Result<(), Diagnostic> {
        if self.expressions.len() >= 256 {
            return Err(sdk_error("Regex Project expression budget exceeded"));
        }
        self.expressions.push(e);
        match &e.kind {
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::String(s) if s.len() <= 4096 => {}
            ResolvedExprKind::Place(p) | ResolvedExprKind::BorrowPlace { place: p, .. }
                if p.projections.is_empty() => {}
            ResolvedExprKind::NativeRustImportCall(c)
                if &c.import == self.constructor || &c.import == self.matcher =>
            {
                for a in &c.args {
                    self.collect(a)?
                }
            }
            ResolvedExprKind::Block { statements, tail } => {
                for s in statements {
                    let ResolvedStatement::Let {
                        binding,
                        value,
                        mutable: false,
                        ..
                    } = s
                    else {
                        return Err(sdk_error("Regex Project permits immutable bindings only"));
                    };
                    self.bindings.insert(binding.id.clone(), (value, false));
                    self.collect(value)?;
                }
                self.collect(tail)?;
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect(condition)?;
                self.collect(then_branch)?;
                self.collect(else_branch)?;
            }
            ResolvedExprKind::Match {
                mode: ResolvedMatchMode::Borrow,
                scrutinee,
                arms,
            } if arms.len() == 2 => {
                self.collect(scrutinee)?;
                for a in arms {
                    let ResolvedMatchPattern::Variant {
                        variant,
                        case,
                        fields,
                    } = &a.pattern
                    else {
                        return Err(sdk_error("Regex Project match requires exact Result cases"));
                    };
                    if variant != &self.variant
                        || (case != &self.ok && case != &self.err)
                        || fields.len() != 1
                        || a.guard.is_some()
                    {
                        return Err(sdk_error(
                            "Regex Project match is outside closed borrowed Result profile",
                        ));
                    }
                    self.bindings
                        .insert(fields[0].binding.id.clone(), (scrutinee, case == &self.err));
                    self.collect(&a.value)?;
                }
            }
            _ => {
                return Err(sdk_error(
                    "Regex Project expression is outside the closed borrowed Result profile",
                ))
            }
        }
        Ok(())
    }
    fn slot(&self, p: &CleanupPlace) -> Result<usize, Diagnostic> {
        if !p.projections.is_empty() && p.projections != [self.ok.clone(), self.payload.clone()] {
            return Err(sdk_error("Regex cleanup projection is outside Ok payload"));
        }
        self.function
            .cleanup_plan
            .slots
            .iter()
            .position(|s| s.storage == p.storage)
            .ok_or_else(|| sdk_error("Regex cleanup storage absent"))
    }
    fn string_slot(&self, place: &CleanupPlace) -> Result<bool, Diagnostic> {
        Ok(place.projections.is_empty()
            && self.function.cleanup_plan.slots[self.slot(place)?].ty == ResolvedType::String)
    }
    fn tag(&self, case: &DeclarationId) -> Result<u8, Diagnostic> {
        if case == &self.ok {
            Ok(1)
        } else if case == &self.err {
            Ok(2)
        } else {
            Err(sdk_error("Regex cleanup case absent"))
        }
    }
    fn status(&self, s: &StatusSourceId) -> Result<String, Diagnostic> {
        if s.lane != StatusLane::OperationFailure {
            return Err(sdk_error(
                "Regex Project status lane is outside borrowed profile",
            ));
        }
        Ok(format!(
            "eval_{}(context,f).status",
            self.index(&s.expression)?
        ))
    }
    fn render(&self) -> Result<String, Diagnostic> {
        let mut out=String::from("#include \"regex_project.h\"\n#include <stdlib.h>\n#include <string.h>\n_Static_assert(sizeof(spx_owner)==24,\"owner layout\");\n_Static_assert(sizeof(spx_result)==40,\"result layout\");\ntypedef struct {int64_t scalar;spx_result owner;const uint8_t *text;uint64_t length;int32_t status;uint8_t done;} value;\nstatic _Thread_local uint64_t string_constructions,live_strings;\nuint64_t spx_regex_project_string_constructions(void){return string_constructions;}\nuint64_t spx_regex_project_live_strings(void){return live_strings;}\nstatic _Thread_local uintptr_t last_pointer;static _Thread_local size_t last_length;\nuintptr_t spx_regex_project_borrow_pointer(void){return last_pointer;}\nsize_t spx_regex_project_borrow_length(void){return last_length;}\n");
        writeln!(
            out,
            "typedef struct {{value values[{}];spx_result owners[{}];uint8_t live[{}];uint8_t *strings[{}];}} frame;",
            self.expressions.len(),
            self.function.cleanup_plan.slots.len().max(1),
            self.function.cleanup_plan.slots.len().max(1),
            self.function.cleanup_plan.slots.len().max(1)
        )
        .unwrap();
        for i in 0..self.expressions.len() {
            writeln!(out, "static value eval_{i}(uint64_t context,frame *f);").unwrap();
        }
        for (i, e) in self.expressions.iter().enumerate() {
            writeln!(out,"static value eval_{i}(uint64_t context,frame *f){{if(f->values[{i}].done)return f->values[{i}];value v={{0}};").unwrap();
            match &e.kind {
                ResolvedExprKind::Int(x) => {
                    writeln!(out, "v.scalar=(int64_t)UINT64_C({});", *x as u64).unwrap();
                }
                ResolvedExprKind::Bool(x) => {
                    writeln!(out, "v.scalar={};", u8::from(*x)).unwrap();
                }
                ResolvedExprKind::String(s) => {
                    let bytes = s
                        .as_bytes()
                        .iter()
                        .map(u8::to_string)
                        .chain(std::iter::once("0".to_owned()))
                        .collect::<Vec<_>>()
                        .join(",");
                    writeln!(
                        out,
                        "static const uint8_t text[]={{{bytes}}};v.text=text;v.length={};",
                        s.len()
                    )
                    .unwrap();
                }
                ResolvedExprKind::Place(p) | ResolvedExprKind::BorrowPlace { place: p, .. } => {
                    let (source, error) = self
                        .bindings
                        .get(&p.root)
                        .ok_or_else(|| sdk_error("Regex binding absent"))?;
                    writeln!(out, "v={};", self.eval(source)?).unwrap();
                    if *error {
                        out.push_str("v.scalar=v.owner.error;\n")
                    }
                }
                ResolvedExprKind::NativeRustImportCall(c) => {
                    for (a, e) in c.args.iter().enumerate() {
                        writeln!(
                            out,
                            "value a{a}={};if(a{a}.status)return a{a};",
                            self.eval(e)?
                        )
                        .unwrap();
                    }
                    if &c.import == self.constructor {
                        out.push_str("last_pointer=(uintptr_t)a0.text;last_length=(size_t)a0.length;v.status=spx_result_owner_new_utf8(context,a0.text,a0.length,&v.owner);\n");
                    } else {
                        out.push_str("last_pointer=(uintptr_t)a1.text;last_length=(size_t)a1.length;uint8_t result=0;v.status=spx_result_owner_is_match_utf8(context,a0.owner.owner,a1.text,a1.length,&result);if(!v.status)v.scalar=result;\n");
                    }
                }
                ResolvedExprKind::Block { statements, tail } => {
                    for s in statements {
                        writeln!(out, "v={};if(v.status)return v;", self.eval(s.value())?).unwrap();
                    }
                    writeln!(out, "v={};", self.eval(tail)?).unwrap();
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    writeln!(
                        out,
                        "v={};if(v.status)return v;v=v.scalar?{}:{};",
                        self.eval(condition)?,
                        self.eval(then_branch)?,
                        self.eval(else_branch)?
                    )
                    .unwrap();
                }
                ResolvedExprKind::Match {
                    scrutinee, arms, ..
                } => {
                    writeln!(out, "v={};if(v.status)return v;", self.eval(scrutinee)?).unwrap();
                    for (n, a) in arms.iter().enumerate() {
                        let ResolvedMatchPattern::Variant { case, .. } = &a.pattern else {
                            unreachable!()
                        };
                        writeln!(
                            out,
                            "{}if(v.owner.tag=={})v={};",
                            if n == 0 { "" } else { "else " },
                            self.tag(case)?,
                            self.eval(&a.value)?
                        )
                        .unwrap();
                    }
                    out.push_str("else v.status=7;\n");
                }
                _ => unreachable!("closed collection"),
            }
            writeln!(out, "v.done=1;f->values[{i}]=v;return v;}}").unwrap();
        }
        out.push_str("int32_t spx_regex_project_entry(uint64_t context,int64_t *result){if(!result)return 3;if(live_strings)return 7;string_constructions=0;frame storage={0},*f=&storage;int32_t status=0;value v={0};\n");
        writeln!(out, "goto block_{};", self.function.cleanup_plan.entry.0).unwrap();
        for b in &self.function.cleanup_plan.blocks {
            writeln!(out, "block_{}:;", b.id.0).unwrap();
            for t in &b.transitions {
                match t {
                    CleanupTransition::Initialize { at, destination }
                        if self.string_slot(destination)? =>
                    {
                        let slot = self.slot(destination)?;
                        let value = self.index(at)?;
                        writeln!(out,"v=eval_{value}(context,f);if(v.status||f->live[{slot}])return 7;if(v.length>4096)abort();uint8_t *s{slot}=malloc((size_t)v.length+1);if(!s{slot})abort();memcpy(s{slot},v.text,(size_t)v.length);s{slot}[v.length]=0;f->values[{value}].text=s{slot};f->strings[{slot}]=s{slot};f->live[{slot}]=1;string_constructions++;live_strings++;").unwrap();
                    }
                    CleanupTransition::Transfer {
                        source,
                        destination,
                        ..
                    } if self.string_slot(source)? && self.string_slot(destination)? => {
                        let source = self.slot(source)?;
                        let destination = self.slot(destination)?;
                        writeln!(out,"if(!f->live[{source}]||f->live[{destination}])return 7;f->strings[{destination}]=f->strings[{source}];f->live[{source}]=0;f->live[{destination}]=1;").unwrap();
                    }
                    CleanupTransition::InitializeVariant {
                        at,
                        destination,
                        variant,
                    } if variant == &self.variant => {
                        let s = self.slot(destination)?;
                        writeln!(out,"v=eval_{}(context,f);if(v.status||f->live[{s}]||(v.owner.tag!=1&&v.owner.tag!=2))return 7;f->owners[{s}]=v.owner;f->live[{s}]=1;",self.index(at)?).unwrap();
                    }
                    CleanupTransition::TransferVariant {
                        source,
                        destination,
                        variant,
                        ..
                    } if variant == &self.variant => {
                        let s = self.slot(source)?;
                        let d = self.slot(destination)?;
                        writeln!(out,"if(!f->live[{s}]||f->live[{d}])return 7;f->owners[{d}]=f->owners[{s}];f->live[{s}]=0;f->live[{d}]=1;").unwrap();
                    }
                    CleanupTransition::CallCommit { arguments, .. } if arguments.is_empty() => {}
                    CleanupTransition::SelectFailure { source } => {
                        writeln!(out, "if(!status)status={};", self.status(source)?).unwrap();
                    }
                    _ => {
                        return Err(Diagnostic::io(
                            "SPX-B112",
                            format!("Regex cleanup transition is outside borrowed profile: {t:?}"),
                        ))
                    }
                }
            }
            match &b.terminator {
                CleanupTerminator::Goto(edge) => {
                    writeln!(
                        out,
                        "goto block_{};",
                        self.function.cleanup_plan.edges[edge.0 as usize].to.0
                    )
                    .unwrap();
                }
                CleanupTerminator::Branch(edges) => {
                    for id in edges {
                        let edge = &self.function.cleanup_plan.edges[id.0 as usize];
                        let cond = match &edge.condition {
                            EdgeCondition::BooleanResult(e, yes) => format!(
                                "{}eval_{}(context,f).scalar",
                                if *yes { "" } else { "!" },
                                self.index(e)?
                            ),
                            EdgeCondition::VariantCase {
                                scrutinee,
                                case,
                                matches,
                            } => format!(
                                "eval_{}(context,f).owner.tag{}{}",
                                self.index(scrutinee)?,
                                if *matches { "==" } else { "!=" },
                                self.tag(case)?
                            ),
                            EdgeCondition::StatusZero(s) => format!("!{}", self.status(s)?),
                            EdgeCondition::StatusNonzero(s) => self.status(s)?,
                            _ => return Err(sdk_error("Regex cleanup branch outside profile")),
                        };
                        writeln!(out, "if({cond})goto block_{};", edge.to.0).unwrap();
                    }
                    out.push_str("return 7;\n");
                }
                CleanupTerminator::Exit(id) => {
                    let exit = &self.function.cleanup_plan.exits[id.0 as usize];
                    for action in &exit.finalize_in_order {
                        if action.lifecycle_id.as_str()
                            == semaprax::cleanup::STRING_DROP_LIFECYCLE_ID
                            && self.string_slot(&action.source)?
                            && action.active_case.is_none()
                        {
                            let slot = self.slot(&action.source)?;
                            writeln!(out,"if(f->live[{slot}]){{f->live[{slot}]=0;free(f->strings[{slot}]);live_strings--;}}").unwrap();
                            continue;
                        }
                        if action.lifecycle_id != self.lifecycle
                            || action.source.projections != [self.ok.clone(), self.payload.clone()]
                            || !action.active_case.as_ref().is_some_and(|g| {
                                g.storage == action.source.storage
                                    && g.variant == self.variant
                                    && g.case == self.ok
                            })
                        {
                            return Err(sdk_error(
                                "Regex cleanup finalizer outside authenticated Ok payload",
                            ));
                        }
                        let s = self.slot(&action.source)?;
                        writeln!(out,"if(f->live[{s}]&&f->owners[{s}].tag==1){{f->live[{s}]=0;int32_t dropped=spx_result_owner_drop(context,f->owners[{s}].owner);if(!status)status=dropped;}}").unwrap();
                    }
                    match &exit.continuation {
                        ExitContinuation::Continue(edge) => {
                            writeln!(
                                out,
                                "goto block_{};",
                                self.function.cleanup_plan.edges[edge.0 as usize].to.0
                            )
                            .unwrap();
                        }
                        ExitContinuation::ReturnFailure { .. } => out.push_str("return status;\n"),
                        ExitContinuation::CommitResult {
                            source: CleanupResultSource::Scalar { expression },
                        } => {
                            writeln!(out,"if(status)return status;v=eval_{}(context,f);if(v.status)return v.status;*result=v.scalar;return 0;",self.index(expression)?).unwrap();
                        }
                        _ => {
                            return Err(sdk_error(
                                "Regex cleanup continuation outside scalar profile",
                            ))
                        }
                    }
                }
            }
        }
        out.push_str("}\n");
        Ok(out)
    }
}
