//! Explicit lexical scopes; unfamiliar lowerings keep the complete old charge.
use super::*;

#[derive(Clone, Copy)]
enum Frame<'a> {
    Expr(&'a Expr),
    Block(&'a [Statement], usize, &'a Expr, usize),
    Bind(&'a str),
    Restore(usize),
    Args(&'a [Expr], usize),
}

struct Scratch<'a> {
    frames: [Option<Frame<'a>>; MAX_FRAMES],
    length: usize,
    names: [Option<&'a str>; MAX_NAMES],
    bound: usize,
}
type Entry<'a> = (
    &'a Function,
    &'a Function,
    &'a Headers<'a>,
    &'a Headers<'a>,
    &'a Invoice,
    &'a Invoice,
    Scratch<'a>,
    Push<'a>,
    std::slice::Iter<'a, crate::ast::Param>,
    usize,
    Result<Option<usize>, Failure>,
    Option<usize>,
);
type Push<'a> = (
    &'a mut Scratch<'a>,
    &'a mut Scratch<'a>,
    Frame<'a>,
    Frame<'a>,
    Option<Frame<'a>>,
    &'a mut [Option<Frame<'a>>; MAX_FRAMES],
    &'a mut [Option<Frame<'a>>; MAX_FRAMES],
    usize,
    usize,
    usize,
    usize,
    usize,
    bool,
    bool,
);
type Turn<'a> = (
    Push<'a>,
    Push<'a>,
    Push<'a>,
    &'a Invoice,
    &'a mut Scratch<'a>,
    &'a [Option<Frame<'a>>; MAX_FRAMES],
    &'a mut [Option<Frame<'a>>; MAX_FRAMES],
    usize,
    usize,
    Option<Frame<'a>>,
    Frame<'a>,
    Frame<'a>,
    &'a Expr,
    &'a ExprKind,
    &'a [Statement],
    &'a Statement,
    Option<&'a Statement>,
    &'a [Expr],
    Option<&'a Expr>,
    &'a Option<Type>,
    bool,
    bool,
    bool,
    bool,
    usize,
);
type Binding<'a> = (
    Result<bool, Failure>,
    Result<bool, Failure>,
    &'a mut Scratch<'a>,
    &'a str,
    Option<&'a str>,
    usize,
    usize,
    &'a mut Scratch<'a>,
    &'a str,
    &'a Invoice,
    &'a mut [Option<&'a str>; MAX_NAMES],
    usize,
    Option<&'a str>,
    bool,
    bool,
    usize,
);
type Probe<'a> = (
    Result<bool, Failure>,
    Result<bool, Failure>,
    &'a Scratch<'a>,
    &'a str,
    Option<&'a str>,
    Option<&'a str>,
    usize,
    usize,
    &'a Scratch<'a>,
    &'a str,
    &'a Invoice,
    &'a [Option<&'a str>; MAX_NAMES],
    usize,
    &'a Option<&'a str>,
    &'a str,
    &'a str,
    bool,
    bool,
);

impl<'a> Scratch<'a> {
    fn push(&mut self, frame: Frame<'a>) -> bool {
        if self.length == MAX_FRAMES {
            return false;
        }
        self.frames[self.length] = Some(frame);
        self.length += 1;
        true
    }
    fn bind(&mut self, name: &'a str, invoice: &Invoice) -> Result<bool, Failure> {
        invoice.pay::<Binding<'_>>()?;
        if self.bound == MAX_NAMES {
            return Ok(false);
        }
        self.names[self.bound] = Some(name);
        self.bound += 1;
        Ok(true)
    }
    fn shadowed(&self, name: &str, invoice: &Invoice) -> Result<bool, Failure> {
        let mut index = 0;
        loop {
            invoice.pay::<Probe<'_>>()?;
            if index == self.bound {
                break;
            }
            invoice.work(name.len().saturating_add(1))?;
            if self.names[index] == Some(name) {
                return Ok(true);
            }
            index += 1;
        }
        Ok(false)
    }
}

