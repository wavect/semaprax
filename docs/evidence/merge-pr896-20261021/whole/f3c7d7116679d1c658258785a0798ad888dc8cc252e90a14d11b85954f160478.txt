use super::*;
pub(super) const SOURCE: &str = r#"module replay.flat_records;
@id("flat.Packet") record Packet {
 @id("flat.Packet.n") n:i64,@id("flat.Packet.text") text:string,@id("flat.Packet.data") data:Bytes,
}
@id("flat.identity") fn identity(value:own Packet,flag:bool)->Packet {
 let mut next=0;while next<1 {next=next+1;if flag {return value;} else {0} 0} value
}
@id("flat.make") fn make(input:borrow Slice<u8>)->Packet {
 let key=str"x";let value=Packet{n:7,text:"label",data:bytes_copy(input)};
 let mut next=0;while next<1 {next=next+1;if byte_len(input)>0usize {return value;} else {0} 0} value
}
@id("flat.read") fn read(value:borrow Packet)->i64 {value.n}
@id("flat.probe") fn probe(flag:bool)->i64 {
 let input=[1u8,2u8];let packet=identity(make(array_as_slice(input)),flag);
 let mut next=0;while next<2 {next=next+1;if flag {continue;} else {0} if next==2 {return read(packet);} else {0} 0} read(packet)
}
@id("app.main") fn main()->i64 {probe(false)}
"#;
#[test]
fn genuine_retained_nominal_success_matches_whole_old_and_full_ordinary() {
    let standalone = program(SOURCE);
    crate::cleanup_plan::replay::validate_program(&standalone).unwrap();
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(SOURCE);
    fixture.with_context(|context| {
        for id in [
            "flat.make",
            "flat.identity",
            "flat.probe",
            "flat.read",
            "app.main",
        ] {
            let function = context
                .functions()
                .iter()
                .find(|f| f.id.as_str() == id)
                .unwrap();
            let old = owner(context, function, true);
            let new = owner(context, function, false);
            assert_eq!(new, old, "all six fields nominal {id}");
            new.unwrap();
        }
    });
    let (_, observed) = measure(|| {
        fixture.with_context(|context| {
            let function = context
                .functions()
                .iter()
                .find(|f| f.id.as_str() == "flat.probe")
                .unwrap();
            for old in [true, false] {
                owner(context, function, old).unwrap();
            }
        })
    });
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}
#[test]
fn held_nominal_unknown_entry_and_exit_order_keep_complete_old_owner() {
    for case in 0..2 {
        let mut fixture = crate::workspace_graph::SourceRecordReplayFixture::new(SOURCE);
        let function = fixture.function_mut("flat.probe");
        if case == 0 {
            function.cleanup_plan.entry = BlockId(u32::MAX);
        } else {
            function.cleanup_plan.exits[0]
                .leaves_regions
                .push(CleanupRegionId(0));
        }
        fixture.with_context(|context| {
            let function = context
                .functions()
                .iter()
                .find(|f| f.id.as_str() == "flat.probe")
                .unwrap();
            let old = owner(context, function, true);
            let new = owner(context, function, false);
            assert_eq!(new, old, "nominal first refusal {case}");
            assert!(new.is_err());
        });
    }
}
