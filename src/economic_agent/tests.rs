use super::*;
use std::collections::BTreeMap;

fn limits() -> Limits {
    Limits {
        max_policy_bytes: MAX_POLICY_BYTES as u64,
        max_intent_bytes: MAX_INTENT_BYTES as u64,
        max_invoice_bytes: MAX_INVOICE_BYTES as u64,
        max_snapshot_bytes: MAX_SNAPSHOT_BYTES as u64,
        max_plan_bytes: MAX_PLAN_BYTES as u64,
        max_simulation_bytes: MAX_SIMULATION_BYTES as u64,
        max_approval_request_bytes: MAX_APPROVAL_REQUEST_BYTES as u64,
        max_approval_bytes: MAX_APPROVAL_BYTES as u64,
        max_journal_bytes: MAX_JOURNAL_BYTES as u64,
        max_unsigned_transaction_bytes: MAX_UNSIGNED_BYTES as u64,
        max_signed_transaction_bytes: MAX_SIGNED_BYTES as u64,
        max_broadcast_receipt_bytes: MAX_BROADCAST_BYTES as u64,
        max_reconciliation_bytes: MAX_RECONCILIATION_BYTES as u64,
        max_trace_events: MAX_TRACE_EVENTS as u64,
        max_trace_bytes: MAX_TRACE_BYTES as u64,
        max_evidence_bytes: MAX_EVIDENCE_BYTES as u64,
        max_builder_bytes: MAX_BUILDER_BYTES as u64,
        max_json_depth: MAX_JSON_DEPTH as u64,
        max_identifier_bytes: MAX_IDENTIFIER_BYTES as u64,
        max_memo_bytes: MAX_MEMO_BYTES as u64,
        max_recipients: MAX_RECIPIENTS as u64,
        max_network_policies: MAX_NETWORK_POLICIES as u64,
        max_x402_origins: MAX_X402_ORIGINS as u64,
        max_utxos: MAX_UTXOS as u64,
        max_reconciliations: 64,
        max_elapsed_ms: 600_000,
        max_amount_atomic: 1_000_000_000_000_000_000,
        max_fee_atomic: 1_000_000_000_000_000,
        max_compute_units: 200_000,
        max_confirmation_target: 144,
        max_concurrency: 1,
        max_unexpected_authority_calls: 0,
    }
}
fn evm_policy() -> Policy {
    let mut policy = Policy {
        economic_agent_id: "fixture.economic".to_owned(),
        wallet_id: "fixture.wallet".to_owned(),
        networks: vec![NetworkPolicy {
            rail: EconomicRail::Evm,
            network: "sepolia".to_owned(),
            asset: "native:eth".to_owned(),
            recipients: vec!["0x1111111111111111111111111111111111111111".to_owned()],
            max_amount: 1_000_000,
            max_fee: 1_000_000,
            max_rolling: 1_000_000,
        }],
        origins: vec![],
        limits: limits(),
        source: String::new(),
        digest: String::new(),
    };
    policy.source = render_policy(&policy);
    policy.digest = digest(POLICY_DOMAIN, policy.source.as_bytes());
    policy
}
fn evm_intent() -> Intent {
    let mut intent = Intent {
        intent_id: "fixture.intent".to_owned(),
        wallet_id: "fixture.wallet".to_owned(),
        rail_text: "evm".to_owned(),
        idempotency_key: "fixture.payment.evm".to_owned(),
        created_at: 1_700_000_000_000,
        expires_at: 1_700_000_300_000,
        memo: None,
        payment: Payment::Evm {
            recipient: "0x1111111111111111111111111111111111111111".to_owned(),
            amount: 10,
            max_fee: 100_000,
        },
        source: String::new(),
        digest: String::new(),
    };
    intent.source = render_intent(&intent);
    intent.digest = digest(INTENT_DOMAIN, intent.source.as_bytes());
    intent
}

fn regtest_recipient(program: [u8; 20]) -> String {
    let charset = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    let mut data = vec![0];
    data.extend(convert_bits(&program, 8, 5, true).unwrap());
    let mut values = vec![3, 3, 3, 3, 0, 2, 3, 18, 20];
    values.extend_from_slice(&data);
    values.extend([0; 6]);
    let mut polymod = 1u32;
    for value in values {
        let top = polymod >> 25;
        polymod = ((polymod & 0x01ff_ffff) << 5) ^ u32::from(value);
        for (index, generator) in [
            0x3b6a_57b2,
            0x2650_8e6d,
            0x1ea1_19fa,
            0x3d42_33dd,
            0x2a14_62b3,
        ]
        .iter()
        .enumerate()
        {
            if ((top >> index) & 1) != 0 {
                polymod ^= generator;
            }
        }
    }
    polymod ^= 1;
    let mut encoded = String::from("bcrt1");
    for value in data
        .into_iter()
        .chain((0..6).map(|index| ((polymod >> (5 * (5 - index))) & 31) as u8))
    {
        encoded.push(charset[usize::from(value)] as char);
    }
    assert!(decode_regtest_p2wpkh(&encoded).is_some());
    encoded
}

fn rail_fixture(rail: EconomicRail) -> (Policy, Intent) {
    let (network, asset, recipient, payment, rail_text) = match rail {
        EconomicRail::Evm => {
            let recipient = "0x1111111111111111111111111111111111111111".to_owned();
            (
                "sepolia",
                "native:eth",
                recipient.clone(),
                Payment::Evm {
                    recipient,
                    amount: 10,
                    max_fee: 100_000,
                },
                "evm",
            )
        }
        EconomicRail::Solana => {
            let recipient = encode_base58(&[3; 32]);
            (
                "devnet",
                "native:sol",
                recipient.clone(),
                Payment::Solana {
                    recipient,
                    amount: 10,
                    max_fee: 6_000,
                    compute: 200_000,
                    priority: 1_000,
                },
                "solana",
            )
        }
        EconomicRail::Bitcoin => {
            let recipient = regtest_recipient([9; 20]);
            (
                "regtest",
                "native:btc",
                recipient.clone(),
                Payment::Bitcoin {
                    recipient,
                    amount: 10_000,
                    max_fee: 10_000,
                    confirmations: 1,
                },
                "bitcoin",
            )
        }
    };
    let mut policy = Policy {
        economic_agent_id: "fixture.economic".to_owned(),
        wallet_id: "fixture.wallet".to_owned(),
        networks: vec![NetworkPolicy {
            rail,
            network: network.to_owned(),
            asset: asset.to_owned(),
            recipients: vec![recipient],
            max_amount: 1_000_000,
            max_fee: 1_000_000,
            max_rolling: 1_000_000,
        }],
        origins: vec![],
        limits: limits(),
        source: String::new(),
        digest: String::new(),
    };
    policy.source = render_policy(&policy);
    policy.digest = digest(POLICY_DOMAIN, policy.source.as_bytes());
    let mut intent = Intent {
        intent_id: format!("fixture.intent.{rail_text}"),
        wallet_id: "fixture.wallet".to_owned(),
        rail_text: rail_text.to_owned(),
        idempotency_key: format!("fixture.payment.{rail_text}"),
        created_at: 1_700_000_000_000,
        expires_at: 1_700_000_300_000,
        memo: None,
        payment,
        source: String::new(),
        digest: String::new(),
    };
    intent.source = render_intent(&intent);
    intent.digest = digest(INTENT_DOMAIN, intent.source.as_bytes());
    (policy, intent)
}

