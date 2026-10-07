//! Compiler-owned owned-string operations, including Text Toolkit v1.
use super::*;

impl Evaluator<'_> {
    pub(super) fn evaluate_string_op(
        &mut self,
        op: crate::string_ops::StringOp,
        args: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        self.charge()?;
        let mut values = Vec::with_capacity(args.len());
        for argument in args {
            values.push(self.evaluate(argument, environment, depth)?);
        }
        if op.is_text_toolkit() {
            return self.evaluate_text_toolkit(op, &values);
        }
        match op {
            crate::string_ops::StringOp::Len => match values.first() {
                Some(Value::String(value)) => Ok(Value::Int(value.len() as i64)),
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::IsEmpty => match values.first() {
                Some(Value::String(value)) => Ok(Value::Bool(value.is_empty())),
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::Concat => match (values.first(), values.get(1)) {
                (Some(Value::String(left)), Some(Value::String(right))) => {
                    let length = left.len().checked_add(right.len()).ok_or(
                        Flow::Utf8MaterializationLimitExceeded {
                            attempted_materializations: u64::MAX,
                            attempted_bytes: u64::MAX,
                        },
                    )?;
                    self.charge_utf8_materialization(length)?;
                    let mut result = String::with_capacity(length);
                    result.push_str(left);
                    result.push_str(right);
                    Ok(Value::String(result))
                }
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::StartsWith => match (values.first(), values.get(1)) {
                (Some(Value::String(value)), Some(Value::String(prefix))) => {
                    Ok(Value::Bool(value.starts_with(prefix.as_str())))
                }
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::Contains => match (values.first(), values.get(1)) {
                (Some(Value::String(value)), Some(Value::String(needle))) => {
                    Ok(Value::Bool(value.contains(needle.as_str())))
                }
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::LenChars => match values.first() {
                Some(Value::String(value)) => Ok(Value::Int(value.chars().count() as i64)),
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::FromChar => match values.first() {
                Some(Value::Char(scalar)) => match char::from_u32(*scalar) {
                    Some(value) => {
                        let mut bytes = [0u8; 4];
                        let value = value.encode_utf8(&mut bytes);
                        Ok(Value::String(self.materialize_utf8_copy(value)?))
                    }
                    None => Err(Flow::Guard("ill-typed string operation operand")),
                },
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::FromI64 => match values.first() {
                Some(Value::Int(value)) => Ok(Value::String(
                    self.materialize_utf8_copy(&value.to_string())?,
                )),
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::FromUsize => match values.first() {
                Some(Value::Usize(value)) => Ok(Value::String(
                    self.materialize_utf8_copy(&value.to_string())?,
                )),
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            _ => Err(Flow::Guard("ill-typed string operation operand")),
        }
    }

    /// Text Toolkit v1. Offsets are byte offsets; every failure is the
    /// checked `semaprax.text.v1` status the native backend selects too.
    fn evaluate_text_toolkit(
        &mut self,
        op: crate::string_ops::StringOp,
        values: &[Value],
    ) -> Result<Value, Flow> {
        use crate::string_ops::StringOp;
        match (op, values) {
            (StringOp::Slice, [Value::String(text), Value::Int(start), Value::Int(end)]) => {
                let (start, end) = text_range(text, *start, *end)?;
                if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                    return Err(text_failure(crate::string_ops::TEXT_NOT_CHAR_BOUNDARY_CODE));
                }
                Ok(Value::String(
                    self.materialize_utf8_copy(&text[start..end])?,
                ))
            }
            (StringOp::Find, [Value::String(text), Value::String(needle), Value::Int(from)]) => {
                let (from, _) = text_range(text, *from, text.len() as i64)?;
                Ok(Value::Int(
                    crate::string_ops::find_bytes(text.as_bytes(), needle.as_bytes(), from)
                        .map_or(-1, |offset| offset as i64),
                ))
            }
            (StringOp::ToI64, [Value::String(text)]) => Ok(Value::OptionI64(
                crate::string_ops::parse_i64(text.as_bytes()),
            )),
            (StringOp::Trim, [Value::String(text)]) => {
                let trimmed = crate::string_ops::trim_range(text.as_bytes());
                Ok(Value::String(self.materialize_utf8_copy(&text[trimmed])?))
            }
            (StringOp::ByteAt, [Value::String(text), Value::Int(index)]) => {
                let (index, _) = text_range(text, *index, text.len() as i64)?;
                text.as_bytes()
                    .get(index)
                    .map(|byte| Value::Int(i64::from(*byte)))
                    .ok_or_else(|| text_failure(crate::string_ops::TEXT_OUT_OF_RANGE_CODE))
            }
            (StringOp::FileReadText, [Value::BorrowedStr(path)]) => {
                let bytes = self.read_file_text(path.bytes.as_ref())?;
                let text = String::from_utf8(bytes)
                    .map_err(|_| text_failure(crate::string_ops::TEXT_INVALID_UTF8_CODE))?;
                self.charge_utf8_materialization(text.len())?;
                Ok(Value::String(text))
            }
            _ => Err(Flow::Guard("ill-typed Text Toolkit operand")),
        }
    }
}

/// Check `0 <= start <= end <= len` and convert to native offsets.
fn text_range(text: &str, start: i64, end: i64) -> Result<(usize, usize), Flow> {
    let length = text.len() as i64;
    if start < 0 || start > end || end > length {
        return Err(text_failure(crate::string_ops::TEXT_OUT_OF_RANGE_CODE));
    }
    Ok((start as usize, end as usize))
}

fn text_failure(code: u32) -> Flow {
    Flow::Failure(
        NormalizedStatus::try_new(
            crate::string_ops::TEXT_STATUS_DOMAIN,
            code,
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .expect("compiler-owned text status table is valid"),
    )
}
