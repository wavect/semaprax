# project/image_facets/relationships.rs

- MAX_DEPTH · constant · L15-L15 — const MAX_DEPTH: usize = 256;
- MAX_VISITS · constant · L16-L16 — const MAX_VISITS: usize = 65_536;
- Access · enum · L18-L23 — enum Access
- name · function · L25-L32 — fn name(self) -> &'static str
- from_mode · function · L33-L39 — fn from_mode(mode: OwnershipMode) -> Self
- Context · struct · L42-L47 — struct Context<'a>
- Task · enum · L48-L51 — enum Task<'a>
- items · function · L53-L379 — pub(super) fn items(
- Collector · struct · L381-L388 — struct Collector<'a>
- push · function · L390-L412 — fn push(
- expression · function · L413-L476 — fn expression(
- place_facts · function · L478-L480 — fn place_facts(place: &Place) -> Value
- limit · function · L481-L486 — fn limit() -> Vec<Diagnostic>