fn x402_fixture() -> (Policy, Intent, Invoice) {
    let (mut policy, mut intent) = rail_fixture(EconomicRail::Evm);
    let invoice = Invoice {
        origin: "https://pay.example.com".to_owned(),
        method: "POST".to_owned(),
        resource: "/v1/payments".to_owned(),
        invoice_id: "fixture.invoice".to_owned(),
        payee: "0x1111111111111111111111111111111111111111".to_owned(),
        rail: EconomicRail::Evm,
        network: "sepolia".to_owned(),
        asset: "native:eth".to_owned(),
        amount: 10,
        max_fee: 100_000,
        expires: intent.expires_at - 1,
        nonce: "fixture.nonce".to_owned(),
        idempotency: "fixture.payment.x402".to_owned(),
        doc: Doc {
            source: String::new(),
            digest: String::new(),
        },
    };
    let invoice_source = render_invoice(&invoice);
    let mut invoice = invoice;
    invoice.doc = Doc {
        digest: digest(INVOICE_DOMAIN, invoice_source.as_bytes()),
        source: invoice_source,
    };
    policy.origins = vec![OriginPolicy {
        origin: invoice.origin.clone(),
        methods: vec![invoice.method.clone()],
        resources: vec![invoice.resource.clone()],
        rails: vec![EconomicRail::Evm],
        max_amount: 1_000_000,
    }];
    policy.source = render_policy(&policy);
    policy.digest = digest(POLICY_DOMAIN, policy.source.as_bytes());
    intent.intent_id = "fixture.intent.x402".to_owned();
    intent.rail_text = "x402".to_owned();
    intent.idempotency_key = invoice.idempotency.clone();
    intent.payment = Payment::X402 {
        origin: invoice.origin.clone(),
        method: invoice.method.clone(),
        resource: invoice.resource.clone(),
        invoice_digest: invoice.doc.digest.clone(),
        payee: invoice.payee.clone(),
        rail: invoice.rail,
        network: invoice.network.clone(),
        asset: invoice.asset.clone(),
        amount: invoice.amount,
        max_fee: invoice.max_fee,
        invoice_expires: invoice.expires,
        nonce: invoice.nonce.clone(),
    };
    intent.source = render_intent(&intent);
    intent.digest = digest(INTENT_DOMAIN, intent.source.as_bytes());
    (policy, intent, invoice)
}

#[test]
fn policy_and_intent_are_exact_canonical_documents() {
    let policy = evm_policy();
    assert_eq!(parse_policy(&policy.source).unwrap().digest, policy.digest);
    let intent = evm_intent();
    assert_eq!(parse_intent(&intent.source).unwrap().digest, intent.digest);
    assert_eq!(
        parse_policy(&policy.source.replace("\n", "\r\n"))
            .err()
            .unwrap()
            .code,
        "SPX-G210"
    );
    let mut over = intent.source.clone();
    over.insert_str(1, "\"extra\":0,");
    assert_eq!(parse_intent(&over).err().unwrap().code, "SPX-G210");
}

#[test]
fn sealed_agent_fixture_binds_exact_canonical_payment_intent() {
    let intent = evm_intent();
    let run = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
    let binding = run.economic_binding();
    assert_eq!(binding.status, AgentRunStatus::Completed);
    assert_eq!(binding.final_message, Some(intent.source.as_str()));
    assert_eq!(
        parse_intent(binding.final_message.unwrap()).unwrap().digest,
        intent.digest
    );
    assert_eq!(binding.evidence_digest, run.evidence_digest());
}

#[test]
fn origin_resource_and_base58_identity_are_fail_closed() {
    for origin in [
        "https://127.0.0.1",
        "https://10.0.0.1",
        "https://localhost",
        "https://wallet.local",
        "https://example.com:443",
        "http://example.com",
    ] {
        assert!(!valid_origin(origin), "{origin}");
    }
    assert!(valid_origin("https://pay.example.com"));
    for path in [
        "//admin",
        "/../admin",
        "/%2e%2e/admin",
        "/a%2Fb",
        "/a%5cb",
        "/a?b",
    ] {
        assert!(!valid_resource(path), "{path}");
    }
    assert!(valid_resource("/v1/payments"));
    let system = "11111111111111111111111111111111";
    assert_eq!(encode_base58(&decode_base58_32(system).unwrap()), system);
    assert!(decode_base58_32(&format!("1{system}")).is_none());
}

#[test]
fn evm_unsigned_and_signed_replay_bind_every_field() {
    let intent = evm_intent();
    let snapshot = Snapshot {
        rail: EconomicRail::Evm,
        observed: intent.created_at + 1,
        expires: intent.expires_at - 1,
        state: SnapshotState::Evm {
            from: "0x2222222222222222222222222222222222222222".to_owned(),
            nonce: 7,
            base_fee: 1,
            priority: 2,
            gas: 21_000,
        },
        doc: Doc {
            source: "snapshot\n".to_owned(),
            digest: "sha256:fixture".to_owned(),
        },
    };
    let (unsigned, format) = build_unsigned(&intent, &snapshot).unwrap();
    assert_eq!(format, "eip1559-unsigned-v1");
    let mut fields = rlp_list_items(&unsigned[1..])
        .unwrap()
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    fields.extend([rlp_u64(1), rlp_u64(1), rlp_u64(1)]);
    let mut signed = vec![2];
    signed.extend(rlp_list(&fields));
    verify_signed(EconomicRail::Evm, &unsigned, &signed).unwrap();
    let last = signed.len() - 1;
    signed[last] = 0;
    assert_eq!(
        verify_signed(EconomicRail::Evm, &unsigned, &signed)
            .unwrap_err()
            .code,
        "SPX-G213"
    );
}

#[test]
fn solana_fee_conversion_and_v0_shape_are_exact() {
    let payer = encode_base58(&[2; 32]);
    let recipient = encode_base58(&[3; 32]);
    let blockhash = encode_base58(&[4; 32]);
    let intent = Intent {
        intent_id: "i".into(),
        wallet_id: "w".into(),
        rail_text: "solana".into(),
        idempotency_key: "k".into(),
        created_at: 1,
        expires_at: 10,
        memo: None,
        payment: Payment::Solana {
            recipient,
            amount: 7,
            max_fee: 6_000,
            compute: 200_000,
            priority: 1_000,
        },
        source: String::new(),
        digest: String::new(),
    };
    let snapshot = Snapshot {
        rail: EconomicRail::Solana,
        observed: 2,
        expires: 9,
        state: SnapshotState::Solana {
            payer,
            blockhash,
            last_height: 5,
            fee: 5_000,
        },
        doc: Doc {
            source: String::new(),
            digest: String::new(),
        },
    };
    let (bytes, format) = build_unsigned(&intent, &snapshot).unwrap();
    assert_eq!(format, "solana-message-v0");
    assert_eq!(&bytes[..4], &[0x80, 1, 0, 2]);
    assert_eq!(bytes.last(), Some(&0));
}

#[test]
fn keccak_and_rail_transaction_id_vectors_are_pinned() {
    let empty = keccak256(b"");
    assert_eq!(
        empty
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    );
    let abc = keccak256(b"abc");
    assert_eq!(
        abc.iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
    );
    let mut solana = vec![1];
    solana.extend([7u8; 64]);
    assert_eq!(
        transaction_id(EconomicRail::Solana, &solana),
        Some(encode_base58(&[7u8; 64]))
    );
}

#[test]
fn simulation_requires_exact_native_value_conservation() {
    let intent = evm_intent();
    let plan = Plan {
        doc: Doc {
            source: "plan\n".into(),
            digest: "sha256:plan".into(),
        },
        unsigned: vec![],
        unsigned_digest: "sha256:u".into(),
        format: "eip1559-unsigned-v1",
        observed: intent.created_at + 1,
        expires: intent.expires_at - 1,
        utxos: 0,
    };
    let good=format!("{{\"schema\":\"{SIMULATION_SCHEMA}\",\"plan\":{},\"success\":true,\"fee_atomic\":5,\"balance_before_atomic\":115,\"balance_after_atomic\":100,\"allowance_atomic\":0,\"units\":21000,\"expires_at_ms\":{}}}\n",doc_ref(PLAN_SCHEMA,&plan.doc),plan.expires);
    assert!(parse_simulation(&good, &plan, &intent).is_ok());
    let hostile = good.replace(
        "\"balance_before_atomic\":115",
        "\"balance_before_atomic\":116",
    );
    assert_eq!(
        parse_simulation(&hostile, &plan, &intent)
            .err()
            .unwrap()
            .code,
        "SPX-G213"
    );
}

type RollingKey = (String, String, String, String);
type RollingRows = Vec<(String, u64, u64)>;

struct FixedEconomicProbe(u64);
impl EconomicBoundaryProbe for FixedEconomicProbe {
    fn elapsed_ms(&self) -> u64 {
        self.0
    }
}

struct FullHost {
    journals: BTreeMap<String, String>,
    calls: Vec<&'static str>,
    intent: Intent,
    invoice: Option<Invoice>,
    broadcast_disposition: EconomicAdapterDisposition,
    reconciliation_status: &'static str,
    trusted_now_ms: u64,
    elapsed_ms: u64,
    rolling: BTreeMap<RollingKey, RollingRows>,
    malformed_simulation: bool,
    documents: BTreeMap<&'static str, String>,
    cas_fault: Option<(usize, EconomicAdapterDisposition, bool)>,
    cancel_after_version: Option<(u64, AgentCancellation)>,
    elapsed_after_version: Option<(u64, u64)>,
    rolling_updates: Vec<&'static str>,
}

