//! The Wasm stage executor's injected-driver result plan: how one bound
//! stage's record/variant result type is broken into admitted scalar/`Bytes`
//! driver leaves so the value itself never crosses the arena boundary. Split
//! out of `wasm_executor.rs` verbatim (module-size cap); see that module's
//! doc comment for the wider design this plan serves.

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, ResolvedFieldDeclaration, ResolvedType, ResolvedTypeDeclarationKind,
};

use crate::agent_lifecycle::stages::invariant;

// ---------------------------------------------------------------------------
// The injected-driver path: the record/variant value never leaves the module.
// ---------------------------------------------------------------------------

/// One admitted projection leaf: what a single driver function returns.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Leaf {
    I64,
    Bool,
    Usize,
    U8,
    Bytes,
}

impl Leaf {
    fn of(ty: &ResolvedType) -> Option<Self> {
        match ty {
            ResolvedType::I64 => Some(Self::I64),
            ResolvedType::Bool => Some(Self::Bool),
            ResolvedType::Usize => Some(Self::Usize),
            ResolvedType::U8 => Some(Self::U8),
            ResolvedType::Bytes => Some(Self::Bytes),
            _ => None,
        }
    }
}

/// How one driver hands its leaf back across the arena boundary.
///
/// `IndexedBytes` exists because of a real, cited language rule, not an
/// executor shortcut: a `Bytes` payload carried by a variant CASE cannot
/// leave a `match own` arm at all. `SPX-T216` ("owned variant match arms
/// must return a Copy i64 or bool value") and `SPX-T258` ("aggregate-valued
/// match arms are outside the executable match profile") both refuse it. So
/// a variant case's `Bytes` leaf is read one byte at a time through an
/// ordinary `i64`-returning call inside the arm -- exactly the idiom
/// `std/bytes/src/bytes.spx::get_or` already uses -- with `-1` for "past the
/// end", a value no byte can take. A record's `Bytes` field has no such
/// restriction and is returned whole.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Projection {
    I64,
    Bool,
    Usize,
    U8,
    OwnedBytes,
    IndexedBytes,
}

impl Projection {
    pub(super) fn signature(self, name: &str) -> String {
        match self {
            Self::I64 => format!("fn {name}() -> i64"),
            Self::Bool => format!("fn {name}() -> bool"),
            Self::Usize => format!("fn {name}() -> usize"),
            // The public owned-data boundary has no `u8` result kind. A
            // source-level helper widens the already-checked byte to its
            // exact 0..=255 `i64` representation before it crosses.
            Self::U8 => format!("fn {name}() -> i64"),
            Self::OwnedBytes => format!("fn {name}() -> Bytes"),
            Self::IndexedBytes => format!("fn {name}(spx_index: i64) -> i64"),
        }
    }
}

/// The internal bounded Core-Wasm profile's maximum `Bytes` payload. It
/// bounds both source-synthesized arguments and one indexed result stream;
/// exceeding it refuses the dispatch rather than truncating a payload. This
/// is crate-private test/implementation vocabulary, not a public API.
pub(in crate::agent_lifecycle) const BYTE_STREAM_CAP: usize = 65_536;

/// The helper functions an `IndexedBytes` projection calls from inside a
/// `match own` arm. They are ordinary checked SPX -- `u8`-to-`i64` and
/// `i64`-to-`usize` widening written as the repository's own `std/bytes`
/// package writes them, because the language admits no cast for either.
pub(super) const BYTE_HELPERS: &str = r#"
@id("wasm.stage.helper.byte-to-i64")
fn spx_wasm_stage_helper_byte_to_i64(byte: u8) -> i64
{
    let mut value = 0;
    let mut probe = 0u8;
    while probe != byte {
        value = value + 1;
        probe = probe + 1u8;
        probe != byte
    }
    value
}

@id("wasm.stage.helper.index")
fn spx_wasm_stage_helper_index(value: i64) -> usize
{
    let mut remaining = value;
    let mut count = 0usize;
    while remaining > 0 {
        remaining = remaining - 1;
        count = count + 1usize;
        remaining > 0
    }
    count
}

