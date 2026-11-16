//! Bougette's local storage: every change is a timestamped message in a
//! SQLite log, and the current state is rebuilt from those messages.
//!
//! Two tables:
//! - `messages`: the log, one row per changed field, never modified. In
//!   M1 these are what devices exchange to sync.
//! - `fields`: the latest value of each field, i.e. the message with the
//!   greatest timestamp for that field ("the latest change wins").
//!
//! Saving a budget compares it field by field with what is stored, and
//! writes a message for each difference.

mod clock;
mod codec;

use std::path::Path;

use bougette_core::budget::Budget;
use bougette_core::id::BudgetId;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::clock::Clock;
// `pub use` re-exports: users of this crate see `bougette_store::Corrupt`.
pub use crate::codec::Corrupt;
use crate::codec::{DELETED, Key, Records};

/// Why a storage operation failed.
#[derive(Debug)]
pub enum StoreError {
    /// SQLite reported an error.
    Sqlite(rusqlite::Error),
    /// The stored fields don't make a valid budget.
    Corrupt(Corrupt),
}

// Implementing `From` lets the `?` operator convert a rusqlite error into
// a StoreError on the way out of a function, like an implicit converting
// constructor, but only where `?` is used.
impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Sqlite(error)
    }
}

impl From<Corrupt> for StoreError {
    fn from(error: Corrupt) -> Self {
        StoreError::Corrupt(error)
    }
}

/// The storage of one device.
pub struct Store {
    connection: Connection,
    clock: Clock,
}

/// The database schema. `IF NOT EXISTS` makes opening an existing file
/// harmless.
const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS messages (
        timestamp TEXT PRIMARY KEY,
        budget TEXT NOT NULL,
        entity TEXT NOT NULL,
        id TEXT NOT NULL,
        field TEXT NOT NULL,
        value TEXT
    );
    CREATE TABLE IF NOT EXISTS fields (
        budget TEXT NOT NULL,
        entity TEXT NOT NULL,
        id TEXT NOT NULL,
        field TEXT NOT NULL,
        value TEXT,
        timestamp TEXT NOT NULL,
        PRIMARY KEY (budget, entity, id, field)
    );
";

