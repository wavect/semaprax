# browser-capture REPLAY

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The exact raw job log is retained losslessly in job.log.gz. Verify compressed-log.json and decompress only into external-SSD scratch, then use the unchanged strict read-graph-capture.py predicates with current head 487253096928c72bf2ac9180b6b65a0e3838d5af, run 39163104281 and job 117780935995. The reader checks provenance, strict base64/JSON and the domain/u64 graph digest, then proves complete wire equality except builder-byte usage and the resulting self digest against the actual 31e capture.
