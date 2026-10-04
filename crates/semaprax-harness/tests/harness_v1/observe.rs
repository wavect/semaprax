//! HP-15 observation tests (prefix `hp-hp15`). Selector: `observe::`.
use crate::support::*;
use semaprax_harness::cli::Environment;
use semaprax_harness::observe::*;
use serde_json::Value;
use std::collections::BTreeMap;

const SECRET: &str = "sk-hp15-PLANTED-SECRET-VALUE";

fn nt(v: u64) -> Option<TokenCount> {
    Some(TokenCount::named("tok-a", "fp-a", v))
}

fn step(
    stage: Stage,
    pid: &str,
    before: Option<TokenCount>,
    after: Option<TokenCount>,
    fin: bool,
) -> Observation {
    let mut o = Observation::new(
        "org.test/p",
        "context.repository",
        stage,
        Role::Transform,
        "inv-1",
    );
    o.payload_id = Some(pid.into());
    o.before = before;
    o.after = after;
    o.model_visible = fin;
    o
}

fn incurred(stage: Stage, c: Option<TokenCount>) -> Observation {
    let mut o = Observation::new(
        "org.test/router",
        "decision.evaluate",
        stage,
        Role::Incurred,
        "inv-2",
    );
    o.parent_invocation = Some("inv-1".into());
    o.incurred = c;
    o.cost.hidden_attempts = Some(0);
    o
}

fn observer() -> Observer {
    Observer::new(None, ObserverLimits::default())
}

fn complete(o: &mut Observer) {
    o.declare_host_traffic(HostTraffic {
        observed: 1,
        unobserved: 0,
    });
}

fn pipeline() -> Observer {
    let mut o = observer();
    o.record(step(Stage::ContextSelect, "p1", nt(1000), nt(800), false));
    o.record(step(Stage::Compression, "p1", nt(800), nt(700), true));
    o.record(incurred(Stage::Decision, nt(100)));
    o
}

fn group(r: &Report) -> &Value {
    &r.json["groups"][0]
}

#[test]
fn hp_hp15_lineage_end_to_end_not_sum_of_stage_reductions() {
    let mut o = pipeline();
    complete(&mut o);
    let r = build_report(o.events(), o.dropped(), o.host_traffic());
    let g = group(&r);
    assert_eq!(
        g["end_to_end_reduction"], 300,
        "1000->800->700 is 300 fewer, not 500"
    );
    assert_eq!(g["stage_local"]["context_select"]["reduction"], 200);
    assert_eq!(g["stage_local"]["compression"]["reduction"], 100);
    assert_eq!(
        g["incurred"]["total"], 100,
        "routing request is a separate incurred cost"
    );
    assert_eq!(g["net_savings"], 200);
    assert_eq!(g["paired_payloads"], 1, "no double counting across stages");
    assert_eq!(r.json["whole_task_claim_allowed"], true);
}

#[test]
fn hp_hp15_cache_hit_with_repeated_exposure_counts_twice() {
    let mut o = observer();
    for pid in ["a", "b"] {
        let mut s = step(Stage::Generation, pid, nt(500), nt(500), true);
        s.cache = CacheState::Hit;
        s.after_digest = Some("sha256:same".into());
        o.record(s);
    }
    let r = build_report(o.events(), 0, None);
    let g = group(&r);
    assert_eq!(
        g["exposure"]["model_visible_total"], 1000,
        "identical text sent twice counts twice"
    );
    assert_eq!(g["exposure"]["repeated_identical_exposures"], 1);
    assert_eq!(
        g["end_to_end_reduction"], 0,
        "cache hit is not a token saving"
    );
    assert_eq!(r.json["costs"]["cache_hits"], 2);
}

#[test]
fn hp_hp15_unknown_gateway_retry_usage_stays_unknown() {
    let mut o = pipeline();
    let mut gw = incurred(Stage::Generation, nt(50));
    gw.cost.hidden_attempts = None;
    gw.cost.provider_billed = None;
    gw.outcome = Outcome::Failed;
    o.record(gw);
    complete(&mut o);
    let r = build_report(o.events(), 0, o.host_traffic());
    assert_eq!(r.json["costs"]["provider_billed"]["total"], "unknown");
    assert_eq!(r.json["costs"]["hidden_attempts"]["unknown_events"], 1);
    assert_eq!(r.json["costs"]["failed_attempts"], 1);
    assert_eq!(group(&r)["incurred"]["failed_attempts"], 1);
    assert_eq!(r.json["whole_task_claim_allowed"], false);
    assert!(r.text().contains("unknown"));
}

