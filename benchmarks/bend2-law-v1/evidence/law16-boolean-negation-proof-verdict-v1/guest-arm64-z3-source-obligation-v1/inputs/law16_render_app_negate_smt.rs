use std::path::Path;

use semaprax::assurance_manifest::smt_discharge::{
    render_postcondition_script, translate_function,
};

fn main() {
    let path = std::env::args().nth(1).expect("source path argument");
    let source = std::fs::read_to_string(&path).expect("read source");
    let program = semaprax::parse(&source, Path::new(&path)).expect("parse source");
    let function = program
        .functions
        .iter()
        .find(|function| function.stable_id == "app.negate")
        .expect("app.negate declaration");
    let encoding = translate_function(function).expect("SMT subset translation");
    let rendered = render_postcondition_script(&encoding, 0, 10_000);
    let script = rendered
        .strip_suffix("(get-model)\n")
        .expect("installed project route expects a model query suffix");
    print!("{script}");
}
