//! Unfunded legacy disposal reuses already-owned recursive argument backing.
//! It neither discovers a payer nor changes the funded index workspace path.
use super::ResolvedType;

pub(super) fn dispose(mut current: ResolvedType) {
    let mut pending = Vec::new();
    loop {
        match current {
            ResolvedType::Nominal { mut arguments, .. } => {
                if let Some(next) = arguments.pop() {
                    // A unary/deep root needs no new queue: its actual owned
                    // argument Vec is the continuation carrier. Every root is
                    // fully drained before the next field is visited.
                    if pending.is_empty() {
                        pending = arguments;
                    } else {
                        // Wider legacy frontiers may retain ordinary scratch
                        // growth. No zero-allocation or funding claim for it.
                        pending.append(&mut arguments);
                    }
                    current = next;
                    continue;
                }
            }
            ResolvedType::Function {
                mut parameters,
                result,
            } => {
                if pending.is_empty() {
                    pending = parameters;
                } else {
                    pending.append(&mut parameters);
                }
                current = *result;
                continue;
            }
            _ => {}
        }
        match pending.pop() {
            Some(next) => current = next,
            None => return,
        }
    }
}

#[cfg(test)]
mod before;
#[cfg(test)]
mod tests;
