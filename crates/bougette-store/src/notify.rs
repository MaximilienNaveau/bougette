//! Notifications inside the app (doc/scope.md: Notifications internes):
//! what other members did that this member should know. They are found
//! by comparing a budget before and after receiving messages (`receive`),
//! and kept on this device only, never synced.
//!
//! What notifies, and nothing else:
//! - an amount this member wrote, overwritten by another member;
//! - a member joining or leaving the budget;
//! - this member's role changed, or this member removed.

use std::collections::{BTreeMap, HashMap};

use rusqlite::params;

use crate::access::Role;
use crate::{Store, StoreError};

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// An amount this member wrote was overwritten: `subject` names the
    /// line or transaction, `before` and `after` are in cents.
    AmountChanged {
        subject: String,
        before: i64,
        after: i64,
    },
    Joined,
    Left,
    /// This member's role is now this one.
    RoleChanged(Role),
    /// This member was removed from the budget.
    Removed,
}

/// One notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: i64,
    pub budget: String,
    /// When it was noticed, in milliseconds since 1970.
    pub millis: u64,
    /// The member it is about (who changed the amount, who joined or
    /// left): their name as known now, or the start of their identifier.
    /// Empty for a role change or a removal.
    pub who: String,
    pub event: Event,
    pub read: bool,
}

/// A budget as this member knew it before receiving messages: the amounts
/// they wrote, and the members with their roles.
#[derive(Debug, Default)]
pub(crate) struct Before {
    /// (entity, id) → the amount's value.
    amounts: HashMap<(String, String), Option<String>>,
    roles: BTreeMap<String, Role>,
}

impl Store {
    /// What `budget` holds now that notifications compare against.
    pub(crate) fn before(&self, budget: &str) -> Result<Before, StoreError> {
        // No identity yet: nothing is this member's.
        let Some(me) = self.identity.as_ref().map(|identity| identity.member()) else {
            return Ok(Before::default());
        };
        let mut statement = self.connection.prepare(
            "SELECT entity, id, value FROM fields
             WHERE budget = ?1 AND field = 'amount' AND author = ?2",
        )?;
        let rows = statement.query_map(params![budget, me], |row| {
            Ok(((row.get(0)?, row.get(1)?), row.get(2)?))
        })?;
        let amounts = rows.collect::<Result<_, _>>()?;
        let access = self.access(budget)?;
        Ok(Before {
            amounts,
            roles: access
                .members
                .into_iter()
                .map(|(member, info)| (member, info.role))
                .collect(),
        })
    }

    /// Records what changed in `budget` since `before`, done by others.
    pub(crate) fn notice(&mut self, budget: &str, before: Before) -> Result<(), StoreError> {
        let Some(me) = self.identity.as_ref().map(|identity| identity.member()) else {
            return Ok(());
        };
        // (the member it is about, by identifier; the event)
        let mut events: Vec<(String, Event)> = Vec::new();

        // Amounts: overwritten by someone else, with another value.
        for ((entity, id), old) in &before.amounts {
            let now: Option<(Option<String>, String)> = self
                .connection
                .query_row(
                    "SELECT value, author FROM fields
                     WHERE budget = ?1 AND entity = ?2 AND id = ?3 AND field = 'amount'",
                    params![budget, entity, id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();
            let Some((value, author)) = now else { continue };
            if author == me || value == *old {
                continue;
            }
            // `as_deref`: `Option<String>` to `Option<&str>`; unparsable
            // amounts count as 0.
            let cents = |v: &Option<String>| v.as_deref().and_then(|v| v.parse().ok()).unwrap_or(0);
            events.push((
                author,
                Event::AmountChanged {
                    subject: self.subject(budget, entity, id)?,
                    before: cents(old),
                    after: cents(&value),
                },
            ));
        }

        // Members: compared only with a budget known before, else joining
        // it would announce everyone already in it.
        if !before.roles.is_empty() {
            let after = self.access(budget)?;
            match (before.roles.get(&me), after.role(&me)) {
                (Some(_), None) => events.push((String::new(), Event::Removed)),
                (Some(old), Some(new)) if *old != new => {
                    events.push((String::new(), Event::RoleChanged(new)))
                }
                _ => {}
            }
            for member in after.members.keys() {
                if *member != me && !before.roles.contains_key(member) {
                    events.push((member.clone(), Event::Joined));
                }
            }
            for member in before.roles.keys() {
                if *member != me && !after.members.contains_key(member) {
                    events.push((member.clone(), Event::Left));
                }
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as i64);
        for (who, event) in events {
            // The event as columns: its kind, and what it carries.
            let (kind, subject, old, new, role) = match &event {
                Event::AmountChanged {
                    subject,
                    before,
                    after,
                } => ("amount", subject.as_str(), *before, *after, ""),
                Event::Joined => ("joined", "", 0, 0, ""),
                Event::Left => ("left", "", 0, 0, ""),
                Event::RoleChanged(role) => ("role", "", 0, 0, role.as_str()),
                Event::Removed => ("removed", "", 0, 0, ""),
            };
            self.connection.execute(
                "INSERT INTO notifications
                 (budget, millis, who, kind, subject, old, new, role)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![budget, now, who, kind, subject, old, new, role],
            )?;
        }
        Ok(())
    }

    /// The notifications, latest first, with the members' names as known
    /// now.
    pub fn notifications(&self) -> Result<Vec<Notification>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, budget, millis, who, kind, subject, old, new, role, read
             FROM notifications ORDER BY id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            let kind: String = row.get(4)?;
            let role: String = row.get(8)?;
            let event = match kind.as_str() {
                "amount" => Event::AmountChanged {
                    subject: row.get(5)?,
                    before: row.get(6)?,
                    after: row.get(7)?,
                },
                "joined" => Event::Joined,
                "left" => Event::Left,
                // An unknown role can't have been written: Reader, safest.
                "role" => Event::RoleChanged(Role::parse(&role).unwrap_or(Role::Reader)),
                _ => Event::Removed,
            };
            Ok(Notification {
                id: row.get(0)?,
                budget: row.get(1)?,
                millis: row.get::<_, i64>(2)?.max(0) as u64,
                who: row.get(3)?,
                event,
                read: row.get(9)?,
            })
        })?;
        let mut notifications: Vec<Notification> = rows.collect::<Result<_, _>>()?;
        for notification in &mut notifications {
            if !notification.who.is_empty() {
                notification.who = self.who(&notification.budget, &notification.who)?;
            }
        }
        Ok(notifications)
    }

