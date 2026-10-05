use super::super::private::admit_profile;
use super::*;

// Codes come from the runtime's own constructors, so this file adds no source
// occurrence to the build-scanned installed diagnostic catalog.
fn g204_code() -> &'static str {
    g204("profile", PROFILE_SCHEMA).code
}

fn g205_code() -> &'static str {
    g205("profile").code
}

fn g208_code() -> &'static str {
    g208("profile_bytes", 0).code
}

fn counted_profile(source: &str) -> (Result<Profile, Diagnostic>, u64) {
    counting_document_deserializations(|| parse_profile(source))
}

fn rejected_profile(source: &str) -> (&'static str, String, u64) {
    match counted_profile(source) {
        (Ok(_), _) => panic!("profile unexpectedly parsed"),
        (Err(error), parses) => (error.code, error.message, parses),
    }
}

/// Replaces the first model capability list with `depth` nested empty arrays.
fn nested_capability_profile(profile: &str, depth: usize) -> String {
    let nested = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
    let mutated = profile.replacen(
        "\"capabilities\":[\"text\"]",
        &format!("\"capabilities\":[{nested}]"),
        1,
    );
    assert_ne!(mutated, profile);
    mutated
}

#[test]
fn profile_admission_deserializes_its_document_exactly_once() {
    let profile = fixture_profile();
    let (parsed, parses) = counted_profile(&profile);
    let parsed = parsed.unwrap();
    assert_eq!(parses, 1);
    assert_eq!(parsed.source, profile);
    assert_eq!(parsed.digest, digest(PROFILE_DOMAIN, profile.as_bytes()));
    assert_eq!(render_profile(&parsed), profile);

    // The task path shares the same single parsed value.
    let task = fixture_task();
    let (parsed_task, parses) = counting_document_deserializations(|| parse_task(&task));
    assert_eq!(parsed_task.unwrap().source, task);
    assert_eq!(parses, 1);
}

#[test]
fn profile_rejections_keep_their_order_and_parse_at_most_once() {
    let profile = fixture_profile();
    let body = profile.trim_end();
    let schema_member = format!("\"schema\":\"{PROFILE_SCHEMA}\",");
    let reordered = format!(
        "{{{},{}}}\n",
        &body[1 + schema_member.len()..body.len() - 1],
        &schema_member[..schema_member.len() - 1]
    );
    // Framing rejects before any deserialization.
    for hostile in [
        body.to_owned(),
        format!("{profile}\n"),
        profile.replace('\n', "\r\n"),
        format!("\u{feff}{profile}"),
        String::from("\n"),
    ] {
        let (code, _, parses) = rejected_profile(&hostile);
        assert_eq!((code, parses), (g204_code(), 0), "{hostile:?}");
    }
    // Syntax, schema, key order, unknown and duplicate keys reject after one parse.
    for hostile in [
        profile.replacen('{', "{{", 1),
        profile.replacen(PROFILE_SCHEMA, TASK_SCHEMA, 1),
        reordered,
        profile.replacen("\"agent_id\":", "\"extra\":0,\"agent_id\":", 1),
        profile.replacen("\"schema\":", "\"schema\":\"x\",\"schema\":", 1),
        profile.replacen("\"agent_id\":", "\"agent_id\":\"x\",\"agent_id\":", 1),
    ] {
        let (code, _, parses) = rejected_profile(&hostile);
        assert_eq!((code, parses), (g204_code(), 1), "{hostile:?}");
    }
    // Semantic invariants still run after the shared parse.
    let mut semantic = parse_profile(&profile).unwrap();
    semantic.policy.granted_capabilities = vec!["*".to_owned()];
    let (code, _, parses) = rejected_profile(&render_profile(&semantic));
    assert_eq!((code, parses), (g205_code(), 1));
}

#[test]
fn profile_byte_and_depth_boundaries_are_exact_after_sharing_the_parse() {
    let profile = fixture_profile();
    let body = profile.trim_end();

    // Exactly the byte limit is admitted to parsing, then fails canonical re-render.
    let padded = format!("{body}{}\n", " ".repeat(MAX_PROFILE_BYTES - profile.len()));
    assert_eq!(padded.len(), MAX_PROFILE_BYTES);
    let (code, _, parses) = rejected_profile(&padded);
    assert_eq!((code, parses), (g204_code(), 1));
    // One byte more is the first size rejection and is never deserialized.
    let over = format!(
        "{body}{}\n",
        " ".repeat(MAX_PROFILE_BYTES + 1 - profile.len())
    );
    assert_eq!(over.len(), MAX_PROFILE_BYTES + 1);
    let (code, message, parses) = rejected_profile(&over);
    assert_eq!((code, parses), (g208_code(), 0));
    assert_eq!(
        message,
        format!("profile_bytes exceeds {MAX_PROFILE_BYTES}")
    );

    // The base fixture nests the model capability list at depth 5.
    let at_limit = nested_capability_profile(&profile, MAX_JSON_DEPTH - 4);
    assert_eq!(
        json_depth(&serde_json::from_str::<Value>(at_limit.trim_end()).unwrap()),
        MAX_JSON_DEPTH
    );
    let (code, _, parses) = rejected_profile(&at_limit);
    assert_eq!((code, parses), (g204_code(), 1));
    let over_depth = nested_capability_profile(&profile, MAX_JSON_DEPTH - 3);
    assert_eq!(
        json_depth(&serde_json::from_str::<Value>(over_depth.trim_end()).unwrap()),
        MAX_JSON_DEPTH + 1
    );
    let (code, message, parses) = rejected_profile(&over_depth);
    assert_eq!((code, parses), (g208_code(), 1));
    assert_eq!(message, format!("json_depth exceeds {MAX_JSON_DEPTH}"));
}

