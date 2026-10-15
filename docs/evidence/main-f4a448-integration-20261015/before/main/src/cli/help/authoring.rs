//! Bounded, authority-free discovery over the embedded checked library catalog.
use std::fmt::Write as _;

mod discovery;

const MAX_BYTES: usize = 2_048;
const ROUTES: &str = "Authoring routes (complete):\n  author:vec-constructor typed Vec source; ordinary per-item commits\n  author:lexical-exits statement-tail break/continue/return\n  author:stdin-json v27 bounded native stream command\n  author:stream-data-v2           v29 private record/Vec stream command\n  author:owned-data v30 private owned-leaf collections\n  author:collection-records v31 nested records containing Vec\n  author:nested-outcomes v32 typed nested success/error carriers\n  author:file-text native UTF-8 file command\n  author:selected-file native file\n  author:selected-file-resource v34\n  author:source-web single-source web build\n  author:literal-format literal String rendering\n  author:string-take named String owner transfer\n  author:static-utf8              borrowed UTF-8\n  author:string-from-utf8         strict bytes-to-String\n  author:copy-record-vec flat Copy-record vectors\n  author:record-sort-keys explicit stable field sort policies\n  author:unique-by field duplicate check\n  author:record-field-views borrowed record and Vec field reads\n  author:record-traversal scoped borrowed rows and optional indices\n  author:byte-traversal named immutable byte-slice traversal\n  author:json-codec checked source JSON codec derivation\n  author:json-identifier-views one identifier as a token view\n  author:json-request-views bounded identifier and record arrays\n  author:json-stream-request-views bounded native stream request view\n  author:json-owned-request bounded owning request collections\n  author:json-utf8-owned-request bounded UTF-8 string values\n  author:json-stream-utf8-owned-request bounded UTF-8 stdin requests\n  author:json-collection-response nested collection encoder\n  author:json-nested-request finite nested request decoder\n  author:json-stream-nested-request finite nested stdin decoder\n  author:json-nested-response finite nested borrowed encoder\n  author:json-request-response typed request/response composition boundaries\nCard search: help language author:find:<word>:0\nLibrary-only search: help language find:<word>:0\n";

#[cfg(test)]
pub(super) fn assert_guide_contract() {
    let guide = super::AUTHORING_GUIDE;
    assert_eq!(
        guide,
        include_str!("../../../docs/AGENT-AUTHORING-GUIDE.md")
    );
    assert!(guide.len() <= 2_048);
    assert!(semaprax::agent_economics::lexical_tokens(guide) <= 400);
    assert!(guide.contains("internal-strings-v1` or `text-toolkit-v1`"));
    assert!(guide.contains("Project profiles are distinct"));
    assert!(guide.contains("source-command.v1"));
    assert!(guide.contains("help language all"));
    assert!(guide.contains("fused-construction-v1"));
    assert!(guide.contains("--managed-bytes N"));
    assert!(!guide.contains("there is no native `--profile` option"));
}

