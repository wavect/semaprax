# Image agent application error data v1

Status: implemented with focused local codec and generated-client evidence;
no hosted, cross-platform, packaged-SDK, or exact-subject archive is claimed.

Audience: protocol implementers, generated-client authors, embedding hosts,
and workflow reviewers.

## Purpose

The v5 image protocol keeps the ordinary JSON-RPC error `code` and `message`.
When an admitted method reaches compiler application logic and returns one or
more SEMAPRAX diagnostics, the error also carries a closed `data` value:

```json
{
  "schema": "semaprax.image-agent-application-error-data.v1",
  "diagnostics": [
    {
      "code": "SPX-G282",
      "severity": "error",
      "message": "v5 expected image revision is stale",
      "path": null,
      "location": null,
      "help": null
    }
  ]
}
```

The diagnostic array preserves compiler order. Every row contains all six
fields. `path`, `location`, and `help` are explicitly nullable; omission is not
equivalent to `null`. A location, when present, contains `line`, `column`,
`start`, and `end` as nonnegative integers. Unknown fields, foreign schema
identities, invalid severities, and malformed locations are rejected by the
typed generated-client decoder.

## Wire boundary

Structured data is emitted only for v5 application errors after a method has
been selected and its compiler operation returns diagnostics. Parse errors,
invalid request grammar, unavailable methods, invalid parameters, and bounded-
response overflow keep the generic JSON-RPC `{code,message}` error. Clients
must therefore model `data` as optional and must not infer its absence as
success, absence of diagnostics, or permission to retry.

This is an exact-profile extension. An older generated v5 client whose closed
error shape predates `data` will reject the new application-error response.
Hosts must distribute and use the generated client from the same selected
discovery document; this profile does not claim wire compatibility with those
older generated v5 clients.

The complete response remains under the existing v5 response-byte cap. If the
structured response cannot fit, the existing correlated overflow response is
returned without `data`. The server never truncates or partially serializes the
diagnostic array to make it fit.

## Generated clients

The selected TypeScript, Python, and Rust clients expose a typed decoder that
returns the ordinary success envelope or a distinct typed RPC failure retaining
the optional application-error data. The generic-surface decoder keeps its
existing success type and error surface. Neither decoder performs I/O,
selects a capability, chooses a workflow transition, applies a repair, retries
a request, or grants authority.

The structured diagnostic is evidence about one failed request only. Its code
does not by itself establish source freshness, publication state, rollback,
runtime behavior, or a valid repair. The caller must combine it with the
selected workflow state and the workflow's closed transition policy.

## Nonclaims

This profile does not provide typed transport failures, MCP request
cancellation, automatic repair selection, automatic retries, exactly-once
delivery, session durability, or publication-state recovery. It does not alter
older protocol profiles or turn diagnostics into source, test, build, network,
filesystem, approval, or publication authority.
