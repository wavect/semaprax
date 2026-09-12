# public_generic_abi/boundary_profile.rs

- BOUNDARY_PROFILE_SCHEMA · constant · L19-L19 — pub const BOUNDARY_PROFILE_SCHEMA: &str = "semaprax.public-generic-boundary-profile.v1";
- MAX_INSTANCE_TERM_BYTES · constant · L23-L23 — pub const MAX_INSTANCE_TERM_BYTES: usize = MAX_TERM_BYTES;
- MAX_NESTING_DEPTH · constant · L27-L27 — pub const MAX_NESTING_DEPTH: usize = MAX_RECORD_DEPTH;
- MAX_OWNED_LEAVES_PER_INSTANCE · constant · L31-L31 — pub const MAX_OWNED_LEAVES_PER_INSTANCE: usize = MAX_OWNED_LEAVES;
- MAX_VISITED_NODES_PER_INSTANCE · constant · L37-L37 — pub const MAX_VISITED_NODES_PER_INSTANCE: usize = MAX_VISITED_NODES;
- MAX_TEMPLATE_ARITY_BOUND · constant · L40-L40 — pub const MAX_TEMPLATE_ARITY_BOUND: usize = MAX_TEMPLATE_ARITY;
- OWNED_INPUT_PARAMETER_COUNT · constant · L44-L44 — pub const OWNED_INPUT_PARAMETER_COUNT: usize = 1;
- OWNED_RESULT_COUNT · constant · L47-L47 — pub const OWNED_RESULT_COUNT: usize = 1;
- MAX_FIELDS_PER_RECORD · constant · L53-L53 — pub const MAX_FIELDS_PER_RECORD: usize = 256;
- MAX_BYTES_PER_LEAF · constant · L57-L57 — pub const MAX_BYTES_PER_LEAF: usize = 64 * 1024;
- MAX_TOTAL_PAYLOAD_BYTES · constant · L63-L63 — pub const MAX_TOTAL_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
- MAX_LIVE_HANDLES · constant · L68-L68 — pub const MAX_LIVE_HANDLES: usize = MAX_OWNED_LEAVES_PER_INSTANCE + 1;
- MAX_DESCRIPTOR_WIRE_BYTES · constant · L74-L74 — pub const MAX_DESCRIPTOR_WIRE_BYTES: usize = 2 * MAX_INSTANCE_TERM_BYTES;
- tests · module · L77-L101 — mod tests
- leaf_and_payload_bounds_are_mutually_consistent · function · L84-L89 — fn leaf_and_payload_bounds_are_mutually_consistent()
- handle_bound_is_leaf_bound_plus_the_root_handle · function · L92-L94 — fn handle_bound_is_leaf_bound_plus_the_root_handle()
- exactly_one_input_and_one_result_is_the_v1_invariant · function · L97-L100 — fn exactly_one_input_and_one_result_is_the_v1_invariant()