#[test]
fn hp_hp15_absent_tokenizer_is_missing_not_zero() {
    let m = measure("hello", None);
    assert!(m.count.is_none());
    let mut o = observer();
    o.record(step(Stage::Compression, "p", None, None, true));
    complete(&mut o);
    let r = build_report(o.events(), 0, o.host_traffic());
    assert_eq!(r.json["coverage"]["missing_tokenizer"], 1);
    assert_eq!(r.json["unpaired"][0]["reason"], "missing_baseline");
    assert_eq!(r.json["whole_task_claim_allowed"], false);
    // External helper that cannot provide a tokenizer is refused, not zero.
    let err = ExternalTokenizer::spawn(
        &std::path::PathBuf::from("/nonexistent/helper"),
        &[],
        &BTreeMap::new(),
    );
    assert_eq!(err.err().unwrap().code, "SPX-HPO003");
}

struct FailingSink;
impl Sink for FailingSink {
    fn write_line(&mut self, _: &str) -> Result<(), SinkError> {
        Err(SinkError)
    }
}

/// Stand-in for authoritative dispatch: status, attempts and budget.
fn dispatch(obs: &mut Observer) -> (String, u32, u64) {
    let (mut attempts, mut budget) = (0u32, 1000u64);
    let mut status = "failed";
    for _ in 0..3 {
        attempts += 1;
        budget -= 100;
        obs.record(incurred(Stage::Generation, nt(100)));
        if attempts == 2 {
            status = "ok";
            break;
        }
    }
    (status.into(), attempts, budget)
}

#[test]
fn hp_hp15_observer_failure_leaves_execution_identical() {
    let mut quiet = observer();
    let mut broken = Observer::new(Some(Box::new(FailingSink)), ObserverLimits::default());
    let mut tiny = Observer::new(None, ObserverLimits { max_events: 0 });
    let base = dispatch(&mut quiet);
    assert_eq!(dispatch(&mut broken), base);
    assert_eq!(dispatch(&mut tiny), base);
    assert_eq!(quiet.dropped(), 0);
    assert_eq!(broken.dropped(), 2, "failures are counted, not propagated");
    assert_eq!(tiny.dropped(), 2);
    let r = build_report(broken.events(), broken.dropped(), None);
    assert_eq!(r.json["coverage"]["dropped"], 2);
    assert_eq!(r.json["whole_task_claim_allowed"], false);
}

#[test]
fn hp_hp15_final_envelope_counts_match_tokenizer_independently() {
    let envelope = "tool: ctx\nskill: build — wrapper<retrieved>héllo 日本語 😀</retrieved>";
    let m = measure(envelope, Some(&ByteTokenizer));
    assert_eq!(
        m.count,
        Some(TokenCount::bytes(envelope.as_bytes().len() as u64))
    );
    assert_eq!(m.bytes, envelope.len());
    assert!(m.digest.starts_with("sha256:"));
    // Named tokenizer through the helper protocol, checked against an
    // independent whitespace count.
    let tok = fake_tokenizer();
    let m = measure(envelope, Some(&tok));
    assert_eq!(
        m.count.as_ref().unwrap().value,
        envelope.split_whitespace().count() as u64
    );
    assert_eq!(
        m.count.unwrap().tokenizer,
        TokenizerId::Named {
            name: "fake-words".into(),
            fingerprint: "sha256:fake".into()
        }
    );
}

fn fake_tokenizer() -> ExternalTokenizer {
    let script =
        repo_root().join("crates/semaprax-harness/tests/fixtures/observe/fake_tokenizer.py");
    let env = BTreeMap::from([(
        "PATH".to_string(),
        "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin".to_string(),
    )]);
    ExternalTokenizer::spawn(
        std::path::Path::new("/usr/bin/env"),
        &["python3".into(), script.display().to_string()],
        &env,
    )
    .expect("python3 available")
}

