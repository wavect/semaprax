//! Native lowering for String Collections v1 (`docs/STRING-COLLECTIONS-V1.md`).
//!
//! A `Map<string, i64>` is one heap carrier, `spx_map_v1 *`, holding its
//! entries sorted by ascending bytewise key order. Keys are owned
//! length-delimited strings the map copies on insertion, so every key operand
//! stays borrowed. `map_add` and `map_set` receive the map through the call's
//! canonical argument epoch and hand the same carrier back as their result;
//! a failed call leaves the staged carrier untouched for the epilogue to
//! release. Fallible helpers write their out-parameter only on success.

use super::{backend_error, CEmitter, COutput, CValue};
use crate::diagnostic::Diagnostic;
use crate::hir::{ExpressionId, ResolvedType};
use crate::string_ops::StringOp;

/// Whether a value owns String text: a `string`, or a String Collections v1
/// map, whose keys are length-delimited Strings.
pub(super) fn owns_text(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::String | ResolvedType::StringMap) || crate::map_ops::is_typed_collection(ty)
}

/// Emit the map helpers when a length-delimited String profile reaches them;
/// every other program keeps its exact committed bytes.
pub(super) fn emit_runtime(
    output: &mut impl super::COutput,
    program: &crate::hir::ResolvedProgram,
    strings: super::output_profile::StringRuntimeSelection,
) {
    if strings.length_delimited && program_uses_collections(program, strings.include_instances) {
        output.push_str(RUNTIME_C);
        if crate::string_ops::program_uses_operation(program, StringOp::MapRemove) || crate::map_ops::resolved_program_uses(program) {
            output.push_str(REMOVE_RUNTIME_C);
        }
    }
}

/// Whether any resolved body or contract reaches String Collections v1: a
/// collection call or a `Map<string, i64>` value.
pub(super) fn program_uses_collections(
    program: &crate::hir::ResolvedProgram,
    include_instances: bool,
) -> bool {
    let mut pending = Vec::new();
    if program.types.iter().any(|declaration|match &declaration.kind {
        crate::hir::ResolvedTypeDeclarationKind::Record{fields}|crate::hir::ResolvedTypeDeclarationKind::Class{fields,..}=>fields.iter().any(|field|field.ty==ResolvedType::StringMap),_=>false,
    }){return true;}
    for function in super::string_runtime_functions(program, include_instances) {
        if function.return_type==ResolvedType::StringMap || function.params.iter().any(|param|param.ty==ResolvedType::StringMap){return true;}
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if expression.ty == ResolvedType::StringMap {
            return true;
        }
        if let crate::hir::ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str()).is_some_and(StringOp::is_collection) {
                return true;
            }
        }
        pending.extend(super::function_value::resolved_expr_children(expression));
    }
    false
}