impl FullHost {
    fn new(intent: Intent) -> Self {
        Self {
            journals: BTreeMap::new(),
            calls: vec![],
            intent,
            invoice: None,
            broadcast_disposition: EconomicAdapterDisposition::Succeeded,
            reconciliation_status: "confirmed",
            trusted_now_ms: 1_700_000_000_000,
            elapsed_ms: 0,
            rolling: BTreeMap::new(),
            malformed_simulation: false,
            documents: BTreeMap::new(),
            cas_fault: None,
            cancel_after_version: None,
            elapsed_after_version: None,
            rolling_updates: vec![],
        }
    }

    fn with_invoice(intent: Intent, invoice: Invoice) -> Self {
        Self {
            invoice: Some(invoice),
            ..Self::new(intent)
        }
    }

    fn record_call(&mut self, call: &'static str) {
        self.calls.push(call);
        if let Some(directory) = std::env::var_os("SEMAPRAX_ECONOMIC_DURABLE_DIR") {
            use std::io::Write as _;
            let path = std::path::PathBuf::from(directory).join("calls");
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            writeln!(file, "{call}").unwrap();
        }
    }

    fn stop_after_effect_if_requested(&self, stage: &str) {
        if std::env::var("SEMAPRAX_ECONOMIC_KILL_STAGE").as_deref() != Ok(stage) {
            return;
        }
        let directory =
            std::path::PathBuf::from(std::env::var_os("SEMAPRAX_ECONOMIC_DURABLE_DIR").unwrap());
        std::fs::write(directory.join("ready"), stage.as_bytes()).unwrap();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    fn simulation(&mut self, plan: &str, sink: &mut EconomicDocumentSink) {
        self.record_call("simulate");
        if self.malformed_simulation {
            assert!(sink.push(b"{\"secret\":\"economic-secret-sentinel\"}\n"));
            return;
        }
        let value: Value = serde_json::from_str(plan.trim_end()).unwrap();
        let plan_doc = Doc {
            source: plan.to_owned(),
            digest: digest(PLAN_DOMAIN, plan.as_bytes()),
        };
        let amount = value["amount_atomic"].as_u64().unwrap();
        let fee = match self.intent.settlement_rail() {
            EconomicRail::Evm => 63_000,
            EconomicRail::Solana => 6_000,
            EconomicRail::Bitcoin => 10_000,
        };
        let after = 1_000_000;
        let expires = value["expires_at_ms"].as_u64().unwrap();
        let units = match self.intent.settlement_rail() {
            EconomicRail::Evm => 21_000,
            EconomicRail::Solana => 200_000,
            EconomicRail::Bitcoin => 1,
        };
        let allowance = if self.intent.settlement_rail() == EconomicRail::Evm {
            "0"
        } else {
            "null"
        };
        let simulation = format!(
                "{{\"schema\":\"{SIMULATION_SCHEMA}\",\"plan\":{},\"success\":true,\"fee_atomic\":{fee},\"balance_before_atomic\":{},\"balance_after_atomic\":{after},\"allowance_atomic\":{allowance},\"units\":{units},\"expires_at_ms\":{expires}}}\n",
                doc_ref(PLAN_SCHEMA, &plan_doc),
                after + amount + fee,
            );
        self.documents.insert("plan", plan.to_owned());
        self.documents.insert("simulation", simulation.clone());
        assert!(sink.push(simulation.as_bytes()));
    }

    fn broadcast(&mut self, signed: &[u8], sink: &mut EconomicDocumentSink) {
        self.record_call("broadcast");
        let rail = self.intent.settlement_rail();
        let (network, _) = self.intent.network_asset();
        let signed_digest = digest(SIGNED_DOMAIN, signed);
        let txid = transaction_id(rail, signed).unwrap();
        let disposition =
            if self.broadcast_disposition == EconomicAdapterDisposition::FailedUncertain {
                "unknown"
            } else {
                "accepted"
            };
        let source = format!(
                "{{\"schema\":\"{BROADCAST_SCHEMA}\",\"rail\":{},\"network\":{},\"signed_transaction_digest\":{},\"transaction_id\":{},\"disposition\":{},\"observed_at_ms\":{}}}\n",
                quote_json(rail.text()),
                quote_json(network),
                quote_json(&signed_digest),
                quote_json(&txid),
                quote_json(disposition),
                self.intent.created_at + 2,
            );
        self.documents.insert("broadcast", source.clone());
        assert!(sink.push(source.as_bytes()));
        self.stop_after_effect_if_requested("broadcast_effect");
    }

    fn reconciliation(&mut self, transaction_id: &str, sink: &mut EconomicDocumentSink) {
        self.record_call("reconcile");
        let rail = self.intent.settlement_rail();
        let (network, _) = self.intent.network_asset();
        let (height, confirmations, block) = if self.reconciliation_status == "confirmed" {
            ("1", "1", quote_json("fixture.block"))
        } else {
            ("null", "null", "null".to_owned())
        };
        let source = format!(
                "{{\"schema\":\"{RECONCILIATION_SCHEMA}\",\"rail\":{},\"network\":{},\"transaction_id\":{},\"status\":{},\"observed_at_ms\":{},\"observed_height\":{height},\"confirmations\":{confirmations},\"canonical_block_id\":{block}}}\n",
                quote_json(rail.text()),
                quote_json(network),
                quote_json(transaction_id),
                quote_json(self.reconciliation_status),
                self.intent.created_at + 3,
            );
        self.documents.insert("reconciliation", source.clone());
        assert!(sink.push(source.as_bytes()));
    }
}

impl EconomicAgentHost for FullHost {
    fn boundary_probe(&self) -> Box<dyn EconomicBoundaryProbe> {
        Box::new(FixedEconomicProbe(self.elapsed_ms))
    }
}

impl PaymentJournal for FullHost {
    fn load(
        &mut self,
        idempotency_key: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicJournalLoad {
        self.record_call("load");
        if !self.journals.contains_key(idempotency_key) {
            if let Some(directory) = std::env::var_os("SEMAPRAX_ECONOMIC_DURABLE_DIR") {
                let path = std::path::PathBuf::from(directory).join("journal");
                if let Ok(source) = std::fs::read_to_string(path) {
                    self.journals.insert(idempotency_key.to_owned(), source);
                }
            }
        }
        match self.journals.get(idempotency_key) {
            Some(source) => {
                assert!(sink.push(source.as_bytes()));
                EconomicJournalLoad::Present
            }
            None => EconomicJournalLoad::Missing,
        }
    }

    fn compare_and_swap(
        &mut self,
        idempotency_key: &str,
        expected_version: u64,
        journal: &str,
        rolling: EconomicRollingReservationUpdate<'_>,
    ) -> EconomicAdapterDisposition {
        self.record_call("cas");
        self.rolling_updates.push(match rolling {
            EconomicRollingReservationUpdate::Reserve(_) => "reserve",
            EconomicRollingReservationUpdate::Retain => "retain",
            EconomicRollingReservationUpdate::Release => "release",
        });
        let cas_ordinal = self.calls.iter().filter(|call| **call == "cas").count();
        let fault = self
            .cas_fault
            .filter(|(ordinal, _, _)| *ordinal == cas_ordinal);
        if let Some((_, disposition, false)) = fault {
            return disposition;
        }
        let actual = self
            .journals
            .get(idempotency_key)
            .and_then(|source| serde_json::from_str::<Value>(source.trim_end()).ok())
            .and_then(|value| value["version"].as_u64())
            .unwrap_or(0);
        if actual != expected_version {
            return EconomicAdapterDisposition::FailedUncertain;
        }
        if expected_version == 0 {
            let EconomicRollingReservationUpdate::Reserve(reservation) = rolling else {
                return EconomicAdapterDisposition::FailedUncertain;
            };
            assert_eq!(reservation.wallet_id(), "fixture.wallet");
            assert_eq!(reservation.rail(), self.intent.settlement_rail());
            let (network, asset) = self.intent.network_asset();
            assert_eq!(reservation.network(), network);
            assert_eq!(reservation.asset(), asset);
            assert_eq!(reservation.requested_at_ms(), self.intent.created_at);
            assert_eq!(reservation.amount_atomic(), self.intent.amount());
            assert_eq!(reservation.max_rolling_24h_atomic(), 1_000_000);
            let key = (
                reservation.wallet_id().to_owned(),
                reservation.rail().text().to_owned(),
                reservation.network().to_owned(),
                reservation.asset().to_owned(),
            );
            let rows = self.rolling.entry(key).or_default();
            rows.retain(|(_, admitted_at, _)| {
                self.trusted_now_ms.saturating_sub(*admitted_at) < 86_400_000
            });
            let Some(total) = rows
                .iter()
                .try_fold(reservation.amount_atomic(), |sum, (_, _, amount)| {
                    sum.checked_add(*amount)
                })
            else {
                return EconomicAdapterDisposition::PolicyRejected;
            };
            if total > reservation.max_rolling_24h_atomic() {
                return EconomicAdapterDisposition::PolicyRejected;
            }
            rows.push((
                idempotency_key.to_owned(),
                self.trusted_now_ms,
                reservation.amount_atomic(),
            ));
        }
        self.journals
            .insert(idempotency_key.to_owned(), journal.to_owned());
        self.documents.insert("journal", journal.to_owned());
        let committed_version = serde_json::from_str::<Value>(journal.trim_end()).unwrap()
            ["version"]
            .as_u64()
            .unwrap();
        if self
            .cancel_after_version
            .as_ref()
            .is_some_and(|(version, _)| *version == committed_version)
        {
            self.cancel_after_version.as_ref().unwrap().1.cancel();
        }
        if let Some((version, elapsed)) = self.elapsed_after_version {
            if version == committed_version {
                self.elapsed_ms = elapsed;
            }
        }
        if let Some(directory) = std::env::var_os("SEMAPRAX_ECONOMIC_DURABLE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::write(directory.join("journal"), journal).unwrap();
            if let Some(stage) = std::env::var_os("SEMAPRAX_ECONOMIC_KILL_STAGE") {
                let value: Value = serde_json::from_str(journal.trim_end()).unwrap();
                let version = value["version"].as_u64().unwrap();
                let state = value["state"].as_str().unwrap();
                let matches = match stage.to_str().unwrap() {
                    "v4" => version == 4,
                    "v5" => version == 5,
                    "v6" => version == 6,
                    "odd" => version >= 7 && version % 2 == 1 && state != "approved",
                    "even" => version >= 8 && version.is_multiple_of(2),
                    _ => false,
                };
                if matches {
                    std::fs::write(directory.join("ready"), stage.to_string_lossy().as_bytes())
                        .unwrap();
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                }
            }
        }
        fault.map_or(
            EconomicAdapterDisposition::Succeeded,
            |(_, disposition, _)| disposition,
        )
    }
}

impl X402InvoiceAdapter for FullHost {
    fn fetch_invoice(
        &mut self,
        origin: &str,
        method: &str,
        resource: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.record_call("invoice");
        let Some(invoice) = &self.invoice else {
            return EconomicAdapterDisposition::DefinitelyNotStarted;
        };
        assert_eq!(
            (origin, method, resource),
            (&*invoice.origin, &*invoice.method, &*invoice.resource)
        );
        assert!(sink.push(invoice.doc.source.as_bytes()));
        EconomicAdapterDisposition::Succeeded
    }
}

impl EvmPaymentAdapter for FullHost {
    fn evm_snapshot(
        &mut self,
        _: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.record_call("snapshot");
        let snapshot = Snapshot {
            rail: EconomicRail::Evm,
            observed: self.intent.created_at + 1,
            expires: self.intent.expires_at - 1,
            state: SnapshotState::Evm {
                from: "0x2222222222222222222222222222222222222222".to_owned(),
                nonce: 7,
                base_fee: 1,
                priority: 2,
                gas: 21_000,
            },
            doc: Doc {
                source: String::new(),
                digest: String::new(),
            },
        };
        let source = render_snapshot(&snapshot);
        self.documents.insert("snapshot", source.clone());
        assert!(sink.push(source.as_bytes()));
        EconomicAdapterDisposition::Succeeded
    }