#[test]
fn hp_hp15_utf8_and_json_escaping_boundaries_count_serialized_bytes() {
    let raw = "quote\" back\\ nl\n tab\t é 日 😀";
    let serialized = serde_json::to_string(&Value::String(raw.into())).unwrap();
    assert_ne!(serialized.len(), raw.len());
    let m = measure(&serialized, Some(&ByteTokenizer));
    assert_eq!(m.count.unwrap().value, serialized.len() as u64);
    // Hand-computed: `"quote\" back\\ nl\n tab\t é 日 😀"` with 3 two-char
    // escapes + escaped quote, back, nl, tab: raw bytes + 4 escape extras + 2 quotes.
    assert_eq!(serialized.len(), raw.len() + 4 + 2);
    assert_eq!("é".len(), 2);
    assert_eq!("日".len(), 3);
    assert_eq!("😀".len(), 4);
    assert_eq!(
        measure("é日😀", Some(&ByteTokenizer)).count.unwrap().value,
        9
    );
}

#[test]
fn hp_hp15_negative_net_savings_visible_in_both_views() {
    let mut o = observer();
    o.record(step(Stage::Compression, "p", nt(100), nt(90), true));
    o.record(incurred(Stage::Decision, nt(300)));
    complete(&mut o);
    let r = build_report(o.events(), 0, o.host_traffic());
    assert_eq!(group(&r)["net_savings"], -290);
    let text = r.text();
    assert!(text.contains("-290") && text.contains("NEGATIVE"), "{text}");
    // Also through the CLI in both views.
    let dir = fixture_dir("hp-hp15-neg");
    let file = dir.join("t.jsonl");
    let mut sink_obs = Observer::new(
        Some(Box::new(JsonlFileSink::create(&file, 1 << 20).unwrap())),
        ObserverLimits::default(),
    );
    for e in o.events() {
        sink_obs.record(e.clone());
    }
    sink_obs.declare_host_traffic(HostTraffic {
        observed: 1,
        unobserved: 0,
    });
    sink_obs.finish();
    let env = Environment {
        cwd: dir.clone(),
        ..Default::default()
    };
    let j = cli_report(&["t.jsonl".into(), "--json".into()], &env);
    assert_eq!(j.code, 0, "{}", j.stderr);
    let v: Value = serde_json::from_str(&j.stdout).unwrap();
    assert_eq!(v["groups"][0]["net_savings"], -290);
    let h = cli_report(&["t.jsonl".into()], &env);
    assert!(h.stdout.contains("-290"));
}

#[test]
fn hp_hp15_partial_coverage_blocks_whole_task_claim_in_both_views() {
    let mut o = pipeline();
    o.declare_host_traffic(HostTraffic {
        observed: 2,
        unobserved: 3,
    });
    let r = build_report(o.events(), 4, o.host_traffic());
    assert_eq!(r.json["whole_task_claim_allowed"], false);
    assert_eq!(r.json["partial"], true);
    let reasons = r.json["coverage"]["reasons"].to_string();
    assert!(
        reasons.contains("unobserved_host_traffic") && reasons.contains("dropped_observations")
    );
    let t = r.text();
    assert!(
        t.contains("PARTIAL") && t.contains("whole_task_claim_allowed: false"),
        "{t}"
    );
    // Undeclared traffic is not complete either.
    let o = pipeline();
    assert_eq!(
        build_report(o.events(), 0, None).json["whole_task_claim_allowed"],
        false
    );
}