/// A host that counts every observation; admission must never reach it.
struct ObservedHost(Rc<Cell<u64>>);

impl ObservedHost {
    fn observe(&self) {
        self.0.set(self.0.get() + 1);
    }
}

impl AgentHost for ObservedHost {
    fn policy_epoch(&self) -> u64 {
        self.observe();
        0
    }
    fn elapsed_ms(&self) -> u64 {
        self.observe();
        0
    }
    fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
        self.observe();
        panic!("profile admission requested a boundary probe")
    }
    fn tokenize(&mut self, _: &str, _: &str) -> Option<u64> {
        self.observe();
        None
    }
    fn attempt_provider(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
        _: u64,
        _: &mut ProviderSink,
    ) -> ProviderAttempt {
        self.observe();
        panic!("profile admission attempted a provider")
    }
    fn invoke_tool(&mut self, _: &str, _: &str, _: &str, _: &mut ToolResultSink) -> bool {
        self.observe();
        panic!("profile admission invoked a tool")
    }
}

fn outcome(result: Result<u64, Vec<Diagnostic>>) -> Result<u64, Vec<(&'static str, String)>> {
    result.map_err(|errors| {
        errors
            .into_iter()
            .map(|error| (error.code, error.message))
            .collect()
    })
}

#[test]
fn pure_admission_and_the_constructor_accept_and_reject_the_same_corpus() {
    let profile = fixture_profile();
    let body = profile.trim_end();
    let mutate = |change: &dyn Fn(&mut Profile)| {
        let mut parsed = parse_profile(&profile).unwrap();
        change(&mut parsed);
        render_profile(&parsed)
    };
    let corpus = [
        profile.clone(),
        body.to_owned(),
        profile.replace('\n', "\r\n"),
        profile.replacen('{', "{{", 1),
        profile.replacen(PROFILE_SCHEMA, TASK_SCHEMA, 1),
        nested_capability_profile(&profile, MAX_JSON_DEPTH - 3),
        format!("{body}{}\n", " ".repeat(MAX_PROFILE_BYTES - profile.len())),
        format!(
            "{body}{}\n",
            " ".repeat(MAX_PROFILE_BYTES + 1 - profile.len())
        ),
        mutate(&|p| p.models.push(p.models[0].clone())),
        mutate(&|p| p.models[0].model_id = String::new()),
        mutate(&|p| p.policy.allowed_model_ids = vec!["fake-other".to_owned()]),
        mutate(&|p| p.policy.granted_capabilities = vec!["*".to_owned()]),
        mutate(&|p| p.policy.allowed_tool_ids = vec!["fixture.other".to_owned()]),
        mutate(&|p| p.tools[0].required_capabilities = vec!["tool.read".to_owned(); 2]),
        mutate(&|p| p.limits.max_turns = MAX_TURNS + 1),
        mutate(&|p| p.limits.max_concurrency = 0),
        mutate(&|p| p.limits.max_builder_bytes = MAX_BUILDER_BYTES as u64),
        mutate(&|p| p.limits.max_builder_bytes = MAX_BUILDER_BYTES as u64 + 1),
    ];
    let mut accepted = 0;
    for (index, candidate) in corpus.iter().enumerate() {
        let observations = Rc::new(Cell::new(0));
        let constructed = outcome(
            Agent::new(
                candidate,
                ObservedHost(observations.clone()),
                AgentCancellation::new(),
            )
            .map(|agent| agent.profile_builder_bytes),
        );
        let admitted = outcome(admit_profile(candidate).map(|admitted| admitted.builder_bytes()));
        assert_eq!(constructed, admitted, "corpus entry {index}");
        assert_eq!(
            admit_runtime_v1_profile(candidate).is_ok(),
            admitted.is_ok(),
            "corpus entry {index}"
        );
        assert_eq!(
            observations.get(),
            0,
            "corpus entry {index} observed the host"
        );
        if let Ok(builder_bytes) = admitted {
            accepted += 1;
            // The builder charge still covers the conservative parse bound.
            assert!(builder_bytes >= candidate.len() as u64 * 8 + 4096);
        }
    }
    assert_eq!(accepted, 2);
}