    fn evm_simulate(
        &mut self,
        plan: &str,
        _: &[u8],
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.simulation(plan, sink);
        EconomicAdapterDisposition::Succeeded
    }

    fn evm_broadcast(
        &mut self,
        signed: &[u8],
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.broadcast(signed, sink);
        self.broadcast_disposition
    }

    fn evm_reconcile(
        &mut self,
        transaction_id: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.reconciliation(transaction_id, sink);
        EconomicAdapterDisposition::Succeeded
    }
}

impl SolanaPaymentAdapter for FullHost {
    fn solana_snapshot(
        &mut self,
        _: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.record_call("snapshot");
        let snapshot = Snapshot {
            rail: EconomicRail::Solana,
            observed: self.intent.created_at + 1,
            expires: self.intent.expires_at - 1,
            state: SnapshotState::Solana {
                payer: encode_base58(&[2; 32]),
                blockhash: encode_base58(&[4; 32]),
                last_height: 5,
                fee: 5_000,
            },
            doc: Doc {
                source: String::new(),
                digest: String::new(),
            },
        };
        let source = render_snapshot(&snapshot);
        self.documents.insert("snapshot", source.clone());
        assert!(sink.push(source.as_bytes()));
        EconomicAdapterDisposition::Succeeded
    }

    fn solana_simulate(
        &mut self,
        plan: &str,
        _: &[u8],
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.simulation(plan, sink);
        EconomicAdapterDisposition::Succeeded
    }

    fn solana_broadcast(
        &mut self,
        signed: &[u8],
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.broadcast(signed, sink);
        self.broadcast_disposition
    }

    fn solana_reconcile(
        &mut self,
        transaction_id: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.reconciliation(transaction_id, sink);
        EconomicAdapterDisposition::Succeeded
    }
}

impl BitcoinPaymentAdapter for FullHost {
    fn bitcoin_snapshot(
        &mut self,
        _: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.record_call("snapshot");
        let snapshot = Snapshot {
            rail: EconomicRail::Bitcoin,
            observed: self.intent.created_at + 1,
            expires: self.intent.expires_at - 1,
            state: SnapshotState::Bitcoin {
                wallet_script: format!("0014{}", "11".repeat(20)),
                height: 100,
                fee_rate: 1,
                utxos: vec![Utxo {
                    txid: format!("{}01", "00".repeat(31)),
                    vout: 0,
                    value: 100_000,
                    script: format!("0014{}", "11".repeat(20)),
                    confirmations: 1,
                }],
            },
            doc: Doc {
                source: String::new(),
                digest: String::new(),
            },
        };
        let source = render_snapshot(&snapshot);
        self.documents.insert("snapshot", source.clone());
        assert!(sink.push(source.as_bytes()));
        EconomicAdapterDisposition::Succeeded
    }

    fn bitcoin_simulate(
        &mut self,
        plan: &str,
        _: &[u8],
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.simulation(plan, sink);
        EconomicAdapterDisposition::Succeeded
    }

    fn bitcoin_broadcast(
        &mut self,
        signed: &[u8],
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.broadcast(signed, sink);
        self.broadcast_disposition
    }