impl Store {
    /// Opens the database file at `path`, creating it if needed.
    // `&Path` is a borrowed file path, like `const std::filesystem::path&`.
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        Store::with_connection(Connection::open(path)?)
    }

    /// A database in memory, gone when the Store is dropped: for tests.
    pub fn open_in_memory() -> Result<Store, StoreError> {
        Store::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(connection: Connection) -> Result<Store, StoreError> {
        connection.execute_batch(SCHEMA)?;
        // This device's identifier: read it, or draw and keep one. The
        // last 16 hex digits of a version 7 UUID are random.
        let node = match Store::meta(&connection, "node")? {
            Some(node) => node,
            None => {
                let node = Uuid::now_v7().simple().to_string()[16..].to_string();
                connection.execute(
                    "INSERT INTO meta (key, value) VALUES ('node', ?1)",
                    params![node],
                )?;
                node
            }
        };
        // Continue the clock after the latest message written.
        let last: Option<String> =
            connection.query_row("SELECT MAX(timestamp) FROM messages", [], |row| row.get(0))?;
        let clock = Clock::new(node, last.as_deref());
        Ok(Store { connection, clock })
    }

    /// A value of the `meta` table, if set.
    fn meta(connection: &Connection, key: &str) -> Result<Option<String>, StoreError> {
        // `query_row` fails when there is no row; `.optional()` turns that
        // case into `Ok(None)`.
        Ok(connection
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Saves `budget`: one message per field that differs from what is
    /// stored. Entities missing from `budget` are marked deleted.
    pub fn save(&mut self, budget: &Budget) -> Result<(), StoreError> {
        let budget_id = budget.id.to_string();
        let old = self.records(&budget_id)?;
        let new = codec::encode(budget);

        // The changes, in key order: new or different fields…
        let mut changes: Vec<(Key, Option<String>)> = Vec::new();
        for (key, value) in &new {
            if old.get(key) != Some(value) {
                changes.push((key.clone(), value.clone()));
            }
        }
        // …and a deletion mark for each entity that is gone.
        for (entity, id, field) in old.keys() {
            let gone = !new.contains_key(&(entity.clone(), id.clone(), field.clone()));
            let marked = old.get(&(entity.clone(), id.clone(), DELETED.to_string()))
                == Some(&Some("true".to_string()));
            if field == DELETED && gone && !marked {
                changes.push((
                    (entity.clone(), id.clone(), DELETED.to_string()),
                    Some("true".to_string()),
                ));
            }
        }

        // All or nothing: a SQLite transaction, committed at the end. If
        // anything fails before, `transaction` is dropped and rolls back,
        // like an RAII guard.
        let transaction = self.connection.transaction()?;
        for ((entity, id, field), value) in changes {
            let timestamp = self.clock.tick();
            apply(
                &transaction,
                &timestamp,
                &budget_id,
                &entity,
                &id,
                &field,
                value.as_deref(),
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// The budget `id` as currently stored, `None` if unknown or deleted.
    pub fn load(&self, id: BudgetId) -> Result<Option<Budget>, StoreError> {
        Ok(codec::decode(&self.records(&id.to_string())?)?)
    }

    /// The stored budgets that aren't deleted: identifier and name.
    pub fn budgets(&self) -> Result<Vec<(BudgetId, String)>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, value FROM fields
             WHERE entity = 'budget' AND field = 'name' AND id NOT IN (
                 SELECT id FROM fields
                 WHERE entity = 'budget' AND field = 'deleted' AND value = 'true')
             ORDER BY value",
        )?;
        // `query_map` runs the query and maps each row; the closure's `?`
        // propagates a failure to read a column.
        let rows = statement.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok((id, name))
        })?;
        let mut budgets = Vec::new();
        for row in rows {
            let (id, name) = row?;
            let uuid = Uuid::parse_str(&id)
                .map_err(|_| Corrupt(format!("invalid budget identifier {id:?}")))?;
            budgets.push((BudgetId(uuid), name));
        }
        Ok(budgets)
    }

    /// Every stored field of budget `budget`.
    fn records(&self, budget: &str) -> Result<Records, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT entity, id, field, value FROM fields WHERE budget = ?1")?;
        let rows = statement.query_map(params![budget], |row| {
            Ok(((row.get(0)?, row.get(1)?, row.get(2)?), row.get(3)?))
        })?;
        let mut records = Records::new();
        for row in rows {
            let (key, value) = row?;
            records.insert(key, value);
        }
        Ok(records)
    }
}

