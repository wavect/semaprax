//! Transactional i64 receiver carrier. The scalar update body computes a
//! candidate; status success is the sole state and result publication point.
use super::*;

pub(super) fn declarations(output: &mut impl COutput) {
    output.push_str("typedef spx_status_token (*spx_mut_i64_entry_v1)(struct spx_context *, int64_t, int64_t, int64_t *);\n");
    output.push_str("typedef struct { spx_mut_i64_entry_v1 entry; int64_t state; bool active; } spx_mut_i64_v1;\n");
    output.push_str(
        r#"static __attribute__((unused)) void spx_mut_i64_check_v1(const spx_mut_i64_v1 *receiver) {
    if (!receiver->entry || receiver->active)
        spx_runtime_invariant_failure("invalid or active mutable receiver");
}
static __attribute__((unused)) spx_status_token spx_mut_i64_invoke_v1(
    struct spx_context *spx_ctx, spx_mut_i64_v1 *receiver,
    int64_t argument, int64_t *result_out
) {
    spx_mut_i64_check_v1(receiver);
    int64_t candidate = INT64_C(0);
    receiver->active = true;
    spx_status_token status = receiver->entry(spx_ctx, receiver->state, argument, &candidate);
    receiver->active = false;
    if (status == SPX_STATUS_SUCCESS) {
        receiver->state = candidate;
        *result_out = candidate;
    }
    return status;
}
"#,
    );
}

pub(super) fn signature(output: &mut impl COutput, symbol: &str) {
    write!(output, "static spx_status_token {symbol}(struct spx_context *spx_ctx, int64_t spx_state, int64_t spx_argument, int64_t *spx_result_out)")
        .expect("writing to a string cannot fail");
}

pub(super) fn thunk(
    output: &mut impl COutput,
    expression: &ResolvedExpr,
    emission: &NativeEmissionContext<'_>,
) -> Result<(), Diagnostic> {
    let execution = FunctionExecutionId::Monomorphic(hir::closure::closure_id(&expression.id));
    let target = emission
        .functions
        .get(&execution)
        .ok_or_else(|| backend_error("mutable closure body is absent from native emission"))?;
    signature(output, &super::closure::thunk_symbol(&expression.id));
    writeln!(
        output,
        " {{ return {}(spx_ctx, spx_state, spx_argument, spx_result_out); }}\n",
        target.symbol
    )
    .expect("writing to a string cannot fail");
    Ok(())
}

pub(super) fn construct<O: COutput>(
    emitter: &mut CEmitter<'_, O>,
    expression: &ResolvedExpr,
) -> Result<CValue, Diagnostic> {
    hir::closure::mutable::validate(emitter.program, expression)?;
    let ResolvedExprKind::Closure { captures, .. } = &expression.kind else {
        unreachable!()
    };
    let state = emitter.emit_expr(&captures[0].value)?;
    let destination = emitter.temporary(&expression.ty)?;
    emitter.line(&format!(
        "{destination} = (spx_mut_i64_v1){{ .entry = {}, .state = {}, .active = false }};",
        super::closure::thunk_symbol(&expression.id),
        state.code
    ));
    Ok(CValue {
        code: destination,
        ty: expression.ty.clone(),
    })
}

pub(super) fn invoke<O: COutput>(
    emitter: &mut CEmitter<'_, O>,
    expression: &ResolvedExpr,
    callable: &ResolvedExpr,
    args: &[ResolvedExpr],
) -> Result<CValue, Diagnostic> {
    hir::function_value::validate_invocation(expression)?;
    let receiver = emitter.emit_expr(callable)?;
    emitter.line(&format!("spx_mut_i64_check_v1(&{});", receiver.code));
    let argument = emitter.emit_expr(&args[0])?;
    let staged = emitter.temporary(&ResolvedType::I64)?;
    emitter.line(&format!("{staged} = {};", argument.code));
    let result = emitter.call_result_temporary(&ResolvedType::I64)?;
    emitter.line(&format!(
        "spx_status = spx_mut_i64_invoke_v1(spx_ctx, &{}, {staged}, &{result});",
        receiver.code
    ));
    emitter.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
    Ok(CValue {
        code: result,
        ty: ResolvedType::I64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    #[test]
    fn mutable_carrier_preserves_failed_state_and_refuses_active_receiver() {
        let directory =
            std::env::temp_dir().join(format!("spx-mutable-carrier-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut source = String::from(
            r#"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
typedef uint64_t spx_status_token;
#define SPX_STATUS_SUCCESS UINT64_C(0)
struct spx_context { int unused; };
static void spx_runtime_invariant_failure(const char *message) { (void)message; abort(); }
"#,
        );
        declarations(&mut source);
        source.push_str(r#"
static spx_status_token update(struct spx_context *ctx, int64_t state, int64_t arg, int64_t *out) {
    (void)ctx; *out = state + arg;
    return arg < 0 ? UINT64_C(7) : SPX_STATUS_SUCCESS;
}
int main(int argc, char **argv) {
    (void)argv;
    struct spx_context ctx = {0};
    spx_mut_i64_v1 receiver = { .entry = update, .state = 10, .active = false };
    int64_t out = 999;
    if (argc > 1) { receiver.active = true; spx_mut_i64_invoke_v1(&ctx, &receiver, 1, &out); return 80; }
    if (spx_mut_i64_invoke_v1(&ctx, &receiver, 2, &out) || receiver.state != 12 || out != 12) return 1;
    out = 999;
    if (spx_mut_i64_invoke_v1(&ctx, &receiver, -4, &out) != 7 || receiver.state != 12 || out != 999 || receiver.active) return 2;
    if (spx_mut_i64_invoke_v1(&ctx, &receiver, 3, &out) || receiver.state != 15 || out != 15) return 3;
    return 0;
}
"#);
        let input = directory.join("carrier.c");
        let binary = directory.join("carrier");
        std::fs::write(&input, source).unwrap();
        for optimization in ["-O0", "-O2"] {
            let compiled =
                Command::new(std::env::var_os("CLANG").unwrap_or_else(|| "clang".into()))
                    .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
                    .arg(&input)
                    .arg("-o")
                    .arg(&binary)
                    .output()
                    .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            assert!(Command::new(&binary).status().unwrap().success());
            let rejected = Command::new(&binary).arg("active").output().unwrap();
            assert!(!rejected.status.success());
            assert_ne!(
                rejected.status.code(),
                Some(80),
                "active invocation reached its body"
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
