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
//!
//! To sync, two stores compare their version vectors (`versions`): for
//! each device, the latest of its messages they hold. A device's own
//! messages have increasing timestamps and are never changed, and a store
//! always receives a device's messages in order and all at once, so each
//! store holds a *prefix* of every device's messages. What a store lacks
//! is therefore exactly each device's messages after its entry
//! (`missing_for`).

mod clock;
mod codec;

use std::collections::BTreeMap;
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

/// One change of one field, as devices exchange them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// When and on which device it was made (see `clock`). Unique.
    pub timestamp: String,
    pub budget: String,
    pub entity: String,
    pub id: String,
    pub field: String,
    /// The new value; `None` for an empty optional field.
    pub value: Option<String>,
}

/// A version vector: for each device, the timestamp of its latest message
/// held here.
pub type Versions = BTreeMap<String, String>;

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

    /// A setting of this device, if set. Settings stay on the device: they
    /// are never synced.
    pub fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        Store::meta(&self.connection, key)
    }

    /// Sets a setting of this device.
    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
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
            let message = Message {
                timestamp: self.clock.tick(),
                budget: budget_id.clone(),
                entity,
                id,
                field,
                value,
            };
            apply(&transaction, &message)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// This store's version vector.
    pub fn versions(&self) -> Result<Versions, StoreError> {
        // The latest timestamp of each device: the device is what follows
        // the 22 characters of time and counter (SQLite counts from 1).
        let mut statement = self
            .connection
            .prepare("SELECT MAX(timestamp) FROM messages GROUP BY substr(timestamp, 23)")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut versions = Versions::new();
        for row in rows {
            let timestamp = row?;
            let node = clock::node_of(&timestamp)
                .ok_or_else(|| Corrupt(format!("invalid timestamp {timestamp:?}")))?;
            versions.insert(node.to_string(), timestamp);
        }
        Ok(versions)
    }

    /// The messages held here that a store with version vector `theirs`
    /// lacks, oldest first.
    pub fn missing_for(&self, theirs: &Versions) -> Result<Vec<Message>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT timestamp, budget, entity, id, field, value
             FROM messages ORDER BY timestamp",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Message {
                timestamp: row.get(0)?,
                budget: row.get(1)?,
                entity: row.get(2)?,
                id: row.get(3)?,
                field: row.get(4)?,
                value: row.get(5)?,
            })
        })?;
        let mut missing = Vec::new();
        for row in rows {
            let message = row?;
            // Missing if they have nothing from its device, or only older
            // messages. `is_none_or` is true for `None`, else the test.
            let node = clock::node_of(&message.timestamp).unwrap_or_default();
            if theirs
                .get(node)
                .is_none_or(|latest| message.timestamp > *latest)
            {
                missing.push(message);
            }
        }
        Ok(missing)
    }

    /// Records messages from another device, all or nothing, and returns
    /// how many were new. Fails on a malformed timestamp.
    pub fn receive(&mut self, messages: &[Message]) -> Result<usize, StoreError> {
        let transaction = self.connection.transaction()?;
        let mut new = 0;
        for message in messages {
            if clock::parse(&message.timestamp).is_none() {
                return Err(Corrupt(format!("invalid timestamp {:?}", message.timestamp)).into());
            }
            if apply(&transaction, message)? {
                new += 1;
            }
        }
        transaction.commit()?;
        // Only once committed: our next changes must sort after theirs.
        for message in messages {
            self.clock.observe(&message.timestamp);
        }
        Ok(new)
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
/// field, makes it the field's current value; returns whether the message
/// was new. Applying the same message twice, or messages in any order,
/// gives the same result: that's what lets devices exchange them freely.
fn apply(connection: &Connection, message: &Message) -> Result<bool, StoreError> {
    // Destructure the message into its fields, borrowed.
    let Message {
        timestamp,
        budget,
        entity,
        id,
        field,
        value,
    } = message;
    // `execute` returns the number of rows changed: 0 if already there.
    let inserted = connection.execute(
        "INSERT OR IGNORE INTO messages (timestamp, budget, entity, id, field, value)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![timestamp, budget, entity, id, field, value],
    )?;
    if inserted == 0 {
        return Ok(false);
    }
    // Insert the field, or update it only if this message is newer.
    connection.execute(
        "INSERT INTO fields (budget, entity, id, field, value, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (budget, entity, id, field) DO UPDATE
         SET value = excluded.value, timestamp = excluded.timestamp
         WHERE excluded.timestamp > fields.timestamp",
        params![budget, entity, id, field, value, timestamp],
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bougette_core::id::MemberId;
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
        budget.personal = true;
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
        paid.payer = Some(MemberId("ab".repeat(32)));
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
        let rename = |timestamp: &str, name: &str| Message {
            timestamp: timestamp.to_string(),
            budget: "b".to_string(),
            entity: "budget".to_string(),
            id: "b".to_string(),
            field: "name".to_string(),
            value: Some(name.to_string()),
        };
        // Two messages for the same field, applied newest first, and the
        // newest one twice.
        assert!(apply(connection, &rename("2", "New")).unwrap());
        assert!(apply(connection, &rename("1", "Old")).unwrap());
        assert!(!apply(connection, &rename("2", "New")).unwrap());
        let name: String = connection
            .query_row("SELECT value FROM fields WHERE field = 'name'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(name, "New");
    }

    /// Sends `from` what `to` lacks; returns how many messages were new.
    fn sync(from: &Store, to: &mut Store) -> usize {
        let missing = from.missing_for(&to.versions().unwrap()).unwrap();
        to.receive(&missing).unwrap()
    }

    #[test]
    fn two_devices_converge() {
        let mut laptop = Store::open_in_memory().unwrap();
        let mut phone = Store::open_in_memory().unwrap();
        let mut budget = sample();
        laptop.save(&budget).unwrap();
        let sent = sync(&laptop, &mut phone);
        assert!(sent > 0);
        assert_eq!(phone.load(budget.id).unwrap(), Some(budget.clone()));
        // Nothing left to send either way.
        assert_eq!(sync(&laptop, &mut phone), 0);
        assert_eq!(sync(&phone, &mut laptop), 0);

        // Both rename the budget offline. The two changes are concurrent:
        // the one with the greater timestamp wins, which may be either
        // (same millisecond and counter: the device identifiers, drawn at
        // random, break the tie). Whichever it is, both end up with it.
        budget.name = "From the laptop".to_string();
        laptop.save(&budget).unwrap();
        let mut on_phone = phone.load(budget.id).unwrap().unwrap();
        on_phone.name = "From the phone".to_string();
        phone.save(&on_phone).unwrap();
        assert_eq!(sync(&phone, &mut laptop), 1);
        assert_eq!(sync(&laptop, &mut phone), 1);
        let on_laptop = laptop.load(budget.id).unwrap().unwrap();
        assert_eq!(phone.load(budget.id).unwrap(), Some(on_laptop.clone()));
        assert!(["From the laptop", "From the phone"].contains(&on_laptop.name.as_str()));
        assert_eq!(laptop.versions().unwrap(), phone.versions().unwrap());
    }

    #[test]
    fn refuses_malformed_timestamps() {
        let mut store = Store::open_in_memory().unwrap();
        let message = Message {
            timestamp: "yesterday".to_string(),
            budget: "b".to_string(),
            entity: "budget".to_string(),
            id: "b".to_string(),
            field: "name".to_string(),
            value: None,
        };
        assert!(matches!(
            store.receive(&[message]),
            Err(StoreError::Corrupt(_))
        ));
        assert!(store.versions().unwrap().is_empty());
    }
}
