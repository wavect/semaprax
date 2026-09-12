# public_generic_consumer/cxx_calling/render.rs

- wrapper_header · function · L36-L52 — pub(super) fn wrapper_header(input: &RecordShape, output: &RecordShape) -> String
- HEADER_PRELUDE · constant · L54-L200 — const HEADER_PRELUDE: &str = r#"/*
- input_struct · function · L202-L219 — fn input_struct(input: &RecordShape) -> String
- output_class · function · L221-L263 — fn output_class(output: &RecordShape) -> String
- PROVIDER_CLASS_AND_ASSERTIONS · constant · L265-L340 — const PROVIDER_CLASS_AND_ASSERTIONS: &str = r#"/* Move-only RAII owner of one opened spx_pg_calling_consumer handle.
- DETAIL_MAKE_OWNED_BYTES · constant · L342-L367 — const DETAIL_MAKE_OWNED_BYTES: &str = r#"namespace detail
- PROVIDER_OPEN_DEFINITIONS · constant · L369-L385 — const PROVIDER_OPEN_DEFINITIONS: &str = r#"inline Result<Provider> Provider::open(const std::uint8_t *descriptor_bytes, std::size_t descriptor_len,
- provider_transform_definition · function · L395-L436 — fn provider_transform_definition(input: &RecordShape) -> String
- HEADER_TRAILER · constant · L438-L442 — const HEADER_TRAILER: &str = r#"
- round_trip_cpp · function · L455-L472 — pub(super) fn round_trip_cpp(input: &RecordShape, output: &RecordShape) -> String
- ROUND_TRIP_PRELUDE · constant · L474-L536 — const ROUND_TRIP_PRELUDE: &str = r#"/*
- sample_input_fn · function · L538-L552 — fn sample_input_fn(input: &RecordShape) -> String
- assert_reversed_fn · function · L554-L567 — fn assert_reversed_fn(input: &RecordShape, output: &RecordShape) -> String
- first_leaf_fn · function · L572-L577 — fn first_leaf_fn(output: &RecordShape) -> String
- zero_bytes_input_fn · function · L585-L601 — fn zero_bytes_input_fn(input: &RecordShape) -> String
- ROUND_TRIP_BODY · constant · L603-L864 — const ROUND_TRIP_BODY: &str = r#"static void test_success_round_trip()
- per_leaf_bound_tests_fn · function · L866-L915 — fn per_leaf_bound_tests_fn(input: &RecordShape, output: &RecordShape) -> String
- main_fn · function · L917-L943 — fn main_fn() -> String