@id("wasm.stage.helper.byte-at")
fn spx_wasm_stage_helper_byte_at(payload: own Bytes, index: i64) -> i64
{
    let view = bytes_as_slice(payload);
    match byte_get(view, spx_wasm_stage_helper_index(index)) { Option::Some { value: byte } => spx_wasm_stage_helper_byte_to_i64(byte), Option::None {} => -1, }
}
"#;

pub(super) struct FieldLeaf {
    pub(super) field: DeclarationId,
    pub(super) name: String,
    pub(super) leaf: Leaf,
}

fn field_leaves(fields: &[ResolvedFieldDeclaration]) -> Result<Vec<FieldLeaf>, Diagnostic> {
    if fields.is_empty() {
        return Err(invariant("wasm_executor.result.empty_fields"));
    }
    fields
        .iter()
        .map(|field| {
            Leaf::of(&field.ty)
                .map(|leaf| FieldLeaf {
                    field: field.id.clone(),
                    name: field.name.clone(),
                    leaf,
                })
                .ok_or_else(|| invariant("wasm_executor.result.leaf"))
        })
        .collect()
}

pub(super) struct CasePlan {
    pub(super) case: DeclarationId,
    pub(super) name: String,
    pub(super) fields: Vec<FieldLeaf>,
}

pub(super) enum ResultPlan {
    Record {
        record: DeclarationId,
        fields: Vec<FieldLeaf>,
    },
    Variant {
        variant: DeclarationId,
        name: String,
        cases: Vec<CasePlan>,
    },
}

pub(super) fn nominal_declaration<'a>(
    program: &'a hir::ResolvedProgram,
    ty: &ResolvedType,
) -> Result<&'a hir::ResolvedTypeDeclaration, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(invariant("wasm_executor.result.shape"));
    };
    if !arguments.is_empty() {
        return Err(invariant("wasm_executor.result.generic"));
    }
    program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| invariant("wasm_executor.result.declaration"))
}

impl ResultPlan {
    pub(super) fn derive(
        program: &hir::ResolvedProgram,
        ty: &ResolvedType,
    ) -> Result<Self, Diagnostic> {
        let declaration = nominal_declaration(program, ty)?;
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { fields } => Ok(Self::Record {
                record: declaration.id.clone(),
                fields: field_leaves(fields)?,
            }),
            ResolvedTypeDeclarationKind::Variant { cases } => {
                if cases.is_empty() {
                    return Err(invariant("wasm_executor.result.empty_variant"));
                }
                let mut planned = Vec::with_capacity(cases.len());
                for case in cases {
                    let fields = field_leaves(&case.fields)?;
                    // `match own` may return a Copy `i64` or `bool` arm, but
                    // not `usize`. A variant `usize` projection would make
                    // the synthesized driver fail at source checking, so
                    // refuse it explicitly before any target artifact is
                    // prepared. Record projections do not use `match own`
                    // and retain their full `usize` support.
                    if fields.iter().any(|field| field.leaf == Leaf::Usize) {
                        return Err(invariant("wasm_executor.result.variant_usize"));
                    }
                    planned.push(CasePlan {
                        case: case.id.clone(),
                        name: case.name.clone(),
                        fields,
                    });
                }
                Ok(Self::Variant {
                    variant: declaration.id.clone(),
                    name: declaration.name.clone(),
                    cases: planned,
                })
            }
            _ => Err(invariant("wasm_executor.result.kind")),
        }
    }
}

/// One synthesized driver function: one leaf of the stage's real result.
pub(super) struct Driver {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) projection: Projection,
    pub(super) tail: String,
}

