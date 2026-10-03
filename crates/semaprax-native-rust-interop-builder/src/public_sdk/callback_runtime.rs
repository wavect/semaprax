//! Safe generated runtime for a private C-compatible callback body.
//! Rust closure values are constructed normally; no unstable Fn trait impls,
//! Rust trait-object layout assumptions, unsafe Send/Sync, or raw handle API.
const TRAIT_IMPL: &str = "impl $TRAIT for SpxStatefulProxy {\n    type $ERROR=SpxCallbackError;\n    fn $METHOD(&mut self,value:i64)->Result<i64,Self::$ERROR> {\n        self.callback.invoke(value,true,||Ok(()))\n    }\n}";

pub(super) fn render(
    snapshot: &str,
    transition: &str,
    trait_path: &str,
    method: &str,
    error: &str,
) -> String {
    scalar_template()
        .replace("$TRAIT_IMPL", TRAIT_IMPL)
        .replace("$SNAPSHOT", snapshot)
        .replace("$TRANSITION", transition)
        .replace("$TRAIT", trait_path)
        .replace("$METHOD", method)
        .replace("$ERROR", error)
}

pub(super) fn render_iterator(snapshot: &str, transition: &str) -> String {
    scalar_template()
        .replace("$TRAIT_IMPL", "")
        .replace("$SNAPSHOT", snapshot)
        .replace("$TRANSITION", transition)
}

fn scalar_template() -> String {
    include_str!("callback_runtime.template")
        .replace("$SOURCE_DOMAIN_ERROR", "")
        .replace("$HOST_SETUP", "")
        .replace("$HOST_VALUE", "SpxCallbackHost")
        .replace("$HOST", "struct SpxCallbackHost;\nimpl NativeRustImports for SpxCallbackHost {}")
        .replace("$ENV_FIELDS", "")
        .replace("$CAPABILITIES", "&[]")
        .replace("$ENV_INIT", "")
        .replace("$CALL_RESULT", "let output=if transition{bridge.$TRANSITION(capture,value)}else{bridge.$SNAPSHOT(capture,value)}.map_err(spx_callback_error)?;")
}

pub(in crate::public_sdk) fn render_result(
    entry: &str,
    publish: &str,
    trait_path: &str,
    method: &str,
    error: &str,
) -> String {
    let host = r#"struct SpxCallbackHost {pending:std::rc::Rc<std::cell::Cell<Option<(i64,i64)>>>}
impl NativeRustImports for SpxCallbackHost {
    fn $PUBLISH(&mut self,tag:i64,value:i64)->NativeRustImportResult<i64> {
        if !matches!(tag,0|1) || self.pending.get().is_some() {return NativeRustImportResult::HostFailure}
        self.pending.set(Some((tag,value)));NativeRustImportResult::Success(0)
    }
}"#.replace("$PUBLISH", publish);
    include_str!("callback_runtime.template")
        .replace("$TRAIT_IMPL", TRAIT_IMPL)
        .replace("$SOURCE_DOMAIN_ERROR", "SourceDomain(i64),")
        .replace(
            "$HOST_SETUP",
            "let pending=std::rc::Rc::new(std::cell::Cell::new(None));",
        )
        .replace("$HOST_VALUE", "SpxCallbackHost{pending:pending.clone()}")
        .replace("$HOST", &host)
        .replace(
            "$ENV_FIELDS",
            "pending:std::rc::Rc<std::cell::Cell<Option<(i64,i64)>>>,",
        )
        .replace("$ENV_INIT", "pending,")
        .replace("$CAPABILITIES", "&[\"callback.result\"]")
        .replace(
            "$CALL_RESULT",
            r#"environment.pending.set(None);
            let outcome=bridge.$ENTRY(capture,value);
            let publication=environment.pending.take();
            let acknowledgment=outcome.map_err(spx_callback_error)?;
            if acknowledgment!=0 {return Err(SpxCallbackError::AdapterRejected)}
            let output=match publication {
                Some((0,value))=>value,
                Some((1,error))=>return Err(SpxCallbackError::SourceDomain(error)),
                _=>return Err(SpxCallbackError::AdapterRejected),
            };"#,
        )
        .replace("$ENTRY", entry)
        .replace("$TRAIT", trait_path)
        .replace("$METHOD", method)
        .replace("$ERROR", error)
}
