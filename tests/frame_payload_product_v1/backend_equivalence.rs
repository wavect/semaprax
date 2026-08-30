//! Shared original interpreter/native corpus oracles for isolated and linked HIR.
use super::*;

pub(super) fn assert_interpreter_corpus(program: &hir::ResolvedProgram) {
    for (name, frame, valid, error) in corpus_frames() {
        let expected = valid.then(|| frame[8..].to_vec());
        let maybe =
            evaluate_resolved_owned_data(program, "frame.payload-maybe", &frame, DEFAULT_MAX_STEPS)
                .unwrap();
        assert_eq!(maybe.function_id.as_str(), "frame.payload-maybe", "{name}");
        assert_eq!(
            maybe.outcome,
            OwnedDataEvaluationOutcome::Returned(OwnedDataValue::OptionBytes(expected.clone())),
            "{name}"
        );
        assert_eq!(
            maybe.cleanup_events,
            if valid {
                vec![OwnedDataCleanupEvent::CopyOutAndSettleBytes]
            } else {
                Vec::new()
            },
            "{name}"
        );

        let result = evaluate_resolved_owned_data(
            program,
            "frame.payload-result",
            &frame,
            DEFAULT_MAX_STEPS,
        )
        .unwrap();
        assert_eq!(
            result.function_id.as_str(),
            "frame.payload-result",
            "{name}"
        );
        let expected_result = match expected.clone() {
            Some(payload) => Ok(payload),
            None => Err(error),
        };
        assert_eq!(
            result.outcome,
            OwnedDataEvaluationOutcome::Returned(OwnedDataValue::ResultBytesI64(expected_result)),
            "{name}"
        );
        assert_eq!(
            result.cleanup_events,
            if valid {
                vec![OwnedDataCleanupEvent::CopyOutAndSettleBytes]
            } else {
                Vec::new()
            },
            "{name}"
        );

        if let Some(expected) = expected {
            let direct =
                evaluate_resolved_owned_data(program, "frame.payload", &frame, DEFAULT_MAX_STEPS)
                    .unwrap();
            assert_eq!(direct.function_id.as_str(), "frame.payload", "{name}");
            assert_eq!(
                direct.outcome,
                OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(expected)),
                "{name}"
            );
            assert_eq!(
                direct.cleanup_events,
                [OwnedDataCleanupEvent::CopyOutAndSettleBytes],
                "{name}"
            );
        }
    }
}

