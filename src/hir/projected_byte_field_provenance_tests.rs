use super::*;

const SOURCE: &str = r#"
module test.projected_provenance_hostile;
@id("packet") record Packet {
    @id("packet.payload") payload: Bytes,
    @id("packet.sibling") sibling: Bytes,
}
@id("packet.view") fn view(packet: own Packet) -> usize {
    let direct = bytes_as_slice(packet.payload);
    let alias = direct;
    let range = byte_range(alias, 0usize, byte_len(alias));
    byte_len(range)
}
@id("app.main") fn main() -> i64 { 0 }
"#;

fn fixture() -> ResolvedProgram {
    let parsed = crate::parse(SOURCE, "projected-provenance-hostile.spx").unwrap();
    let program = resolve(&parsed).unwrap();
    validate(&program).unwrap();
    program
}

fn projected_value(program: &ResolvedProgram) -> ValueId {
    program
        .declarations
        .byte_slice_roots
        .iter()
        .find_map(|(value, provenance)| (!provenance.projections.is_empty()).then(|| value.clone()))
        .unwrap()
}

#[test]
fn projected_provenance_projection_and_type_are_independently_replayed() {
    let mut missing = fixture();
    let value = projected_value(&missing);
    missing
        .declarations
        .byte_slice_roots
        .get_mut(&value)
        .unwrap()
        .projections
        .clear();
    assert_eq!(validate(&missing).unwrap_err().code, "SPX-H006");

    let mut substituted = fixture();
    let value = projected_value(&substituted);
    substituted
        .declarations
        .byte_slice_roots
        .get_mut(&value)
        .unwrap()
        .projections = vec![PlaceProjection::Field(DeclarationId::new("packet.sibling"))];
    assert_eq!(validate(&substituted).unwrap_err().code, "SPX-H006");

    let mut wrong_type = fixture();
    let value = projected_value(&wrong_type);
    wrong_type
        .declarations
        .byte_slice_roots
        .get_mut(&value)
        .unwrap()
        .projected_type = ResolvedType::SliceU8;
    assert_eq!(validate(&wrong_type).unwrap_err().code, "SPX-H006");
}
