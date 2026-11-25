//! Bougette's local storage: every change is a signed message in a SQLite
//! log, and the current state is rebuilt from those messages.
//!
//! Tables:
//! - `messages`: the log, one row per changed field, never modified. These
//!   are what devices exchange. Their content is encrypted with a key of
//!   their budget, except access changes (see `access`), and each is
//!   signed by its author (see `message`).
//! - `fields`: the current value of each field, decrypted: the latest
//!   change by someone allowed to make it at the time (see `rebuild`).
//! - `keys`: the budget keys this device holds.
//! - `invitations`: the invitations this device made, with their secret.
//! - `meta`: the device's settings, never synced.
//!
//! To sync, two stores compare their version vectors (`versions`): for
//! each budget and device, the latest of its messages they hold. A
//! device's own messages have increasing timestamps and are never changed,
//! and a store always receives a device's messages of a budget in order
//! and all at once, so each store holds a *prefix* of them. What a store
//! lacks is therefore exactly the messages after its entry (`missing_for`,
//! ADR 0004).
//!
//! A server (`open_server`) only keeps and passes on messages: it never
//! decrypts, and only reads the access changes.

mod access;
mod backup;
mod bytes;
mod clock;
mod codec;
pub mod crypto;
mod members;
mod message;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use bougette_core::budget::Budget;
use bougette_core::id::{BudgetId, LineId, MemberId, TransactionId};
use bougette_core::shared::{Reflections, reflections};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

pub use crate::access::{Access, Role};
use crate::clock::Clock;
// `pub use` re-exports: users of this crate see `bougette_store::Corrupt`.
pub use crate::codec::{Corrupt, Trashed};
use crate::codec::{DELETED, Records};
use crate::crypto::Identity;
pub use crate::members::{Invitation, MemberInfo};
pub use crate::message::{Body, Change, Message};

/// Why a storage operation failed.
#[derive(Debug)]
pub enum StoreError {
    /// SQLite reported an error.
    Sqlite(rusqlite::Error),
    /// Stored or received data isn't valid.
    Corrupt(Corrupt),
    /// The member may not do this, e.g. a Reader changing a budget.
    NotAllowed(String),
    /// This device holds no key of the budget yet.
    NoKey,
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

/// A version vector: for each budget and device, the timestamp of the
/// latest message held here.
pub type Versions = BTreeMap<(String, String), String>;

/// Who created a line or a transaction, and who changed it last (see
/// `Store::trace`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub created: Stamp,
    pub changed: Stamp,
}

/// When, and by which member, something was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    /// Milliseconds since 1970.
    pub millis: u64,
    /// The member's identifier.
    pub member: String,
    /// The name they gave themselves in the budget, if any.
    pub name: Option<String>,
}

/// The storage of one device, or of a server.
pub struct Store {
    connection: Connection,
    clock: Clock,
    /// The member this device belongs to; drawn when first needed.
    identity: Option<Identity>,
    /// Whether to decrypt and rebuild budgets: false on a server.
    replays: bool,
}

/// The database schema, version 3 (SQLite's `user_version`). `IF NOT
/// EXISTS` makes opening an existing file harmless.
const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS messages (
        timestamp TEXT PRIMARY KEY,
        budget TEXT NOT NULL,
        author TEXT NOT NULL,
        previous TEXT NOT NULL,
        key INTEGER,
        body BLOB NOT NULL,
        signature BLOB NOT NULL
    );
    CREATE INDEX IF NOT EXISTS messages_by_budget ON messages (budget, timestamp);
    CREATE TABLE IF NOT EXISTS fields (
        budget TEXT NOT NULL,
        entity TEXT NOT NULL,
        id TEXT NOT NULL,
        field TEXT NOT NULL,
        value TEXT,
        timestamp TEXT NOT NULL,
        author TEXT NOT NULL,
        PRIMARY KEY (budget, entity, id, field)
    );
    CREATE TABLE IF NOT EXISTS keys (
        budget TEXT NOT NULL,
        number INTEGER NOT NULL,
        key BLOB NOT NULL,
        PRIMARY KEY (budget, number)
    );
    CREATE TABLE IF NOT EXISTS invitations (
        id TEXT PRIMARY KEY,
        budget TEXT NOT NULL,
        secret BLOB NOT NULL,
        role TEXT NOT NULL,
        expires INTEGER NOT NULL,
        used INTEGER NOT NULL DEFAULT 0
    );
    PRAGMA user_version = 3;
