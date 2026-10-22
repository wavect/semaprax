//! Existing clause-local owner/result-view admission; no new language authority.
pub(super) const CARD: &str = r#"Contract owners: source implementation; executable qualification pending.
```spx
module help.contracts;
@id("help.consume") fn consume(value:own Bytes)->bool {byte_len(bytes_as_slice(value))==2usize}
@id("help.make") fn make()->Bytes
 requires {let local=bytes_zeroed(2usize);let moved=local;consume(moved)}
 ensures {let text="ok";string_len(text)==2}
 ensures byte_len(bytes_as_slice(result))==2usize {return bytes_zeroed(2usize);}
@id("help.text") fn text()->string
 ensures byte_len(str_as_bytes(string_as_str(result)))==2usize {return "ok";}
@id("main") fn main()->i64 {let output=make();i64_from_usize(byte_len(bytes_as_slice(output)))}
```
Each requires/ensures clause admits only fresh literal-sized bytes_zeroed (0..131072 bytes), String literals and available named aliases of those owners. Bind before an owning pure call; inline constructors and general owned call results remain closed.
SPX-O105: do not transfer a parameter, body owner or result in a contract. Duplicate moves retain SPX-O101; a local cannot cross clauses (SPX-T202). Captures/aggregates/projections do not establish freshness.
Only ensures may borrow its exact provisional Bytes result with bytes_as_slice(result), or String result with string_as_str(result). result before completion: SPX-T201; wrong view type: SPX-T263. Projected/temporary result roots and escaping views stay refused. No result transfer or invented owner.
SPX-T267: requires/body/ensures share the existing 32 allocation-event and 2097152-byte cumulative census, without refunds. Requires locals settle before body; ensures locals and non-result owners settle before publication. Failure stays sticky and publishes no result; physical OOM has no new recovery.
docs/LEXICAL-STRUCTURED-EXITS-V1.md; help language author:lexical-exits.
"#;

#[cfg(test)]
mod tests;
