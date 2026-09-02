use super::*;

struct Writer(Vec<u8>);

impl Writer {
    fn new(event_count: u32, scenario: &str, root: &str) -> Self {
        let mut writer = Self(Vec::new());
        writer.0.extend_from_slice(MAGIC);
        writer.u32(VERSION);
        writer.u32(event_count);
        writer.text(scenario);
        writer.text(root);
        writer
    }

    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn text(&mut self, value: &str) {
        self.u32(value.len().try_into().unwrap());
        self.0.extend_from_slice(value.as_bytes());
    }

    fn storage(&mut self, tag: u32, first: &str) {
        self.u32(tag);
        if tag == 1 || tag == 2 {
            self.text(first);
        }
    }

    fn place(&mut self, storage_tag: u32, id: &str, projections: &[&str]) {
        self.storage(storage_tag, id);
        self.u32(projections.len().try_into().unwrap());
        for projection in projections {
            self.text(projection);
        }
    }

    fn status(&mut self, class: u32, retryability: u32) {
        let (domain, code) = match class {
            1 => ("semaprax.contract.v1", 1),
            2 => ("semaprax.arithmetic.v1", 8),
            _ => ("example.domain", 41),
        };
        self.raw_status("semaprax.status.v1", domain, code, class, retryability);
    }

    fn raw_status(&mut self, schema: &str, domain: &str, code: u32, class: u32, retryability: u32) {
        self.text(schema);
        self.text(domain);
        self.u32(code);
        self.u32(class);
        self.u32(retryability);
    }

    fn event_header(&mut self, tag: u32, function: &str, invocation: &[&str]) {
        self.u32(tag);
        self.text(function);
        self.u32(invocation.len().try_into().unwrap());
        for expression in invocation {
            self.text(expression);
        }
    }

    fn success_unit(&mut self) {
        self.u32(1);
        self.u32(3);
    }
}

fn all_event_frame() -> Vec<u8> {
    let mut writer = Writer::new(5, "scenario-🦀", "fn.root");

    writer.event_header(2, "fn.root", &["expr.call"]);
    writer.text("expr.move");
    writer.place(2, "expr.source", &["field.inner"]);
    writer.place(4, "", &[]);

    writer.event_header(6, "fn.root", &[]);
    writer.text("expr.contract");
    writer.u32(2);
    writer.status(1, 1);

    writer.event_header(7, "fn.root", &[]);
    writer.place(1, "value.resource", &[]);
    writer.text("life.resource");
    writer.u32(9);
    writer.u32(1);
    writer.text("import.drop");

    writer.event_header(8, "fn.root", &[]);
    writer.place(1, "value.resource", &[]);
    writer.text("life.resource");
    writer.u32(9);
    writer.u32(0);

    writer.event_header(9, "fn.root", &[]);
    writer.u32(2);
    writer.place(4, "", &[]);

    writer.success_unit();
    writer.0
}

fn select_failure_frame(
    schema: &str,
    domain: &str,
    code: u32,
    class: u32,
    retryability: u32,
) -> Vec<u8> {
    let mut writer = Writer::new(1, "hostile-status", "fn.root");
    writer.event_header(6, "fn.root", &[]);
    writer.text("expr.failed");
    writer.u32(1);
    writer.raw_status(schema, domain, code, class, retryability);
    writer.success_unit();
    writer.0
}

