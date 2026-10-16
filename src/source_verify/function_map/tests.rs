use super::*;
use crate::ast::Function;
use std::mem::{align_of, size_of};
use std::path::Path;

#[test]
fn borrowed_map_preserves_old_header_pair_layout_and_duplicate_cell_semantics() {
    assert_eq!(
        size_of::<HashMap<&str, &Function>>(),
        size_of::<FunctionMap<'_>>()
    );
    assert_eq!(
        align_of::<HashMap<&str, &Function>>(),
        align_of::<FunctionMap<'_>>()
    );
    assert_eq!(
        size_of::<(&str, &Function)>(),
        size_of::<(&str, &FunctionRef<'_>)>()
    );
    assert_eq!(
        align_of::<(&str, &Function)>(),
        align_of::<(&str, &FunctionRef<'_>)>()
    );
    // Real target-layout receivers of the unchanged HashMap algorithm.
    assert_eq!(
        size_of::<std::collections::hash_map::Iter<'_, &str, &Function>>(),
        size_of::<std::collections::hash_map::Iter<'_, &str, &FunctionRef<'_>>>()
    );
    assert_eq!(
        align_of::<std::collections::hash_map::Iter<'_, &str, &Function>>(),
        align_of::<std::collections::hash_map::Iter<'_, &str, &FunctionRef<'_>>>()
    );
    assert_eq!(
        size_of::<std::collections::hash_map::Keys<'_, &str, &Function>>(),
        size_of::<std::collections::hash_map::Keys<'_, &str, &FunctionRef<'_>>>()
    );
    assert_eq!(
        align_of::<std::collections::hash_map::Keys<'_, &str, &Function>>(),
        align_of::<std::collections::hash_map::Keys<'_, &str, &FunctionRef<'_>>>()
    );
    let program = crate::parse(
        "module duplicate; fn run()->i64{1} fn run()->i64{2}",
        Path::new("cells.spx"),
    )
    .unwrap();
    let cells = crate::source_verify::capacity::source_capacity_functions(&program);
    let mut map = FunctionMap::new();
    assert!(map.insert(cells[0].1.name(), &cells[0].1).is_none());
    assert!(std::ptr::eq(
        map.insert(cells[1].1.name(), &cells[1].1).unwrap(),
        &cells[0].1
    ));
    assert!(std::ptr::eq(map.get("run").unwrap(), &cells[1].1));
    let (key, value) = map.entries.get_key_value("run").unwrap();
    assert_eq!(key.as_ptr(), cells[0].1.name().as_ptr());
    assert!(std::ptr::eq(*value, &cells[1].1));
    assert!(map.get("absent").is_none());
}

#[test]
fn current_cell_loan_does_not_bind_ast_recovery_lifetime_and_preserves_empty_scope() {
    let program = crate::parse(
        "module current; fn main()->i64{0}",
        Path::new("current-cell.spx"),
    )
    .unwrap();
    let cells = crate::source_verify::capacity::source_capacity_functions(&program);
    let functions = cells
        .iter()
        .map(|(_, function)| (function.name(), function))
        .collect();
    let types = crate::source_verify::type_table::TypeTable::new(&program);
    let mut variables = HashMap::new();
    let mut diagnostics = Vec::new();
    let (value, refusal) = {
        // The current cell deliberately has a shorter lifetime than the AST
        // and the recovery result. Nothing in the frame stores this local.
        let current = FunctionRef::ordinary(&program.functions[0]);
        crate::source_verify::iterative::check_expr_with_signature_cell(
            &program,
            &current,
            &program.functions[0].body,
            &mut variables,
            &functions,
            &types,
            None,
            false,
            &mut diagnostics,
            None,
            None,
            None,
        )
    };
    assert!(value.is_some());
    assert!(refusal.is_none());
    assert!(variables.is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn packed_capacity_cell_layout_and_class_name_identity_match_the_old_owner() {
    use crate::ast::program_source::FunctionCell;
    use crate::ast::{TypeDeclaration, TypeDeclarationKind};
    use std::alloc::Layout;
    type OldCell<'a> = (Option<&'a str>, &'a Function);
    assert_eq!(size_of::<OldCell<'_>>(), size_of::<FunctionCell<'_>>());
    assert_eq!(align_of::<OldCell<'_>>(), align_of::<FunctionCell<'_>>());
    assert_eq!(
        size_of::<Option<OldCell<'_>>>(),
        size_of::<Option<FunctionCell<'_>>>()
    );
    assert_eq!(
        align_of::<Option<OldCell<'_>>>(),
        align_of::<Option<FunctionCell<'_>>>()
    );
    for count in [0, 1, 16, crate::byte_data_capacity::MAX_FUNCTIONS] {
        assert_eq!(
            Layout::array::<OldCell<'_>>(count).unwrap(),
            Layout::array::<FunctionCell<'_>>(count).unwrap()
        );
    }
    let program = crate::parse(r#"module owner; @id("owner.Cell") class Cell { @id("owner.read") fn read()->i64{1} } @id("owner.main") fn main()->i64{0}"#, Path::new("class-owner-cell.spx")).unwrap();
    let cells = crate::source_verify::function_cells(&program);
    let class: &TypeDeclaration = &program.types[0];
    let TypeDeclarationKind::Class { methods, .. } = &class.kind else {
        panic!("class fixture");
    };
    assert_eq!(cells.len(), program.functions.len() + methods.len());
    for (index, function) in program.functions.iter().enumerate() {
        assert!(cells[index].0.is_none());
        assert!(std::ptr::eq(cells[index].1.payload(), function));
    }
    for (index, method) in methods.iter().enumerate() {
        let cell = &cells[program.functions.len() + index];
        let owner = cell.0.unwrap();
        assert!(std::ptr::eq(owner, class));
        assert_eq!(owner.name.as_str().as_ptr(), class.name.as_str().as_ptr());
        assert!(std::ptr::eq(cell.1.payload(), method));
        assert_eq!(cell.1.name(), method.name);
    }
}