#[test]
fn hp_hp15_byte_and_named_counts_are_never_mixed() {
    assert_eq!(
        TokenCount::bytes(5)
            .checked_sub(&TokenCount::named("t", "f", 3))
            .unwrap_err()
            .code,
        "SPX-HPO004"
    );
    let mut o = observer();
    o.record(step(
        Stage::Compression,
        "mixed",
        Some(TokenCount::bytes(900)),
        nt(500),
        true,
    ));
    o.record(step(
        Stage::Compression,
        "bytes",
        Some(TokenCount::bytes(900)),
        Some(TokenCount::bytes(600)),
        true,
    ));
    o.record(step(Stage::Compression, "named", nt(900), nt(500), true));
    let r = build_report(o.events(), 0, None);
    assert_eq!(
        r.json["groups"].as_array().unwrap().len(),
        2,
        "separate groups, never summed"
    );
    assert_eq!(r.json["unpaired"][0]["reason"], "mixed_tokenizer_kinds");
    let units: Vec<&str> = r.json["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["unit"].as_str().unwrap())
        .collect();
    assert_eq!(units, ["bytes", "model_tokens"]);
    assert_eq!(r.json["whole_task_claim_allowed"], false);
}

#[test]
fn hp_hp15_no_raw_text_retained_in_sink_bytes() {
    let dir = fixture_dir("hp-hp15-secret");
    let file = dir.join("t.jsonl");
    let mut o = Observer::new(
        Some(Box::new(JsonlFileSink::create(&file, 1 << 20).unwrap())),
        ObserverLimits::default(),
    );
    let text = format!("prompt with {SECRET} inside");
    let m = measure(&text, Some(&ByteTokenizer));
    let mut e = step(
        Stage::Generation,
        "p",
        Some(TokenCount::bytes(m.bytes as u64 * 2)),
        m.count,
        true,
    );
    e.after_digest = Some(m.digest);
    o.record(e);
    // Oversized (text-like) field is refused and counted, never stored.
    let mut leak = step(Stage::Generation, "q", None, None, false);
    leak.provider = text.repeat(20);
    o.record(leak);
    o.finish();
    let bytes = std::fs::read_to_string(&file).unwrap();
    assert!(!bytes.contains(SECRET) && !bytes.contains("prompt with"));
    assert_eq!(o.dropped(), 1);
    assert!(bytes.contains("sha256:"));
}

#[test]
fn hp_hp15_sinks_are_bounded_and_event_roundtrips() {
    let mut mem = MemorySink::new(1);
    assert!(mem.write_line("a").is_ok() && mem.write_line("b").is_err());
    let dir = fixture_dir("hp-hp15-bound");
    let mut f = JsonlFileSink::create(&dir.join("x.jsonl"), 10).unwrap();
    assert!(f.write_line("12345").is_ok() && f.write_line("123456789").is_err());
    assert!(
        JsonlFileSink::create(&dir.join("x.jsonl"), 10).is_err(),
        "never overwrites"
    );
    let mut e = incurred(Stage::IndexBuild, None);
    e.upstream_model = Some("m-1".into());
    e.warmth = Warmth::Cold;
    e.cost.provider_billed = None;
    let back = Observation::from_json(&e.to_json()).unwrap();
    assert_eq!(back, e);
    assert_eq!(e.to_json()["cost"]["provider_billed"], "unknown");
    assert_eq!(
        incurred(Stage::Decision, None).to_json()["upstream_model"],
        "unknown"
    );
}

#[test]
fn hp_hp15_cold_warm_and_decision_costs_separate_from_billing() {
    let mut o = observer();
    let mut cold = Observation::new(
        "p",
        "context.repository",
        Stage::IndexBuild,
        Role::Local,
        "i",
    );
    cold.warmth = Warmth::Cold;
    cold.latency_ms = 900;
    cold.cost.provider_billed = Some(0);
    cold.cost.local_compute_ms = 850;
    let mut warm = Observation::new(
        "p",
        "context.repository",
        Stage::ContextSelect,
        Role::Local,
        "i",
    );
    warm.warmth = Warmth::Warm;
    warm.latency_ms = 12;
    warm.cost.provider_billed = Some(0);
    o.record(cold);
    o.record(warm);
    o.record(incurred(Stage::RetrievalWrapper, nt(40)));
    let r = build_report(o.events(), 0, None);
    let l = &r.json["costs"]["local"];
    assert_eq!(
        (
            l["cold_load_ms"].as_u64(),
            l["warm_query_ms"].as_u64(),
            l["compute_ms"].as_u64()
        ),
        (Some(900), Some(12), Some(850))
    );
    assert_eq!(r.json["costs"]["extra_retrievals"], 1);
    assert_eq!(
        r.json["costs"]["provider_billed"]["total"], "unknown",
        "incurred request has unknown billing"
    );
}

#[test]
fn hp_hp15_real_helper_without_tiktoken_is_refused_not_zero() {
    let have = std::process::Command::new("/usr/bin/env")
        .args(["python3", "-c", "import tiktoken"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if have {
        return; // provisioned path is covered by the ignored real-tool test
    }
    let script = repo_root().join("scripts/harness_tokenize.py");
    let env = BTreeMap::from([(
        "PATH".to_string(),
        "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin".to_string(),
    )]);
    let err = ExternalTokenizer::spawn(
        std::path::Path::new("/usr/bin/env"),
        &[
            "python3".into(),
            script.display().to_string(),
            "cl100k_base".into(),
        ],
        &env,
    );
    assert_eq!(err.err().expect("refused").code, "SPX-HPO003");
}

#[test]
#[ignore = "provisioned: needs HP15_PYTHON=<abs python3 with tiktoken>, HP15_TIKTOKEN_CACHE_DIR=<abs cached encodings dir>"]
fn hp_hp15_real_tiktoken_named_tokenizer_matches_independent_count() {
    let py = required_tool("HP15_PYTHON");
    let cache = required_tool("HP15_TIKTOKEN_CACHE_DIR");
    let script = repo_root().join("scripts/harness_tokenize.py");
    let env = BTreeMap::from([(
        "TIKTOKEN_CACHE_DIR".to_string(),
        cache.display().to_string(),
    )]);
    let tok = ExternalTokenizer::spawn(
        &py,
        &[script.display().to_string(), "cl100k_base".into()],
        &env,
    )
    .expect("tiktoken cached");
    let text = serde_json::to_string("hello wörld 日本語 \"q\"\n").unwrap();
    let want = std::process::Command::new(&py)
        .args(["-c", "import sys,tiktoken;print(len(tiktoken.get_encoding('cl100k_base').encode(sys.argv[1],disallowed_special=())))", &text])
        .env("TIKTOKEN_CACHE_DIR", &cache)
        .output()
        .unwrap();
    let want: u64 = String::from_utf8_lossy(&want.stdout)
        .trim()
        .parse()
        .unwrap();
    assert_eq!(measure(&text, Some(&tok)).count.unwrap().value, want);
    assert!(tok.fingerprint().starts_with("sha256:"));
}

// ---- hpwire: token-observation export ----

#[test]
fn hp_hpwire_export_rows_use_the_token_observation_schema() {
    use semaprax_harness::observe::export;
    use semaprax_harness::observe::{Observation, Role, Stage, TokenCount};
    let mut paired = Observation::new(
        "p/view",
        "command.view",
        Stage::CommandView,
        Role::Transform,
        "inv-1",
    );
    paired.payload_id = Some("pl".into());
    paired.before = Some(TokenCount::named("cl100k_base", "fp", 900));
    paired.after = Some(TokenCount::named("cl100k_base", "fp", 120));
    paired.seq = 1;
    let mut bytes_only = Observation::new(
        "p/skills",
        "skill.catalog",
        Stage::SkillCatalog,
        Role::Transform,
        "inv-2",
    );
    bytes_only.payload_id = Some("pl".into());
    bytes_only.after = Some(TokenCount::bytes(512));
    bytes_only.seq = 2;
    let mut model = Observation::new(
        "p/model",
        "model.generate",
        Stage::Generation,
        Role::Incurred,
        "inv-3",
    );
    model.incurred = Some(TokenCount::bytes(2048));
    model.seq = 3;
    let rows = export::rows(&[paired, bytes_only, model], "sess");
    assert_eq!(rows[0]["status"], "measured");
    assert_eq!(
        (
            rows[0]["tokens"].as_u64(),
            rows[0]["baselineTokens"].as_u64()
        ),
        (Some(120), Some(900))
    );
    assert_eq!(rows[0]["referenceKind"], "source_context");
    assert_eq!(rows[1]["status"], "tokenizer_unavailable");
    assert!(rows[1]["tokens"].is_null() && rows[1]["bytes"] == 512);
    assert_eq!(rows[2]["bytes"], 2048);
    for r in &rows {
        assert_eq!(r["schema"], "semaprax.token-observation.v1");
        assert_eq!(r["sessionId"], "sess");
    }
}