#[test]
fn decodes_every_event_variant_and_nested_shape() {
    let trace = decode(&all_event_frame()).unwrap();
    assert_eq!(trace.scenario_id, "scenario-🦀");
    assert_eq!(trace.root_function_id, "fn.root");
    assert_eq!(trace.events.len(), 5);
    assert!(matches!(
        &trace.events[0].kind,
        WireEventKind::Transfer { at, source, destination }
            if at == "expr.move"
                && source.projections == ["field.inner"]
                && matches!(destination.storage, WireStorage::ProvisionalResult)
    ));
    assert!(matches!(
        &trace.events[1].kind,
        WireEventKind::SelectFailure {
            source: WireStatusSource {
                lane: WireStatusLane::ContractFalse,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        trace.outcome,
        WireOutcome::Success(WireResult::Unit)
    ));
}

#[test]
fn decodes_all_trace_results_and_failure_outcome() {
    let cases = [
        (
            1,
            i64::MIN.to_le_bytes().to_vec(),
            WireResult::I64(i64::MIN),
        ),
        (2, 1_u32.to_le_bytes().to_vec(), WireResult::Bool(true)),
        (3, Vec::new(), WireResult::Unit),
    ];
    for (tag, payload, expected) in cases {
        let mut writer = Writer::new(0, "scenario", "fn.root");
        writer.u32(1);
        writer.u32(tag);
        writer.0.extend_from_slice(&payload);
        assert_eq!(
            decode(&writer.0).unwrap().outcome,
            WireOutcome::Success(expected)
        );
    }

    let mut owned = Writer::new(0, "scenario", "fn.root");
    owned.u32(1);
    owned.u32(4);
    owned.text("resource.Token");
    assert_eq!(
        decode(&owned.0).unwrap().outcome,
        WireOutcome::Success(WireResult::Owned {
            type_id: "resource.Token".into()
        })
    );

    let mut failure = Writer::new(0, "scenario", "fn.root");
    failure.u32(2);
    failure.text("expr.failed");
    failure.u32(1);
    failure.status(2, 1);
    assert!(matches!(
        decode(&failure.0).unwrap().outcome,
        WireOutcome::Failure {
            selected_source: WireStatusSource {
                lane: WireStatusLane::OperationFailure,
                ..
            },
            status: WireStatus {
                class: WireStatusClass::Arithmetic,
                retryability: WireRetryability::False,
                ..
            }
        }
    ));
}

#[test]
fn every_proper_prefix_is_rejected_as_truncated() {
    let frame = all_event_frame();
    for end in 0..frame.len() {
        assert!(
            matches!(
                decode(&frame[..end]),
                Err(WireDecodeError::Truncated { .. })
            ),
            "prefix ending at {end} was not reported as truncated"
        );
    }
}

#[test]
fn rejects_header_version_unknown_tags_and_trailing_bytes() {
    let mut bad_magic = all_event_frame();
    bad_magic[0] ^= 1;
    assert_eq!(decode(&bad_magic), Err(WireDecodeError::BadMagic));

    let mut bad_version = all_event_frame();
    bad_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        decode(&bad_version),
        Err(WireDecodeError::UnsupportedVersion(2))
    );

    let mut unknown_event = Writer::new(1, "s", "f");
    unknown_event.u32(77);
    unknown_event.text("f");
    unknown_event.u32(0);
    unknown_event.success_unit();
    assert_eq!(
        decode(&unknown_event.0),
        Err(WireDecodeError::UnknownTag {
            context: "event",
            tag: 77
        })
    );

    let mut trailing = Writer::new(0, "s", "f");
    trailing.success_unit();
    trailing.0.push(0);
    assert_eq!(
        decode(&trailing.0),
        Err(WireDecodeError::TrailingBytes { count: 1 })
    );
}

#[test]
fn rejects_invalid_utf8_and_nul_identities() {
    let mut invalid_utf8 = Writer::new(0, "s", "f");
    let scenario_offset = 16;
    invalid_utf8.0[scenario_offset + 4] = 0xff;
    assert!(matches!(
        decode(&invalid_utf8.0),
        Err(WireDecodeError::InvalidUtf8 { .. })
    ));

    let mut nul = Writer::new(0, "s", "f\0hostile");
    nul.success_unit();
    assert!(matches!(
        decode(&nul.0),
        Err(WireDecodeError::IdentityContainsNul { .. })
    ));
}

#[test]
fn enforces_frame_event_string_and_collection_limits_before_allocation() {
    let frame = all_event_frame();
    let mut limits = DEFAULT_LIMITS;
    limits.max_frame_bytes = frame.len() - 1;
    assert!(matches!(
        decode_with_limits(&frame, limits),
        Err(WireDecodeError::FrameTooLarge { .. })
    ));

    let mut event_bomb = Writer::new(u32::MAX, "s", "f");
    event_bomb.success_unit();
    let mut limits = DEFAULT_LIMITS;
    limits.max_events = 4;
    assert_eq!(
        decode_with_limits(&event_bomb.0, limits),
        Err(WireDecodeError::EventLimitExceeded {
            actual: u32::MAX as usize,
            maximum: 4
        })
    );

    let mut long_string = Writer::new(0, "five!", "f");
    long_string.success_unit();
    let mut limits = DEFAULT_LIMITS;
    limits.max_string_bytes = 4;
    assert!(matches!(
        decode_with_limits(&long_string.0, limits),
        Err(WireDecodeError::StringTooLarge {
            actual: 5,
            maximum: 4
        })
    ));

    let mut collection_bomb = Writer::new(1, "s", "f");
    collection_bomb.event_header(2, "f", &[]);
    collection_bomb.text("e");
    collection_bomb.storage(4, "");
    collection_bomb.u32(3);
    collection_bomb.success_unit();
    let mut limits = DEFAULT_LIMITS;
    limits.max_collection_items = 2;
    assert!(matches!(
        decode_with_limits(&collection_bomb.0, limits),
        Err(WireDecodeError::CollectionLimitExceeded { maximum: 2 })
    ));
}

#[test]
fn enforces_aggregate_string_budget_and_checked_length_arithmetic() {
    let mut frame = Writer::new(0, "abc", "def");
    frame.success_unit();
    let mut limits = DEFAULT_LIMITS;
    limits.max_total_string_bytes = 5;
    assert_eq!(
        decode_with_limits(&frame.0, limits),
        Err(WireDecodeError::StringBudgetExceeded { maximum: 5 })
    );

    let mut reader = Reader::new(&[], DEFAULT_LIMITS);
    reader.position = usize::MAX;
    assert_eq!(reader.read_exact(1), Err(WireDecodeError::LengthOverflow));

    reader.position = 0;
    reader.string_bytes = usize::MAX;
    reader.bytes = &[1, 0, 0, 0, b'a'];
    assert_eq!(reader.read_text(), Err(WireDecodeError::LengthOverflow));
}

#[test]
fn rejects_noncanonical_boolean_and_nested_tags() {
    let mut boolean = Writer::new(0, "s", "f");
    boolean.u32(1);
    boolean.u32(2);
    boolean.u32(2);
    assert_eq!(
        decode(&boolean.0),
        Err(WireDecodeError::UnknownTag {
            context: "boolean",
            tag: 2
        })
    );

    let mut storage = Writer::new(1, "s", "f");
    storage.event_header(2, "f", &[]);
    storage.text("e");
    storage.u32(99);
    storage.success_unit();
    assert_eq!(
        decode(&storage.0),
        Err(WireDecodeError::UnknownTag {
            context: "storage",
            tag: 99
        })
    );
}

#[test]
fn accepts_only_exact_compiler_owned_status_mappings() {
    for code in 1..=2 {
        decode(&select_failure_frame(
            "semaprax.status.v1",
            "semaprax.contract.v1",
            code,
            1,
            1,
        ))
        .unwrap();
    }
    for code in 1..=8 {
        decode(&select_failure_frame(
            "semaprax.status.v1",
            "semaprax.arithmetic.v1",
            code,
            2,
            1,
        ))
        .unwrap();
    }
}

#[test]
fn rejects_malformed_status_v1_fields() {
    let cases = [
        (
            select_failure_frame("semaprax.status.v2", "semaprax.contract.v1", 1, 1, 1),
            "schema must be semaprax.status.v1",
        ),
        (
            select_failure_frame("semaprax.status.v1", "", 1, 1, 1),
            "domain identity cannot be empty",
        ),
        (
            select_failure_frame("semaprax.status.v1", "bad\0domain", 1, 1, 1),
            "domain identity cannot contain NUL",
        ),
        (
            select_failure_frame("semaprax.status.v1", "semaprax.contract.v1", 0, 1, 1),
            "status code zero is reserved for success",
        ),
    ];
    for (frame, reason) in cases {
        assert_eq!(decode(&frame), Err(WireDecodeError::InvalidStatus(reason)));
    }

    let long_domain = "x".repeat(256);
    assert_eq!(
        decode(&select_failure_frame(
            "semaprax.status.v1",
            &long_domain,
            1,
            1,
            1,
        )),
        Err(WireDecodeError::InvalidStatus(
            "domain identity cannot exceed 255 UTF-8 bytes"
        ))
    );
}

#[test]
fn rejects_forged_compiler_status_mappings_and_external_classes() {
    let cases = [
        (
            "semaprax.arithmetic.v1",
            1,
            1,
            1,
            "contract class requires semaprax.contract.v1 and code 1 or 2",
        ),
        (
            "semaprax.contract.v1",
            1,
            2,
            1,
            "arithmetic class requires semaprax.arithmetic.v1 and a StatusCase code 1 through 8",
        ),
        (
            "semaprax.contract.v1",
            3,
            1,
            1,
            "contract class requires semaprax.contract.v1 and code 1 or 2",
        ),
        (
            "semaprax.arithmetic.v1",
            9,
            2,
            1,
            "arithmetic class requires semaprax.arithmetic.v1 and a StatusCase code 1 through 8",
        ),
        (
            "semaprax.contract.v1",
            1,
            1,
            0,
            "compiler-owned statuses must have retryability false",
        ),
        (
            "semaprax.arithmetic.v1",
            1,
            2,
            2,
            "compiler-owned statuses must have retryability false",
        ),
    ];
    for (domain, code, class, retryability, reason) in cases {
        assert_eq!(
            decode(&select_failure_frame(
                "semaprax.status.v1",
                domain,
                code,
                class,
                retryability,
            )),
            Err(WireDecodeError::InvalidStatus(reason))
        );
    }

    for class in 3..=5 {
        assert_eq!(
            decode(&select_failure_frame(
                "semaprax.status.v1",
                "external.example",
                1,
                class,
                1,
            )),
            Err(WireDecodeError::InvalidStatus(
                "external status classes are outside the current native trace slice"
            ))
        );
    }
}

#[test]
fn rejects_infeasible_counts_before_large_reserves() {
    let mut events = Writer::new(100, "s", "f");
    events.success_unit();
    let mut limits = DEFAULT_LIMITS;
    limits.max_events = 100;
    assert!(matches!(
        decode_with_limits(&events.0, limits),
        Err(WireDecodeError::Truncated { .. })
    ));

    let mut nested = Writer::new(1, "s", "f");
    nested.u32(2);
    nested.text("f");
    nested.u32(100);
    nested.success_unit();
    let mut limits = DEFAULT_LIMITS;
    limits.max_collection_items = 101;
    assert!(matches!(
        decode_with_limits(&nested.0, limits),
        Err(WireDecodeError::Truncated { .. })
    ));
}