";

impl Store {
    /// Opens the database file of a device at `path`, creating it if
    /// needed.
    // `&Path` is a borrowed file path, like `const std::filesystem::path&`.
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        Store::with_connection(Connection::open(path)?, true)
    }

    /// A device's database in memory, gone when the Store is dropped.
    pub fn open_in_memory() -> Result<Store, StoreError> {
        Store::with_connection(Connection::open_in_memory()?, true)
    }

    /// Opens a server's database: messages are kept, never decrypted.
    pub fn open_server(path: &Path) -> Result<Store, StoreError> {
        Store::with_connection(Connection::open(path)?, false)
    }

    fn with_connection(connection: Connection, replays: bool) -> Result<Store, StoreError> {
        // A database of an earlier version (M0 and M1: 0, M2: 2) has
        // another layout: refuse it rather than misread it.
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let tables: i64 =
            connection.query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))?;
        if version != 3 && tables > 0 {
            return Err(Corrupt(
                "this database comes from an older version of Bougette: start a new one".into(),
            )
            .into());
        }
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
        // Continue the clock after the latest message held.
        let last: Option<String> =
            connection.query_row("SELECT MAX(timestamp) FROM messages", [], |row| row.get(0))?;
        let clock = Clock::new(node, last.as_deref());
        let identity = Store::meta(&connection, "identity")?
            .and_then(|seed| crypto::from_hex_array::<32>(&seed))
            .map(Identity::from_seed);
        Ok(Store {
            connection,
            clock,
            identity,
            replays,
        })
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

    /// The member this device belongs to: drawn and kept the first time.
    fn identity(&mut self) -> Result<Identity, StoreError> {
        if self.identity.is_none() {
            let identity = Identity::generate();
            self.set_setting("identity", &crypto::hex(&identity.seed()))?;
            self.identity = Some(identity);
        }
        // `clone`: the caller gets its own copy, so it doesn't keep `self`
        // borrowed (the clock and the connection are needed alongside).
        Ok(self.identity.clone().expect("set above"))
    }

    /// This device's member identifier.
    pub fn member(&mut self) -> Result<String, StoreError> {
        Ok(self.identity()?.member())
    }

    /// The member's secret seed, to link another device (it gives full
    /// access to the member's budgets: show it only on request).
    pub fn identity_seed(&mut self) -> Result<[u8; 32], StoreError> {
        Ok(self.identity()?.seed())
    }

    /// Makes this device one of the member's devices with `seed`. Only on
    /// a device whose member hasn't written anything yet (it may have
    /// received messages, e.g. when recovering).
    pub fn adopt_identity(&mut self, seed: [u8; 32]) -> Result<(), StoreError> {
        let written: i64 = match &self.identity {
            Some(ours) if ours.seed() != seed => self.connection.query_row(
                "SELECT COUNT(*) FROM messages WHERE author = ?1",
                params![ours.member()],
                |row| row.get(0),
            )?,
            _ => 0,
        };
        if written > 0 {
            return Err(StoreError::NotAllowed(
                "this device already has data: link a new one".into(),
            ));
        }
        self.set_setting("identity", &crypto::hex(&seed))?;
        self.identity = Some(Identity::from_seed(seed));
        Ok(())
    }

    /// Proves that the device with network identifier `device` belongs to
    /// this member: their identifier, and their signature of it.
    pub fn prove_device(&mut self, device: &str) -> Result<(String, Vec<u8>), StoreError> {
        let identity = self.identity()?;
        Ok((
            identity.member(),
            identity.sign(device_claim(device).as_bytes()),
        ))
    }

    /// Creates `budget` as a new budget of this member, its first Admin,
    /// and saves it. Its identifier changes: it is derived from the
    /// creator's key (ADR 0005), so the budget is returned.
    pub fn create(&mut self, mut budget: Budget) -> Result<Budget, StoreError> {
        let identity = self.identity()?;
        let (id, origin) = access::origin(&identity);
        budget.id = BudgetId(id);
        let budget_id = id.to_string();
        let number = self.new_key_number(&budget_id)?;
        let key = self.key(&budget_id, number)?.expect("just drawn");
        let sealed = members::seal_key(&identity.member(), &identity.receiving_key(), number, &key)
            .expect("our own key is valid");
        self.write(&budget_id, vec![origin, sealed])?;
        self.save(&budget)?;
        self.introduce(&budget_id)?;
        Ok(budget)
    }

    /// Saves `budget`: one message per field that differs from what is
    /// stored. Entities missing from `budget` are marked deleted. Only an
    /// Admin or an Editor may.
    pub fn save(&mut self, budget: &Budget) -> Result<(), StoreError> {
        let budget_id = budget.id.to_string();
        let me = self.member()?;
        if !matches!(
            self.access(&budget_id)?.role(&me),
            Some(Role::Admin | Role::Editor)
        ) {
            return Err(StoreError::NotAllowed(
                "only an Admin or an Editor changes a budget".into(),
            ));
        }
        let old = self.records(&budget_id)?;
        let new = codec::encode(budget);

        // The changes, in key order: new or different fields…
        let mut changes = Vec::new();
        for ((entity, id, field), value) in &new {
            if old.get(&(entity.clone(), id.clone(), field.clone())) != Some(value) {
                changes.push(Change::new(
                    entity.as_str(),
                    id.as_str(),
                    field.as_str(),
                    value.clone(),
                ));
            }
        }
        // …and a deletion mark for each entity that is gone.
        for (entity, id, field) in old.keys() {
            let gone = !new.contains_key(&(entity.clone(), id.clone(), field.clone()));
            let marked = old.get(&(entity.clone(), id.clone(), DELETED.to_string()))
                == Some(&Some("true".to_string()));
            if field == DELETED && gone && !marked {
                changes.push(Change::new(
                    entity.as_str(),
                    id.as_str(),
                    DELETED,
                    Some("true".to_string()),
                ));
            }
        }
        self.write(&budget_id, changes)
    }

    /// Writes `changes` to budget `budget` as messages of this member,
    /// content encrypted with the latest key held, then rebuilds it.
    fn write(&mut self, budget: &str, changes: Vec<Change>) -> Result<(), StoreError> {
        if changes.is_empty() {
            return Ok(());
        }
        let identity = self.identity()?;
        let access = self.access(budget)?;
        let keys = self.keys(budget)?;
        // The latest announced key this device holds.
        let current = access
            .keys
            .iter()
            .rev()
            .find_map(|number| keys.get(number).map(|key| (*number, *key)));
        // The last message of this device in this budget: the next one
        // links to it.
        let node = self.setting("node")?.unwrap_or_default();
        let mut previous = self
            .messages_where(
                "budget = ?1 AND substr(timestamp, 23) = ?2 \
                 AND timestamp = (SELECT MAX(timestamp) FROM messages \
                 WHERE budget = ?1 AND substr(timestamp, 23) = ?2)",
                &[budget, &node],
            )?
            .first()
            .map(Message::hash)
            .unwrap_or_default();
        // All or nothing: a SQLite transaction, committed at the end. If
        // anything fails before, `transaction` is dropped and rolls back,
        // like an RAII guard.
        let transaction = self.connection.transaction()?;
        for change in changes {
            let message = Message::new(
                &identity,
                self.clock.tick(),
                budget.to_string(),
                previous,
                change,
                // `as_ref().map(...)`: borrow the key inside the Option.
                current.as_ref().map(|(number, key)| (*number, key)),
            )
            .ok_or(StoreError::NoKey)?;
            insert(&transaction, &message)?;
            previous = message.hash();
        }
        transaction.commit()?;
        self.rebuild(budget)
    }

    /// The budget `id` as currently stored, `None` if unknown, deleted, or
    /// not readable by this device.
    pub fn load(&self, id: BudgetId) -> Result<Option<Budget>, StoreError> {
        Ok(codec::decode(&self.records(&id.to_string())?)?)
    }

    /// The lines and transactions of budget `budget` deleted at or after
    /// `since` (milliseconds since 1970), most recent first, each with
    /// when it was deleted (milliseconds). Restoring one is adding it back
    /// to the budget and saving: only its deletion mark changes.
    pub fn trash(&self, budget: BudgetId, since: u64) -> Result<Vec<(u64, Trashed)>, StoreError> {
        let budget = budget.to_string();
        let mut statement = self.connection.prepare(
            "SELECT entity, id, timestamp FROM fields
             WHERE budget = ?1 AND field = 'deleted' AND value = 'true'",
        )?;
        let rows = statement.query_map(params![budget], |row| {
            Ok(((row.get(0)?, row.get(1)?), row.get(2)?))
        })?;
        // `collect` into a Result: the first error stops it, else the map.
        let deleted_at: HashMap<(String, String), String> = rows.collect::<Result<_, _>>()?;
        let trashed = codec::trashed(&self.records(&budget)?, &deleted_at)?;
        Ok(trashed
            .into_iter()
            // A timestamp starts with its milliseconds (clock.rs);
            // `filter_map` drops what fails to parse or is too old.
            .filter_map(|(when, item)| {
                let millis: u64 = when.get(..16)?.parse().ok()?;
                (millis >= since).then_some((millis, item))
            })
            .collect())
    }

    /// Who created `entity` `id` of budget `budget` and when, and who
    /// changed it last and when (doc/scope.md: Traçabilité); `None` if it
    /// has no field. A line's changes include those of its revisions.
    pub fn trace(
        &self,
        budget: BudgetId,
        entity: &str,
        id: &str,
    ) -> Result<Option<Trace>, StoreError> {
        let budget = budget.to_string();
        // The oldest field was written when the entity was created: only
        // the fields that change are written again (see `save`).
        let mut statement = self.connection.prepare(
            "SELECT timestamp, author FROM fields
             WHERE budget = ?1 AND ((entity = ?2 AND id = ?3)
                OR (?2 = 'line' AND entity = 'revision' AND id LIKE ?3 || '/%'))
             ORDER BY timestamp",
        )?;
        let rows = statement.query_map(params![budget, entity, id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let stamps: Vec<(String, String)> = rows.collect::<Result<_, _>>()?;
        // `first` and `last` are Options: `let ... else` leaves on `None`.
        let (Some(first), Some(last)) = (stamps.first(), stamps.last()) else {
            return Ok(None);
        };
        Ok(Some(Trace {
            created: self.stamp(&budget, first)?,
            changed: self.stamp(&budget, last)?,
        }))
    }

    /// A field's (timestamp, author) as a Stamp, with the author's name.
    fn stamp(
        &self,
        budget: &str,
        (timestamp, author): &(String, String),
    ) -> Result<Stamp, StoreError> {
        Ok(Stamp {
            // A timestamp starts with its milliseconds (clock.rs).
            millis: timestamp
                .get(..16)
                .and_then(|m| m.parse().ok())
                .unwrap_or(0),
            member: author.clone(),
            name: self.member_name(budget, author)?,
        })
    }

    /// `budget` as this member sees it: a personal budget also shows what
    /// they pay or receive in the other budgets (see
    /// `bougette_core::shared::reflections`). Computed, never saved.
    pub fn shown(&mut self, budget: Budget) -> Result<Budget, StoreError> {
        if !budget.personal {
            return Ok(budget);
        }
        let me = MemberId(self.member()?);
        let mut reflected = Reflections::default();
        for (id, _) in self.budgets()? {
            if id != budget.id
                && let Some(other) = self.load(id)?
            {
                let mut more = reflections(&other, &me);
                // `append` moves every item of `more` to the end, leaving
                // it empty: no copies.
                reflected.lines.append(&mut more.lines);
                reflected.transactions.append(&mut more.transactions);
            }
        }
        Ok(budget.with_reflections(reflected))
    }

    /// The budget that holds line `line`, among those this device can
    /// read: where a line seen in a personal budget comes from.
    pub fn budget_of_line(&self, line: LineId) -> Result<Option<Budget>, StoreError> {
        self.find_budget(|budget| budget.line(line).is_some())
    }

    /// The budget that holds transaction `transaction` (see
    /// `budget_of_line`).
    pub fn budget_of_transaction(
        &self,
        transaction: TransactionId,
    ) -> Result<Option<Budget>, StoreError> {
        self.find_budget(|budget| budget.transactions().iter().any(|t| t.id == transaction))
    }

    /// The first budget for which `test` is true.
    fn find_budget(&self, test: impl Fn(&Budget) -> bool) -> Result<Option<Budget>, StoreError> {
        for (id, _) in self.budgets()? {
            if let Some(budget) = self.load(id)?
                && test(&budget)
            {
                return Ok(Some(budget));
            }
        }
        Ok(None)
    }

    /// The budgets this device can read and that aren't deleted:
    /// identifier and name.
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

    /// Every budget this store has messages of, readable or not.
    pub fn budget_ids(&self) -> Result<Vec<String>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT DISTINCT budget FROM messages ORDER BY budget")?;
        let rows = statement.query_map([], |row| row.get(0))?;
        // Collecting an iterator of `Result`s into a `Result<Vec<_>, _>`:
        // the first error stops it.
        Ok(rows.collect::<Result<Vec<String>, _>>()?)
    }

    /// The access of budget `budget`, as of all its access changes held
    /// (but those cut off, see `access::cut_off`).
    pub fn access(&self, budget: &str) -> Result<Access, StoreError> {
        let messages = self.messages_where("budget = ?1", &[budget])?;
        let excluded = access::cut_off(budget, &messages);
        let mut access = Access::default();
        for (position, message) in messages.iter().enumerate() {
            if let Body::Clear(change) = &message.body
                && !excluded.contains(&position)
            {
                access.apply(budget, &message.author, &message.timestamp, change);
            }
        }
        Ok(access)
    }

    /// This store's version vector.
    pub fn versions(&self) -> Result<Versions, StoreError> {
        // The latest timestamp of each budget and device: the device is
        // what follows the 22 characters of time and counter (SQLite
        // counts from 1).
        let mut statement = self.connection.prepare(
            "SELECT budget, MAX(timestamp) FROM messages
             GROUP BY budget, substr(timestamp, 23)",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut versions = Versions::new();
        for row in rows {
            let (budget, timestamp) = row?;
            let node = clock::node_of(&timestamp)
                .ok_or_else(|| Corrupt(format!("invalid timestamp {timestamp:?}")))?;
            versions.insert((budget, node.to_string()), timestamp);
        }
        Ok(versions)
    }

    /// The messages of the budgets `allowed` keeps that a store with
    /// version vector `theirs` lacks, oldest first.
    pub fn missing_for(
        &self,
        theirs: &Versions,
        allowed: impl Fn(&str) -> bool,
    ) -> Result<Vec<Message>, StoreError> {
        let mut missing = Vec::new();
        for message in self.messages_where("1", &[])? {
            if !allowed(&message.budget) {
                continue;
            }
            // Missing if they have nothing from its budget and device, or
            // only older messages. `is_none_or` is true for `None`, else
            // the test.
            let node = clock::node_of(&message.timestamp).unwrap_or_default();
            let key = (message.budget.clone(), node.to_string());
            if theirs
                .get(&key)
                .is_none_or(|latest| message.timestamp > *latest)
            {
                missing.push(message);
            }
        }
        Ok(missing)
    }

    /// Records messages from another device, all or nothing, and returns
    /// how many were new. Fails on a malformed or forged message. On a
    /// device, the budgets concerned are then rebuilt, and this member
    /// follows up as needed (new members, new keys: see `members`).
    pub fn receive(&mut self, messages: &[Message]) -> Result<usize, StoreError> {
        for message in messages {
            if clock::parse(&message.timestamp).is_none() || !message.is_authentic() {
                return Err(Corrupt(format!("invalid message {:?}", message.timestamp)).into());
            }
        }
        let transaction = self.connection.transaction()?;
        let mut new = 0;
        for message in messages {
            if insert(&transaction, message)? {
                new += 1;
            }
        }
        transaction.commit()?;
        // Only once committed: our next changes must sort after theirs.
        for message in messages {
            self.clock.observe(&message.timestamp);
        }
        // A `BTreeSet` of the budgets concerned: each once, in order.
        let budgets: BTreeSet<&str> = messages.iter().map(|m| m.budget.as_str()).collect();
        for budget in budgets {
            self.rebuild(budget)?;
            self.follow_up(budget)?;
        }
        Ok(new)
    }

    /// Rebuilds the fields of budget `budget` from its messages: opens the
    /// budget keys sealed for this member, then replays every message in
    /// timestamp order, keeping the content changes whose author was
    /// allowed to make them at the time.
    fn rebuild(&mut self, budget: &str) -> Result<(), StoreError> {
        if !self.replays {
            return Ok(());
        }
        let messages = self.messages_where("budget = ?1", &[budget])?;
        // First pass: the keys sealed for us.
        if let Some(identity) = &self.identity {
            let me = identity.member();
            let access = self.access(budget)?;
            let known = self.keys(budget)?;
            for ((member, number), sealed) in &access.sealed {
                if *member != me || known.contains_key(number) {
                    continue;
                }
                let key = identity
                    .open(sealed)
                    .and_then(|key| <[u8; 32]>::try_from(key).ok());
                if let Some(key) = key {
                    self.add_key(budget, *number, &key)?;
                }
            }
        }
        // Second pass: everything, in order, but what was cut off.
        let keys = self.keys(budget)?;
        let excluded = access::cut_off(budget, &messages);
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM fields WHERE budget = ?1", params![budget])?;
        let mut access = Access::default();
        for (position, message) in messages.iter().enumerate() {
            if excluded.contains(&position) {
                continue;
            }
            if let Body::Clear(change) = &message.body {
                access.apply(budget, &message.author, &message.timestamp, change);
                continue;
            }
            // `copied` turns the `Option<&[u8; 32]>` into an owned copy.
            let Some(change) = message.change(|number| keys.get(&number).copied()) else {
                continue; // no key for it (yet)
            };
            if !access.may_write(&message.author, &change) {
                continue;
            }
            // In timestamp order, so a later change of a field replaces
            // the earlier one: "the latest change wins".
            transaction.execute(
                "INSERT OR REPLACE INTO fields
                 (budget, entity, id, field, value, timestamp, author)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    budget,
                    change.entity,
                    change.id,
                    change.field,
                    change.value,
                    message.timestamp,
                    message.author
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// The messages matching the SQL condition `filter`, with `?1`, `?2`...
    /// bound to `values`, in timestamp order.
    fn messages_where(&self, filter: &str, values: &[&str]) -> Result<Vec<Message>, StoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT timestamp, budget, author, previous, key, body, signature
             FROM messages WHERE {filter} ORDER BY timestamp"
        ))?;
        // `params_from_iter` binds the values of a list, however many.
        let rows = statement.query_map(rusqlite::params_from_iter(values), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<u32>>(4)?,
                row.get::<_, Vec<u8>>(5)?,
                row.get::<_, Vec<u8>>(6)?,
            ))
        })?;
        let mut messages = Vec::new();
        for row in rows {
            let (timestamp, budget, author, previous, key, body, signature) = row?;
            let body = Message::body_from(key, body)
                .ok_or_else(|| Corrupt(format!("invalid message {timestamp:?}")))?;
            messages.push(Message {
                timestamp,
                budget,
                author,
                previous,
                body,
                signature,
            });
        }
        Ok(messages)
    }

    /// The budget keys of `budget` this device holds, by number.
    fn keys(&self, budget: &str) -> Result<BTreeMap<u32, [u8; 32]>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT number, key FROM keys WHERE budget = ?1")?;
        let rows = statement.query_map(params![budget], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut keys = BTreeMap::new();
        for row in rows {
            let (number, key) = row?;
            let key = <[u8; 32]>::try_from(key).map_err(|_| Corrupt("invalid key".into()))?;
            keys.insert(number, key);
        }
        Ok(keys)
    }

    fn key(&self, budget: &str, number: u32) -> Result<Option<[u8; 32]>, StoreError> {
        Ok(self.keys(budget)?.get(&number).copied())
    }

    fn add_key(&self, budget: &str, number: u32, key: &[u8; 32]) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT OR IGNORE INTO keys (budget, number, key) VALUES (?1, ?2, ?3)",
            params![budget, number, key.as_slice()],
        )?;
        Ok(())
    }

    /// Draws a new key for `budget`, keeps it, and returns its number.
    fn new_key_number(&self, budget: &str) -> Result<u32, StoreError> {
        let number = u32::from_be_bytes(crypto::random());
        self.add_key(budget, number, &crypto::random())?;
        Ok(number)
    }

    /// Every stored field of budget `budget`, members' names excepted.
    fn records(&self, budget: &str) -> Result<Records, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT entity, id, field, value FROM fields
             WHERE budget = ?1 AND entity != 'member'",
        )?;
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