pub(super) fn assert_native_corpus(provider: &str, label: &str) {
    use std::fmt::Write as _;

    for symbol in [
        "spx_owned_data_call_spx_frame_dot_payload_v1",
        "spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1",
        "spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1",
    ] {
        assert!(provider.contains(symbol));
    }

    let frames = corpus_frames();
    let valid_count = frames.iter().filter(|row| row.2).count();
    let mut declarations = String::new();
    let mut cases = String::new();
    for (index, (name, frame, valid, error)) in frames.iter().enumerate() {
        let pointer = if frame.is_empty() {
            "NULL".to_owned()
        } else {
            writeln!(
                declarations,
                "static const uint8_t case_{index}[]={{ {} }};",
                c_bytes(frame)
            )
            .unwrap();
            format!("case_{index}")
        };
        writeln!(
            cases,
            "if(run_case(context,{pointer},UINT64_C({}),{},INT64_C({}))!=0)return {}; /* {} */",
            frame.len(),
            u8::from(*valid),
            error,
            40 + index,
            name
        )
        .unwrap();
    }
    let probe = format!(
        r#"
{declarations}
static uint32_t drops=UINT32_C(0);
static int copy_drop(spx_context_v1 *context,uint64_t handle,const uint8_t *expected,uint64_t length){{
    uint64_t actual=UINT64_MAX;static uint8_t output[UINT64_C(65528)];
    if(handle==UINT64_C(0)||spx_owned_bytes_len_v1(context,handle,&actual)!=0||actual!=length)return 1;
    if(spx_owned_bytes_copy_v1(context,handle,length==0?NULL:output,length)!=0)return 2;
    if(length!=0&&memcmp(output,expected,(size_t)length)!=0)return 3;
    if(spx_owned_bytes_drop_v1(context,handle)!=0)return 4;
    ++drops;return 0;
}}
static int run_case(spx_context_v1 *context,const uint8_t *frame,uint64_t length,uint8_t valid,int64_t expected_error){{
    uint32_t tag=UINT32_C(99);uint64_t handle=UINT64_C(0);int64_t error=INT64_C(99);
    uint64_t payload_length=valid?length-UINT64_C(8):UINT64_C(0);
    const uint8_t *payload=valid?frame+UINT64_C(8):NULL;
    if(spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1(context,frame,length,&tag,&handle,&error)!=SPX_OWNED_DATA_SUCCESS)return 10;
    if(valid){{if(tag!=UINT32_C(1)||error!=INT64_C(0)||copy_drop(context,handle,payload,payload_length)!=0)return 11;}}
    else if(tag!=UINT32_C(0)||handle!=UINT64_C(0)||error!=INT64_C(0))return 12;
    tag=UINT32_C(99);handle=UINT64_C(0);error=INT64_C(99);
    if(spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1(context,frame,length,&tag,&handle,&error)!=SPX_OWNED_DATA_SUCCESS)return 13;
    if(valid){{if(tag!=UINT32_C(0)||error!=INT64_C(0)||copy_drop(context,handle,payload,payload_length)!=0)return 14;}}
    else if(tag!=UINT32_C(1)||handle!=UINT64_C(0)||error!=expected_error)return 15;
    if(valid){{
        tag=UINT32_C(99);handle=UINT64_C(0);error=INT64_C(99);
        if(spx_owned_data_call_spx_frame_dot_payload_v1(context,frame,length,&tag,&handle,&error)!=SPX_OWNED_DATA_SUCCESS)return 16;
        if(tag!=UINT32_C(0)||error!=INT64_C(0)||copy_drop(context,handle,payload,payload_length)!=0)return 17;
    }}
    return context->live_slots==UINT32_C(0)?0:18;
}}
int main(void){{
    uint64_t size=spx_owned_data_context_size_v1();void *storage=malloc((size_t)size);
    if(storage==NULL)return 20;
    if(spx_owned_data_context_init_v1(storage,size)!=SPX_OWNED_DATA_SUCCESS)return 21;
    spx_context_v1 *context=(spx_context_v1*)storage;
    {cases}
    if(drops!=UINT32_C({}))return 22;
    if(context->live_slots!=UINT32_C(0))return 23;
    if(spx_owned_data_context_drop_v1(context)!=SPX_OWNED_DATA_SUCCESS)return 24;
    free(storage);return 0;
}}
"#,
        valid_count * 3
    );

    let root = temporary(label);
    fs::create_dir_all(&root).unwrap();
    for optimization in ["-O0", "-O2"] {
        let source = root.join(format!("provider-{optimization}.c"));
        let executable = root.join(format!("provider-{optimization}"));
        fs::write(&source, format!("{provider}\n{probe}")).unwrap();
        let compile = Command::new("clang")
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{optimization} compile stderr={}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let execute = Command::new(&executable).output().unwrap();
        assert!(
            execute.status.success(),
            "{optimization} status={:?} stdout={} stderr={}",
            execute.status.code(),
            String::from_utf8_lossy(&execute.stdout),
            String::from_utf8_lossy(&execute.stderr)
        );
    }
    // Keep the plain-provider loop above unchanged. This additional lane
    // observes actual libc calls made by that same provider, rather than
    // treating successful handle drops alone as deallocation evidence.
    let instrumented = format!(
        "{}\n{provider}\n{}\n{declarations}\nint main(void){{spx_context_v1 *context=fixture_begin();\n{cases}\nfixture_finish(context);return 0;}}\n",
        include_str!("../native_owned_tuple_admission_v1/allocations.c"),
        include_str!("native_settlement.c"),
    );
    for optimization in ["-O0", "-O2"] {
        let source = root.join(format!("settlement-{optimization}.c"));
        let executable = root.join(format!(
            "settlement-{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        fs::write(&source, instrumented.as_bytes()).unwrap();
        let compile = Command::new("clang")
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{optimization} settlement compile stderr={}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let execute = Command::new(&executable).output().unwrap();
        assert!(
            execute.status.success(),
            "{optimization} settlement status={:?} stdout={} stderr={}",
            execute.status.code(),
            String::from_utf8_lossy(&execute.stdout),
            String::from_utf8_lossy(&execute.stderr)
        );
        assert!(execute.stdout.is_empty());
        assert!(execute.stderr.is_empty());
    }
    fs::remove_dir_all(root).unwrap();
}