    /// How many notifications are not read yet.
    pub fn unread(&self) -> Result<usize, StoreError> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM notifications WHERE read = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Marks every notification read.
    pub fn mark_read(&mut self) -> Result<(), StoreError> {
        self.connection
            .execute("UPDATE notifications SET read = 1", [])?;
        Ok(())
    }

    /// A member's name in `budget`, or the start of their identifier.
    fn who(&self, budget: &str, member: &str) -> Result<String, StoreError> {
        Ok(self
            .member_name(budget, member)?
            .unwrap_or_else(|| member.chars().take(8).collect()))
    }

    /// What an amount belongs to, in words: a line's name, or a
    /// transaction's category and date.
    fn subject(&self, budget: &str, entity: &str, id: &str) -> Result<String, StoreError> {
        let field = |entity: &str, id: &str, field: &str| -> Result<String, StoreError> {
            let value: Option<Option<String>> = self
                .connection
                .query_row(
                    "SELECT value FROM fields
                     WHERE budget = ?1 AND entity = ?2 AND id = ?3 AND field = ?4",
                    params![budget, entity, id, field],
                    |row| row.get(0),
                )
                .ok();
            Ok(value.flatten().unwrap_or_default())
        };
        Ok(match entity {
            // A revision's identifier is `line/from`.
            "revision" => field("line", id.split('/').next().unwrap_or(id), "name")?,
            _ => format!(
                "{}, {}",
                field("transaction", id, "category")?,
                field("transaction", id, "date")?
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bougette_core::budget::Budget;
    use bougette_core::line::Kind;
    use bougette_core::money::{Currency, Money};
    use bougette_core::transaction::Transaction;
    use jiff::civil::date;

    /// Sends `from` what `to` lacks, as a server would pass it on.
    fn send(from: &Store, to: &mut Store) {
        let missing = from.missing_for(&to.versions().unwrap(), |_| true).unwrap();
        to.receive(&missing).unwrap();
    }

    fn named(name: &str) -> Store {
        let mut store = Store::open_in_memory().unwrap();
        store.set_name(name).unwrap();
        store
    }

    /// The events, without their identifiers and times.
    fn events(store: &Store) -> Vec<(String, Event)> {
        store
            .notifications()
            .unwrap()
            .into_iter()
            .rev() // oldest first
            .map(|n| (n.who, n.event))
            .collect()
    }

    #[test]
    fn others_changes_notify_and_ones_own_dont() {
        let mut alice = named("Alice");
        let mut bob = named("Bob");
        let mut budget = Budget::new("Home", Currency::EUR);
        let paid = Transaction::new(
            date(2026, 11, 5),
            Kind::Cost,
            Money::new(4200, Currency::EUR),
            "Food",
            None,
        );
        budget.add_transaction(paid.clone()).unwrap();
        let budget = alice.create(budget).unwrap();
        let invitation = alice.invite(budget.id, Role::Editor, u64::MAX).unwrap();
        send(&alice, &mut bob);
        bob.request_join(&invitation).unwrap();
        send(&bob, &mut alice);
        send(&alice, &mut bob);
        send(&bob, &mut alice); // Bob's name, written once he is in
        // Alice's device let Bob in; Bob joining tells him nothing.
        assert_eq!(events(&alice), [("Bob".to_string(), Event::Joined)]);
        assert_eq!(events(&bob), []);

        // Bob changes Alice's amount.
        let mut changed = bob.load(budget.id).unwrap().unwrap();
        let mut transaction = paid.clone();
        transaction.amount = Money::new(4500, Currency::EUR);
        changed.replace_transaction(transaction).unwrap();
        bob.save(&changed).unwrap();
        send(&bob, &mut alice);
        assert_eq!(
            events(&alice)[1],
            (
                "Bob".to_string(),
                Event::AmountChanged {
                    subject: "Food, 2026-11-05".to_string(),
                    before: 4200,
                    after: 4500,
                }
            )
        );
        assert_eq!(alice.unread().unwrap(), 2);
        alice.mark_read().unwrap();
        assert_eq!(alice.unread().unwrap(), 0);

        // Bob made a Reader, then removed.
        let bob_id = bob.member().unwrap();
        alice.set_role(budget.id, &bob_id, Role::Reader).unwrap();
        send(&alice, &mut bob);
        alice.remove(budget.id, &bob_id).unwrap();
        send(&alice, &mut bob);
        assert_eq!(
            events(&bob),
            [
                (String::new(), Event::RoleChanged(Role::Reader)),
                (String::new(), Event::Removed),
            ]
        );
        // Alice's own changes don't notify her.
        assert_eq!(alice.notifications().unwrap().len(), 2);
    }
}
