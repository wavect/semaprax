//! Native lowering shared by total byte-slice and borrowed-text reads.
use super::{backend_error, c_case_symbol, c_field_symbol, CEmitter, COutput};
use crate::diagnostic::Diagnostic;
use crate::hir::{CompilerDeclaration, DeclarationId, ResolvedType};

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
        let none_id = DeclarationId::compiler(CompilerDeclaration::OptionNone);
        let some_id = DeclarationId::compiler(CompilerDeclaration::OptionSome);
        let value_id = DeclarationId::compiler(CompilerDeclaration::OptionSomeValue);
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
    pub(super) fn emit_option_word_read(
        &mut self,
        return_type: &ResolvedType,
        slice: &str,
        index: &str,
        width: &str,
        temporary: &str,
    ) -> Result<(), Diagnostic> {
        let none_id = DeclarationId::compiler(CompilerDeclaration::OptionNone);
        let some_id = DeclarationId::compiler(CompilerDeclaration::OptionSome);
        let value_id = DeclarationId::compiler(CompilerDeclaration::OptionSomeValue);
        let layout = self.variant_layout(return_type)?;
        let none = layout
            .case(&none_id)
            .ok_or_else(|| backend_error("byte_read_le Option has no None case"))?;
        let some = layout
            .case(&some_id)
            .ok_or_else(|| backend_error("byte_read_le Option has no Some case"))?;
        let field = some
            .field(&value_id)
            .ok_or_else(|| backend_error("byte_read_le Option has no Some payload"))?;
        self.require_type(&field.ty, &ResolvedType::Usize, "byte_read_le Some payload")?;
        self.line(&format!("memset(&{temporary}, 0, sizeof({temporary}));"));
        self.line(&format!("{temporary}.spx_tag = UINT32_C({});", none.tag));
        self.line(&format!("if ({width} >= UINT64_C(1) && {width} <= UINT64_C(8) && {index} <= ({slice}).len && {width} <= ({slice}).len - {index}) {{"));
        self.indent += 1;
        self.line("uint64_t spx_word_read_value = UINT64_C(0);");
        self.line(&format!("for (uint64_t spx_word_read_index = 0; spx_word_read_index < {width}; ++spx_word_read_index) {{"));
        self.indent += 1;
        self.line(&format!("spx_word_read_value |= ((uint64_t)({slice}).ptr[{index} + spx_word_read_index]) << (8 * spx_word_read_index);"));
        self.indent -= 1;
        self.line("}");
        self.line(&format!(
            "{temporary}.spx_payload.{}.{} = spx_word_read_value;",
            c_case_symbol(&some_id),
            c_field_symbol(&value_id)
        ));
        self.line(&format!("{temporary}.spx_tag = UINT32_C({});", some.tag));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }
}