impl<O: COutput> CEmitter<'_, O> {
    /// Lower one String Collections v1 call into `temporary`, whose storage
    /// the caller selected from the operation's result type.
    pub(super) fn emit_collection_op(
        &mut self,
        op: StringOp,
        arguments: &[CValue],
        temporary: &str,
        expression: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        if !self.output_profile.string_runtime().length_delimited {
            return Err(backend_error(format!(
                "String Collections v1 operation `{}` requires a length-delimited native String profile",
                op.name()
            )));
        }
        let argument = |index: usize| arguments[index].code.as_str();
        let fallible = match op {
            StringOp::Compare => {
                self.line(&format!(
                    "{temporary} = spx_string_compare_v1({}, {});",
                    argument(0),
                    argument(1)
                ));
                return Ok(());
            }
            StringOp::MapLen => {
                self.line(&format!("{temporary} = {}->len;", argument(0)));
                return Ok(());
            }
            StringOp::MapHas => {
                self.line(&format!(
                    "{temporary} = spx_map_find_v1({}, {}, NULL);",
                    argument(0),
                    argument(1)
                ));
                return Ok(());
            }
            StringOp::MapGetOr => {
                self.line(&format!(
                    "{temporary} = spx_map_get_or_v1({}, {}, {});",
                    argument(0),
                    argument(1),
                    argument(2)
                ));
                return Ok(());
            }
            StringOp::MapNew => format!("spx_map_new_v1(spx_ctx, {}, &{temporary})", argument(0)),
            StringOp::MapKeyAt => format!(
                "spx_map_key_at_v1(spx_ctx, {}, {}, &{temporary})",
                argument(0),
                argument(1)
            ),
            StringOp::MapValueAt => format!(
                "spx_map_value_at_v1(spx_ctx, {}, {}, &{temporary})",
                argument(0),
                argument(1)
            ),
            StringOp::MapRemove => {
                let plan = self.bytes_plan.ok_or_else(|| backend_error("map removal has no cleanup plan"))?;
                let (source, source_flag, _) = plan.call_argument(expression, 0)?;
                let (source, source_flag) = (source.to_owned(), source_flag.to_owned());
                if source != argument(0) {
                    return Err(backend_error("map removal operand is not its canonical call argument"));
                }
                self.line(&format!("{temporary} = spx_map_remove_v2({source}, {});", argument(1)));
                self.line(&format!("{source_flag} = false;"));
                self.line(&format!("{source} = NULL;"));
                return Ok(());
            }
            StringOp::MapAdd | StringOp::MapSet => {
                let plan = self
                    .bytes_plan
                    .ok_or_else(|| backend_error("map reopen has no cleanup plan"))?;
                let (source, source_flag, _) = plan.call_argument(expression, 0)?;
                let (source, source_flag) = (source.to_owned(), source_flag.to_owned());
                if source != argument(0) {
                    return Err(backend_error(
                        "map reopen operand is not its canonical call argument",
                    ));
                }
                self.line(&format!(
                    "spx_status = spx_map_add_v1(spx_ctx, {source}, {}, {}, {}, &{temporary});",
                    argument(1),
                    argument(2),
                    if op == StringOp::MapSet {
                        "true"
                    } else {
                        "false"
                    }
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                // The call consumed the staged generation; the result slot now
                // owns the same carrier.
                self.line(&format!("{source_flag} = false;"));
                self.line(&format!("{source} = NULL;"));
                return Ok(());
            }
            _ => {
                return Err(backend_error(
                    "operation is not part of String Collections v1",
                ))
            }
        };
        self.line(&format!("spx_status = {fallible};"));
        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        Ok(())
    }
}

/// String Collections v1 helpers over the length-delimited String header.
pub(super) const RUNTIME_C: &str = r#"#define SPX_MAP_STATUS_DOMAIN_V1 "semaprax.map.v1"
#define SPX_MAP_MAX_CAPACITY_V1 UINT64_C(65536)
typedef struct spx_map_entry_v1 { char *key; int64_t value; } spx_map_entry_v1;
typedef struct spx_map_v1 {
    uint64_t len;
    uint64_t capacity;
    uint64_t allocated;
    spx_map_entry_v1 *entries;
} spx_map_v1;
static __attribute__((unused)) spx_status_token spx_map_failure_v1(
    struct spx_context *spx_ctx, uint32_t code
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(spx_ctx, SPX_MAP_STATUS_DOMAIN_V1, code,
            SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token))
        spx_runtime_invariant_failure("map status could not be recorded");
    return token;
}
static __attribute__((unused)) int spx_string_order_v1(const char *a, const char *b) {
    uint64_t a_len = spx_string_length_v10(a), b_len = spx_string_length_v10(b);
    uint64_t shared = a_len < b_len ? a_len : b_len;
    int order = shared == UINT64_C(0) ? 0 : memcmp(a, b, (size_t)shared);
    if (order != 0) return order < 0 ? -1 : 1;
    return a_len < b_len ? -1 : (a_len > b_len ? 1 : 0);
}
static __attribute__((unused)) int64_t spx_string_compare_v1(const char *a, const char *b) {
    return (int64_t)spx_string_order_v1(a, b);
}
static __attribute__((unused)) bool spx_map_find_v1(
    const spx_map_v1 *map, const char *key, uint64_t *index_out
) {
    uint64_t low = UINT64_C(0), high = map->len;
    while (low < high) {
        uint64_t middle = low + (high - low) / UINT64_C(2);
        int order = spx_string_order_v1(map->entries[middle].key, key);
        if (order == 0) {
            if (index_out != NULL) *index_out = middle;
            return true;
        }
        if (order < 0) low = middle + UINT64_C(1); else high = middle;
    }
    if (index_out != NULL) *index_out = low;
    return false;
}
static __attribute__((unused)) spx_status_token spx_map_new_v1(
    struct spx_context *spx_ctx, uint64_t capacity, spx_map_v1 **result_out
) {
    if (capacity > SPX_MAP_MAX_CAPACITY_V1) return spx_map_failure_v1(spx_ctx, UINT32_C(3));
    spx_map_v1 *map = (spx_map_v1 *)malloc(sizeof(spx_map_v1));
    if (map == NULL) spx_runtime_invariant_failure("map allocation failed");
    map->len = UINT64_C(0);
    map->capacity = capacity;
    map->allocated = UINT64_C(0);
    map->entries = NULL;
    *result_out = map;
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_map_add_v1(
    struct spx_context *spx_ctx, spx_map_v1 *map, const char *key, int64_t value,
    bool replace, spx_map_v1 **result_out
) {
    uint64_t index = UINT64_C(0);
    if (spx_map_find_v1(map, key, &index)) {
        int64_t current = map->entries[index].value;
        if (!replace && ((value > INT64_C(0) && current > INT64_MAX - value) ||
                (value < INT64_C(0) && current < INT64_MIN - value)))
            return spx_map_failure_v1(spx_ctx, UINT32_C(4));
        map->entries[index].value = replace ? value : current + value;
        *result_out = map;
        return SPX_STATUS_SUCCESS;
    }
    if (map->len == map->capacity) return spx_map_failure_v1(spx_ctx, UINT32_C(1));
    if (map->len == map->allocated) {
        uint64_t grown = map->allocated == UINT64_C(0) ? UINT64_C(8) : map->allocated * UINT64_C(2);
        if (grown > map->capacity) grown = map->capacity;
        spx_map_entry_v1 *entries = (spx_map_entry_v1 *)malloc(
            (size_t)grown * sizeof(spx_map_entry_v1));
        if (entries == NULL) spx_runtime_invariant_failure("map allocation failed");
        if (map->len != UINT64_C(0))
            memcpy(entries, map->entries, (size_t)map->len * sizeof(spx_map_entry_v1));
        free(map->entries);
        map->entries = entries;
        map->allocated = grown;
    }
    char *copy = spx_string_from_literal(key, spx_string_length_v10(key));
    if (index < map->len)
        memmove(&map->entries[index + UINT64_C(1)], &map->entries[index],
            (size_t)(map->len - index) * sizeof(spx_map_entry_v1));
    map->entries[index].key = copy;
    map->entries[index].value = value;
    map->len += UINT64_C(1);
    *result_out = map;
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) int64_t spx_map_get_or_v1(
    const spx_map_v1 *map, const char *key, int64_t fallback
) {
    uint64_t index = UINT64_C(0);
    return spx_map_find_v1(map, key, &index) ? map->entries[index].value : fallback;
}
static __attribute__((unused)) spx_status_token spx_map_key_at_v1(
    struct spx_context *spx_ctx, const spx_map_v1 *map, uint64_t index, char **result_out
) {
    if (index >= map->len) return spx_map_failure_v1(spx_ctx, UINT32_C(2));
    const char *key = map->entries[index].key;
    *result_out = spx_string_from_literal(key, spx_string_length_v10(key));
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_map_value_at_v1(
    struct spx_context *spx_ctx, const spx_map_v1 *map, uint64_t index, int64_t *result_out
) {
    if (index >= map->len) return spx_map_failure_v1(spx_ctx, UINT32_C(2));
    *result_out = map->entries[index].value;
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) void spx_map_drop_v1(spx_map_v1 *map) {
    if (map == NULL) return;
    for (uint64_t index = UINT64_C(0); index < map->len; ++index)
        spx_string_drop(map->entries[index].key);
    free(map->entries);
    free(map);
}
"#;

/// Additive removal helper; the v1 helper bytes remain frozen.
const REMOVE_RUNTIME_C: &str = r#"
static __attribute__((unused)) spx_map_v1 *spx_map_remove_v2(spx_map_v1 *map, const char *key) {
    uint64_t index = UINT64_C(0);
    if (!spx_map_find_v1(map, key, &index)) return map;
    spx_string_drop(map->entries[index].key);
    if (index + UINT64_C(1) < map->len)
        memmove(map->entries + index, map->entries + index + 1,
            (size_t)(map->len - index - UINT64_C(1)) * sizeof(spx_map_entry_v1));
    map->len -= UINT64_C(1);
    map->entries[map->len].key = NULL;
    map->entries[map->len].value = INT64_C(0);
    return map;
}
"#;
