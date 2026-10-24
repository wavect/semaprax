// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_7 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $enter_phase_3<'a: 'e, 'e>(
            expression: &'e ResolvedExpr,
            scope: scope_checkpoint::Scope,
            path: &'e str,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
        ) -> Result<(), Diagnostic> {
            match &expression.kind {
                ResolvedExprKind::Try { operand, .. } => {
                    frames.push($Frame::Try {
                        expression,
                        path,
                        option: false,
                    })?;
                    frames.push($Frame::Enter {
                        expression: operand,
                        scope,
                        path: path_label::Label::child(path, ".operand", [0, 0])?,
                    })?;
                }
                ResolvedExprKind::TryOption { operand, .. } => {
                    frames.push($Frame::Try {
                        expression,
                        path,
                        option: true,
                    })?;
                    frames.push($Frame::Enter {
                        expression: operand,
                        scope,
                        path: path_label::Label::child(path, ".operand", [0, 0])?,
                    })?;
                }
                _ => unreachable!("expression was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