    fn bitcoin_reconcile(
        &mut self,
        transaction_id: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.reconciliation(transaction_id, sink);
        EconomicAdapterDisposition::Succeeded
    }
}

impl PaymentApprover for FullHost {
    fn approve(
        &mut self,
        request: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.record_call("approve");
        let value: Value = serde_json::from_str(request.trim_end()).unwrap();
        let ref_text = |name: &str| {
            let row = value[name].as_object().unwrap();
            format!(
                "{{\"schema\":{},\"digest\":{},\"bytes\":{}}}",
                quote_json(row["schema"].as_str().unwrap()),
                quote_json(row["digest"].as_str().unwrap()),
                row["bytes"].as_u64().unwrap(),
            )
        };
        let request_doc = Doc {
            source: request.to_owned(),
            digest: digest(APPROVAL_REQUEST_DOMAIN, request.as_bytes()),
        };
        let source = format!(
                "{{\"schema\":\"{APPROVAL_SCHEMA}\",\"approval_id\":\"fixture.approval\",\"approver_id\":\"fixture.approver\",\"policy\":{},\"intent\":{},\"plan\":{},\"simulation\":{},\"approval_request\":{},\"decision\":\"approved\",\"approved_amount_atomic\":{},\"approved_fee_atomic\":{},\"expires_at_ms\":{}}}\n",
                ref_text("policy"), ref_text("intent"), ref_text("plan"), ref_text("simulation"),
                doc_ref(APPROVAL_REQUEST_SCHEMA, &request_doc),
                value["amount_atomic"].as_u64().unwrap(), value["max_fee_atomic"].as_u64().unwrap(), value["expires_at_ms"].as_u64().unwrap(),
            );
        self.documents
            .insert("approval_request", request.to_owned());
        self.documents.insert("approval", source.clone());
        assert!(sink.push(source.as_bytes()));
        EconomicAdapterDisposition::Succeeded
    }
}

impl WalletCustody for FullHost {
    fn sign(
        &mut self,
        _: &str,
        _: EconomicRail,
        _: &str,
        unsigned: &[u8],
        _: &str,
        sink: &mut EconomicBytesSink,
    ) -> EconomicAdapterDisposition {
        self.record_call("sign");
        let signed = match self.intent.settlement_rail() {
            EconomicRail::Evm => {
                let mut fields = rlp_list_items(&unsigned[1..])
                    .unwrap()
                    .into_iter()
                    .map(<[u8]>::to_vec)
                    .collect::<Vec<_>>();
                fields.extend([rlp_u64(1), rlp_u64(1), rlp_u64(1)]);
                let mut signed = vec![2];
                signed.extend(rlp_list(&fields));
                signed
            }
            EconomicRail::Solana => {
                let mut signed = vec![1];
                signed.extend([7; 64]);
                signed.extend(unsigned);
                signed
            }
            EconomicRail::Bitcoin => {
                let template = parse_psbt_template(unsigned).unwrap();
                let mut signed = 2i32.to_le_bytes().to_vec();
                signed.extend([0, 1]);
                signed.extend(compact_size(template.inputs.len()));
                for input in &template.inputs {
                    signed.extend(input.txid);
                    signed.extend(input.vout.to_le_bytes());
                    signed.push(0);
                    signed.extend(input.sequence.to_le_bytes());
                }
                signed.extend(compact_size(template.outputs.len()));
                for output in &template.outputs {
                    signed.extend(output.value.to_le_bytes());
                    signed.extend(compact_size(output.script.len()));
                    signed.extend(&output.script);
                }
                let signature = [0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01, 0x01];
                for _ in &template.inputs {
                    signed.push(2);
                    signed.extend(compact_size(signature.len()));
                    signed.extend(signature);
                    signed.push(33);
                    signed.extend([2]);
                    signed.extend([1; 32]);
                }
                signed.extend(template.locktime.to_le_bytes());
                signed
            }
        };
        assert!(sink.push(&signed));
        self.stop_after_effect_if_requested("sign_effect");
        EconomicAdapterDisposition::Succeeded
    }
}

#[test]
fn full_evm_authority_route_is_ordered_and_self_replayed() {
    let policy = evm_policy();
    let intent = evm_intent();
    let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
    let mut agent = EconomicAgent::new(
        &policy.source,
        FullHost::new(intent),
        AgentCancellation::new(),
    )
    .unwrap();
    let run = agent.execute(&source).unwrap();
    assert_eq!(run.status(), EconomicRunStatus::Confirmed);
    assert!(run.transaction_id().is_some());
    assert_eq!(run.confirmation_status(), Some("confirmed"));
    assert!(run.trace().ends_with('\n'));
    assert!(run.evidence().contains("\"used_builder_bytes\":"));
    assert_eq!(
        run.trace_digest(),
        "sha256:ce7bec5f627a6d48990573353370dc0953203153f0db2ab60a6101cc9a5146d0"
    );
    assert_eq!(
        run.evidence_digest(),
        digest(EVIDENCE_DOMAIN, run.evidence().as_bytes())
    );
    assert_eq!(
        agent.host.calls,
        [
            "load",
            "cas",
            "snapshot",
            "simulate",
            "cas",
            "approve",
            "cas",
            "cas",
            "sign",
            "cas",
            "cas",
            "broadcast",
            "cas",
            "cas",
            "reconcile",
            "cas"
        ]
    );
}

#[test]
fn solana_bitcoin_and_x402_routes_are_chain_distinct_and_self_replayed() {
    for rail in [EconomicRail::Solana, EconomicRail::Bitcoin] {
        let (policy, intent) = rail_fixture(rail);
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut agent = EconomicAgent::new(
            &policy.source,
            FullHost::new(intent),
            AgentCancellation::new(),
        )
        .unwrap();
        let run = agent.execute(&source).unwrap();
        assert_eq!(run.status(), EconomicRunStatus::Confirmed, "{rail:?}");
        assert_eq!(run.confirmation_status(), Some("confirmed"));
        assert!(run.transaction_id().is_some());
        assert!(run.trace().ends_with('\n'));
        assert!(run.evidence().ends_with('\n'));
        assert_eq!(
            agent.host.calls,
            [
                "load",
                "cas",
                "snapshot",
                "simulate",
                "cas",
                "approve",
                "cas",
                "cas",
                "sign",
                "cas",
                "cas",
                "broadcast",
                "cas",
                "cas",
                "reconcile",
                "cas"
            ]
        );
    }

    let (policy, intent, invoice) = x402_fixture();
    let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
    let mut agent = EconomicAgent::new(
        &policy.source,
        FullHost::with_invoice(intent, invoice),
        AgentCancellation::new(),
    )
    .unwrap();
    let run = agent.execute(&source).unwrap();
    assert_eq!(run.status(), EconomicRunStatus::Confirmed);
    assert_eq!(run.confirmation_status(), Some("confirmed"));
    assert_eq!(agent.host.calls[2], "invoice");
    assert_eq!(
        agent
            .host
            .calls
            .iter()
            .filter(|call| **call == "invoice")
            .count(),
        1
    );
}

#[test]
fn uncertain_broadcast_is_never_retried_and_restart_reconciles_retained_capsule() {
    let (policy, intent) = rail_fixture(EconomicRail::Evm);
    let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
    let mut host = FullHost::new(intent.clone());
    host.broadcast_disposition = EconomicAdapterDisposition::FailedUncertain;
    let mut first = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
    let first_run = first.execute(&source).unwrap();
    assert_eq!(first_run.status(), EconomicRunStatus::BroadcastUnknown);
    assert_eq!(
        first
            .host
            .calls
            .iter()
            .filter(|call| **call == "broadcast")
            .count(),
        1
    );
    let journals = std::mem::take(&mut first.host.journals);
    let mut restart_host = FullHost::new(intent.clone());
    restart_host.journals = journals;
    let mut restart =
        EconomicAgent::new(&policy.source, restart_host, AgentCancellation::new()).unwrap();
    let reconciled = restart.reconcile(&intent.idempotency_key, &source).unwrap();
    assert_eq!(reconciled.status(), EconomicRunStatus::Confirmed);
    assert_eq!(
        restart
            .host
            .calls
            .iter()
            .filter(|call| **call == "broadcast")
            .count(),
        0
    );
    assert_eq!(
        restart
            .host
            .calls
            .iter()
            .filter(|call| **call == "sign")
            .count(),
        0
    );
    assert_eq!(restart.host.calls, ["load", "cas", "reconcile", "cas"]);
}

#[test]
fn rolling_window_uses_trusted_admission_time_and_expires_at_exact_24h() {
    let (_, mut intent) = rail_fixture(EconomicRail::Evm);
    intent.payment = Payment::Evm {
        recipient: "0x1111111111111111111111111111111111111111".to_owned(),
        amount: 600_000,
        max_fee: 100_000,
    };
    intent.source = render_intent(&intent);
    intent.digest = digest(INTENT_DOMAIN, intent.source.as_bytes());
    let mut host = FullHost::new(intent.clone());
    host.trusted_now_ms = intent.created_at + 300_000;
    let reservation = EconomicRollingReservation {
        wallet_id: "fixture.wallet".to_owned(),
        rail: EconomicRail::Evm,
        network: "sepolia".to_owned(),
        asset: "native:eth".to_owned(),
        requested_at_ms: intent.created_at,
        amount_atomic: intent.amount(),
        max_rolling_24h_atomic: 1_000_000,
    };
    assert_eq!(
        host.compare_and_swap(
            "rolling.first",
            0,
            "{\"version\":1}\n",
            EconomicRollingReservationUpdate::Reserve(&reservation),
        ),
        EconomicAdapterDisposition::Succeeded
    );
    let inventory = host.journals.clone();
    host.trusted_now_ms += 86_399_999;
    assert_eq!(
        host.compare_and_swap(
            "rolling.second",
            0,
            "{\"version\":1}\n",
            EconomicRollingReservationUpdate::Reserve(&reservation),
        ),
        EconomicAdapterDisposition::PolicyRejected
    );
    assert_eq!(host.journals, inventory);
    host.trusted_now_ms += 1;
    assert_eq!(
        host.compare_and_swap(
            "rolling.second",
            0,
            "{\"version\":1}\n",
            EconomicRollingReservationUpdate::Reserve(&reservation),
        ),
        EconomicAdapterDisposition::Succeeded
    );
    assert_eq!(host.rolling.values().flatten().count(), 1);
    assert_eq!(
        host.rolling.values().next().unwrap()[0].1,
        intent.created_at + 300_000 + 86_400_000
    );
}

#[test]
fn rolling_window_distinct_keys_race_to_one_atomic_winner() {
    let (_, mut intent) = rail_fixture(EconomicRail::Evm);
    intent.payment = Payment::Evm {
        recipient: "0x1111111111111111111111111111111111111111".to_owned(),
        amount: 600_000,
        max_fee: 100_000,
    };
    let host = std::sync::Arc::new(std::sync::Mutex::new(FullHost::new(intent.clone())));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let mut workers = Vec::new();
    for key in ["rolling.race.a", "rolling.race.b"] {
        let host = std::sync::Arc::clone(&host);
        let barrier = std::sync::Arc::clone(&barrier);
        let intent = intent.clone();
        workers.push(std::thread::spawn(move || {
            let reservation = EconomicRollingReservation {
                wallet_id: "fixture.wallet".to_owned(),
                rail: EconomicRail::Evm,
                network: "sepolia".to_owned(),
                asset: "native:eth".to_owned(),
                requested_at_ms: intent.created_at,
                amount_atomic: intent.amount(),
                max_rolling_24h_atomic: 1_000_000,
            };
            barrier.wait();
            host.lock().unwrap().compare_and_swap(
                key,
                0,
                "{\"version\":1}\n",
                EconomicRollingReservationUpdate::Reserve(&reservation),
            )
        }));
    }
    barrier.wait();
    let dispositions = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        dispositions
            .iter()
            .filter(|value| **value == EconomicAdapterDisposition::Succeeded)
            .count(),
        1
    );
    assert_eq!(
        dispositions
            .iter()
            .filter(|value| **value == EconomicAdapterDisposition::PolicyRejected)
            .count(),
        1
    );
}

#[test]
fn malformed_post_effect_adapter_output_is_terminal_replayable_and_secret_free() {
    let (policy, intent) = rail_fixture(EconomicRail::Evm);
    let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
    let mut host = FullHost::new(intent);
    host.malformed_simulation = true;
    let mut agent = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
    let run = agent.execute(&source).unwrap();
    assert_eq!(run.status(), EconomicRunStatus::AdapterFailed);
    assert!(run.trace().contains("SPX-G210"));
    assert!(run.evidence().contains("SPX-G210"));
    assert!(!run.trace().contains("economic-secret-sentinel"));
    assert!(!run.evidence().contains("economic-secret-sentinel"));
    assert_eq!(
        agent.host.calls,
        ["load", "cas", "snapshot", "simulate", "cas"]
    );
}

#[test]
fn pre_effect_cancellation_is_diagnostic_only_and_invokes_no_authority() {
    let (policy, intent) = rail_fixture(EconomicRail::Evm);
    let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
    let cancellation = AgentCancellation::new();
    cancellation.cancel();
    let mut agent =
        EconomicAgent::new(&policy.source, FullHost::new(intent), cancellation).unwrap();
    let diagnostics = match agent.execute(&source) {
        Ok(_) => panic!("pre-effect cancellation unexpectedly returned Evidence"),
        Err(diagnostics) => diagnostics,
    };
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-I228");
    assert_eq!(diagnostics[0].message, "Economic Agent run was cancelled");
    assert!(agent.host.calls.is_empty());
    assert!(agent.host.journals.is_empty());
}

#[test]
fn cancellation_and_deadline_after_durable_markers_block_the_next_effect() {
    for version in [4, 6] {
        let (policy, intent) = rail_fixture(EconomicRail::Evm);
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let cancellation = AgentCancellation::new();
        let mut host = FullHost::new(intent);
        host.cancel_after_version = Some((version, cancellation.clone()));
        let mut agent = EconomicAgent::new(&policy.source, host, cancellation).unwrap();
        let run = agent.execute(&source).unwrap();
        assert_eq!(run.status(), EconomicRunStatus::Cancelled);
        if version == 4 {
            assert!(!agent.host.calls.contains(&"sign"));
        } else {
            assert!(!agent.host.calls.contains(&"broadcast"));
        }
    }
    let (policy, intent) = rail_fixture(EconomicRail::Evm);
    let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
    let mut host = FullHost::new(intent);
    host.elapsed_after_version = Some((4, policy.limits.max_elapsed_ms + 1));
    let mut agent = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
    let run = agent.execute(&source).unwrap();
    assert_eq!(run.status(), EconomicRunStatus::DeadlineExceeded);
    assert!(!agent.host.calls.contains(&"sign"));
}

#[test]
fn chain_documents_reject_key_order_schema_reference_and_identity_mutations() {
    let intent = evm_intent();
    let mut snapshot = Snapshot {
        rail: EconomicRail::Evm,
        observed: intent.created_at + 1,
        expires: intent.expires_at - 1,
        state: SnapshotState::Evm {
            from: "0x2222222222222222222222222222222222222222".to_owned(),
            nonce: 7,
            base_fee: 1,
            priority: 2,
            gas: 21_000,
        },
        doc: Doc {
            source: String::new(),
            digest: String::new(),
        },
    };
    snapshot.doc.source = render_snapshot(&snapshot);
    snapshot.doc.digest = digest(SNAPSHOT_DOMAIN, snapshot.doc.source.as_bytes());
    assert!(parse_snapshot(&snapshot.doc.source, EconomicRail::Evm).is_ok());
    for hostile in [
        snapshot.doc.source.replace(
            "\"schema\":\"semaprax.economic-agent-chain-snapshot.v1\",\"rail\":\"evm\"",
            "\"rail\":\"evm\",\"schema\":\"semaprax.economic-agent-chain-snapshot.v1\"",
        ),
        snapshot
            .doc
            .source
            .replace("\"network\":\"sepolia\"", "\"network\":\"devnet\""),
    ] {
        assert!(parse_snapshot(&hostile, EconomicRail::Evm).is_err());
    }
    let mutated = parse_snapshot(
        &snapshot.doc.source.replace("\"nonce\":7", "\"nonce\":8"),
        EconomicRail::Evm,
    )
    .unwrap();
    assert!(matches!(mutated.state, SnapshotState::Evm { nonce: 8, .. }));

    let (unsigned, _) = build_unsigned(&intent, &snapshot).unwrap();
    let mut fields = rlp_list_items(&unsigned[1..])
        .unwrap()
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    fields.extend([rlp_u64(1), rlp_u64(1), rlp_u64(1)]);
    let mut signed = vec![2];
    signed.extend(rlp_list(&fields));
    let signed_digest = digest(SIGNED_DOMAIN, &signed);
    let txid = transaction_id(EconomicRail::Evm, &signed).unwrap();
    let broadcast = format!(
            "{{\"schema\":\"{BROADCAST_SCHEMA}\",\"rail\":\"evm\",\"network\":\"sepolia\",\"signed_transaction_digest\":{},\"transaction_id\":{},\"disposition\":\"accepted\",\"observed_at_ms\":{}}}\n",
            quote_json(&signed_digest),
            quote_json(&txid),
            intent.created_at + 2,
        );
    assert!(parse_broadcast(
        &broadcast,
        EconomicRail::Evm,
        "sepolia",
        &signed_digest,
        Some(&txid)
    )
    .is_ok());
    for hostile in [
        broadcast.replace(
            &txid,
            "0x0000000000000000000000000000000000000000000000000000000000000000",
        ),
        broadcast.replace(
            &signed_digest,
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        ),
    ] {
        assert!(parse_broadcast(
            &hostile,
            EconomicRail::Evm,
            "sepolia",
            &signed_digest,
            Some(&txid)
        )
        .is_err());
    }
    assert_eq!(
        parse_broadcast(
            &broadcast.replace(
                "\"disposition\":\"accepted\"",
                "\"disposition\":\"rejected\""
            ),
            EconomicRail::Evm,
            "sepolia",
            &signed_digest,
            Some(&txid)
        )
        .unwrap()
        .disposition,
        "rejected"
    );

    let reconciliation = format!(
            "{{\"schema\":\"{RECONCILIATION_SCHEMA}\",\"rail\":\"evm\",\"network\":\"sepolia\",\"transaction_id\":{},\"status\":\"confirmed\",\"observed_at_ms\":{},\"observed_height\":1,\"confirmations\":1,\"canonical_block_id\":\"fixture.block\"}}\n",
            quote_json(&txid),
            intent.created_at + 3,
        );
    assert!(parse_reconciliation(&reconciliation, EconomicRail::Evm, "sepolia", &txid).is_ok());
    for hostile in [
        reconciliation.replace("\"confirmations\":1", "\"confirmations\":null"),
        reconciliation.replace("\"status\":\"confirmed\"", "\"status\":\"unknown\""),
        reconciliation.replace("\"network\":\"sepolia\"", "\"network\":\"devnet\""),
    ] {
        assert!(parse_reconciliation(&hostile, EconomicRail::Evm, "sepolia", &txid).is_err());
    }
}

#[test]
fn configured_child_limits_are_exact_and_lower_than_global_caps() {
    let (mut policy, intent, invoice) = x402_fixture();
    policy.limits.max_intent_bytes = intent.source.len() as u64;
    assert!(admit_intent(&policy, &intent).is_ok());
    policy.limits.max_intent_bytes -= 1;
    let diagnostic = admit_intent(&policy, &intent).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-G216");
    assert_eq!(
        diagnostic.message,
        format!("intent_bytes exceeds {}", intent.source.len() - 1)
    );

    policy.limits.max_intent_bytes = intent.source.len() as u64;
    policy.limits.max_identifier_bytes = intent.idempotency_key.len() as u64;
    assert!(admit_intent(&policy, &intent).is_ok());
    policy.limits.max_identifier_bytes -= 1;
    assert_eq!(admit_intent(&policy, &intent).unwrap_err().code, "SPX-G216");

    policy.limits.max_identifier_bytes = MAX_IDENTIFIER_BYTES as u64;
    let intent_depth = depth(&serde_json::from_str::<Value>(intent.source.trim_end()).unwrap());
    policy.limits.max_json_depth = intent_depth as u64;
    assert!(admit_intent(&policy, &intent).is_ok());
    policy.limits.max_json_depth -= 1;
    assert_eq!(admit_intent(&policy, &intent).unwrap_err().code, "SPX-G216");

    let mut invoice_limits = limits();
    invoice_limits.max_invoice_bytes = invoice.doc.source.len() as u64;
    assert!(parse_invoice_limited(&invoice.doc.source, &intent, &invoice_limits).is_ok());
    invoice_limits.max_invoice_bytes -= 1;
    let diagnostic = match parse_invoice_limited(&invoice.doc.source, &intent, &invoice_limits) {
        Ok(_) => panic!("over-limit invoice was admitted"),
        Err(diagnostic) => diagnostic,
    };
    assert_eq!(diagnostic.code, "SPX-G216");

    let mut snapshot = Snapshot {
        rail: EconomicRail::Bitcoin,
        observed: intent.created_at + 1,
        expires: intent.expires_at - 1,
        state: SnapshotState::Bitcoin {
            wallet_script: format!("0014{}", "11".repeat(20)),
            height: 100,
            fee_rate: 1,
            utxos: vec![
                Utxo {
                    txid: format!("{}01", "00".repeat(31)),
                    vout: 0,
                    value: 100_000,
                    script: format!("0014{}", "11".repeat(20)),
                    confirmations: 1,
                },
                Utxo {
                    txid: format!("{}02", "00".repeat(31)),
                    vout: 0,
                    value: 100_000,
                    script: format!("0014{}", "11".repeat(20)),
                    confirmations: 1,
                },
            ],
        },
        doc: Doc {
            source: String::new(),
            digest: String::new(),
        },
    };
    snapshot.doc.source = render_snapshot(&snapshot);
    snapshot.doc.digest = digest(SNAPSHOT_DOMAIN, snapshot.doc.source.as_bytes());
    let mut snapshot_limits = limits();
    snapshot_limits.max_snapshot_bytes = snapshot.doc.source.len() as u64;
    snapshot_limits.max_utxos = 2;
    assert!(parse_snapshot_limited(
        &snapshot.doc.source,
        EconomicRail::Bitcoin,
        &snapshot_limits
    )
    .is_ok());
    snapshot_limits.max_utxos = 1;
    let diagnostic = match parse_snapshot_limited(
        &snapshot.doc.source,
        EconomicRail::Bitcoin,
        &snapshot_limits,
    ) {
        Ok(_) => panic!("over-limit UTXO set was admitted"),
        Err(diagnostic) => diagnostic,
    };
    assert_eq!(diagnostic.code, "SPX-G216");
    assert_eq!(diagnostic.message, "utxos exceeds 1");
    snapshot_limits.max_utxos = 2;
    snapshot_limits.max_snapshot_bytes -= 1;
    let diagnostic = match parse_snapshot_limited(
        &snapshot.doc.source,
        EconomicRail::Bitcoin,
        &snapshot_limits,
    ) {
        Ok(_) => panic!("over-limit snapshot was admitted"),
        Err(diagnostic) => diagnostic,
    };
    assert_eq!(diagnostic.code, "SPX-G216");
}

#[test]
fn thirteen_document_x402_raw_sha_and_domain_digest_ledger_is_pinned() {
    let (policy, intent, invoice) = x402_fixture();
    let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
    let mut agent = EconomicAgent::new(
        &policy.source,
        FullHost::with_invoice(intent.clone(), invoice.clone()),
        AgentCancellation::new(),
    )
    .unwrap();
    let run = agent.execute(&source).unwrap();
    let mut documents = vec![
        ("policy", policy.source.as_str(), POLICY_DOMAIN),
        ("intent", intent.source.as_str(), INTENT_DOMAIN),
        ("invoice", invoice.doc.source.as_str(), INVOICE_DOMAIN),
    ];
    for (name, domain) in [
        ("snapshot", SNAPSHOT_DOMAIN),
        ("plan", PLAN_DOMAIN),
        ("simulation", SIMULATION_DOMAIN),
        ("approval_request", APPROVAL_REQUEST_DOMAIN),
        ("approval", APPROVAL_DOMAIN),
        ("journal", JOURNAL_DOMAIN),
        ("broadcast", BROADCAST_DOMAIN),
        ("reconciliation", RECONCILIATION_DOMAIN),
    ] {
        documents.push((name, agent.host.documents[name].as_str(), domain));
    }
    documents.extend([
        ("trace", run.trace(), TRACE_DOMAIN),
        ("evidence", run.evidence(), EVIDENCE_DOMAIN),
    ]);
    assert_eq!(documents.len(), 13);
    let ledger = documents
        .iter()
        .map(|(name, source, domain)| {
            let raw = Sha256::digest(source.as_bytes());
            format!(
                "{name}|{}|{}|{}",
                raw.iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
                digest(domain, source.as_bytes()),
                source.len()
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(ledger, [
            "policy|57ce4d3844f49c9102eb1a2c17f1946305c623e587d4f52a31744ba96ff6114a|sha256:ee623062817928e0088f24b8215705f9aad8e19a52861db6d6051679889c0b53|2987",
            "intent|bfe0695c7e2a5bdfd545b264fb79777cfdadaa449d9089c59753ae3739e36d86|sha256:2a13c2a14cfafba6b4087e647de9e5609c8bb65ddad25c305aa8f5bc28091e2c|670",
            "invoice|38b5b00511f2e461f8df0fe1a830e89109376c893e3c52cea5d23a8d36d8733b|sha256:24cb1025c6beb2a081a05ab504f7d7f6cbb37b27e003da35cbbea003a52ac095|417",
            "snapshot|d005d0f573f337d804d80b8489b63a9f6b03099837b230af69e18a4692b4b9eb|sha256:4123e22e7449e4bbcef812af71337f2e3e5390b4cce20e59f7080b74eeb727d0|309",
            "plan|75418fad0967fa4791d9f146f6997af67b3e76f51dcb2320bfbe2211814bde45|sha256:81dad7aa8e82bdf8ef7b02e2b5a94b899715c86e7d82895c7cdb18c5e7ed28d8|1391",
            "simulation|b3508d24fd29028a9fad89703ba72ade9f4e620eec30f0dd2017b711b96db483|sha256:3a1ac9d741be20bf0d5e35a78e475369a5ccad5c3662bbc5f5365f123df81f1d|369",
            "approval_request|fc932dcef1eb518ba05f463df9e3dd7193ce96df408edb60f3aaa9214a3f19b9|sha256:0833d896f4be4e4feb08d0558e43e7512d589a6c96c368c6ce73ef3a8435adf1|1056",
            "approval|48f716162ae5ec67b28303c5e5c09b641a16a1711b7b83bd4d16a1be6094a56c|sha256:63f3e81facdc9e0ece43b28cbd47310b1a7898662cd4afe3abaae24d06eab8db|1022",
            "journal|f65a8f115c405b086d9a6edb1366a594c87b9f295be8739ea2a56724297f69c9|sha256:9f3d1f568a090c280cfe645536e912d4eb0c18c740bb7309d434ebfd1d1cb169|2394",
            "broadcast|51479da80d60c4e4c363302963010a6675278e53958ce563dafb2892da3c537f|sha256:64882648d5bac5fb58a7408d38e5fa737ba314d27decba34e2106c2310c651a6|335",
            "reconciliation|b1b449375018c27465332384d67205438bea8a2660d3144c417e5de5d5198ba1|sha256:e26e7655d758b53867228950de241c3265675f18687110550fc89ecdc46f2b4a|301",
            "trace|a388543ab6c1a57a0b7798fbd0c5d721bb33c0ab7f7123f3bb8f24c4c965db58|sha256:f28c44894b93948068381bb9047fedade3b855cdd1831a992466a08fa97f6f11|11023",
            "evidence|2d4d4164476bd4fdd037f138b264d0a72728b125d6819baae87da165242788b0|sha256:9dd80e5a13aaaa02b5b854cee0f68870ac22dfdabca14e79d32857ae35980cc6|17399",
        ]);
    for (name, source, domain) in documents {
        let mut mutated = source.as_bytes().to_vec();
        let index = mutated.iter().position(|byte| *byte == b'v').unwrap();
        mutated[index] = b'w';
        assert_ne!(
            Sha256::digest(source.as_bytes()),
            Sha256::digest(&mutated),
            "{name}"
        );
        assert_ne!(
            digest(domain, source.as_bytes()),
            digest(domain, &mutated),
            "{name}"
        );
    }
}

#[test]
fn journal_uncertainty_never_retries_in_process_and_reload_governs_persistence() {
    for persisted in [false, true] {
        let (policy, intent) = rail_fixture(EconomicRail::Evm);
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut host = FullHost::new(intent.clone());
        host.cas_fault = Some((2, EconomicAdapterDisposition::FailedUncertain, persisted));
        let mut agent = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
        let run = agent.execute(&source).unwrap();
        assert_eq!(run.status(), EconomicRunStatus::JournalFailed);
        assert_eq!(
            agent
                .host
                .calls
                .iter()
                .filter(|call| **call == "cas")
                .count(),
            2
        );
        assert_eq!(
            agent
                .host
                .calls
                .iter()
                .filter(|call| **call == "approve")
                .count(),
            0
        );
        let journals = std::mem::take(&mut agent.host.journals);
        let retained_version =
            serde_json::from_str::<Value>(journals[&intent.idempotency_key].trim_end()).unwrap()
                ["version"]
                .as_u64()
                .unwrap();
        assert_eq!(retained_version, if persisted { 2 } else { 1 });

        let mut restart_host = FullHost::new(intent.clone());
        restart_host.journals = journals;
        let mut restart =
            EconomicAgent::new(&policy.source, restart_host, AgentCancellation::new()).unwrap();
        let restarted = restart.execute(&source).unwrap();
        assert_eq!(
            restarted.status(),
            if persisted {
                EconomicRunStatus::JournalFailed
            } else {
                EconomicRunStatus::Confirmed
            }
        );
        assert_eq!(restart.host.calls.contains(&"snapshot"), !persisted);
        if persisted {
            assert_eq!(restart.host.calls, ["load"]);
            assert!(restart.host.rolling_updates.is_empty());
        } else {
            assert!(restart
                .host
                .rolling_updates
                .iter()
                .all(|update| *update == "retain"));
        }
    }

    let (policy, intent) = rail_fixture(EconomicRail::Evm);
    let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
    let mut host = FullHost::new(intent);
    host.cas_fault = Some((1, EconomicAdapterDisposition::DefinitelyNotStarted, false));
    let mut agent = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
    let run = agent.execute(&source).unwrap();
    assert_eq!(run.status(), EconomicRunStatus::JournalFailed);
    assert_eq!(agent.host.calls, ["load", "cas"]);
    assert!(agent.host.journals.is_empty());
    assert!(agent.host.rolling.values().all(Vec::is_empty));
}

#[test]
fn economic_process_kill_markers_never_repeat_sign_or_broadcast() {
    const ROLE: &str = "SEMAPRAX_ECONOMIC_KILL_ROLE";
    const DIRECTORY: &str = "SEMAPRAX_ECONOMIC_DURABLE_DIR";
    const STAGE: &str = "SEMAPRAX_ECONOMIC_KILL_STAGE";
    if std::env::var_os(ROLE).is_some() {
        let (policy, intent) = rail_fixture(EconomicRail::Evm);
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut agent = EconomicAgent::new(
            &policy.source,
            FullHost::new(intent),
            AgentCancellation::new(),
        )
        .unwrap();
        let result = agent.execute(&source);
        if std::env::var_os(STAGE).is_none() {
            assert!(result.is_ok());
        }
        return;
    }
    let executable = std::env::current_exe().unwrap();
    for stage in [
        "v4",
        "sign_effect",
        "v5",
        "v6",
        "broadcast_effect",
        "odd",
        "even",
    ] {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-economic-kill-{}-{stage}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let mut child = std::process::Command::new(&executable)
                .args([
                    "economic_agent::tests::economic_process_kill_markers_never_repeat_sign_or_broadcast",
                    "--exact",
                    "--nocapture",
                ])
                .env(ROLE, "child")
                .env(DIRECTORY, &directory)
                .env(STAGE, stage)
                .spawn()
                .unwrap();
        let ready = directory.join("ready");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !ready.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(ready.exists(), "child did not reach {stage}");
        child.kill().unwrap();
        let _ = child.wait().unwrap();
        let status = std::process::Command::new(&executable)
                .args([
                    "economic_agent::tests::economic_process_kill_markers_never_repeat_sign_or_broadcast",
                    "--exact",
                    "--nocapture",
                ])
                .env(ROLE, "resume")
                .env(DIRECTORY, &directory)
                .status()
                .unwrap();
        assert!(status.success(), "resume failed at {stage}");
        let calls = std::fs::read_to_string(directory.join("calls")).unwrap();
        let sign_calls = calls.lines().filter(|call| *call == "sign").count();
        let broadcast_calls = calls.lines().filter(|call| *call == "broadcast").count();
        assert!(sign_calls <= 1);
        assert!(broadcast_calls <= 1);
        if stage == "sign_effect" {
            assert_eq!(sign_calls, 1);
            assert_eq!(broadcast_calls, 0);
            let journal = std::fs::read_to_string(directory.join("journal")).unwrap();
            let value: Value = serde_json::from_str(journal.trim_end()).unwrap();
            assert_eq!(value["version"], 4);
            assert_eq!(value["state"], "approved");
        }
        if stage == "broadcast_effect" {
            assert_eq!(sign_calls, 1);
            assert_eq!(broadcast_calls, 1);
            let journal = std::fs::read_to_string(directory.join("journal")).unwrap();
            let value: Value = serde_json::from_str(journal.trim_end()).unwrap();
            assert_eq!(value["state"], "confirmed");
            assert!(value["version"].as_u64().unwrap() >= 8);
        }
        for name in ["ready", "journal", "calls"] {
            let path = directory.join(name);
            if path.exists() {
                std::fs::remove_file(path).unwrap();
            }
        }
        std::fs::remove_dir(directory).unwrap();
    }
}

#[test]
fn reconciliation_authority_is_durably_bounded_at_exact_sixty_four() {
    let (policy, intent) = rail_fixture(EconomicRail::Evm);
    let idempotency = intent.idempotency_key.clone();
    let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
    let mut host = FullHost::new(intent);
    host.reconciliation_status = "pending";
    let mut agent = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
    let first = agent.execute(&source).unwrap();
    assert_eq!(first.status(), EconomicRunStatus::Pending);
    for _ in 1..64 {
        let observation = agent.reconcile(&idempotency, &source).unwrap();
        assert_eq!(observation.status(), EconomicRunStatus::Pending);
    }
    let calls = agent
        .host
        .calls
        .iter()
        .filter(|call| **call == "reconcile")
        .count();
    assert_eq!(calls, 64);
    let exhausted = agent.reconcile(&idempotency, &source).unwrap();
    assert_eq!(exhausted.status(), EconomicRunStatus::BudgetExhausted);
    assert_eq!(
        agent
            .host
            .calls
            .iter()
            .filter(|call| **call == "reconcile")
            .count(),
        64
    );
}
