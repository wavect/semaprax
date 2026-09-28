//! Genuine runtime/lease data checks. These do not attest authorize execution.
use super::*;
use crate::agent_lifecycle::authorization::checked_owned_wait_ready_commitments_v8;

fn encode(
    context: &CheckedOwnedWaitJournalContextV8,
    key: &SourceCheckpointKey,
    rows: &[EntryV8],
) -> Vec<u8> {
    let mut document = Vec::new();
    let mut prev = "0".repeat(64);
    for (seq, row) in rows.iter().enumerate() {
        let bytes = wire::encode(
            row,
            &ExpectedRowV8 {
                invocation: context.ordinary().invocation(),
                generation: context.generation(),
                seq: u32::try_from(seq).unwrap(),
                prev_mac: &prev,
                ordinary: context.ordinary(),
            },
            key,
        )
        .unwrap();
        prev = wire::parse(&bytes[..bytes.len() - 1]).unwrap()["authentication"]
            .as_str()
            .unwrap()
            .to_owned();
        document.extend(bytes);
    }
    document
}
impl CheckedOwnedWaitJournalContextV8 {
    pub(crate) fn test_ready_without_runtime(
        &self,
        lease: &SourceOwnedWaitLeaseV8,
        key: &SourceCheckpointKey,
        document: &[u8],
    ) -> Result<usize, SourceJournalError> {
        let plain = checked_owned_wait_journal_context_v8(
            Arc::clone(&self.execution),
            lease,
            &self.registration,
        )?;
        plain.test_inventory_len(lease, key, document)
    }
    pub(crate) fn test_ready_documents(
        &self,
        key: &SourceCheckpointKey,
    ) -> (Vec<u8>, Vec<Vec<u8>>) {
        let mut rows = super::super::inventory::tests::rows_for(self.fold(), key);
        let EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationStaged {
            turn,
            attempt,
            state_digest,
            decision,
            decision_digest,
            ..
        }) = &rows[17]
        else {
            panic!()
        };
        let EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferCompleted { state, .. }) =
            &rows[15]
        else {
            panic!()
        };
        let EntryV8::Ordinary(SourceJournalEntry::AttemptSettled { response, .. }) = &rows[9]
        else {
            panic!()
        };
        let decoded = self
            .execution
            .wait()
            .lifecycle()
            .proposal_schema()
            .decode(std::str::from_utf8(response).unwrap())
            .unwrap();
        let scope = &self.registration.expected_facts().scope;
        let proposal = crate::resumable_effects::owned_frame::v2::bind_owned_wait_proposal_v8(
            self.execution.wait(),
            scope,
            &decoded,
        )
        .unwrap();
        let (runtime, execution) = self.ready_runtime().unwrap();
        let commitments = checked_owned_wait_ready_commitments_v8(
            runtime, execution, scope, *turn, state, decision, &proposal,
        )
        .unwrap();
        // All three facts remain inert. Changing the source turn changes the
        // existing policy precursor and both authorization/grant commitments.
        let other_turn = checked_owned_wait_ready_commitments_v8(
            runtime,
            execution,
            scope,
            turn + 1,
            state,
            decision,
            &proposal,
        )
        .unwrap();
        assert_ne!(
            commitments.authorization_binding(),
            other_turn.authorization_binding()
        );
        assert_ne!(commitments.grant_digest(), other_turn.grant_digest());
        assert_eq!(commitments.argument_digest(), other_turn.argument_digest());
        let turn = *turn;
        let attempt = *attempt;
        let ready = model::OwnedBodyV8::OwnedAuthorizationReady {
            turn,
            attempt,
            staged: 17,
            state_digest: state_digest.clone(),
            decision_digest: decision_digest.clone(),
            grant_digest: commitments.grant_digest().into(),
        };
        rows.push(EntryV8::Owned(ready));
        rows.push(EntryV8::Ordinary(
            SourceJournalEntry::AuthorizationConsumed {
                turn,
                attempt,
                grant_digest: commitments.grant_digest().into(),
            },
        ));
        let positive = encode(self, key, &rows);
        let mut negatives = Vec::new();
        // Coherent inert commitments do not prove that source authorize ran.
        // Here only the seal and its Decision sidecar are changed, leaving the
        // previously recorded Ready/Consumed grant stale; admission must fail.
        let mut seal = rows.clone();
        let EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationStaged {
            decision,
            decision_digest,
            ..
        }) = &mut seal[17]
        else {
            panic!()
        };
        let field = decision["fields"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|f| f["identity"] == self.execution.wait().authorize().seal().as_str())
            .unwrap();
        field["value"]["hex"] = serde_json::json!("00");
        *decision_digest = wire::recipe_digest(wire::RecipeV8::Decision, &serde_json::json!({"scope":self.fold.created_scope_for_test(),"turn":turn,"attempt":attempt,"authorize":self.fold.authorize,"decision":decision})).unwrap();
        let new_digest = decision_digest.clone();
        let EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationReady {
            decision_digest, ..
        }) = &mut seal[18]
        else {
            panic!()
        };
        *decision_digest = new_digest;
        negatives.push(encode(self, key, &seal));
        let mut grant = rows.clone();
        let EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationReady { grant_digest, .. }) =
            &mut grant[18]
        else {
            panic!()
        };
        *grant_digest = format!("sha256:{}", "9".repeat(64));
        let EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed { grant_digest, .. }) =
            &mut grant[19]
        else {
            panic!()
        };
        *grant_digest = format!("sha256:{}", "9".repeat(64));
        negatives.push(encode(self, key, &grant));
        let mut consumed = rows.clone();
        let EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed { grant_digest, .. }) =
            &mut consumed[19]
        else {
            panic!()
        };
        *grant_digest = format!("sha256:{}", "8".repeat(64));
        negatives.push(encode(self, key, &consumed));
        let mut sequence = rows.clone();
        let EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationReady { staged, .. }) =
            &mut sequence[18]
        else {
            panic!()
        };
        *staged = 16;
        negatives.push(encode(self, key, &sequence));
        let mut sdk = rows.clone();
        let EntryV8::Ordinary(SourceJournalEntry::AttemptSettled {
            response,
            response_digest,
            ..
        }) = &mut sdk[9]
        else {
            panic!()
        };
        *response = std::str::from_utf8(response)
            .unwrap()
            .replace(
                "\"fixture.agent.type.proposal.sequence\":\"1\"",
                "\"fixture.agent.type.proposal.sequence\":\"0\"",
            )
            .into_bytes();
        *response_digest = super::super::super::source_response_digest(response);
        negatives.push(encode(self, key, &sdk));
        let mut state_rows = rows.clone();
        let EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferCompleted { state, .. }) =
            &mut state_rows[15]
        else {
            panic!()
        };
        state["fields"][1]["value"]["value"] = serde_json::json!(11);
        negatives.push(encode(self, key, &state_rows));
        (positive, negatives)
    }
}
impl FoldContextV8 {
    fn created_scope_for_test(&self) -> &Value {
        let model::OwnedBodyV8::OwnedRunCreated { scope, .. } = &self.created else {
            panic!()
        };
        scope
    }
}