pub(super) fn lookup(query: &str) -> Result<String, String> {
    if query.starts_with("author:find:") {
        return discovery::search(query, MAX_BYTES);
    }
    match query {
        "author:routes" => Ok(ROUTES.to_owned()),
        "author:vec-constructor" => Ok(concat!(
            "Typed Vec source constructor; executable qualification pending.\n",
            "let values = vec_of<i64>(19, 23); empty: vec_of<i64>(). One explicit admitted element type; exact count/capacity, existing Vec/event/payload/source bounds.\n",
            "Capacity is allocated first; items evaluate once left to right. Each vec_push keeps ordinary staged-argument commit. Failure settles the committed prefix and staged owners once, keeps the selected status and evaluates no suffix.\n",
            "P220: one explicit type, reserved vec_of name, and bounded expansion. T318: generated-owner shape/hygiene disagrees; reparse canonical source. Use an explicit Vec chain when intermediate owners must be authored/read.\n",
            "A constructor is not a variadic Call or bulk allocator; String clone and Bytes event rules stay unchanged. Graph v81 preserves source provenance; frozen public/interop profiles refuse it. No new ABI or capability. docs/TYPED-VEC-CONSTRUCTORS-V1.md.\n"
        ).to_owned()),
        "author:lexical-exits" => Ok(concat!(
            "Lexical structured exits v1: source implementation; executable qualification pending.\n",
            "Inside an admitted loop: if skip { continue; }; if done { break; }; followed by the ordinary body tail. An unconditional exit ends its block: do not append dummy 0 or unreachable effects.\n",
            "An intermediate Copy-valued if/else can discard its result with a semicolon. A bare guard containing an exit may end with or without one before the next item: if skip { continue; } else { 0 }; canonical text drops it. let _ = guard or an operator suffix stays SPX-T312; a final ; } stays SPX-P106.\n",
            "Normal guard arm: exact else { 0 }; nonzero/bool/owned/view arms refuse SPX-T312. Move work after the guard.\n",
            "break; and continue; target the nearest loop. while continue checks its condition; for continue advances its authenticated iterator exactly once; for own break settles remaining items. No labels or valued break.\n",
            "return value; targets this function with its exact result type. A loop may hand off a whole named owner; its operand still follows ordinary loop operation/loan rules. Accepted acyclic guard/terminal returns keep their value projection.\n",
            "Exits are direct function/loop/control-block statement tails, never inside initializers, call arguments, aggregates, return operands, closures, unsafe or yields functions. SPX-T312: move the exit to a statement guard. SPX-T313: break/continue need an enclosing loop. SPX-T103/O104: preserve exact result type and owner transfer.\n",
            "Owned returns stage the result before inner cleanup; root locals remain through ensures. Failure never publishes a result; existing staged-argument failure rules remain. A moved outer owner cannot be revived by break/continue.\n",
            "No public bottom type, ABI, capability, new carrier or old report-profile admission. H006 refuses forged target/latch identities. Cleanup v18 and loan v3 are selective; authenticated Bytes-helper renewal additionally selects v19. Old functions retain their schemas. Contract and pending gates: docs/LEXICAL-STRUCTURED-EXITS-V1.md.\n"
        ).to_owned()),
        "author:stdin-json" => Ok(concat!(
            "Native stream data route (complete guidance; exact library lookup follows):\n",
            "new app --template stdin-stream-data\n",
            "Project profile language-command-io.stream-data.v1; input argv-utf8+stdin-stream.v1; native64 only.\n",
            "Capabilities: process.args.read, process.stderr.write, process.stdin.read, process.stdout.write.\n",
            "Follow generated semaprax.toml and README; check app; test app; build app --target native -o app-bin.\n",
            "Public entry and command stay fn() -> i64. Private borrow Vec<Copy scalar> helpers are admitted; authored nominal helper closure is refused by this profile.\n",
            "JSON is parsed by ordinary declared source dependencies; scan bounded input views, preserve the reader owner and chunk lifetime.\n",
            "Exact prerequisites/signatures: help library std.data.json.scan; help library std.data.json.token.\n",
            "Search bundled declarations: help language find:json:0. For v29 private record helpers: help language author:stream-data-v2.\n",
            "Web, Wasm and npm targets refuse this command profile before emission. Help grants no provider authority.\n"
        ).to_owned()),
        "author:stream-data-v2" => Ok(concat!(
            "Project v29 source implementation; focused and hosted qualification pending.\n",
            "Select [package] profile = \"language-command-io.stream-data.v2\" with input argv-utf8+stdin-stream.v1; native64 selected command only.\n",
            "The command and entry remain fn() -> i64. Private helpers admit explicit flat Copy records, owned/borrowed Vec<R>, and the exact codec/stream/collection outcomes.\n",
            "The v27 profile stays scalar-vector-only. No public record ABI, generic collection helper, or provider grant is added.\n",
            "Generate checked request views with help language author:json-codec; use source-owned codec helpers and keep Ready bytes live through decoding.\n",
            "Exact shapes and target boundary: docs/STREAM-DATA-COMMAND-V2.md; Copy Vec: help language author:copy-record-vec.\n"
        ).to_owned()),
        "author:owned-data" => Ok(concat!(
            "Project v30 source implementation; focused current-head qualification pending.\n",
            "Select [package] profile = \"language-command-io.owned-data.v1\" with input argv-utf8+stdin-stream.v1; native64 selected command only.\n",
            "Entry and command remain fn() -> i64 with the same four explicit command grants. Private helpers admit checked owned-leaf Vec/Iter carriers alongside v29's Copy records and codec outcomes.\n",
            "Choose v30 when private helpers need owned-leaf Vec/Iter runtime; v29 covers private Copy-record Vec helpers and codec outcomes, while v27 remains scalar-Vec-only. Schema declarations alone grant no carrier.\n",
            "vec_clone_at deep-copies an element; vec_replace, vec_reserve_owned and vec_sort_owned transfer the collection owner. Consuming traversal uses for own and vec_into_iter.\n",
            "Source spells String as lowercase string. Write an owning parameter as text: string (not text: own string, SPX-O002); give each user record and field its own @id. Vec<string> push consumes the String.\n",
            "Use string_from_utf8(bytes: borrow Slice<u8>) -> string (core.string.from_utf8) to copy exact strict UTF-8 into a detached String. NUL, BOM and noncharacters stay; malformed UTF-8 fails semaprax.convert.v1 code 1, never replacement or normalization. This adds no public ABI, carrier layout or capability.\n",
            "Bytes allocation/copy stays refused in loops (SPX-T267), except independently proved empty Vec clone refusal before copying. Unknown/nonempty clones and allocating helpers stay closed; stage clones outside loops. Checked String loops remain distinct.\n",
            "V27 and v29 stay closed to these owned carriers, including unused helpers. No public nominal ABI, ambient grant or Web/Wasm/npm command route is added.\n",
            "For the exact strict byte-to-String shape and refusal domain: help language author:string-from-utf8.\n",
            "Outer Row Vec renewal: out=vec_push<Row>(out,item) in for own. Contract: docs/CONSUMING-OWNED-RECORD-VEC-RENEWAL-V1.md.\n",
            "Exact source shapes, admission, cleanup and gates: docs/STREAM-OWNED-DATA-COMMAND-V1.md and docs/OWNED-LEAF-COLLECTIONS-V1.md.\n"
        ).to_owned()),
        "author:string-from-utf8" => Ok(concat!(
            "Strict owned String; current-head qualification pending.\n",
            "let view = bytes_as_slice(bytes); let text = string_from_utf8(view);\n",
            "bytes_zeroed_bounded(requested,131072usize): literal bound <=131072, fully charged; oversize refuses before allocation, semaprax.byte-buffer.v1 code 2. Allocate outside arbitrary while loops; fill by same-owner stores or checked helpers: docs/NONALLOCATING-BYTE-HELPER-RENEWAL-V1.md. Finite owned Vec<Bytes> traversal admits literal local scratch within cumulative32-event/2MiB caps; unknown bounds/escaping scratch refuse: docs/FINITE-OWNED-ITERATOR-BYTE-SCRATCH-V1.md. Native allocation exhaustion remains fail-stop; bytes_zeroed stays literal-only. docs/OWNED-BOUNDED-BYTE-BUFFER-V1.md.\n",
            "string_from_utf8(input: borrow Slice<u8>) -> string (core.string.from_utf8): call-scoped read, fresh detached String; ordinary input-owner loan rules.\n",
            "Input must be strict UTF-8. Empty input and exact NUL, BOM, noncharacters, and 1/2/3/4-byte scalars are retained byte-for-byte. Overlong, lone-continuation, truncated, surrogate, and >U+10FFFF spellings fail semaprax.convert.v1 code 1 with no result; there is no replacement or normalization.\n",
            "Owned String commands select language-command-io.owned-data.v1 (v30). No new public/standalone adapter, ABI, layout, capability, raw-input limit or lossy conversion.\n",
            "Existing Fixed interpreter meters charge one materialization plus its bytes atomically across helper calls; empty success still charges one. V10 refuses this intrinsic. Native/v30 command evaluation has no corresponding 4096 counter; neither caps nor raw-input limits change.\n",
            "JSON fast paths first validate the complete token/schema; only unescaped token payloads use this copy. Escaped tokens keep their scalar path; this operation does not unescape or quote JSON.\n",
            "Wrong arity/type/generic use and reserved-name forgery remain T204/T205/T225/S113 refusals. Contract: docs/BULK-UTF8-STRING-V1.md. Core interpreter/native/aggregate-Wasm and codec fast-path qualification are still pending.\n"
        ).to_owned()),
        "author:collection-records" => Ok(concat!(
            "Project v31 source only; execution qualification pending.\n",
            "new app --template stdin-stream-collection-record\n",
            "Select `language-command-io.collection-record.v1`; entry/command remain fn() -> i64, native64, four grants.\n",
            "Private helpers: finite acyclic records, admitted Vec elements, nested records and Copy/string/Bytes fields; existing capacities only.\n",
            "Borrow `value: borrow Report`; `vec_len(value.items)` reads length. For queries use `vec_field<Row>(value.items,index,\"selected\")`; SPX-T310 requires one admitted record type, a named Vec, usize index and exact literal field. Copy returns by value; String/Bytes are `borrow str`/`borrow Slice<u8>` views. Don’t deep-clone for scalar queries: `vec_clone_at` is for owned results. Fuse with `str_as_bytes(vec_field<Row>(value.items,index,\"text\"))`.\n",
            "Compare str views with `equals` from std.text, @id(\"std.text.equals\"); no clone.\n",
            "Named paths support `string_as_str(value.inner.text)` and fused `str_as_bytes(string_as_str(value.inner.text))`. Views pin the whole vector generation through last use: do not move, replace, sort, push, reserve, clear or consume it before then. Temporary/call roots and escaping views stay refused.\n",
            "Select v31 for Report{items: Vec<Row>, metrics: Metrics}; v30 keeps its owned-leaf boundary, and older stream profiles refuse nested helpers (SPX-G172). Internal v30/v31/v32 only; no public nominal ABI or declaration-only carrier authority.\n",
            "examples/collection-record-order: Vec<Row> fields department:i64, priority:i64, id:string, ordinal:i64. vec_sort_owned<Row> orders by declared fields (scalar value, String UTF-8 bytes); traverse with for own and vec_into_iter. Empty/full cases cover tied first keys.\n",
            "Checked JSON request decoding: author:json-stream-utf8-owned-request. Encode-only response derivation: author:json-collection-response; select its bounded-collection-response.v1 profile explicitly.\n",
            "Exact runtime boundary and gates: docs/PROJECT-V31-COLLECTION-RECORD-COMMAND-V1.md. Public nominal ABIs and Web/Wasm/npm command targets remain closed.\n"
        ).to_owned()),
        "author:nested-outcomes" => Ok(concat!(
            "Project v32 typed nested outcomes; current-head qualification pending.\n",
            "Select [package] profile = \"language-command-io.nested-outcome.v1\" with argv-utf8+stdin-stream.v1 and native64. Entry and command remain fn() -> i64; independently declare/grant process.args.read, process.stderr.write, process.stdin.read and process.stdout.write.\n",
            "Use v31 for Report{items: Vec<Row>, metrics: Metrics}; use v32 when a private helper returns a variant carrying that whole owned record. V31 and older profiles refuse the new carrier, even in unused helpers.\n",
            "The variant is explicit, monomorphic and invariant-free: exactly two cases, one with one owning record field, the other with i64, usize, i64 fields in that order. Every nested record/case/field needs its own @id; names and case order grant no authority. Existing Vec element/capacity rules apply. Depth<=64 includes the variant, expanded fields<=4096, owned leaves<=256. Generic/nested/recursive variants and optional fields remain closed.\n",
            "Import every named type exposed by an imported helper's signature, including the outcome, payload and nested records, directly from its declaring module (SPX-G172); inferred return types are not import authority.\n",
            "A nested decoder returns Ready{value} or Error{code,offset,field}. Use match own outcome, then match own value to move payload fields; end each arm with a comma inside its match braces. Borrow views only while the bound payload is live; a variant is not a record field root. Error cases need no dummy record.\n",
            "Keep command arms Copy-valued. An owning match result must independently fit the finite collection-outcome join; arbitrary String/record/variant returns are not admitted by a generated name.\n",
            "Example: examples/stream-nested-order-json-project. Contracts: docs/OWNED-NESTED-OUTCOMES-V1.md, docs/PROJECT-V32-NESTED-OUTCOME-COMMAND-V1.md. Codec: author:json-nested-request or author:json-stream-nested-request.\n"
        ).to_owned()),
        "author:static-utf8" => Ok(concat!(
            "Static borrowed UTF-8 source implementation; executable qualification pending.\n",
            "let key = str\"é\\u{0}\"; let view = str_as_bytes(key); byte_len(view) == 3usize. Adjacent str\"...\" is immutable borrow str; str \"...\" is not a literal.\n",
            "Named immutable aliases and synchronous borrowed-text calls use existing loan rules, including otherwise admitted loops. Literal evaluation shares module-held bytes; str_as_bytes creates a checked view, not an owning String or payload copy.\n",
            "Two admitted borrowed str views compare exact UTF-8 bytes with == or !=, including NUL, without cloning owners or normalizing Unicode. Keep roots live; mixed Str/String and Str ordering remain refused. Borrowed Str Equality v1 qualification is pending.\n",
            "Empty, NUL, Unicode, quotes and backslashes preserve decoded UTF-8 bytes; no interpolation or normalization. Each literal <=65536 decoded bytes (SPX-T314). If selected, independent HIR bounds all represented static/owned literals and format templates together to 65536 bytes (SPX-H006); no deduplication discount.\n",
            "Use ordinary \"...\" for an owning String and string_as_str(owner) for its view where the existing String profile admits it. No implicit conversion, borrowed return, aggregate storage, mutable alias or view escape.\n",
            "Native/internal aggregate Core-Wasm and authenticated command Project definitions are authored; template/Project execution qualification pending. Public scalar/text/data adapters, standalone String arenas and selected-final-UTF8 certificates refuse this root. No new capability or foreign-input allowance.\n",
            "Contract and owning gates: docs/STATIC-BORROWED-UTF8-LITERALS-V1.md.\n"
        ).to_owned()),
        "author:string-take" => Ok(concat!(
            "Named String owner transfer (core.string.take); owning qualification pending.\n",
            "Use let output = string_take(input); with a live named owned String local, parameter or destructured leaf. It forwards the same owner and retires input; ordinary owning reads still clone. End all overlapping views before take.\n",
            "Computed values, record projections, borrowed roots and contract calls are refused. Bind a whole String leaf first; take supplies no general move syntax or implicit last-use rule.\n",
            "Arguments stage left to right. A later argument, precondition or postcondition failure keeps ordinary sticky cleanup; it never revives input or publishes output. Empty Strings still own a carrier.\n",
            "Interpreter/native and private String Wasm implementation; frozen scalar/candidate/final-UTF8 profiles remain closed. Exact contract: docs/OWNED-STRING-TRANSFER-V1.md.\n"
        ).to_owned()),
        "author:literal-format" => Ok(concat!(
            "Checked literal format source implementation; current-head executable qualification pending.\n",
            "string_format(\"id={}\", 7) -> owned string. The first argument is a source literal: <=65536 decoded UTF-8 bytes, <=32 sequential {} fields; {{ and }} escape braces.\n",
            "Fields are i64, u8, usize, bool, or owned string, evaluated left to right. Runtime templates, borrowed strings, floats, JSON escaping and generic/closure bodies are refused.\n",
            "Use only where an existing String backend/profile admits the ordinary function. Exact failure and cleanup contract: docs/CHECKED-LITERAL-FORMAT-V1.md.\n"
        ).to_owned()),
        "author:record-sort-keys" => Ok(concat!(
            "Sort admitted flat record Vecs with explicit keys: vec_sort_by<Row>(rows,\"priority:desc,arrival:asc\").\n",
            "One explicit monomorphic record type; 1..8 unique fields, exact literal field:asc or field:desc, comma separated without spaces (<=1024 bytes). Fields resolve to their persistent @id identities, complete ordered layout and existing capacity; names alone carry no authority. No callbacks or computed policy Strings. Invalid policy is SPX-T311.\n",
            "Copy fields use existing scalar total order, including floating IEEE bit total order. Owned String/Bytes keys compare unsigned bytes, then length; NUL and UTF-8 bytes remain exact. Descending reverses comparison operands, so signed minimum is safe. The sort is stable: ties retain input order.\n",
            "This consumes rows and returns the same owner at the next generation; no payload clones or replacement row array. End all field/view loans before sorting. Empty/full vectors keep their existing capacity and limits.\n",
            "vec_sort and vec_sort_owned retain their declaration-order semantics. Choose keys independently of JSON wire order; no negative-key surrogate is needed. Primitive Vecs and generic/nested records remain refused by this operation; older command profiles retain their carrier refusals.\n",
            "The literal is checked metadata, not a runtime String argument. HIR/graph callees use a compiler-owned length-delimited descriptor identity; it cannot be declared or imported. Selective prelude v15 binds new programs only.\n",
            "Contract: docs/EXPLICIT-RECORD-SORT-KEYS-V1.md. Authored regressions await execution; no cost or runtime savings claim.\n"
        ).to_owned()),
        "author:unique-by" => Ok(concat!(
            "Check one record field for duplicates without copying keys: let ok = vec_unique_by<Row>(rows, \"id\"); -> bool.\n",
            "Row is one explicit monomorphic Copy or owned-leaf record with persistent @id fields. rows is a named available own/borrow Vec<Row> (no temporaries, paths or generic T). The field is one literal identifier: a String or i64/i32/u8/usize/bool/char field. f32/f64 and Bytes keys, computed names, field lists and callbacks are refused with SPX-T319.\n",
            "Equality is full key equality: String compares exact bytes and length (NUL and UTF-8 prefixes are distinct data); scalars compare exact values. Empty and one-row Vecs return true. Unselected fields never matter. No digest or fingerprint decides equality.\n",
            "rows is borrowed for the call only: row order, payloads, capacity and generation are unchanged, and it stays usable afterwards. It replaces a temporary Vec<string> of cloned keys plus vec_sort_owned and a neighbor scan.\n",
            "The runtime sorts a paid u32 ordinal scratch array in place and frees it before the result. A scratch refusal is semaprax.vec.v1 code 3 with ordinary sticky status and cleanup. Not admitted in while bodies.\n",
            "The literal is checked metadata, never a runtime String; HIR/graph callees use the compiler-owned core.vec.unique-by.v1 descriptor, which cannot be declared or imported. Selective prelude v18 binds new programs only.\n",
            "Contract: docs/BORROWED-RECORD-FIELD-UNIQUENESS-V1.md. Authored source batch; execution qualification pending; no cost or runtime savings claim.\n"
        ).to_owned()),
        "author:copy-record-vec" => Ok(concat!(
            "Flat Copy-record Vec source implementation; cross-engine and application qualification pending.\n",
            "R is an explicit monomorphic record with 1..8 direct Copy-scalar fields. Vec<R> uses typed vec_*<R> operations; get copies R, mutations transfer the Vec owner.\n",
            "Capacity is bounded by 8192 scalar words divided by R's field count. Private pure helpers may borrow Vec<R> and pass R by value; generic wrappers and nested records remain closed; scoped record traversal uses author:record-traversal.\n",
            "Core interpreter/native/Wasm contracts and exact status: docs/COPY-RECORD-COLLECTIONS-V1.md. Native v29 private command transport: help language author:stream-data-v2.\n"
        ).to_owned()),
        "author:byte-traversal" => Ok(concat!(
            "Borrowed byte-slice traversal v2; source implementation, executable qualification pending.\n",
            "Use one named immutable Slice<u8>: a borrow parameter or local whole/subrange view. No manual counter or Option-get is needed. The item is fresh immutable Copy u8.\n",
            "The loop snapshots length once and visits ascending indices. Continue advances exactly once; Break/Return do not advance. The authenticated underlying owner stays frozen through the traversal, including subrange aliases. Existing provenance, root bounds, loans and ABI authority remain unchanged.\n",
            "Computed or mutable sources and mixed nested traversal refuse SPX-T284; item shadowing refuses SPX-T209 and item reassignment stays closed. Nested byte loops require views named before the outermost byte loop. Every ancestor owner stays frozen; ordinary safe while/Vec operations keep their existing rules. Do not move, replace or mutate a frozen owner.\n",
            "```spx\n",
            "module help.bytes;\n",
            "@id(\"help.sum\") fn sum(view: borrow Slice<u8>) -> i64 {\n",
            " let mut total = 0;\n",
            " for byte in view {\n",
            "  if byte == 0u8 { continue; };\n",
            "  if byte == 255u8 { break; };\n",
            "  total = total + i64_from_u8(byte);\n",
            "  0\n",
            " }\n",
            " total\n",
            "}\n",
            "@id(\"help.main\") fn main() -> i64 {let data = [1u8, 0u8, 2u8]; let view = array_as_slice(data); sum(view)}\n",
            "```\n",
            "Exact scope: docs/BORROWED-BYTE-SLICE-FOR-TRAVERSAL-V2.md. This card grants no additional profile or backend qualification.\n"
        ).to_owned()),
        "author:record-traversal" => Ok(concat!(
            "Scoped record traversal source implementation; executable qualification pending.\n",
            "Use for borrow row in rows or for borrow row at index in rows. rows is an immutable named Vec of an existing admitted explicit monomorphic Copy record or flat owned-leaf record. The length is read once and indices ascend from zero; the optional index is immutable usize.\n",
            "Select row.field: Copy scalars copy, String returns borrow str, Bytes returns borrow Slice<u8>. The row has no aggregate runtime storage and cannot escape. Field views expire within the iteration. Moving, rebinding or mutating the whole Vec inside the loop is refused even for Copy-only rows.\n",
            "Row Loan v1: a flat owned-leaf row may be passed whole only as a direct argument f(row) to an ordinary monomorphic borrow R parameter whose effect-free helper reads only Copy scalar fields of it and returns a Value Copy scalar; the call borrows the frozen row, no record is copied. Storing, returning, capturing, owning, forwarding or viewing the row, generic/function-value helpers and Copy-record rows stay SPX-T284. Contract: docs/SCOPED-RECORD-ROW-LOAN-V1.md.\n",
            "Use statement guards if skip { continue; }; or if done { break; }; under author:lexical-exits. Views settle before continue advances exactly once or break releases the loop freeze. Qualification remains pending. Nested for loops, computed/projected iterable roots, generic rows, primitive Vec<string> and row shadowing remain closed. Existing capacities and cleanup order are unchanged.\n",
            "Exact contract and pending gates: docs/SCOPED-RECORD-VEC-TRAVERSAL-V2.md. Scalar for retains docs/OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md.\n"
        ).to_owned()),
        "author:record-field-views" => Ok(concat!(
            "Authenticated borrowed record and Vec field views; current-head executable qualification pending.\n",
            "Use string_as_str(value.label) or string_as_str(value.inner.label) for an exact live borrowed record path. It returns borrow str without cloning the stored String; str_as_bytes may consume that view only while its owner path remains live.\n",
            "For an admitted Vec<Row>, vec_field<Row>(value.items, index, \"selected\") returns a Copy field by value; String and Bytes fields return borrow str and borrow Slice<u8> views. The type, named Vec root, usize index, literal field, owner generation, and complete field path are checked independently.\n",
            "Primitive Vec<string> slot views use vec_str_at(words,index), with no source type argument, on a whole named live own/borrow Vec. Result is scoped borrow str; str_as_bytes composes without cloning. Prelude v17/Graph v80 and backend qualification are pending. Contract: docs/PRIMITIVE-STRING-VEC-VIEWS-V1.md.\n",
            "Use language-command-io.collection-record.v1 (v31) for nested Record{items: Vec<Row>} command data: help language author:collection-records. Keep the root Vec and every borrowed record path live: moving, replacing, sorting, pushing, reserving, clearing, consuming, or returning an escaping view before last use is refused.\n",
            "A scalar read loans only through its index/read. String/Bytes views pin the whole vector generation until last use; changing the index later does not retarget an existing view. Out-of-range access fails semaprax.vec.v1 code 2.\n",
            "Temporary/call roots, forged or stale paths, unsupported profiles, and nonliteral/unknown fields stay refused; SPX-T310 still requires one admitted record type. vec_clone_at creates an owned clone; use it only for detached owners. Contract: docs/SCOPED-VEC-FIELD-READS-V1.md.\n"
        ).to_owned()),
        "author:json-codec" => Ok(concat!(
            "Read-only shape discovery: json-codec <project> --explain --type <stable-id>. Optional bounds report unmet policy; whole-row pages bind the current revision. Candidate/backend admission is not evaluated. Contract: docs/JSON-CODEC-SUITABILITY-V1.md.\n",
            "Checked source generator implementation; focused/application qualification pending.\n",
            "semaprax json-codec <project> --source <module-path> --type <record-id> --output <new-file> [--profile <selector>]\n",
            "UTF-8 owned request and bounded-collection-response.v1 profiles require --max-string-bytes 1..64. See their authoring cards.\n",
            "bounded-nested-request.v1 additionally requires --max-array-items 1..256; see author:json-nested-request; bounded-nested-response.v1 uses the same bounds for borrowed encoding.\n",
            "Omit --profile for the default flat scalar record (1..8 i64/u8/usize/bool fields). Choose by source shape: help language author:json-identifier-views, author:json-request-views or author:json-stream-request-views.\n",
            "Declare std.data.json.scan/token/digits/write. Single-codec output is a new complete module, never an overwrite. Generated helpers are ordinary checked source.\n",
            "For paired setup, json-codec <project> --plan <plan-json> --output <new-bundle> derives 1..8 distinct source replacements against one immutable revision and validates the combined candidate. The closed JSON plan preserves original authored schemas separately from candidate sources and exact per-entry receipts. Raw bundles have no managed publication authority. Contract: docs/APPLICATION-JSON-CODEC-PLAN-V1.md.\n",
            "Declared Vec<string> in a request-view schema is description only: runtime carries Copy views and Vec<View>, not owned String collections. For owning requests: help language author:json-owned-request. This is not a generic JSON codec.\n",
            "View implementations exist; owning combined gates are pending. Stream errors use raw input offsets; post-Ready request/schema errors use normalized-input offsets. Exact profiles, limits and gates: docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-identifier-views" => Ok(concat!(
            "Select --profile identifier-views.v1 for a flat record with exactly one string identifier and up to six i64/u8/usize/bool fields.\n",
            "The generated view stores source-relative token bounds; retain the original input while using it. std.data.json.query must be available from scan's declared dependency closure or as an explicit dependency.\n",
            "Source implementation; owning combined gates pending. Invocation and limits: help language author:json-codec and docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-request-views" => Ok(concat!(
            "Select --profile request-views.v1 for a request record whose fields are Vec<string> then Vec<IdentifierRecord>. Runtime collections carry Copy views, not owned String values.\n",
            "Bounds are 0..8 server IDs and 0..256 records; IDs are unique 1..16 ASCII letters, digits, underscore or hyphen. Zero servers is valid only with zero records. Source implementation; owning combined gates pending.\n",
            "Invocation and exact schema: help language author:json-codec and docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-stream-request-views" => Ok(concat!(
            "Select --profile stream-request-views.v1 for request views used by a native v29 streaming command. The original schema module must already declare process.stdin.read; derivation grants no capability.\n",
            "The source implementation exists; owning composition gates remain pending. The current composition fixture rejects extra chunks. Exact requirements: docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-utf8-owned-request" => Ok(concat!(
            "UTF-8 owned request source implementation; focused current-head qualification pending.\n",
            "Select --profile utf8-owned-request.v1 and --max-string-bytes N, canonical decimal 1..64 per decoded string in either array. It is not a stream selector.\n",
            "Use Vec<string> (0..8) then Vec<Row> (0..256); Row has one string identifier and up to six i64/u8/usize/bool fields with explicit IDs. Empty and duplicate values are accepted; the second array may be nonempty when the first is empty.\n",
            "Values may contain Unicode, including NUL. External borrowed input shares the existing 65,536-byte root limit; internal owned Bytes views retain their 131,072-byte bound. This profile raises neither limit. Source field identifiers remain ASCII. Runtime owns decoded strings independently; retain declared JSON dependencies and select private owned-data-api.v1 or native v30.\n",
            "Invocation and exact source policy: help language author:json-codec and docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-stream-utf8-owned-request" => Ok(concat!(
            "Streaming UTF-8 owned JSON request source implementation; focused qualification pending.\n",
            "Select --profile stream-utf8-owned-request.v1 and --max-string-bytes N, canonical decimal 1..64 per decoded string; the request shape matches author:json-utf8-owned-request.\n",
            "The schema must already declare process.stdin.read; derivation grants no capability. Decode only the Ready Bytes from the existing bounded stream normalizer.\n",
            "Raw normalizer errors use raw-input offsets; post-Ready request/schema errors use normalized-input offsets. This is a native stream selector.\n",
            "The selector does not widen the foreign 65,536-byte input bound. Exact contract: docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-collection-response" => Ok(concat!(
            "Source-generation selector bounded-collection-response.v1 requires --max-string-bytes N, canonical decimal 1..64. It derives an encode-only view for one Vec<Row> plus one nested flat Copy Metrics record; Row has one or two string fields and up to six scalar fields (at most eight total), Metrics has 1..8 Copy scalars. Each Row string is checked under the byte bound.\n",
            "Each stored Row string is bounded by UTF-8 byte length; complete encoded output remains separately bounded by 131072 bytes and output_limit. The caller's independently checked Project profile must admit the generated helpers. Nested-carrier runtime admission and execution qualification are pending; this is not a v30 command route. No decoder or stream normalizer is generated.\n",
            "Exact signatures and limits: docs/APPLICATION-JSON-COLLECTION-RESPONSE-V1.md. Runtime profile: author:collection-records.\n"
        ).to_owned()),
        "author:json-nested-request" => Ok(concat!(
            "Bounded nested JSON request source implementation; current-head qualification pending.\n",
            "Select --profile bounded-nested-request.v1 with both --max-string-bytes N (canonical decimal 1..64 per decoded String) and --max-array-items M (canonical decimal 1..256). Older codec selectors reject --max-array-items.\n",
            "The finite schema uses explicit monomorphic acyclic records with string/i64/u8/usize/bool fields, nested records and at most one transitive already-admitted Vec. No optional/variant schema, new vector element shape or unbounded JSON tree is admitted.\n",
            "Direct borrowed-input decoding only; no stream normalizer or capability is generated. Helpers are ordinary checked source, and malformed usage refuses before derivation. The caller's Project profile and existing input/fuel/owner bounds remain authoritative.\n",
            "Native nested-outcome command source route: language-command-io.nested-outcome.v1; v29/v30/v31 stay frozen. Route qualification is pending; docs/PROJECT-V32-NESTED-OUTCOME-COMMAND-V1.md.\n",
            "Exact schema, typed errors and detached-owner contract: docs/APPLICATION-JSON-NESTED-REQUEST-V1.md.\n",
            "Whole-payload moves, imports and v31/v32 choice: author:nested-outcomes. Publication/source replay/dependencies: author:json-codec.\n"
        ).to_owned()),
        "author:json-stream-nested-request" => Ok(concat!(
            "Bounded streaming nested JSON source implementation; qualification pending.\n",
            "Select bounded-stream-nested-request.v1 with canonical --max-string-bytes 1..64 and --max-array-items 1..256. It keeps the finite single-Vec schema of author:json-nested-request.\n",
            "The exact original schema must permit process.stdin.read; the selected v32 command manifest independently grants all four existing command effects. No implicit capability is generated.\n",
            "Derivation proves the worst fully escaped valid normalized spelling fits the existing 131072-byte owned buffer. Raw input and whitespace have no byte cap; grammar validation continues after semantic-buffer refusal. Unsupported envelopes fail before publication.\n",
            "json_Root_nested_stream_decode() returns RootJsonNestedDecode::Ready { value: Root } or Error { code, offset, field }. Input Bytes retire only after independent owners are decoded. Stream grammar offsets are raw; downstream schema offsets are normalized.\n",
            "Direct nested decoding remains unchanged. Contract: docs/APPLICATION-JSON-NESTED-REQUEST-V1.md; caller route language-command-io.nested-outcome.v1 remains independently checked.\n"
        ).to_owned()),
        "author:json-nested-response" => Ok(concat!(
            "Bounded nested JSON response source; qualification pending.\n",
            "bounded-nested-response.v1 and additive bounded-nested-response.v2 require canonical --max-string-bytes 1..64 and --max-array-items 1..256. Caller Project/profile/capabilities independently admit ordinary reads and results.\n",
            "Explicit monomorphic acyclic records: depth 8, at most 64 expanded paths, 1..8 fields each, i64/u8/usize/bool/string leaves and at most two expanded Vec fields. Elements: Copy scalars or flat 1..8-field rows with at most two Strings. V1: Primitive Vec<string> refuses (SPX-J180). V2 adds vec_str_at(values,at) inside named borrow Vec<string> helpers; no clones, projected/temporary slot roots, optional/variant schema, recursion or new Vec shape.\n",
            "V1 APIs: json_Root_nested_response_encoded_len(value: borrow Root) -> usize and json_Root_nested_response_encode(value: borrow Root, output_limit: usize) -> RootJsonNestedResponseEncode. V2 uses nested_response_v2 and RootJsonNestedResponseV2Encode, with nested-response-v2 identities. Outcomes: Encoded { text: string } or Refused { required: usize }; usize::MAX means selected value or 131072-byte output bounds exceeded.\n",
            "Complete preflight before output allocation; render in declaration order without String/row clones. One Bytes reservation plus final String copy, not zero-copy. Owners retain runtime bounds; 264 newly decoded String census does not apply to borrowed response inspection. Encode-only; no decoder, normalizer or business-data mapping.\n",
            "Contracts: docs/APPLICATION-JSON-NESTED-RESPONSE-V1.md and docs/APPLICATION-JSON-NESTED-RESPONSE-V2.md. Compare author:json-collection-response and author:json-nested-request.\n"
        ).to_owned()),
        "author:json-request-response" => Ok(concat!(
            "Typed request/response composition; current qualification is pending. Use the bounded plan to derive both replacements against one immutable authored revision and validate their combined candidate. In examples/shiftsim-typed-record-successor, from that directory:\n",
            "semaprax json-codec . --plan codec-plan.json --output codecs.candidate.json\n",
            "The plan selects stream-owned-request.v1 and bounded-collection-response.v1 with max_string_bytes 16 (the single-codec --max-string-bytes 16 bound). It preserves authored schema bytes separately from the complete candidate and exact per-entry receipts.\n",
            "Review the complete bundle before ordinary installation; raw output does not publish a managed generation. Single-codec calls retain one selector per invocation and no-overwrite behavior; no combined request/response profile is invented. The streaming request schema and Project manifest must already permit and grant process.stdin.read; derivation adds no capability.\n",
            "The request helper owns decoded strings and rows; ordinary `vec_field<Patient>(request.patients,index,\"priority\")` reads a selected Copy field (a String field is borrowed through last use). The response selector only encodes `Report { assignments: Vec<Assignment>, metrics: Metrics }`; it does not decode or map business data.\n",
            "Generated helpers are ordinary checked source. v31 command admission and current-head/application qualification remain separate; this draft claims no runtime qualification or general JSON-tree codec. See examples/shiftsim-typed-record-successor/README.md, author:json-owned-request, author:json-collection-response and author:collection-records.\n"
        ).to_owned()),
        "author:json-owned-request" => Ok(concat!(
            "Owned request source implementation; focused cross-backend and application qualification pending.\n",
            "Select --profile owned-request.v1, or stream-owned-request.v1 with the original schema's process.stdin.read permit and the manifest grant. Derivation grants no capability.\n",
            "Request fields are Vec<string> then Vec<Row>; Row has one string identifier and up to six i64/u8/usize/bool fields with explicit record/field IDs. Names are authored, order is declaration order.\n",
            "Bounds: 0..8 first-array identifiers, 0..256 rows, unique identifiers of 1..16 ASCII letters/digits/underscore/hyphen; a nonempty second array needs a nonempty first array. Arbitrary Unicode, recursion and nullable values are outside this policy.\n",
            "Actual runtime collections own independent Strings and Rows. Select private owned-data-api.v1 or native v30 language-command-io.owned-data.v1; v29 stays closed to owned carriers.\n",
            "json_<Request>_owned_decode takes borrow Slice<u8> and returns an owning Decoded or Error{code,offset,field}; malformed input is an application outcome. Stream normalization is separate: decode Ready's normalized slice, then release its Bytes after decode.\n",
            "json_<Request>_owned_encoded_len and _owned_encode borrow both Vecs; encode also takes output_limit. Exact source, cleanup, limits and ordinary failure contracts: docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:file-text" => Ok(concat!(
            "Native file text route (complete guidance; exact library lookup follows):\n",
            "new app --template source-command-file-text\n",
            "Project profile source-command.v1; input argv-utf8+file-text.v1; native64 only.\n",
            "Capabilities: fs.read, process.args.read, process.stderr.write, process.stdout.write.\n",
            "The template declares std.int.decimal and includes a sample file; follow generated README for invocation.\n",
            "check app; test app --target native; build app --target native -o app-bin. Invoke ./app-bin <relative-file>.\n",
            "Project interpreter invocation, Web, Wasm and npm targets refuse this command route.\n",
            "Exact dependency/profile/signatures: help library std.int.decimal; source imports: help language projects.\n",
            "Search: help language find:decimal:0. This route grants no ambient filesystem authority.\n"
        ).to_owned()),
        "author:selected-file" => Ok(concat!(
            "Invocation-selected file input v1: executable qualification pending.\n",
            "Project profile source-command.selected-file.v1; input argv-utf8+selected-file.follow.v1; native64 Unix only.\n",
            "Declare exact source-command effects including fs.read and process.args.read. Existing relative-root profiles retain their provider.\n",
            "build app --target native -o app-bin; invoke ./app-bin --spx-selected-file-v1 INDEX -- <all original application arguments>. INDEX is canonical decimal 0..15 and selects an application argument.\n",
            "Without the optional prefix, all arguments still reach source and no file grant exists. Checked source validates complete usage before file_read_text requests the exact selected argument spelling.\n",
            "The first read follows selected symlinks and holds one regular-file descriptor until cleanup; later reads never reopen. Absolute, parent-relative and hidden filenames are supported. Other Strings confer no authority.\n",
            "Limits: 16 argv, 65536 total argv bytes, 4096 path bytes, 65536 UTF-8 file bytes, 64 read attempts, 1 MiB reserved read bytes. Mutation detection is metadata-based, not an atomic snapshot.\n",
            "Interpreter, Web, Wasm, npm and non-Unix hosts refuse this route. See docs/INVOCATION-SELECTED-FILE-INPUT-V1.md for normalized failures and lifetime.\n"
        ).to_owned()),
        "author:selected-file-resource" => Ok(concat!(
            "Selected-file resource input: authored; executable qualification pending.\n",
            "Project profile source-command.selected-file.resource-output.v1; input argv-utf8-256+selected-file.follow.v1; semaprax.project.v34; native64 Unix only.\n",
            "Declare exact source-command capabilities including fs.read and process.args.read. Checked source validates complete usage before the first file_read_text.\n",
            "Build the ordinary native Project executable. With app arguments invoke ./app --spx-selected-file-v1 INDEX -- <all unchanged arguments>; without arguments invoke directly. INDEX is canonical decimal 0..255.\n",
            "The launcher binds argv and lazily opens only the exact selected key under the explicit follow policy, holding one regular-file descriptor until settlement before output publication. No shell input dirname/cd/readlink or option normalization is required.\n",
            "Bounds: 256 raw arguments, 65536 aggregate argv bytes, 4096 path bytes, 65536 UTF-8 file bytes, 64 attempts/1 MiB reserved reads, 1048576-byte String and combined output envelopes inherited from v28.\n",
            "v33 stays 16 argv and 65536 output; v26/v28 relative-root no-follow and their bounds stay unchanged. Interpreter, Wasm, Web, npm and non-Unix hosts refuse. See docs/INVOCATION-SELECTED-FILE-RESOURCE-V1.md.\n"
        ).to_owned()),
        "author:source-web" => Ok(concat!(
            "Single-source web route (complete guidance):\n",
            "fmt app.spx; check app.spx; build app.spx --target web --profile internal-strings-v1 -o web.\n",
            "text-toolkit-v1 is the other explicit single-source web profile; help build gives exact grammar.\n",
            "These are source build profiles, not Project manifest profiles. Project web builds follow their declared profile and export closure.\n",
            "Read exact boundary limits with help language web; choose checked examples with help shapes kinds.\n",
            "A generated web package does not grant a filesystem, process, network or secret capability.\n"
        ).to_owned()),
        _ if query.starts_with("find:") => search(query, MAX_BYTES),
        _ => Err("unknown authoring route; use `help language author:routes`".to_owned()),
    }
}

fn search(query: &str, budget: usize) -> Result<String, String> {
    let mut parts = query.split(':');
    let (Some("find"), Some(word), Some(offset), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err("library search requires find:<word>:<offset>".to_owned());
    };
    if word.is_empty()
        || word.len() > 64
        || !word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(
            "library search word must be 1..64 ASCII letters, digits, dots, underscores or hyphens"
                .to_owned(),
        );
    }
    let start = offset
        .parse::<usize>()
        .ok()
        .filter(|value| value.to_string() == offset)
        .ok_or_else(|| "library search offset must be canonical unsigned decimal".to_owned())?;
    let catalog: serde_json::Value = serde_json::from_str(super::LIBRARY_INDEX)
        .expect("checked standard library catalog must parse");
    let mut matches = Vec::new();
    let modules = catalog["modules"].as_array().expect("catalog modules");
    for module in modules {
        let module_id = module["module"].as_str().expect("catalog module id");
        for declaration in module["declarations"]
            .as_array()
            .expect("catalog declarations")
        {
            let id = declaration["id"].as_str().expect("catalog declaration id");
            let name = declaration["name"]
                .as_str()
                .expect("catalog declaration name");
            let head = declaration["head"]
                .as_array()
                .expect("catalog declaration head");
            if module_id.contains(word)
                || id.contains(word)
                || name.contains(word)
                || head
                    .iter()
                    .any(|line| line.as_str().expect("catalog head line").contains(word))
            {
                let mut row = format!(
                    "{id}\ndependency {}\nprofile {}\n",
                    module["dependency"].as_str().expect("catalog dependency"),
                    module["required_profile"]
                        .as_str()
                        .expect("catalog profile")
                );
                for line in head {
                    writeln!(row, "{}", line.as_str().expect("catalog head line")).unwrap();
                }
                matches.push((id, row));
            }
        }
    }
    matches.sort_by(|left, right| left.0.cmp(right.0));
    if matches.is_empty() {
        return Err(format!("library search has no match for `{word}`"));
    }
    if start >= matches.len() {
        return Err("library search offset is outside its matching inventory".to_owned());
    }
    let mut output = format!(
        "Library matches: {}; start: {start}; budget: {budget} UTF-8 bytes\n",
        matches.len()
    );
    let mut next = start;
    while next < matches.len() {
        let footer = footer(word, next + 1, matches.len());
        let row = &matches[next].1;
        if output.len() + row.len() + 1 + footer.len() > budget {
            break;
        }
        output.push('\n');
        output.push_str(row);
        next += 1;
    }
    if next == start {
        return Err(format!(
            "library search entry exceeds page budget; use `help library {}`",
            matches[start].0
        ));
    }
    output.push_str(&footer(word, next, matches.len()));
    Ok(output)
}

fn footer(word: &str, next: usize, total: usize) -> String {
    if next == total {
        "\nCoverage: complete matching inventory.\n".to_owned()
    } else {
        format!("\nCoverage: partial; next: help language find:{word}:{next}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_traversal_card_checks_its_actual_example_and_keeps_named_view_authority() {
        let card = lookup("author:byte-traversal").unwrap();
        assert!(card.len() <= MAX_BYTES);
        let routes = lookup("author:routes").unwrap();
        assert!(routes.len() <= MAX_BYTES);
        assert_eq!(routes.matches("author:byte-traversal").count(), 1);
        for fact in [
            "qualification pending",
            "named immutable Slice<u8>",
            "snapshots length once",
            "Continue advances exactly once",
            "Break/Return do not advance",
            "owner stays frozen",
            "subrange aliases",
            "Computed or mutable sources",
            "mixed nested traversal refuse SPX-T284",
            "views named before the outermost byte loop",
            "ordinary safe while/Vec operations",
            "shadowing refuses SPX-T209",
            "provenance, root bounds, loans and ABI authority",
        ] {
            assert!(card.contains(fact), "missing scoped fact: {fact}");
        }
        let source = card
            .split("```spx\n")
            .nth(1)
            .unwrap()
            .split("```\n")
            .next()
            .unwrap();
        assert!(!source.contains("byte_get") && !source.contains("Option::"));
        let parsed = semaprax::check(source, "help-byte-traversal.spx").unwrap();
        let canonical = semaprax::format::canonical(&parsed);
        let reparsed = semaprax::check(&canonical, "help-byte-traversal.spx").unwrap();
        assert_eq!(canonical, semaprax::format::canonical(&reparsed));
        let resolved = semaprax::hir::resolve(&reparsed).unwrap();
        semaprax::hir::validate(&resolved).unwrap();
        let sum = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "help.sum")
            .unwrap();
        let control = sum.cleanup_plan.lexical_control.as_ref().unwrap();
        assert!(control.loops.iter().any(|loop_| matches!(
            &loop_.latch,
            semaprax::hir::LoopLatchKind::SourceForStep {
                kind: semaprax::hir::SourceForKind::BorrowBytes,
                ..
            }
        )));
        for kind in [
            semaprax::hir::LexicalExitKind::Continue,
            semaprax::hir::LexicalExitKind::Break,
        ] {
            assert!(control.exits.iter().any(|exit| exit.kind == kind));
        }
        let computed = source.replacen(
            "for byte in view",
            "for byte in byte_range(view, 0usize, byte_len(view))",
            1,
        );
        let errors = semaprax::check(&computed, "help-byte-computed-source.spx").unwrap_err();
        assert!(errors.iter().any(|d| d.code == "SPX-T284"), "{errors:?}");
    }

    #[test]
    fn traversal_card_describes_only_the_checked_row_loan_shape() {
        let traversal = lookup("author:record-traversal").unwrap();
        assert!(traversal.len() <= MAX_BYTES);
        assert!(traversal.contains("Row Loan v1"));
        assert!(traversal.contains("effect-free helper"));
        assert!(traversal.contains("returns a Value Copy scalar"));
        assert!(traversal.contains("Copy-record rows stay SPX-T284"));
        let row = r#"module help.loan;
@id("help.row") record Row { @id("help.row.n") n:i64, @id("help.row.text") text:string, }
@id("help.weight") fn weight(row:borrow Row)->i64 {row.n+1}
@id("help.main") fn main()->i64 {
 let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{n:3,text:"r"});
 let mut seen=0;
 for borrow row in rows { seen=seen+weight(row); 0 }
 seen
}"#;
        let checked = semaprax::check(row, "help-row-loan.spx").unwrap();
        let resolved = semaprax::hir::resolve(&checked).unwrap();
        semaprax::hir::validate(&resolved).unwrap();
        for (helper, body) in [
            (
                "fn weight(row:own Row)->i64 {row.n+1}",
                "seen=seen+weight(row);",
            ),
            ("fn weight(row:borrow Row)->i64 {row.n+1}", "let kept=row;"),
        ] {
            let source = row
                .replace("fn weight(row:borrow Row)->i64 {row.n+1}", helper)
                .replace("seen=seen+weight(row);", body);
            assert!(semaprax::check(&source, "help-row-loan-refused.spx")
                .unwrap_err()
                .iter()
                .any(|d| d.code == "SPX-T284"));
        }
    }

    #[test]
    fn traversal_and_buffer_cards_match_selective_typed_profiles_without_promoting_qualification() {
        let traversal = lookup("author:record-traversal").unwrap();
        let exits = lookup("author:lexical-exits").unwrap();
        assert!(exits.contains("Copy-valued if/else can discard"));
        assert!(exits.contains("A bare guard containing an exit may end with or without one"));
        assert!(exits.contains("let _ = guard or an operator suffix stays SPX-T312"));
        assert!(exits.contains("a final ; } stays SPX-P106"));
        assert!(exits.contains("Normal guard arm: exact else { 0 }"));
        let rejected = semaprax::check(
            "module help.guard; @id(\"help.main\") fn main()->i64 {let mut i=0;while i<2 {i=i+1;if i==1 {continue;} else {7};0} i}",
            "help-nonzero-exit-guard.spx",
        ).unwrap_err();
        assert!(rejected.iter().any(|d| d.code == "SPX-T312"));
        let utf8 = lookup("author:string-from-utf8").unwrap();
        for card in [&traversal, &exits, &utf8] {
            assert!(card.len() <= MAX_BYTES);
            assert!(card.contains("qualification pending"));
        }
        assert!(traversal.contains("author:lexical-exits"));
        assert!(traversal.contains("continue advances exactly once"));
        assert!(traversal.contains("break releases the loop freeze"));
        assert!(!traversal.contains("break/continue remain closed"));
        assert!(exits.contains("Bytes-helper renewal additionally selects v19"));
        assert!(exits.contains("ordinary loop operation/loan rules"));
        assert!(utf8.contains("NONALLOCATING-BYTE-HELPER-RENEWAL-V1.md"));
        assert!(utf8.contains("Native allocation exhaustion remains fail-stop"));
        assert!(utf8.contains("V10 refuses this intrinsic"));

        // Authenticate the card's source shape through independent typed latch
        // and freeze reconstruction, without asserting backend qualification.
        let source = r#"module help.rows;
@id("help.row") record Row { @id("help.row.priority") priority:i64, }
@id("help.main") fn main()->i64 {
 let rows=vec_push<Row>(vec_push<Row>(vec_with_capacity<Row>(2usize),Row{priority:3}),Row{priority:7});
 let mut seen=0;
 for borrow row at index in rows {
  seen=seen+1;
  if index==0usize {continue;};
  if row.priority==7 {break;};
  0
 }
 if vec_len<Row>(rows)==2usize {seen+2}else{seen}
}"#;
        let checked = semaprax::check(source, "help-record-exits.spx").unwrap();
        let canonical = semaprax::format::canonical(&checked);
        let replay = semaprax::check(&canonical, "help-record-exits.spx").unwrap();
        assert_eq!(canonical, semaprax::format::canonical(&replay));
        let resolved = semaprax::hir::resolve(&replay).unwrap();
        semaprax::hir::validate(&resolved).unwrap();
        let main = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "help.main")
            .unwrap();
        let control = main.cleanup_plan.lexical_control.as_ref().unwrap();
        assert!(control.loops.iter().any(|loop_| matches!(
            &loop_.latch,
            semaprax::hir::LoopLatchKind::SourceForStep {
                kind: semaprax::hir::SourceForKind::BorrowRecord,
                ..
            }
        )));
        assert!(control
            .exits
            .iter()
            .any(|exit| exit.kind == semaprax::hir::LexicalExitKind::Continue));
        assert!(control
            .exits
            .iter()
            .any(|exit| exit.kind == semaprax::hir::LexicalExitKind::Break));
    }

    #[test]
    fn static_utf8_card_preserves_borrowed_scope_and_qualification_boundaries() {
        let card = lookup("author:static-utf8").unwrap();
        assert_eq!(card, lookup("author:static-utf8").unwrap());
        assert!(card.len() <= MAX_BYTES);
        for fact in [
            "str\"é\\u{0}\"",
            "3usize",
            "immutable borrow str",
            "module-held bytes",
            "SPX-T314",
            "SPX-H006",
            "no deduplication discount",
            "string_as_str(owner)",
            "No implicit conversion",
            "view escape",
            "Project execution qualification pending",
            "certificates refuse",
            "docs/STATIC-BORROWED-UTF8-LITERALS-V1.md",
        ] {
            assert!(card.contains(fact), "static UTF-8 guidance omits {fact}");
        }
        assert!(lookup("author:routes")
            .unwrap()
            .contains("author:static-utf8"));
        let source = r#"module help.static_text;
@id("help.static_text.main") fn main()->i64 {
 let key=str"é\u{0}";
 let alias=key;
 let view=str_as_bytes(alias);
 if byte_len(view)==3usize {42}else{0}
}"#;
        let checked = semaprax::check(source, "help-static-text.spx").unwrap();
        let canonical = semaprax::format::canonical(&checked);
        let replay = semaprax::check(&canonical, "help-static-text.spx").unwrap();
        assert_eq!(canonical, semaprax::format::canonical(&replay));
        semaprax::hir::validate(&semaprax::hir::resolve(&replay).unwrap()).unwrap();
    }

    #[test]
    fn field_uniqueness_card_is_bounded_compiles_and_names_closed_authority() {
        let card = lookup("author:unique-by").unwrap();
        assert!(card.len() <= 4096);
        for text in [
            "vec_unique_by<Row>",
            "SPX-T319",
            "persistent @id",
            "full key equality",
            "borrowed for the call only",
            "code 3",
            "never a runtime String",
            "prelude v18",
            "BORROWED-RECORD-FIELD-UNIQUENESS-V1.md",
        ] {
            assert!(card.contains(text), "{text}");
        }
        assert!(lookup("author:routes")
            .unwrap()
            .contains("author:unique-by"));
        let source = "module help.unique; @id(\"help.unique.row\") record Row { @id(\"help.unique.row.id\") id:string, @id(\"help.unique.row.rank\") rank:i64, } @id(\"help.unique.main\") fn main()->i64 { let rows=vec_with_capacity<Row>(0usize); let ok = vec_unique_by<Row>(rows, \"id\"); if ok {0} else {1} }";
        let checked = semaprax::check(source, "help-unique.spx").unwrap();
        semaprax::hir::validate(&semaprax::hir::resolve(&checked).unwrap()).unwrap();
        let refused =
            semaprax::check(&source.replace("\"id\"", "\"rank:asc\""), "bad.spx").unwrap_err();
        assert!(refused.iter().any(|d| d.code == "SPX-T319"));
    }

    #[test]
    fn explicit_sort_card_is_bounded_and_names_closed_literal_authority() {
        let card = lookup("author:record-sort-keys").unwrap();
        assert!(card.len() <= 4096);
        for text in [
            "vec_sort_by<Row>",
            "1..8",
            "SPX-T311",
            "persistent @id",
            "stable",
            "next generation",
            "not a runtime String",
            "prelude v15",
            "EXPLICIT-RECORD-SORT-KEYS-V1.md",
        ] {
            assert!(card.contains(text), "{text}");
        }
        assert!(lookup("author:routes")
            .unwrap()
            .contains("author:record-sort-keys"));
    }

    #[test]
    fn routes_and_search_are_bounded_exact_and_have_complete_continuations() {
        let inventory = lookup("author:routes").unwrap();
        for route in [
            "author:routes",
            "author:vec-constructor",
            "author:lexical-exits",
            "author:stdin-json",
            "author:stream-data-v2",
            "author:owned-data",
            "author:collection-records",
            "author:nested-outcomes",
            "author:file-text",
            "author:selected-file",
            "author:selected-file-resource",
            "author:source-web",
            "author:literal-format",
            "author:string-take",
            "author:static-utf8",
            "author:string-from-utf8",
            "author:copy-record-vec",
            "author:record-sort-keys",
            "author:unique-by",
            "author:record-field-views",
            "author:record-traversal",
            "author:byte-traversal",
            "author:json-codec",
            "author:json-identifier-views",
            "author:json-request-views",
            "author:json-stream-request-views",
            "author:json-owned-request",
            "author:json-utf8-owned-request",
            "author:json-stream-utf8-owned-request",
            "author:json-collection-response",
            "author:json-nested-request",
            "author:json-stream-nested-request",
            "author:json-nested-response",
            "author:json-request-response",
        ] {
            let output = lookup(route).unwrap();
            assert!(output.len() <= MAX_BYTES);
            assert_eq!(output, lookup(route).unwrap());
            if route != "author:routes" {
                assert!(
                    inventory.split_whitespace().any(|word| word == route),
                    "complete authoring inventory omits exact selector {route}"
                );
            }
        }
        for bad in [
            "author:unknown",
            "find::0",
            "find:json:00",
            "find:json:-1",
            "find:json:0:extra",
            "find:json:999999",
            "find:JSON_NOT_REAL:0",
            "find:json space:0",
        ] {
            assert!(lookup(bad).is_err(), "{bad}");
        }
        let exits = lookup("author:lexical-exits").unwrap();
        for text in [
            "SPX-T312",
            "SPX-T313",
            "SPX-T103/O104",
            "H006",
            "exactly once",
            "Failure never publishes",
            "qualification pending",
            "LEXICAL-STRUCTURED-EXITS-V1.md",
        ] {
            assert!(exits.contains(text), "{text}");
        }
        assert!(lookup("author:routes")
            .unwrap()
            .contains("author:lexical-exits"));
        let codec = lookup("author:json-codec").unwrap();
        assert!(codec.contains("--profile <selector>"));
        assert!(codec.contains("--max-string-bytes 1..64"));
        assert!(codec.contains("Omit --profile for the default flat scalar record"));
        assert!(codec.contains("author:json-identifier-views"));
        assert!(codec.contains("Declared Vec<string> in a request-view schema is description only"));
        assert!(codec.contains("post-Ready request/schema errors use normalized-input offsets"));
        let identifier_views = lookup("author:json-identifier-views").unwrap();
        assert!(identifier_views.contains("exactly one string identifier"));
        assert!(identifier_views.contains("combined gates pending"));
        let request_views = lookup("author:json-request-views").unwrap();
        assert!(request_views.contains("Vec<IdentifierRecord>"));
        assert!(request_views.contains("0..256 records"));
        let stream_request_views = lookup("author:json-stream-request-views").unwrap();
        assert!(stream_request_views.contains("process.stdin.read"));
        assert!(stream_request_views.contains("composition fixture rejects extra chunks"));
        let utf8_request = lookup("author:json-utf8-owned-request").unwrap();
        for fact in [
            "utf8-owned-request.v1",
            "--max-string-bytes",
            "per decoded string",
            "0..8",
            "0..256",
            "Unicode",
        ] {
            assert!(
                utf8_request.contains(fact),
                "UTF-8 request guidance omits {fact}"
            );
        }
        let stream_utf8_request = lookup("author:json-stream-utf8-owned-request").unwrap();
        for fact in [
            "stream-utf8-owned-request.v1",
            "--max-string-bytes N",
            "process.stdin.read",
            "Ready Bytes",
            "raw-input offsets",
            "normalized-input offsets",
            "qualification pending",
        ] {
            assert!(
                stream_utf8_request.contains(fact),
                "stream UTF-8 request guidance omits {fact}"
            );
        }
        let collection_response = lookup("author:json-collection-response").unwrap();
        for fact in [
            "bounded-collection-response.v1",
            "--max-string-bytes N",
            "canonical decimal 1..64",
            "encode-only",
            "execution qualification are pending",
            "not a v30 command route",
        ] {
            assert!(
                collection_response.contains(fact),
                "collection response guidance omits {fact}"
            );
        }
        let nested_response = lookup("author:json-nested-response").unwrap();
        for required in [
            "bounded-nested-response.v1",
            "bounded-nested-response.v2",
            "vec_str_at(values,at)",
            "nested-response-v2 identities",
            "final String copy",
            "Primitive Vec<string> refuses (SPX-J180)",
            "usize::MAX",
            "before output allocation",
            "at most two expanded",
            "264 newly decoded String census does not apply",
            "borrow Root",
            "declaration order",
            "docs/APPLICATION-JSON-NESTED-RESPONSE-V1.md",
        ] {
            assert!(nested_response.contains(required), "{required}");
        }
        let nested_request = lookup("author:json-nested-request").unwrap();
        for fact in [
            "bounded-nested-request.v1",
            "--max-string-bytes N",
            "1..64",
            "--max-array-items M",
            "1..256",
            "Direct borrowed-input",
            "no stream normalizer",
            "qualification pending",
            "language-command-io.nested-outcome.v1",
            "at most one transitive",
            "Older codec selectors reject",
            "docs/APPLICATION-JSON-NESTED-REQUEST-V1.md",
        ] {
            assert!(
                nested_request.contains(fact),
                "nested request guidance omits {fact}"
            );
        }
        let stream_nested = lookup("author:json-stream-nested-request").unwrap();
        for fact in [
            "bounded-stream-nested-request.v1",
            "--max-string-bytes 1..64",
            "--max-array-items 1..256",
            "exact original schema",
            "process.stdin.read",
            "131072-byte",
            "Raw input and whitespace have no byte cap",
            "normalized",
            "independently checked",
            "qualification pending",
        ] {
            assert!(
                stream_nested.contains(fact),
                "stream nested guidance omits {fact}"
            );
        }
        let request_response = lookup("author:json-request-response").unwrap();
        for fact in [
            "--plan codec-plan.json",
            "combined candidate",
            "authored schema bytes separately",
            "one selector per invocation",
            "no combined request/response profile",
            "stream-owned-request.v1",
            "bounded-collection-response.v1",
            "--max-string-bytes 16",
            "vec_field<Patient>(request.patients,index,\"priority\")",
            "only encodes",
            "does not decode or map business data",
            "process.stdin.read",
            "derivation adds no capability",
            "v31 command admission",
            "claims no runtime qualification",
        ] {
            assert!(
                request_response.contains(fact),
                "request/response guidance omits {fact}"
            );
        }
        let owned_request = lookup("author:json-owned-request").unwrap();
        for fact in [
            "owned-request.v1",
            "stream-owned-request.v1",
            "0..256 rows",
            "1..16 ASCII",
            "v29 stays closed",
            "normalized slice",
            "qualification pending",
        ] {
            assert!(
                owned_request.contains(fact),
                "owned request guidance omits {fact}"
            );
        }
        let stream = lookup("author:stream-data-v2").unwrap();
        assert!(stream.contains("language-command-io.stream-data.v2"));
        assert!(stream.contains("v27 profile stays scalar-vector-only"));
        let owned = lookup("author:owned-data").unwrap();
        assert!(owned.contains("language-command-io.owned-data.v1"));
        assert!(owned.contains("including unused helpers"));
        assert!(owned.contains("SPX-T267"));
        assert!(owned.contains("CONSUMING-OWNED-RECORD-VEC-RENEWAL-V1.md"));
        assert!(owned.contains("Unknown/nonempty clones"));
        assert!(owned.contains("fn() -> i64"));
        assert!(owned.contains("Choose v30 when private helpers need owned-leaf Vec/Iter runtime"));
        assert!(owned.contains("string_from_utf8(bytes: borrow Slice<u8>) -> string"));
        assert!(owned.contains("core.string.from_utf8"));
        assert!(owned.contains("semaprax.convert.v1 code 1"));
        assert!(owned.contains("never replacement or normalization"));
        assert!(owned.contains("author:string-from-utf8"));
        let bulk_utf8 = lookup("author:string-from-utf8").unwrap();
        for fact in [
            "string_from_utf8(input: borrow Slice<u8>) -> string",
            "core.string.from_utf8",
            "fresh detached String",
            "NUL, BOM, noncharacters",
            "semaprax.convert.v1 code 1",
            "no replacement or normalization",
            "language-command-io.owned-data.v1",
            "public/standalone adapter",
            "T204/T205/T225/S113",
            "across helper calls",
            "empty success still charges one",
            "V10 refuses this intrinsic",
            "no corresponding 4096 counter",
            "does not unescape or quote JSON",
        ] {
            assert!(bulk_utf8.contains(fact), "bulk UTF-8 guidance omits {fact}");
        }
        let nested = lookup("author:collection-records").unwrap();
        assert!(nested.len() <= 2_048);
        for fact in [
            "language-command-io.collection-record.v1",
            "new app --template stdin-stream-collection-record",
            "execution qualification pending",
            "SPX-T310 requires one admitted record type",
            "value: borrow Report",
            "vec_len(value.items)",
            "vec_field<Row>(value.items,index,\"selected\")",
            "Copy returns by value",
            "borrow str`/`borrow Slice<u8>",
            "str_as_bytes(vec_field<Row>(value.items,index,\"text\"))",
            "whole vector generation through last use",
            "Don’t deep-clone for scalar queries",
            "do not move, replace, sort, push, reserve, clear or consume it before then",
            "vec_clone_at` is for owned results",
            "Project v31 source only; execution qualification pending",
            "v30 keeps its owned-leaf boundary",
            "SPX-G172",
            "examples/collection-record-order",
            "department:i64, priority:i64, id:string, ordinal:i64",
            "vec_sort_owned<Row>",
            "String UTF-8 bytes",
            "for own and vec_into_iter",
            "author:json-collection-response",
        ] {
            assert!(
                nested.contains(fact),
                "collection record guidance omits {fact}"
            );
        }
        let outcomes = lookup("author:nested-outcomes").unwrap();
        for fact in [
            "language-command-io.nested-outcome.v1",
            "exactly two cases",
            "i64, usize, i64",
            "even in unused helpers",
            "SPX-G172",
            "match own outcome",
            "match own value",
            "comma inside",
            "no dummy record",
            "finite collection-outcome join",
            "Depth<=64",
            "owned leaves<=256",
            "PROJECT-V32-NESTED-OUTCOME-COMMAND-V1.md",
        ] {
            assert!(
                outcomes.contains(fact),
                "nested-outcome guidance omits {fact}"
            );
        }
        let field_views = lookup("author:record-field-views").unwrap();
        for fact in [
            "string_as_str(value.label)",
            "string_as_str(value.inner.label)",
            "vec_field<Row>(value.items, index, \"selected\")",
            "owner generation",
            "language-command-io.collection-record.v1",
            "SPX-T310",
            "vec_clone_at creates an owned clone",
            "scalar read loans only through its index/read",
            "semaprax.vec.v1 code 2",
            "changing the index later does not retarget",
        ] {
            assert!(
                field_views.contains(fact),
                "field-view guidance omits {fact}"
            );
        }
        for fact in [
            "vec_str_at(words,index)",
            "no source type argument",
            "Prelude v17/Graph v80",
            "PRIMITIVE-STRING-VEC-VIEWS-V1.md",
        ] {
            assert!(field_views.contains(fact), "slot guidance omits {fact}");
        }
        let routes = lookup("author:routes").unwrap();
        for selector in [
            "author:string-from-utf8",
            "author:record-field-views",
            "author:record-traversal",
            "author:collection-records",
            "author:nested-outcomes",
            "author:json-identifier-views",
            "author:json-request-views",
            "author:json-stream-request-views",
            "author:json-owned-request",
            "author:json-utf8-owned-request",
            "author:json-stream-utf8-owned-request",
            "author:json-collection-response",
            "author:json-nested-request",
            "author:json-stream-nested-request",
            "author:json-nested-response",
            "author:json-request-response",
        ] {
            assert!(routes.contains(selector), "route list omits {selector}");
        }
        let mut query = "find:std.data.json.:0".to_owned();
        let mut ids = std::collections::BTreeSet::new();
        loop {
            let page = lookup(&query).unwrap();
            assert!(page.len() <= MAX_BYTES);
            for line in page
                .lines()
                .filter(|line| line.starts_with("std.") && !line.contains(' '))
            {
                assert!(ids.insert(line.to_owned()), "duplicate page entry {line}");
                assert!(super::super::library::library_entry(line).is_ok());
            }
            if let Some(next) = page
                .lines()
                .find_map(|line| line.strip_prefix("Coverage: partial; next: help language "))
            {
                assert_ne!(next, query);
                query = next.to_owned();
            } else {
                assert!(page.ends_with("Coverage: complete matching inventory.\n"));
                break;
            }
        }
        let catalog: serde_json::Value = serde_json::from_str(super::super::LIBRARY_INDEX).unwrap();
        let expected = catalog["modules"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|module| module["declarations"].as_array().unwrap().iter())
            .filter(|declaration| {
                declaration["id"]
                    .as_str()
                    .unwrap()
                    .contains("std.data.json.")
            })
            .map(|declaration| declaration["id"].as_str().unwrap().to_owned())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids, expected);
    }

    #[test]
    fn exact_page_budget_and_one_short_preserve_all_entry_bytes() {
        let query = "find:std.data.json.scan.strict_end:0";
        let mut budget = 256;
        while search(query, budget).is_err() {
            budget += 1;
            assert!(budget < MAX_BYTES);
        }
        let page = search(query, budget).unwrap();
        assert_eq!(page.len(), budget);
        assert!(search(query, budget - 1).is_err());
        assert!(page.contains("dependency ") && page.contains("profile "));
    }
    #[test]
    fn typed_vector_constructor_route_preserves_commits_and_frozen_boundaries() {
        for (code, required) in [
            ("SPX-P220", "vec_of<T>(items)"),
            ("SPX-T318", "explicit Vec chain"),
        ] {
            assert!(super::super::diagnostic_entry(code)
                .unwrap()
                .contains(required));
        }

        let card = lookup("author:vec-constructor").unwrap();
        assert!(card.len() <= 2_048);
        for expected in [
            "vec_of<i64>()",
            "left to right",
            "staged-argument commit",
            "no suffix",
            "P220",
            "T318",
            "Graph v81",
            "frozen public/interop",
            "qualification pending",
        ] {
            assert!(card.contains(expected), "{expected}");
        }
        assert!(lookup("author:routes")
            .unwrap()
            .contains("author:vec-constructor"));
        assert!(lookup("author:vec-constructor-extra").is_err());
    }
}
