//! Native lowering shared by total byte-slice and borrowed-text reads.
use super::{backend_error, c_case_symbol, c_field_symbol, CEmitter, COutput};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedType};

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_option_byte_read(
        &mut self,
        return_type: &ResolvedType,
        slice: &str,
        data_field: &str,
        index: &str,
        temporary: &str,
    ) -> Result<(), Diagnostic> {
        let layout = self.variant_layout(return_type)?;
        let none_id = DeclarationId::new(crate::prelude::OPTION_NONE_ID);
        let some_id = DeclarationId::new(crate::prelude::OPTION_SOME_ID);
        let value_id = DeclarationId::new(crate::prelude::OPTION_SOME_VALUE_ID);
        let none = layout
            .case(&none_id)
            .ok_or_else(|| backend_error("Option<u8> layout has no compiler-owned None case"))?;
        let some = layout
            .case(&some_id)
            .ok_or_else(|| backend_error("Option<u8> layout has no compiler-owned Some case"))?;
        let field = some
            .field(&value_id)
            .ok_or_else(|| backend_error("Option<u8> layout has no compiler-owned Some payload"))?;
        self.require_type(&field.ty, &ResolvedType::U8, "byte_get Some payload")?;
        self.line(&format!("memset(&{temporary}, 0, sizeof({temporary}));"));
        self.line(&format!("if ({index} < ({slice}).len) {{"));
        self.indent += 1;
        self.line(&format!(
            "{temporary}.spx_payload.{}.{} = ({slice}).{data_field}[{index}];",
            c_case_symbol(&some_id),
            c_field_symbol(&value_id)
        ));
        self.line(&format!("{temporary}.spx_tag = UINT32_C({});", some.tag));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.line(&format!("{temporary}.spx_tag = UINT32_C({});", none.tag));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }
}