pub(super) fn count<'p, 'a>(
    context: Context<'p, 'a>,
    function: &'p Function,
    headers: &Headers<'p>,
    invoice: &Invoice,
) -> Result<Option<usize>, Failure> {
    invoice.pay::<Entry<'_>>()?;
    let mut scratch = Scratch {
        frames: [None; MAX_FRAMES],
        length: 0,
        names: [None; MAX_NAMES],
        bound: 0,
    };
    let mut parameters = function.params.iter();
    loop {
        invoice.pay::<invoice::Advance<'_, crate::ast::Param>>()?;
        let Some(parameter) = parameters.next() else {
            break;
        };
        if !scratch.bind(&parameter.name, invoice)? {
            return Ok(None);
        }
    }
    // Only authored body expressions are counted. Contract/default/generic
    // expansion identities retain the old independent allowance in full.
    scratch.push(Frame::Expr(&function.body));
    let mut count = 0usize;
    while scratch.length != 0 {
        invoice.pay::<Turn<'_>>()?;
        invoice.work(1)?;
        scratch.length -= 1;
        let frame = scratch.frames[scratch.length]
            .take()
            .expect("occupied frontier");
        let complete = match frame {
            Frame::Bind(name) => scratch.bind(name, invoice)?,
            Frame::Restore(bound) => {
                scratch.bound = bound;
                true
            }
            Frame::Args(args, index) => {
                if let Some(argument) = args.get(index) {
                    scratch.push(Frame::Args(args, index + 1))
                        && scratch.push(Frame::Expr(argument))
                } else {
                    true
                }
            }
            Frame::Block(statements, index, tail, bound) => {
                if let Some(statement) = statements.get(index) {
                    if !scratch.push(Frame::Block(statements, index + 1, tail, bound)) {
                        return Ok(None);
                    }
                    match statement {
                        Statement::Let {
                            name,
                            declared,
                            value,
                            ..
                        } if match declared {
                            None => true,
                            Some(ty) => plain(ty),
                        } =>
                        {
                            scratch.push(Frame::Bind(name)) && scratch.push(Frame::Expr(value))
                        }
                        Statement::Assign {
                            field: None, value, ..
                        } => scratch.push(Frame::Expr(value)),
                        Statement::While {
                            condition, body, ..
                        } => {
                            scratch.push(Frame::Expr(body)) && scratch.push(Frame::Expr(condition))
                        }
                        _ => false,
                    }
                } else {
                    scratch.push(Frame::Restore(bound)) && scratch.push(Frame::Expr(tail))
                }
            }
            Frame::Expr(expression) => match &expression.kind {
                ExprKind::Call {
                    name,
                    type_arguments,
                    args,
                } => {
                    invoice.pay::<invoice::Query<'_>>()?;
                    invoice.work(name.len().saturating_add(headers.len()).saturating_add(1))?;
                    if type_arguments.is_empty()
                        && !compiler(name, invoice)?
                        && !scratch.shadowed(name, invoice)?
                    {
                        if let Some(Some(target)) = headers.get(name.as_str()) {
                            invoice.work(
                                target
                                    .stable_id
                                    .len()
                                    .saturating_add(target.params.len())
                                    .saturating_add(1),
                            )?;
                            if ordinary(target, invoice)?
                                && target.params.len() == args.len()
                                && local(context, target, invoice)?
                            {
                                count = count.checked_add(1).ok_or(Failure::Arithmetic)?;
                            }
                        }
                    }
                    scratch.push(Frame::Args(args, 0))
                }
                ExprKind::Block { statements, tail } => {
                    scratch.push(Frame::Block(statements, 0, tail, scratch.bound))
                }
                ExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    scratch.push(Frame::Expr(else_branch))
                        && scratch.push(Frame::Expr(then_branch))
                        && scratch.push(Frame::Expr(condition))
                }
                ExprKind::Unary { value, .. } => scratch.push(Frame::Expr(value)),
                ExprKind::Binary { left, right, .. } => {
                    scratch.push(Frame::Expr(right)) && scratch.push(Frame::Expr(left))
                }
                ExprKind::Var(_)
                | ExprKind::Int(_)
                | ExprKind::Int32(_)
                | ExprKind::Uint8(_)
                | ExprKind::Usize(_)
                | ExprKind::Char(_)
                | ExprKind::Float32(_)
                | ExprKind::Float64(_)
                | ExprKind::Bool(_)
                | ExprKind::String(_)
                | ExprKind::BorrowedUtf8Literal(_)
                | ExprKind::ArrayU8(_)
                | ExprKind::RepeatArrayU8 { .. } => true,
                // Includes closure/indirect/method/nominal/pattern/for/default
                // lowerings; a future shape cannot silently enter this proof.
                _ => false,
            },
        };
        if !complete {
            return Ok(None);
        }
    }
    Ok(Some(count))
}
