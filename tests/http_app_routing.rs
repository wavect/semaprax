//! Owning harness for [`docs/HTTP-APPLICATION-ROUTING-V1.md`](../docs/HTTP-APPLICATION-ROUTING-V1.md),
//! the first bounded, offline slice of issue #189's `semaprax-app-http.v1`
//! profile.
//!
//! Two things are exercised here, both entirely offline:
//!
//! - [`fixture_transport`][]: [`examples/http_app_routing.spx`](../examples/http_app_routing.spx)'s
//!   route dispatcher, called directly through the library interpreter with
//!   literal HTTP/1.1 request byte arrays standing in for a deterministic
//!   transport. No socket, port, or process I/O is used anywhere in this
//!   file.
//! - [`refusal`]: the routing profile's refusal behaviour is the *existing*
//!   compiler diagnostics that already govern the primitives it is built
//!   from (effects, match exhaustiveness, match aggregate arms) — this
//!   module proves each one still fires on a route/handler shape the profile
//!   cannot express, with inline hostile source rather than a second parallel
//!   type checker.

use std::path::{Path, PathBuf};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{parse, verify};

fn example_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/http_app_routing.spx")
}

fn returned_value(envelope: &str) -> String {
    let document: serde_json::Value = serde_json::from_str(envelope).unwrap();
    document["payload"]["outcome"]["value"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn call(function: &str, request: &[u8]) -> String {
    let argument = serde_json::to_string(request).unwrap();
    let result = interpreter::interpret(
        &example_path(),
        function,
        &[argument],
        &InterpreterOptions::default(),
    )
    .unwrap_or_else(|diagnostics| panic!("{function} did not interpret: {diagnostics:?}"));
    assert!(result.returned, "{function} did not return a value");
    interpreter::verify_envelope(&result.envelope).unwrap();
    returned_value(&result.envelope)
}

mod fixture_transport {
    use super::*;

    const HEALTH: &[u8] = b"GET /health HTTP/1.1\r\nHost: example.org\r\nConnection: close\r\n\r\n";
    const ECHO: &[u8] = b"GET /echo HTTP/1.1\r\nHost: example.org\r\nConnection: close\r\n\r\n";
    const MISSING: &[u8] =
        b"GET /missing HTTP/1.1\r\nHost: example.org\r\nConnection: close\r\n\r\n";
    const METHOD_NOT_ALLOWED: &[u8] =
        b"DELETE /health HTTP/1.1\r\nHost: example.org\r\nConnection: close\r\n\r\n";
    const MALFORMED: &[u8] = b"GET /";

    const ROUTE_STATUS: &str = "app.http_router.route_status";
    const ROUTE_BODY_LEN: &str = "app.http_router.route_body_len";

    /// Each case is called twice through the interpreter and compared, so
    /// this also exercises the "same source in, identical artifacts out"
    /// determinism invariant rather than assuming it.
    fn deterministic_status(request: &[u8]) -> String {
        let first = call(ROUTE_STATUS, request);
        let second = call(ROUTE_STATUS, request);
        assert_eq!(first, second, "route_status is not deterministic");
        first
    }

    #[test]
    fn health_route_resolves_to_200() {
        assert_eq!(deterministic_status(HEALTH), "200");
    }

    #[test]
    fn echo_route_resolves_to_200_with_a_two_byte_body() {
        assert_eq!(deterministic_status(ECHO), "200");
        assert_eq!(call(ROUTE_BODY_LEN, ECHO), "2usize");
    }

    #[test]
    fn unknown_path_resolves_to_404() {
        assert_eq!(deterministic_status(MISSING), "404");
    }

    #[test]
    fn unsupported_method_resolves_to_405() {
        assert_eq!(deterministic_status(METHOD_NOT_ALLOWED), "405");
    }

    #[test]
    fn a_request_line_with_no_terminator_resolves_to_400() {
        assert_eq!(deterministic_status(MALFORMED), "400");
    }

    #[test]
    fn content_length_is_reported_absent_when_no_header_is_present() {
        let argument = serde_json::to_string(HEALTH).unwrap();
        let result = interpreter::interpret(
            &example_path(),
            "app.http_router.request_content_length",
            &[argument],
            &InterpreterOptions::default(),
        )
        .unwrap();
        assert!(result.returned);
        assert_eq!(returned_value(&result.envelope), "-1");
    }

    /// `examples/http_app_routing.spx`'s own `main` runs the same five cases
    /// through `route_status`/`route_body_len` and returns `0` on success, so
    /// this is a second, whole-module observation of the identical fixture
    /// exercise, independent of calling one function at a time above.
    #[test]
    fn the_committed_example_main_passes_its_own_fixture_exercise() {
        let source = std::fs::read_to_string(example_path()).unwrap();
        let program = parse(&source, example_path()).unwrap();
        assert!(verify::verify(&program).is_empty());
        let result = interpreter::interpret(
            &example_path(),
            "app.main",
            &[],
            &InterpreterOptions::default(),
        )
        .unwrap();
        assert!(result.returned);
        assert_eq!(returned_value(&result.envelope), "0");
    }
}

mod refusal {
    use super::*;

    fn diagnostic_codes(source: &str) -> Vec<String> {
        let program = parse(source, Path::new("http-app-routing-hostile.spx"))
            .unwrap_or_else(|error| panic!("{error}\n{source}"));
        verify::verify(&program)
            .into_iter()
            .map(|diagnostic| diagnostic.code.to_owned())
            .collect()
    }

    /// A handler reaching a capability it never declared is refused, not
    /// silently granted because it is "a route handler".
    #[test]
    fn a_handler_calling_an_undeclared_effect_is_spx_e102() {
        let source = r#"
module app.hostile_routing;

permit { process.stdout.write }

@id("app.hostile_routing.handler")
fn handler(view: borrow Slice<u8>) -> usize
{
    stdout_write(view)
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
        assert_eq!(diagnostic_codes(source), vec!["SPX-E102".to_owned()]);
    }

    /// A handler that declares the effect it needs still cannot reach it
    /// unless the module itself admits that authority: capability is
    /// declared at both ends, never opened implicitly by a route table.
    #[test]
    fn a_handler_effect_without_a_module_permit_is_spx_e101() {
        let source = r#"
module app.hostile_routing;

@id("app.hostile_routing.handler")
fn handler(view: borrow Slice<u8>) -> usize
    uses { process.stdout.write }
{
    stdout_write(view)
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
        assert_eq!(diagnostic_codes(source), vec!["SPX-E101".to_owned()]);
    }

    /// A route dispatcher over the closed route-code domain that forgets one
    /// code (and the catch-all) fails closed at compile time rather than at
    /// the first request that hits the missing arm.
    #[test]
    fn a_route_table_missing_its_catch_all_arm_is_spx_t257() {
        let source = r#"
module app.hostile_routing;

@id("app.hostile_routing.status_for")
fn status_for(route: i64) -> i64
{
    match route { 0 => 200, 1 => 404, }
}

@id("app.main")
fn main() -> i64
{
    status_for(0)
}
"#;
        assert_eq!(diagnostic_codes(source), vec!["SPX-T257".to_owned()]);
    }

    /// A handler cannot build a typed `Response` record directly inside a
    /// `match` arm; the profile reuses the language's existing aggregate
    /// match-arm rule unchanged rather than approximating it.
    #[test]
    fn building_a_typed_response_inside_a_match_arm_is_spx_t258() {
        let source = r#"
module app.hostile_routing;

@id("app.hostile_routing.response")
record Response {
    @id("app.hostile_routing.response.status")
    status: i64,
}

@id("app.hostile_routing.handle")
fn handle(route: i64) -> Response
{
    match route { 0 => Response { status: 200 }, _ => Response { status: 404 }, }
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
        assert_eq!(
            diagnostic_codes(source),
            vec!["SPX-T258".to_owned(), "SPX-T258".to_owned()]
        );
    }
}
