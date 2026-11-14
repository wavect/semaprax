//! Fixed provider storage for the finite owned-Bytes iterator profile.
//! The compiler must prove both constructor capacity and live table demand
//! before selecting these functions. Handles contain a row and its generation.
use wasm_encoder::{BlockType, Encode, Function, Instruction as I, MemArg, ValType};

const SLOTS: i32 = 32;
const ROW_CAPACITY: i32 = 32;
const KIND: u64 = 0;
const LENGTH: u64 = 4;
const CAPACITY: u64 = 8;
const CURSOR: u64 = 12;
const HANDLE: u64 = 16;
const ITEMS: u64 = 24;
const ROW_BYTES: i32 = ITEMS as i32 + ROW_CAPACITY * 8;
pub(super) const TABLE_BYTES: u32 = (SLOTS * ROW_BYTES) as u32;
const VECTOR: i32 = 1;
const ITERATOR: i32 = 2;

#[derive(Clone, Copy)]
pub(super) struct Runtime {
    pub(super) table: u32,
    pub(super) resolve_bytes: u32,
    pub(super) drop_bytes: u32,
}

fn mem(offset: u64, align: u32) -> MemArg {
    MemArg {
        offset,
        align,
        memory_index: 0,
    }
}

fn emit(body: &mut Function, instructions: &[I<'_>]) {
    for instruction in instructions {
        body.instruction(instruction);
    }
}

fn trap_if(body: &mut Function) {
    emit(body, &[I::If(BlockType::Empty), I::Unreachable, I::End]);
}

fn refuse_if(body: &mut Function, result: I<'_>) {
    emit(body, &[I::If(BlockType::Empty), result, I::Return, I::End]);
}

fn finish(code: &mut Vec<u8>, mut body: Function) {
    body.instruction(&I::End);
    body.encode(code);
}

impl Runtime {
    fn authenticate(self, body: &mut Function, row: u32, kind: i32, refusal: Option<I<'_>>) {
        emit(
            body,
            &[
                I::LocalGet(0),
                I::I32WrapI64,
                I::I32Const(SLOTS - 1),
                I::I32And,
                I::I32Const(ROW_BYTES),
                I::I32Mul,
                I::I32Const(self.table as i32),
                I::I32Add,
                I::LocalTee(row),
                I::I64Load(mem(HANDLE, 3)),
                I::LocalGet(0),
                I::I64Ne,
                I::LocalGet(row),
                I::I32Load(mem(KIND, 2)),
                I::I32Const(kind),
                I::I32Ne,
                I::I32Or,
            ],
        );
        match refusal {
            Some(result) => refuse_if(body, result),
            None => trap_if(body),
        }
    }

    // Leave the next handle on the stack. Establish it before mutating owners.
    fn renew(body: &mut Function, row: u32, handle: u32) {
        emit(
            body,
            &[
                I::LocalGet(row),
                I::I64Load(mem(HANDLE, 3)),
                I::I64Const(i64::MAX - i64::from(SLOTS)),
                I::I64GtU,
            ],
        );
        trap_if(body);
        emit(
            body,
            &[
                I::LocalGet(row),
                I::LocalGet(row),
                I::I64Load(mem(HANDLE, 3)),
                I::I64Const(i64::from(SLOTS)),
                I::I64Add,
                I::LocalTee(handle),
                I::I64Store(mem(HANDLE, 3)),
                I::LocalGet(handle),
            ],
        );
    }

    fn owned_bytes(self, body: &mut Function, carrier: u32) {
        emit(body, &[I::LocalGet(carrier), I::I64Const(0), I::I64GeS]);
        trap_if(body);
        emit(
            body,
            &[I::LocalGet(carrier), I::Call(self.resolve_bytes), I::Drop],
        );
    }

    fn output(body: &mut Function, local: u32, bytes: i32) {
        // Generated aggregate frames are in the first shadow-stack page.
        emit(
            body,
            &[I::LocalGet(local), I::I32Const(65_536 - bytes), I::I32GtU],
        );
        refuse_if(body, I::I32Const(2));
    }

    fn item_address(body: &mut Function, row: u32, cursor: u32) {
        emit(
            body,
            &[
                I::LocalGet(row),
                I::LocalGet(cursor),
                I::I32Const(8),
                I::I32Mul,
                I::I32Add,
            ],
        );
    }

    pub(super) fn reset_bytes(self, body: &mut Vec<u8>) {
        let mut instructions = Function::new([]);
        self.reset(&mut instructions);
        body.extend_from_slice(&instructions.into_raw_body()[1..]);
    }

    pub(super) fn verify_settled(self, body: &mut Vec<u8>) {
        for slot in 0..SLOTS {
            let row = self.table as i32 + slot * ROW_BYTES;
            I::I32Const(row).encode(body);
            I::I32Load(mem(KIND, 2)).encode(body);
            I::If(BlockType::Empty).encode(body);
            I::Unreachable.encode(body);
            I::End.encode(body);
        }
    }

    fn reset(self, body: &mut Function) {
        for slot in 0..SLOTS {
            let row = self.table as i32 + slot * ROW_BYTES;
            // Preserve the generation across invocations. Only a fresh row
            // needs its low slot bits initialized.
            emit(
                body,
                &[I::I32Const(row), I::I64Load(mem(HANDLE, 3)), I::I64Eqz],
            );
            emit(
                body,
                &[
                    I::If(BlockType::Empty),
                    I::I32Const(row),
                    I::I64Const(i64::from(slot)),
                    I::I64Store(mem(HANDLE, 3)),
                    I::End,
                ],
            );
        }
    }

    pub(super) fn bodies(self, code: &mut Vec<u8>, include_get_slot: bool) {
        // with_capacity(tag, capacity) -> handle
        let mut body = Function::new_with_locals_types([ValType::I32, ValType::I64]);
        emit(
            &mut body,
            &[
                I::LocalGet(0),
                I::I32Const(9),
                I::I32Ne,
                I::LocalGet(1),
                I::I64Const(i64::from(ROW_CAPACITY)),
                I::I64GtU,
                I::I32Or,
            ],
        );
        refuse_if(&mut body, I::I64Const(0));
        emit(
            &mut body,
            &[
                I::I32Const(self.table as i32),
                I::LocalSet(2),
                I::Block(BlockType::Empty),
                I::Loop(BlockType::Empty),
                I::LocalGet(2),
                I::I32Const((self.table + TABLE_BYTES) as i32),
                I::I32Eq,
            ],
        );
        refuse_if(&mut body, I::I64Const(0));
        emit(
            &mut body,
            &[
                I::LocalGet(2),
                I::I32Load(mem(KIND, 2)),
                I::I32Eqz,
                I::BrIf(1),
                I::LocalGet(2),
                I::I32Const(ROW_BYTES),
                I::I32Add,
                I::LocalSet(2),
                I::Br(0),
                I::End,
                I::End,
            ],
        );
        Self::renew(&mut body, 2, 3);
        emit(
            &mut body,
            &[
                I::LocalGet(2),
                I::I32Const(VECTOR),
                I::I32Store(mem(KIND, 2)),
                I::LocalGet(2),
                I::I32Const(0),
                I::I32Store(mem(LENGTH, 2)),
                I::LocalGet(2),
                I::LocalGet(1),
                I::I32WrapI64,
                I::I32Store(mem(CAPACITY, 2)),
            ],
        );
        finish(code, body);

        // push(handle, tag, item) -> successor
        let mut body = Function::new_with_locals_types([ValType::I32, ValType::I32, ValType::I64]);
        self.authenticate(&mut body, 3, VECTOR, None);
        emit(&mut body, &[I::LocalGet(1), I::I32Const(9), I::I32Ne]);
        trap_if(&mut body);
        self.owned_bytes(&mut body, 2);
        emit(
            &mut body,
            &[
                I::LocalGet(3),
                I::I32Load(mem(LENGTH, 2)),
                I::LocalTee(4),
                I::LocalGet(3),
                I::I32Load(mem(CAPACITY, 2)),
                I::I32GeU,
            ],
        );
        refuse_if(&mut body, I::I64Const(0));
        Self::renew(&mut body, 3, 5);
        Self::item_address(&mut body, 3, 4);
        emit(
            &mut body,
            &[
                I::LocalGet(2),
                I::I64Store(mem(ITEMS, 3)),
                I::LocalGet(3),
                I::LocalGet(4),
                I::I32Const(1),
                I::I32Add,
                I::I32Store(mem(LENGTH, 2)),
            ],
        );
        finish(code, body);

        // len and capacity take a borrowed Vec.
        for offset in [LENGTH, CAPACITY] {
            let mut body = Function::new_with_locals_types([ValType::I32]);
            self.authenticate(&mut body, 2, VECTOR, None);
            emit(&mut body, &[I::LocalGet(1), I::I32Const(9), I::I32Ne]);
            trap_if(&mut body);
            emit(
                &mut body,
                &[I::LocalGet(2), I::I32Load(mem(offset, 2)), I::I64ExtendI32U],
            );
            finish(code, body);
        }
        if include_get_slot {
            // Vec<Bytes> get is never admitted, but occupies its canonical slot.
            let mut body = Function::new([]);
            body.instruction(&I::Unreachable);
            finish(code, body);
        }
        self.drop_suffix(code, VECTOR);

        // into_iter(handle, out) -> status
        let mut body = Function::new_with_locals_types([ValType::I32, ValType::I64]);
        self.authenticate(&mut body, 2, VECTOR, Some(I::I32Const(2)));
        Self::output(&mut body, 1, 16);
        emit(&mut body, &[I::LocalGet(1)]);
        Self::renew(&mut body, 2, 3);
        emit(
            &mut body,
            &[
                I::I64Store(mem(0, 3)),
                I::LocalGet(1),
                I::I64Const(0),
                I::I64Store(mem(8, 3)),
                I::LocalGet(2),
                I::I32Const(ITERATOR),
                I::I32Store(mem(KIND, 2)),
                I::LocalGet(2),
                I::I32Const(0),
                I::I32Store(mem(CURSOR, 2)),
                I::I32Const(0),
            ],
        );
        finish(code, body);

        // next(handle, cursor, out) -> status. Done publishes an all-zero frame.
        let mut body = Function::new_with_locals_types([
            ValType::I32,
            ValType::I32,
            ValType::I64,
            ValType::I64,
        ]);
        self.authenticate(&mut body, 3, ITERATOR, Some(I::I32Const(2)));
        Self::output(&mut body, 2, 32);
        Self::check_cursor(&mut body, 3, 4);
        refuse_if(&mut body, I::I32Const(2));
        emit(
            &mut body,
            &[
                I::LocalGet(4),
                I::LocalGet(3),
                I::I32Load(mem(LENGTH, 2)),
                I::I32Eq,
                I::If(BlockType::Empty),
                I::LocalGet(3),
                I::I32Const(0),
                I::I32Store(mem(KIND, 2)),
                I::LocalGet(2),
                I::I32Const(0),
                I::I32Const(32),
                I::MemoryFill(0),
                I::I32Const(0),
                I::Return,
                I::End,
            ],
        );
        Self::item_address(&mut body, 3, 4);
        emit(&mut body, &[I::I64Load(mem(ITEMS, 3)), I::LocalSet(5)]);
        self.owned_bytes(&mut body, 5);
        Self::renew(&mut body, 3, 6);
        emit(
            &mut body,
            &[
                I::Drop,
                I::LocalGet(3),
                I::LocalGet(4),
                I::I32Const(1),
                I::I32Add,
                I::LocalTee(4),
                I::I32Store(mem(CURSOR, 2)),
                I::LocalGet(2),
                I::I64Const(1),
                I::I64Store(mem(0, 3)),
                I::LocalGet(2),
                I::LocalGet(5),
                I::I64Store(mem(8, 3)),
                I::LocalGet(2),
                I::LocalGet(6),
                I::I64Store(mem(16, 3)),
                I::LocalGet(2),
                I::LocalGet(4),
                I::I64ExtendI32U,
                I::I64Store(mem(24, 3)),
                I::I32Const(0),
            ],
        );
        finish(code, body);
        self.drop_suffix(code, ITERATOR);
    }

    fn check_cursor(body: &mut Function, row: u32, cursor: u32) {
        emit(
            body,
            &[
                I::LocalGet(row),
                I::I32Load(mem(CURSOR, 2)),
                I::LocalTee(cursor),
                I::I64ExtendI32U,
                I::LocalGet(1),
                I::I64Ne,
            ],
        );
    }

    fn drop_suffix(self, code: &mut Vec<u8>, kind: i32) {
        let row = if kind == ITERATOR { 2 } else { 1 };
        let cursor = row + 1;
        let mut body = Function::new_with_locals_types([ValType::I32, ValType::I32]);
        self.authenticate(&mut body, row, kind, None);
        if kind == ITERATOR {
            Self::check_cursor(&mut body, row, cursor);
            trap_if(&mut body);
        }
        emit(
            &mut body,
            &[
                I::LocalGet(row),
                I::I32Const(0),
                I::I32Store(mem(KIND, 2)),
                I::Block(BlockType::Empty),
                I::Loop(BlockType::Empty),
                I::LocalGet(cursor),
                I::LocalGet(row),
                I::I32Load(mem(LENGTH, 2)),
                I::I32GeU,
                I::BrIf(1),
            ],
        );
        Self::item_address(&mut body, row, cursor);
        emit(
            &mut body,
            &[
                I::I64Load(mem(ITEMS, 3)),
                I::Call(self.drop_bytes),
                I::LocalGet(cursor),
                I::I32Const(1),
                I::I32Add,
                I::LocalSet(cursor),
                I::Br(0),
                I::End,
                I::End,
            ],
        );
        finish(code, body);
    }
}

#[cfg(test)]
mod tests;