/// Renders every `match own` arm of one variant projection, binding each
/// case's fields to fresh names so no arm can shadow another.
fn variant_arms(
    variant_name: &str,
    cases: &[CasePlan],
    arm: impl Fn(usize, &CasePlan) -> String,
) -> String {
    let mut rendered = String::new();
    for (index, case) in cases.iter().enumerate() {
        let bindings = case
            .fields
            .iter()
            .enumerate()
            .map(|(position, field)| format!("{}: spx_f{position}", field.name))
            .collect::<Vec<_>>()
            .join(", ");
        rendered.push_str(&format!(
            "        {variant_name}::{} {{ {bindings} }} => {},\n",
            case.name,
            arm(index, case)
        ));
    }
    rendered
}

pub(super) fn drivers_for(plan: &ResultPlan) -> Vec<Driver> {
    let mut drivers = Vec::new();
    match plan {
        ResultPlan::Record { fields, .. } => {
            for field in fields {
                drivers.push(Driver {
                    id: format!("wasm.stage.driver.{}", drivers.len()),
                    name: format!("spx_wasm_stage_driver_{}", drivers.len()),
                    projection: match field.leaf {
                        Leaf::I64 => Projection::I64,
                        Leaf::Bool => Projection::Bool,
                        Leaf::Usize => Projection::Usize,
                        Leaf::U8 => Projection::U8,
                        Leaf::Bytes => Projection::OwnedBytes,
                    },
                    tail: match field.leaf {
                        Leaf::U8 => format!(
                            "    spx_wasm_stage_helper_byte_to_i64(spx_call.{})\n",
                            field.name
                        ),
                        _ => format!("    spx_call.{}\n", field.name),
                    },
                });
            }
        }
        ResultPlan::Variant { name, cases, .. } => {
            // Driver 0 is the discriminant: which case the stage actually
            // took. Every later driver reads one field of one case, so the
            // reconstructed variant carries the real case and the real
            // payload, not a guess.
            let tag = variant_arms(name, cases, |index, _| index.to_string());
            drivers.push(Driver {
                id: "wasm.stage.driver.0".to_owned(),
                name: "spx_wasm_stage_driver_0".to_owned(),
                projection: Projection::I64,
                tail: format!("    match own spx_call {{\n{tag}    }}\n"),
            });
            for (case_index, case) in cases.iter().enumerate() {
                for (position, field) in case.fields.iter().enumerate() {
                    let ordinal = drivers.len();
                    let arms = variant_arms(name, cases, |index, _| {
                        match (index == case_index, field.leaf) {
                            (true, Leaf::I64 | Leaf::Bool | Leaf::Usize) => {
                                format!("spx_f{position}")
                            }
                            (true, Leaf::U8) => {
                                format!("spx_wasm_stage_helper_byte_to_i64(spx_f{position})")
                            }
                            (true, Leaf::Bytes) => {
                                format!("spx_wasm_stage_helper_byte_at(spx_f{position}, spx_index)")
                            }
                            // A non-selected arm never contributes to the
                            // decoded value: driver 0 already fixed which case
                            // the stage took. `-1` is the same "past the end"
                            // sentinel the indexed read uses, so a
                            // non-selected byte stream is empty rather than
                            // wrong.
                            (false, Leaf::I64) => "0".to_owned(),
                            (false, Leaf::Bool) => "false".to_owned(),
                            (false, Leaf::Usize) => "0usize".to_owned(),
                            (false, Leaf::U8) => "0".to_owned(),
                            (false, Leaf::Bytes) => "-1".to_owned(),
                        }
                    });
                    drivers.push(Driver {
                        id: format!("wasm.stage.driver.{ordinal}"),
                        name: format!("spx_wasm_stage_driver_{ordinal}"),
                        projection: match field.leaf {
                            Leaf::I64 => Projection::I64,
                            Leaf::Bool => Projection::Bool,
                            Leaf::Usize => Projection::Usize,
                            Leaf::U8 => Projection::U8,
                            Leaf::Bytes => Projection::IndexedBytes,
                        },
                        tail: format!("    match own spx_call {{\n{arms}    }}\n"),
                    });
                }
            }
        }
    }
    drivers
}