/// Records one message in the log and, if it is the latest change of its
/// field, makes it the field's current value. Applying the same message
/// twice, or messages in any order, gives the same result: that's what
/// will let devices exchange messages in M1.
// Clippy warns about functions with many parameters; these are the
// columns of one message.
#[allow(clippy::too_many_arguments)]
fn apply(
    connection: &Connection,
    timestamp: &str,
    budget: &str,
    entity: &str,
    id: &str,
    field: &str,
    value: Option<&str>,
) -> Result<(), StoreError> {
    connection.execute(
        "INSERT OR IGNORE INTO messages (timestamp, budget, entity, id, field, value)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![timestamp, budget, entity, id, field, value],
    )?;
    // Insert the field, or update it only if this message is newer.
    connection.execute(
        "INSERT INTO fields (budget, entity, id, field, value, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (budget, entity, id, field) DO UPDATE
         SET value = excluded.value, timestamp = excluded.timestamp
         WHERE excluded.timestamp > fields.timestamp",
        params![budget, entity, id, field, value, timestamp],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bougette_core::line::{Kind, Line};
    use bougette_core::money::{Currency, Money};
    use bougette_core::pot::{Goal, Pot};
    use bougette_core::schedule::Schedule;
    use bougette_core::transaction::{PotMove, Transaction};
    use jiff::civil::date;
    use std::num::NonZeroU8;

    fn eur(cents: i64) -> Money {
        Money::new(cents, Currency::EUR)
    }

    /// A budget using every kind of field: a pot with a goal, a line with
    /// two revisions and an end date, an allocation, transactions.
    fn sample() -> Budget {
        let mut budget = Budget::new("Household", Currency::EUR);
        let mut bike = Pot::new("Bike");
        bike.goal = Some(Goal {
            amount: eur(120000),
            by: Some(date(2026, 6, 30)),
        });
        let bike = budget.add_pot(bike);

        let monthly = |first| Schedule::Months {
            every: NonZeroU8::MIN,
            first,
        };
        let mut rent = Line::new(
            "Rent",
            "Housing",
            Kind::Cost,
            date(2026, 1, 1),
            eur(90000),
            monthly(date(2026, 1, 5)),
        )
        .unwrap();
        rent.change_from(date(2026, 7, 1), eur(95000), monthly(date(2026, 7, 5)))
            .unwrap();
        rent.end_on(Some(date(2027, 6, 30))).unwrap();
        let rent = budget.add_line(rent).unwrap();

        let mut savings = Line::new(
            "Bike savings",
            "Savings",
            Kind::Cost,
            date(2026, 1, 1),
            eur(10000),
            Schedule::Weekly {
                first: date(2026, 1, 2),
            },
        )
        .unwrap();
        savings.pot = Some(bike);
        budget.add_line(savings).unwrap();

        let mut paid = Transaction::new(
            date(2026, 1, 5),
            Kind::Cost,
            eur(90000),
            "Housing",
            Some(rent),
        );
        paid.note = "January".to_string();
        budget.add_transaction(paid).unwrap();
        let mut purchase =
            Transaction::new(date(2026, 5, 2), Kind::Cost, eur(-500), "Transport", None);
        purchase.pot = Some(PotMove::PaidFrom(bike));
        budget.add_transaction(purchase).unwrap();
        budget
    }

    fn message_count(store: &Store) -> i64 {
        store
            .connection
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn a_saved_budget_loads_back_identical() {
        let mut store = Store::open_in_memory().unwrap();
        let budget = sample();
        store.save(&budget).unwrap();
        assert_eq!(store.load(budget.id).unwrap(), Some(budget.clone()));
        assert_eq!(
            store.budgets().unwrap(),
            vec![(budget.id, "Household".to_string())]
        );
    }

    #[test]
    fn saving_again_writes_only_what_changed() {
        let mut store = Store::open_in_memory().unwrap();
        let mut budget = sample();
        store.save(&budget).unwrap();
        let before = message_count(&store);

        // Saving the same budget writes nothing.
        store.save(&budget).unwrap();
        assert_eq!(message_count(&store), before);

        // Renaming it writes exactly one message.
        budget.name = "Home".to_string();
        store.save(&budget).unwrap();
        assert_eq!(message_count(&store), before + 1);
        assert_eq!(store.load(budget.id).unwrap(), Some(budget));
    }

    #[test]
    fn a_revision_replaced_by_an_earlier_change_is_deleted() {
        let mut store = Store::open_in_memory().unwrap();
        let mut budget = sample();
        store.save(&budget).unwrap();
        // A change from April replaces the July revision of the rent.
        let rent_id = budget.lines()[0].id;
        budget
            .change_line(
                rent_id,
                date(2026, 4, 1),
                eur(92000),
                Schedule::Months {
                    every: NonZeroU8::MIN,
                    first: date(2026, 4, 5),
                },
            )
            .unwrap();
        store.save(&budget).unwrap();
        let loaded = store.load(budget.id).unwrap().unwrap();
        let rent = loaded.line(rent_id).unwrap();
        assert_eq!(rent.revisions().len(), 2);
        assert_eq!(rent.revisions()[1].from, date(2026, 4, 1));
    }

    #[test]
    fn survives_closing_and_reopening_the_file() {
        // A file in the system's temporary directory, removed at the end.
        let path = std::env::temp_dir().join(format!("bougette-test-{}.sqlite", Uuid::now_v7()));
        let budget = sample();
        {
            // An inner block: `store` is dropped (the file closed) at `}`,
            // like a C++ destructor at the end of a scope.
            let mut store = Store::open(&path).unwrap();
            store.save(&budget).unwrap();
        }
        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.load(budget.id).unwrap(), Some(budget.clone()));
        // The clock goes on after the stored messages: a new change sorts
        // after all of them.
        let last: String = store
            .connection
            .query_row("SELECT MAX(timestamp) FROM messages", [], |row| row.get(0))
            .unwrap();
        assert!(store.clock.tick() > last);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn the_latest_change_of_a_field_wins_whatever_the_order() {
        let store = Store::open_in_memory().unwrap();
        let connection = &store.connection;
        // Two messages for the same field, applied newest first.
        apply(connection, "2", "b", "budget", "b", "name", Some("New")).unwrap();
        apply(connection, "1", "b", "budget", "b", "name", Some("Old")).unwrap();
        let name: String = connection
            .query_row("SELECT value FROM fields WHERE field = 'name'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(name, "New");
    }
}
