# claude resolution (paid-classifier-receiving-f8)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Local exact source resolves reviewer omissions: allocation.rs::builder_refused reads bounded_output::active_refused for ordinary builders; Payer::inventory_work first calls inventory_text::current then budget.charge, which invokes prepare_builder before any zero-unit debit. Entry inventory_text::current calls reserve_bytes(0) before acquisition, so a retry cannot reacquire backing. Index58 budget was constructed before ACTIVE existed; moved construction (not just preparation) selects actual existing active payer. Invocation::exit is authority release and returns Ok even after account refusal; the new controller preserves first receipts and requires successful release. No reviewer speculation used to alter quotas or old receivers.
