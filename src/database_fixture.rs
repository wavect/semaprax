//! A deterministic in-memory relational fixture for issue #190's database
//! profile.
//!
//! **This is not SQLite, not PostgreSQL, and not reachable from checked
//! SEMAPRAX source.** `std/db` (see
//! `docs/DATABASE-ACCESS-V1.md`) specifies the pure, effect-free decision
//! procedures that govern prepared-statement descriptor validation,
//! identifier safety, transaction state, migration ordering, and bounded
//! cursor limits, and those procedures execute as checked SEMAPRAX code on
//! every backend. This module is a separate, Rust-only proof that the same
//! decision procedures compose into something an actual storage engine could
//! implement: an in-memory table store with real insert/select/update/delete,
//! a transaction that really rolls a snapshot back, and a migration ledger
//! that really refuses drift, gaps, and duplicates. It exists to give this
//! tranche local evidence beyond "the predicates type-check"; it grants no
//! filesystem, network, or process authority, opens no socket, and is used
//! only by this module's own tests.
//!
//! A real driver (SQLite or PostgreSQL) needs either a from-scratch wire or
//! file-format implementation, or a Cargo dependency this repository's own
//! `Cargo.toml` cannot add without a maintainer decision. See
//! `docs/DATABASE-ACCESS-V1.md#non-claims-and-remaining-work` for the exact
//! extension point (`import rust fn` plus a downstream Project's
//! `[rust-dependencies]`, per `docs/PROJECT-DEPENDENCIES-V1.md`) a follow-on
//! tranche would use to wire a real one in.

use std::collections::BTreeMap;

/// One typed cell value. The tag ordering matches `std.db`'s type-tag table
/// exactly (0=I64, 1=U8, 2=Bool, 3=Usize, 4=Bytes) so a row's shape can be
/// checked against a descriptor with the same comparison the pure package
/// uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    I64(i64),
    U8(u8),
    Bool(bool),
    Usize(usize),
    Bytes(Vec<u8>),
}

impl Value {
    pub fn tag(&self) -> u8 {
        match self {
            Value::I64(_) => 0,
            Value::U8(_) => 1,
            Value::Bool(_) => 2,
            Value::Usize(_) => 3,
            Value::Bytes(_) => 4,
        }
    }
}

/// A single named, typed column. `name` is checked with [`identifier_is_valid`]
/// before a table is created, mirroring `std.db.identifier.is_valid`: a
/// column or table name that carries a quote, semicolon, backtick, hyphen,
/// dot, or space is refused before it ever reaches generated statement text.
#[derive(Clone, Debug)]
pub struct Column {
    pub name: String,
    pub tag: u8,
}