/// Whether `proof` shows that the device `device` belongs to `member`.
pub fn device_belongs(member: &str, device: &str, proof: &[u8]) -> bool {
    crypto::verify(member, device_claim(device).as_bytes(), proof)
}

/// What a member signs to claim a device.
fn device_claim(device: &str) -> String {
    format!("bougette device {device}")
}

/// Records one message in the log; returns whether it was new.
fn insert(connection: &Connection, message: &Message) -> Result<bool, StoreError> {
    let (key, body) = message.stored_body();
    // `execute` returns the number of rows changed: 0 if already there.
    let inserted = connection.execute(
        "INSERT OR IGNORE INTO messages
         (timestamp, budget, author, previous, key, body, signature)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            message.timestamp,
            message.budget,
            message.author,
            message.previous,
            key,
            body,
            message.signature
        ],
    )?;
    Ok(inserted > 0)
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
        rent.payer = Some(MemberId("cd".repeat(32)));
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

    /// Sends `from` what `to` lacks; returns how many messages were new.
    fn send(from: &Store, to: &mut Store) -> usize {
        let missing = from.missing_for(&to.versions().unwrap(), |_| true).unwrap();
        to.receive(&missing).unwrap()
    }

    /// A second device of the member of `store`.
    fn other_device(store: &mut Store) -> Store {
        let mut device = Store::open_in_memory().unwrap();
        device
            .adopt_identity(store.identity_seed().unwrap())
            .unwrap();
        device
    }

    #[test]
    fn a_saved_budget_loads_back_identical() {
        let mut store = Store::open_in_memory().unwrap();
        let budget = store.create(sample()).unwrap();
        assert_eq!(store.load(budget.id).unwrap(), Some(budget.clone()));
        assert_eq!(
            store.budgets().unwrap(),
            vec![(budget.id, "Household".to_string())]
        );
    }

    #[test]
    fn saving_again_writes_only_what_changed() {
        let mut store = Store::open_in_memory().unwrap();
        let mut budget = store.create(sample()).unwrap();
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
    fn a_line_knows_who_created_and_last_changed_it() {
        let mut store = Store::open_in_memory().unwrap();
        store.set_name("Alice").unwrap();
        let mut budget = store.create(sample()).unwrap();
        let line = budget.lines()[0].id.to_string();
        let created = store.trace(budget.id, "line", &line).unwrap().unwrap();
        // Its fields are written one by one, milliseconds apart.
        assert!(created.created.millis <= created.changed.millis);
        assert_eq!(created.created.name.as_deref(), Some("Alice"));
        assert_eq!(created.created.member, store.member().unwrap());

        // A new amount from March: a revision, which counts as a change
        // of the line.
        let id = budget.lines()[0].id;
        let schedule = budget.lines()[0].revisions()[0].schedule;
        budget
            .change_line(id, date(2026, 3, 1), eur(95000), schedule)
            .unwrap();
        store.save(&budget).unwrap();
        let changed = store.trace(budget.id, "line", &line).unwrap().unwrap();
        assert_eq!(changed.created, created.created);
        assert!(changed.changed.millis >= created.created.millis);
        assert!(changed.changed.millis >= created.changed.millis);

        assert_eq!(store.trace(budget.id, "line", "none").unwrap(), None);
    }

    #[test]
    fn deleted_lines_and_transactions_wait_in_the_trash() {
        let mut store = Store::open_in_memory().unwrap();
        let mut budget = store.create(sample()).unwrap();
        assert!(store.trash(budget.id, 0).unwrap().is_empty());

        // The bike savings: changed from March, then from February (which
        // replaces March), then removed.
        let savings = budget.lines()[1].clone();
        let weekly = Schedule::Weekly {
            first: date(2026, 1, 2),
        };
        budget
            .change_line(savings.id, date(2026, 3, 1), eur(12000), weekly)
            .unwrap();
        store.save(&budget).unwrap();
        budget
            .change_line(savings.id, date(2026, 2, 1), eur(11000), weekly)
            .unwrap();
        store.save(&budget).unwrap();
        budget.remove_line(savings.id).unwrap();
        store.save(&budget).unwrap();
        // Then January's rent payment.
        let paid = budget.transactions()[0].clone();
        budget.remove_transaction(paid.id).unwrap();
        store.save(&budget).unwrap();

        // Both in the trash, the latest first; the line as it was when
        // removed: January and February, not the replaced March.
        let trash = store.trash(budget.id, 0).unwrap();
        assert_eq!(trash.len(), 2);
        assert_eq!(trash[0].1, Trashed::Transaction(paid.clone()));
        let Trashed::Line(line) = &trash[1].1 else {
            panic!("not a line: {:?}", trash[1].1);
        };
        let froms: Vec<_> = line.revisions().iter().map(|r| r.from).collect();
        assert_eq!(froms, [date(2026, 1, 1), date(2026, 2, 1)]);
        assert!(trash[0].0 >= trash[1].0);
        // Nothing deleted after a time in the future.
        assert!(store.trash(budget.id, u64::MAX).unwrap().is_empty());

        // Restored: added back and saved, as it was.
        budget.add_line(line.clone()).unwrap();
        budget.add_transaction(paid.clone()).unwrap();
        store.save(&budget).unwrap();
        let loaded = store.load(budget.id).unwrap().unwrap();
        assert_eq!(loaded.line(savings.id), Some(line));
        assert!(loaded.transactions().contains(&paid));
        assert!(store.trash(budget.id, 0).unwrap().is_empty());
    }

    #[test]
    fn a_revision_replaced_by_an_earlier_change_is_deleted() {
        let mut store = Store::open_in_memory().unwrap();
        let mut budget = store.create(sample()).unwrap();
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
        let budget = {
            // An inner block: `store` is dropped (the file closed) at `}`,
            // like a C++ destructor at the end of a scope.
            let mut store = Store::open(&path).unwrap();
            store.create(sample()).unwrap()
        };
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
    fn refuses_a_database_of_an_older_version() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE messages (timestamp TEXT)")
            .unwrap();
        assert!(matches!(
            Store::with_connection(connection, true),
            Err(StoreError::Corrupt(_))
        ));
    }

    #[test]
    fn the_latest_change_of_a_field_wins_whatever_the_order() {
        let mut laptop = Store::open_in_memory().unwrap();
        let mut phone = other_device(&mut laptop);
        let mut budget = laptop.create(Budget::new("Old", Currency::EUR)).unwrap();
        budget.name = "New".to_string();
        laptop.save(&budget).unwrap();
        // The phone receives the laptop's messages newest first.
        let mut messages = laptop.missing_for(&Versions::new(), |_| true).unwrap();
        messages.reverse();
        phone.receive(&messages).unwrap();
        assert_eq!(phone.load(budget.id).unwrap().unwrap().name, "New");
    }

    #[test]
    fn two_devices_of_a_member_converge() {
        let mut laptop = Store::open_in_memory().unwrap();
        let mut phone = other_device(&mut laptop);
        let mut budget = laptop.create(sample()).unwrap();
        assert!(send(&laptop, &mut phone) > 0);
        assert_eq!(phone.load(budget.id).unwrap(), Some(budget.clone()));
        // Nothing left to send either way.
        assert_eq!(send(&laptop, &mut phone), 0);
        assert_eq!(send(&phone, &mut laptop), 0);

        // Both rename the budget offline. The two changes are concurrent:
        // the one with the greater timestamp wins, which may be either
        // (same millisecond and counter: the device identifiers, drawn at
        // random, break the tie). Whichever it is, both end up with it.
        budget.name = "From the laptop".to_string();
        laptop.save(&budget).unwrap();
        let mut on_phone = phone.load(budget.id).unwrap().unwrap();
        on_phone.name = "From the phone".to_string();
        phone.save(&on_phone).unwrap();
        assert_eq!(send(&phone, &mut laptop), 1);
        assert_eq!(send(&laptop, &mut phone), 1);
        let on_laptop = laptop.load(budget.id).unwrap().unwrap();
        assert_eq!(phone.load(budget.id).unwrap(), Some(on_laptop.clone()));
        assert!(["From the laptop", "From the phone"].contains(&on_laptop.name.as_str()));
        assert_eq!(laptop.versions().unwrap(), phone.versions().unwrap());
    }

    #[test]
    fn refuses_forged_messages() {
        let mut alice = Store::open_in_memory().unwrap();
        let budget = alice.create(sample()).unwrap();
        let mut messages = alice.missing_for(&Versions::new(), |_| true).unwrap();
        messages[0].author = Store::open_in_memory().unwrap().member().unwrap();
        let mut other = Store::open_in_memory().unwrap();
        assert!(matches!(
            other.receive(&messages),
            Err(StoreError::Corrupt(_))
        ));
        assert!(other.versions().unwrap().is_empty());
        assert!(other.load(budget.id).unwrap().is_none());
    }

    #[test]
    fn a_server_keeps_messages_it_cannot_read() {
        let mut alice = Store::open_in_memory().unwrap();
        let budget = alice.create(sample()).unwrap();
        let path = std::env::temp_dir().join(format!("bougette-test-{}.sqlite", Uuid::now_v7()));
        let mut server = Store::open_server(&path).unwrap();
        assert!(send(&alice, &mut server) > 0);
        assert!(server.load(budget.id).unwrap().is_none());
        // It reads the access, though: Alice is the Admin.
        let access = server.access(&budget.id.to_string()).unwrap();
        assert_eq!(access.role(&alice.member().unwrap()), Some(Role::Admin));
        // And it passes everything on to Alice's other device.
        let mut phone = other_device(&mut alice);
        send(&server, &mut phone);
        assert_eq!(phone.load(budget.id).unwrap(), Some(budget));
        std::fs::remove_file(&path).unwrap();
    }
}
