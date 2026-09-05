pub(super) const NATIVE_SCALAR_RUNTIME_C: &str = r#"#include <stdlib.h>

static __attribute__((noreturn, unused)) void spx_runtime_invariant_failure(
    const char *message
) {
    fprintf(stderr, "SEMAPRAX native runtime invariant failure: %s\n", message);
    abort();
}

static __attribute__((unused)) spx_status_token spx_rt_arithmetic_failure(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *operation
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_arithmetic(spx_ctx, code, &token)) {
        spx_runtime_invariant_failure("status arena exhaustion");
    }
    struct spx_status_detail detail = {
        .failure_kind = NULL,
        .failure_function = NULL,
        .failure_expression = NULL,
        .failure_operation = operation,
        .failure_arguments = {0}
    };
    if (!spx_status_attach_detail(spx_ctx, token, detail)) {
        spx_runtime_invariant_failure("arithmetic status detail attachment");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_contract_with_arguments(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *kind,
    const char *function,
    const char *expression,
    const char *arguments
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    bool recorded = code == SPX_STATUS_CONTRACT_REQUIRES_FALSE
        ? spx_status_record_requires_false(spx_ctx, &token)
        : code == SPX_STATUS_CONTRACT_ENSURES_FALSE
            ? spx_status_record_ensures_false(spx_ctx, &token)
            : false;
    if (!recorded) spx_runtime_invariant_failure("status arena exhaustion");
    struct spx_status_detail detail = {
        .failure_kind = kind,
        .failure_function = function,
        .failure_expression = expression,
        .failure_operation = NULL,
        .failure_arguments = {0}
    };
    int arguments_written = snprintf(
        detail.failure_arguments,
        sizeof detail.failure_arguments,
        "%s",
        arguments
    );
    if (arguments_written < 0 ||
        (size_t)arguments_written >= sizeof detail.failure_arguments) {
        spx_runtime_invariant_failure("contract argument detail overflow");
    }
    if (!spx_status_attach_detail(spx_ctx, token, detail)) {
        spx_runtime_invariant_failure("contract status detail attachment");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_contract(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *kind,
    const char *function,
    const char *expression
) {
    return spx_rt_contract_with_arguments(
        spx_ctx, code, kind, function, expression, "none"
    );
}

static __attribute__((unused)) spx_status_token spx_rt_call_depth_failure(
    struct spx_context *spx_ctx
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx,
        "semaprax.runtime.v1",
        UINT32_C(1),
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,
        &token
    )) {
        spx_runtime_invariant_failure("call-depth status arena exhaustion");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_add(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_add_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_sub(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_sub_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_mul(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_mul_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_div(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    if (a == INT64_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_OVERFLOW, "invalid division"
        );
    }
    *result_out = a / b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_rem(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, "invalid remainder"
        );
    }
    if (a == INT64_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_OVERFLOW, "invalid remainder"
        );
    }
    *result_out = a % b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_add_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    int64_t wide = (int64_t)a + (int64_t)b;
    if (wide > INT32_MAX || wide < INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = (int32_t)wide;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_sub_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    int64_t wide = (int64_t)a - (int64_t)b;
    if (wide > INT32_MAX || wide < INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = (int32_t)wide;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_mul_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    int64_t wide = (int64_t)a * (int64_t)b;
    if (wide > INT32_MAX || wide < INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = (int32_t)wide;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_div_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    if (a == INT32_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_OVERFLOW, "invalid division"
        );
    }
    *result_out = a / b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_rem_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, "invalid remainder"
        );
    }
    if (a == INT32_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_OVERFLOW, "invalid remainder"
        );
    }
    *result_out = a % b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_neg_i32(
    struct spx_context *spx_ctx, int32_t value, int32_t *result_out
) {
    if (value == INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW, "negation overflow"
        );
    }
    *result_out = -value;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_neg(
    struct spx_context *spx_ctx, int64_t value, int64_t *result_out
) {
    if (value == INT64_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW, "negation overflow"
        );
    }
    *result_out = -value;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) int spx_public_failure(
    const struct spx_context *spx_ctx,
    spx_status_token token
) {
    const struct spx_normalized_status *status = spx_status_resolve(spx_ctx, token);
    if (status == NULL) {
        fputs("SEMAPRAX native runtime invariant failure: invalid status token\n", stderr);
        return 72;
    }
    const struct spx_status_detail *detail = spx_status_resolve_detail(spx_ctx, token);
    if (status->status_class == SPX_STATUS_CLASS_CONTRACT) {
        if (detail == NULL || detail->failure_kind == NULL ||
            detail->failure_function == NULL || detail->failure_expression == NULL) {
            fputs("SEMAPRAX native runtime invariant failure: missing contract detail\n", stderr);
            return 72;
        }
        fprintf(
            stderr,
            "SEMAPRAX contract failure\n  contract: %s %s in %s\n  arguments: %s\n",
            detail->failure_kind,
            detail->failure_expression,
            detail->failure_function,
            detail->failure_arguments
        );
        return 70;
    }
    if (status->status_class == SPX_STATUS_CLASS_ARITHMETIC) {
        if (detail == NULL || detail->failure_operation == NULL) {
            fputs("SEMAPRAX native runtime invariant failure: missing arithmetic detail\n", stderr);
            return 72;
        }
        fprintf(
            stderr,
            "SEMAPRAX checked arithmetic failure: %s\n",
            detail->failure_operation
        );
        return 71;
    }
    if (strcmp(status->domain_id, "semaprax.runtime.v1") == 0 &&
        status->code == UINT32_C(1)) {
        fprintf(
            stderr,
            "SEMAPRAX runtime failure: call depth exceeded (%u frames)\n",
            (unsigned int)SPX_MAX_CALL_DEPTH
        );
        return 73;
    }
    fprintf(
        stderr,
        "SEMAPRAX operation failure: %s/%u\n",
        status->domain_id,
        status->code
    );
    return 73;
}

"#;