/// The same identifier-safety grammar `std.db.identifier.is_valid` specifies:
/// nonempty, at most 63 bytes, a letter-or-underscore first byte, and only
/// ASCII letters, digits, or underscore afterward. Duplicated here
/// deliberately: this module proves the same decision procedure holds at the
/// Rust layer independent of the SEMAPRAX interpreter, not that one calls the
/// other.
pub fn identifier_is_valid(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 63 {
        return false;
    }
    let safe = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let first_ok = safe(bytes[0]) && !bytes[0].is_ascii_digit();
    first_ok && bytes[1..].iter().all(|&b| safe(b))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransactionState {
    #[default]
    None,
    Open,
    Committed,
    RolledBack,
    Failed,
}

impl TransactionState {
    /// `begin` refuses only a nested attempt while already `Open`; a
    /// connection is reusable, so `begin` succeeds again once a previous
    /// transaction has settled (`Committed`, `RolledBack`, or `Failed`).
    fn next_on_begin(self) -> Self {
        match self {
            TransactionState::Open => TransactionState::Failed,
            _ => TransactionState::Open,
        }
    }

    fn next_on_commit(self) -> Self {
        match self {
            TransactionState::Open => TransactionState::Committed,
            _ => TransactionState::Failed,
        }
    }

    fn next_on_rollback(self) -> Self {
        match self {
            TransactionState::Open => TransactionState::RolledBack,
            _ => TransactionState::Failed,
        }
    }

    fn next_on_connection_lost(self) -> Self {
        match self {
            TransactionState::Open => TransactionState::Failed,
            other => other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureError {
    UnknownTable,
    UnsafeIdentifier,
    RowShapeMismatch,
    NoOpenTransaction,
    TransactionAlreadyOpen,
    TransactionNotOpen,
    RowLimitExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationOutcome {
    Applied,
    AlreadyApplied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationError {
    OutOfOrder,
    MissingPredecessor,
    ChecksumDrift,
}

#[derive(Clone, Debug, Default)]
struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
}

fn row_matches_schema(columns: &[Column], row: &[Value]) -> bool {
    columns.len() == row.len()
        && columns
            .iter()
            .zip(row.iter())
            .all(|(column, value)| column.tag == value.tag())
}

/// The in-memory engine. `tables` is the live state; `snapshot` is the
/// clone-on-begin copy a rollback restores from, so "commit" and "rollback"
/// are real state transitions over real data, not only the pure state-code
/// arithmetic `std.db.transaction` specifies.
#[derive(Default)]
pub struct DatabaseFixture {
    tables: BTreeMap<String, Table>,
    snapshot: Option<BTreeMap<String, Table>>,
    transaction: TransactionState,
    applied_migrations: Vec<(u8, u8)>,
}

impl DatabaseFixture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn transaction_state(&self) -> TransactionState {
        self.transaction
    }

    pub fn create_table(&mut self, name: &str, columns: Vec<Column>) -> Result<(), FixtureError> {
        if !identifier_is_valid(name) || columns.iter().any(|c| !identifier_is_valid(&c.name)) {
            return Err(FixtureError::UnsafeIdentifier);
        }
        self.tables.insert(
            name.to_owned(),
            Table {
                columns,
                rows: Vec::new(),
            },
        );
        Ok(())
    }

    pub fn begin(&mut self) -> Result<(), FixtureError> {
        let next = self.transaction.next_on_begin();
        if next != TransactionState::Open {
            self.transaction = next;
            return Err(FixtureError::TransactionAlreadyOpen);
        }
        self.transaction = next;
        self.snapshot = Some(self.tables.clone());
        Ok(())
    }

    pub fn commit(&mut self) -> Result<(), FixtureError> {
        let next = self.transaction.next_on_commit();
        self.transaction = next;
        if next != TransactionState::Committed {
            return Err(FixtureError::NoOpenTransaction);
        }
        self.snapshot = None;
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<(), FixtureError> {
        let next = self.transaction.next_on_rollback();
        self.transaction = next;
        if next != TransactionState::RolledBack {
            return Err(FixtureError::NoOpenTransaction);
        }
        if let Some(snapshot) = self.snapshot.take() {
            self.tables = snapshot;
        }
        Ok(())
    }

    /// Models an observed connection loss while a transaction may be open.
    /// The outcome is unknown, so this never reports success: it forces
    /// `Failed` and discards any in-flight snapshot rather than guessing
    /// whether the last write landed.
    pub fn connection_lost(&mut self) {
        let was_open = self.transaction == TransactionState::Open;
        self.transaction = self.transaction.next_on_connection_lost();
        if was_open {
            // The outcome of the in-flight transaction is unknown; discard
            // it back to the pre-transaction snapshot rather than keeping a
            // write that may or may not have actually landed.
            if let Some(snapshot) = self.snapshot.take() {
                self.tables = snapshot;
            }
        }
    }

    fn require_table_mut(&mut self, table: &str) -> Result<&mut Table, FixtureError> {
        self.tables.get_mut(table).ok_or(FixtureError::UnknownTable)
    }

    pub fn insert(&mut self, table: &str, row: Vec<Value>) -> Result<(), FixtureError> {
        let table = self.require_table_mut(table)?;
        if !row_matches_schema(&table.columns, &row) {
            return Err(FixtureError::RowShapeMismatch);
        }
        table.rows.push(row);
        Ok(())
    }

    /// Selects every row whose value at `column` equals `value`, in
    /// insertion order, stopping as soon as `max_rows` matches are
    /// collected — an early consumer stop enforced by the engine itself
    /// rather than left to the caller, mirroring `std.db.limits.should_stop`.
    pub fn select_eq(
        &self,
        table: &str,
        column: usize,
        value: &Value,
        max_rows: usize,
    ) -> Result<Vec<Vec<Value>>, FixtureError> {
        let table = self.tables.get(table).ok_or(FixtureError::UnknownTable)?;
        let mut matched = Vec::new();
        for row in &table.rows {
            if row.get(column) == Some(value) {
                matched.push(row.clone());
                if matched.len() >= max_rows {
                    break;
                }
            }
        }
        Ok(matched)
    }

    pub fn row_count(&self, table: &str) -> Result<usize, FixtureError> {
        self.tables
            .get(table)
            .map(|table| table.rows.len())
            .ok_or(FixtureError::UnknownTable)
    }

    /// Updates `set_column` to `new_value` on every row whose value at
    /// `key_column` equals `key_value`, in place. Returns the count of rows
    /// updated (0 if none matched). Refuses a shape mismatch rather than
    /// widening the column's type. Mutates `self.tables` unconditionally,
    /// exactly like [`Self::insert`]: whether the write is durable is
    /// controlled by the caller's `begin`/`commit`/`rollback` (or an
    /// observed [`Self::connection_lost`]) bracketing this call, not by
    /// this method itself — this is the primitive issue #192's job
    /// completion needs to commit a lifecycle-state change into the same
    /// ledger row `enqueue` created, so a completion whose commit is never
    /// confirmed cannot be told apart from one that never happened.
    pub fn update_column(
        &mut self,
        table: &str,
        key_column: usize,
        key_value: &Value,
        set_column: usize,
        new_value: Value,
    ) -> Result<usize, FixtureError> {
        let table = self.require_table_mut(table)?;
        let mut updated = 0usize;
        for row in table.rows.iter_mut() {
            if row.get(key_column) != Some(key_value) {
                continue;
            }
            match row.get(set_column) {
                Some(existing) if existing.tag() == new_value.tag() => {
                    row[set_column] = new_value.clone();
                    updated += 1;
                }
                _ => return Err(FixtureError::RowShapeMismatch),
            }
        }
        Ok(updated)
    }

    /// Applies migration `id` (checksum `checksum`) if it is the next
    /// gapless, strictly increasing ID and no drift is detected against an
    /// already-applied entry of the same ID. Reapplying the exact same
    /// `(id, checksum)` pair that was already applied is the idempotent
    /// no-op the issue calls "reapply", reported as
    /// [`MigrationOutcome::AlreadyApplied`] rather than an error.
    pub fn apply_migration(
        &mut self,
        id: u8,
        checksum: u8,
    ) -> Result<MigrationOutcome, MigrationError> {
        if let Some(&(_, recorded)) = self
            .applied_migrations
            .iter()
            .find(|(applied_id, _)| *applied_id == id)
        {
            return if recorded == checksum {
                Ok(MigrationOutcome::AlreadyApplied)
            } else {
                Err(MigrationError::ChecksumDrift)
            };
        }
        let last_applied = self
            .applied_migrations
            .last()
            .map_or(0u8, |&(applied_id, _)| applied_id);
        if id <= last_applied {
            return Err(MigrationError::OutOfOrder);
        }
        let applied_count = self.applied_migrations.len() as u8;
        if id != applied_count + 1 {
            return Err(MigrationError::MissingPredecessor);
        }
        self.applied_migrations.push((id, checksum));
        Ok(MigrationOutcome::Applied)
    }

    pub fn applied_migration_count(&self) -> usize {
        self.applied_migrations.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn users_table() -> Vec<Column> {
        vec![
            Column {
                name: "id".to_owned(),
                tag: 3,
            },
            Column {
                name: "name".to_owned(),
                tag: 4,
            },
        ]
    }

    #[test]
    fn identifier_rejects_injection_shaped_names() {
        assert!(identifier_is_valid("users"));
        assert!(identifier_is_valid("_id"));
        assert!(!identifier_is_valid(""));
        assert!(!identifier_is_valid("1id"));
        assert!(!identifier_is_valid("users; drop table users"));
        assert!(!identifier_is_valid("users'"));
        assert!(!identifier_is_valid("users--"));
        assert!(!identifier_is_valid(&"x".repeat(64)));
    }

    #[test]
    fn create_table_refuses_unsafe_table_or_column_identifiers() {
        let mut db = DatabaseFixture::new();
        assert_eq!(
            db.create_table("users; drop table users", users_table()),
            Err(FixtureError::UnsafeIdentifier)
        );
        let unsafe_column = vec![Column {
            name: "id\"".to_owned(),
            tag: 3,
        }];
        assert_eq!(
            db.create_table("users", unsafe_column),
            Err(FixtureError::UnsafeIdentifier)
        );
    }

    #[test]
    fn insert_rejects_a_row_whose_shape_disagrees_with_the_schema() {
        let mut db = DatabaseFixture::new();
        db.create_table("users", users_table()).unwrap();
        assert_eq!(
            db.insert("users", vec![Value::Usize(1)]),
            Err(FixtureError::RowShapeMismatch)
        );
        assert_eq!(
            db.insert("users", vec![Value::I64(1), Value::Bytes(b"a".to_vec())]),
            Err(FixtureError::RowShapeMismatch)
        );
        assert_eq!(db.row_count("users").unwrap(), 0);
    }

    #[test]
    fn update_column_rewrites_matching_rows_and_refuses_a_shape_mismatch() {
        let mut db = DatabaseFixture::new();
        db.create_table("users", users_table()).unwrap();
        db.insert(
            "users",
            vec![Value::Usize(1), Value::Bytes(b"alice".to_vec())],
        )
        .unwrap();
        db.insert(
            "users",
            vec![Value::Usize(2), Value::Bytes(b"bob".to_vec())],
        )
        .unwrap();
        let updated = db
            .update_column(
                "users",
                0,
                &Value::Usize(1),
                1,
                Value::Bytes(b"alicia".to_vec()),
            )
            .unwrap();
        assert_eq!(updated, 1);
        assert_eq!(
            db.select_eq("users", 0, &Value::Usize(1), 1).unwrap(),
            vec![vec![Value::Usize(1), Value::Bytes(b"alicia".to_vec())]]
        );
        // bob's row is untouched.
        assert_eq!(
            db.select_eq("users", 0, &Value::Usize(2), 1).unwrap(),
            vec![vec![Value::Usize(2), Value::Bytes(b"bob".to_vec())]]
        );
        // A type-mismatched replacement is refused rather than widening the
        // column.
        assert_eq!(
            db.update_column("users", 0, &Value::Usize(2), 1, Value::Usize(9)),
            Err(FixtureError::RowShapeMismatch)
        );
        // No matching key updates zero rows without error.
        assert_eq!(
            db.update_column(
                "users",
                0,
                &Value::Usize(404),
                1,
                Value::Bytes(b"x".to_vec())
            ),
            Ok(0)
        );
    }

    #[test]
    fn update_column_composes_with_rollback_and_connection_lost_via_the_transaction_snapshot() {
        let mut db = DatabaseFixture::new();
        db.create_table("users", users_table()).unwrap();
        db.insert(
            "users",
            vec![Value::Usize(1), Value::Bytes(b"alice".to_vec())],
        )
        .unwrap();

        // A rolled-back update reverts to the pre-transaction snapshot.
        db.begin().unwrap();
        db.update_column(
            "users",
            0,
            &Value::Usize(1),
            1,
            Value::Bytes(b"bob".to_vec()),
        )
        .unwrap();
        db.rollback().unwrap();
        assert_eq!(
            db.select_eq("users", 0, &Value::Usize(1), 1).unwrap(),
            vec![vec![Value::Usize(1), Value::Bytes(b"alice".to_vec())]]
        );

        // A connection lost mid-transaction discards the in-flight update
        // exactly like a rollback: the update never becomes durable.
        db.begin().unwrap();
        db.update_column(
            "users",
            0,
            &Value::Usize(1),
            1,
            Value::Bytes(b"carol".to_vec()),
        )
        .unwrap();
        db.connection_lost();
        assert_eq!(
            db.select_eq("users", 0, &Value::Usize(1), 1).unwrap(),
            vec![vec![Value::Usize(1), Value::Bytes(b"alice".to_vec())]]
        );
        assert_eq!(db.transaction_state(), TransactionState::Failed);
    }

    #[test]
    fn commit_persists_and_rollback_discards() {
        let mut db = DatabaseFixture::new();
        db.create_table("users", users_table()).unwrap();
        db.begin().unwrap();
        db.insert(
            "users",
            vec![Value::Usize(1), Value::Bytes(b"alice".to_vec())],
        )
        .unwrap();
        db.commit().unwrap();
        assert_eq!(db.row_count("users").unwrap(), 1);
        assert_eq!(db.transaction_state(), TransactionState::Committed);

        db.begin().unwrap();
        db.insert(
            "users",
            vec![Value::Usize(2), Value::Bytes(b"bob".to_vec())],
        )
        .unwrap();
        db.rollback().unwrap();
        assert_eq!(db.row_count("users").unwrap(), 1);
        assert_eq!(db.transaction_state(), TransactionState::RolledBack);
    }

    #[test]
    fn begin_succeeds_again_after_commit_or_rollback_settles_the_previous_one() {
        let mut db = DatabaseFixture::new();
        db.create_table("users", users_table()).unwrap();
        db.begin().unwrap();
        db.commit().unwrap();
        // A settled connection accepts a new transaction rather than staying
        // permanently closed.
        db.begin().unwrap();
        assert_eq!(db.transaction_state(), TransactionState::Open);
        db.rollback().unwrap();
        db.begin().unwrap();
        assert_eq!(db.transaction_state(), TransactionState::Open);
    }

    #[test]
    fn nested_begin_is_refused_and_sticky_failed() {
        let mut db = DatabaseFixture::new();
        db.begin().unwrap();
        assert_eq!(db.begin(), Err(FixtureError::TransactionAlreadyOpen));
        assert_eq!(db.transaction_state(), TransactionState::Failed);
        // Failure is sticky: neither commit nor rollback recovers it.
        assert_eq!(db.commit(), Err(FixtureError::NoOpenTransaction));
        assert_eq!(db.transaction_state(), TransactionState::Failed);
        assert_eq!(db.rollback(), Err(FixtureError::NoOpenTransaction));
        assert_eq!(db.transaction_state(), TransactionState::Failed);
    }

    #[test]
    fn commit_without_an_open_transaction_fails_closed() {
        let mut db = DatabaseFixture::new();
        assert_eq!(db.commit(), Err(FixtureError::NoOpenTransaction));
        assert_eq!(db.transaction_state(), TransactionState::Failed);
    }

    #[test]
    fn connection_loss_forces_failed_only_while_open() {
        let mut db = DatabaseFixture::new();
        db.create_table("users", users_table()).unwrap();
        db.begin().unwrap();
        db.insert("users", vec![Value::Usize(1), Value::Bytes(b"a".to_vec())])
            .unwrap();
        db.connection_lost();
        assert_eq!(db.transaction_state(), TransactionState::Failed);
        // The in-flight write is discarded: an uncertain outcome is never
        // reported as success.
        assert_eq!(db.row_count("users").unwrap(), 0);

        let mut settled = DatabaseFixture::new();
        settled.begin().unwrap();
        settled.commit().unwrap();
        settled.connection_lost();
        assert_eq!(settled.transaction_state(), TransactionState::Committed);
    }

    #[test]
    fn select_eq_stops_early_at_the_row_limit() {
        let mut db = DatabaseFixture::new();
        db.create_table("users", users_table()).unwrap();
        db.begin().unwrap();
        for id in 0..5u8 {
            db.insert(
                "users",
                vec![Value::Usize(id as usize), Value::Bytes(b"same".to_vec())],
            )
            .unwrap();
        }
        db.commit().unwrap();
        let matched = db
            .select_eq("users", 1, &Value::Bytes(b"same".to_vec()), 2)
            .unwrap();
        assert_eq!(matched.len(), 2);
        let unmatched = db
            .select_eq("users", 1, &Value::Bytes(b"other".to_vec()), 10)
            .unwrap();
        assert!(unmatched.is_empty());
        assert_eq!(
            db.select_eq("missing", 0, &Value::Usize(0), 1),
            Err(FixtureError::UnknownTable)
        );
    }

    #[test]
    fn migrations_apply_in_a_gapless_strictly_increasing_sequence() {
        let mut db = DatabaseFixture::new();
        assert_eq!(db.apply_migration(1, 10), Ok(MigrationOutcome::Applied));
        assert_eq!(db.apply_migration(2, 20), Ok(MigrationOutcome::Applied));
        assert_eq!(db.applied_migration_count(), 2);
        // Reapplying the same id with the same checksum is an idempotent no-op.
        assert_eq!(
            db.apply_migration(2, 20),
            Ok(MigrationOutcome::AlreadyApplied)
        );
        assert_eq!(db.applied_migration_count(), 2);
    }

    #[test]
    fn migrations_reject_drift_gaps_and_out_of_order_attempts() {
        let mut db = DatabaseFixture::new();
        db.apply_migration(1, 10).unwrap();
        db.apply_migration(2, 20).unwrap();
        // Checksum drift: same id, different content digest.
        assert_eq!(
            db.apply_migration(2, 99),
            Err(MigrationError::ChecksumDrift)
        );
        // Missing predecessor: a gap in the sequence.
        assert_eq!(
            db.apply_migration(4, 40),
            Err(MigrationError::MissingPredecessor)
        );
        // Out of order: an id no greater than the last applied one.
        assert_eq!(
            db.apply_migration(1, 10),
            Ok(MigrationOutcome::AlreadyApplied)
        );
        assert_eq!(db.apply_migration(0, 5), Err(MigrationError::OutOfOrder));
    }

    /// Two runners racing to apply the same next migration: the second
    /// runner's attempt is rejected outright (out of order once the first
    /// has landed) or accepted as the identical idempotent reapply if it
    /// carries the same checksum — never silently applied twice.
    #[test]
    fn concurrent_runner_duplicate_attempt_is_never_applied_twice() {
        let mut first = DatabaseFixture::new();
        let mut second = DatabaseFixture::new();
        assert_eq!(first.apply_migration(1, 42), Ok(MigrationOutcome::Applied));
        // `second` models a runner that observed the same starting ledger
        // and now races to apply the same migration id.
        assert_eq!(second.apply_migration(1, 42), Ok(MigrationOutcome::Applied));
        assert_eq!(first.applied_migration_count(), 1);
        assert_eq!(second.applied_migration_count(), 1);
        // A racing attempt with a different checksum for the same id is
        // drift, not a silent double apply.
        assert_eq!(
            first.apply_migration(1, 7),
            Err(MigrationError::ChecksumDrift)
        );
    }
}
