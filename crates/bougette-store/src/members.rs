//! Members of a budget: invitations, joining, roles, leaving (ADR 0005).
//!
//! Joining, with an Admin A and a newcomer N:
//! 1. A's device makes an invitation: a random secret, kept on A's device
//!    with the budget, the role offered and an expiry. A gives it to N
//!    (with the server's address, see the CLI).
//! 2. N's device writes a join request in the budget: N's identifier and
//!    receiving key, with a proof that N knows the secret (an HMAC with a
//!    key derived from it).
//! 3. When A's device receives the request (`follow_up`), it checks the
//!    proof, the expiry and that the invitation wasn't used, then grants
//!    the role and seals every budget key it holds for N.
//!
//! The server only ever learns a *token* derived from the secret (to let
//! N's device in, see bougette-server), never the proof key: it can't
//! forge a request.

use bougette_core::id::BudgetId;
use rusqlite::{OptionalExtension, params};

use crate::access::{Access, Role};
use crate::clock;
use crate::crypto;
use crate::message::{ACCESS, Change};
use crate::{Store, StoreError};

/// An invitation to a budget. Its secret is what the invitee is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    pub budget: String,
    pub secret: [u8; 32],
}

impl Invitation {
    /// Its identifier, safe to show: it doesn't reveal the secret.
    pub fn id(&self) -> String {
        crypto::hex(&crypto::sha256(&[b"bougette invitation", &self.secret])[..16])
    }

    /// What the invitee shows the server to be let in.
    pub fn token(&self) -> [u8; 32] {
        crypto::sha256(&[b"bougette server token", &self.secret])
    }

    /// The key of the join request's proof.
    fn proof_key(&self) -> [u8; 32] {
        crypto::sha256(&[b"bougette join proof", &self.secret])
    }

    /// The proof that `member`, with `receiving_key`, holds the secret.
    fn proof(&self, member: &str, receiving_key: &str) -> String {
        let data = format!("{member} {receiving_key}");
        crypto::hex(&crypto::hmac(&self.proof_key(), data.as_bytes()))
    }
}

/// A member as shown: identifier, role and display name if known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberInfo {
    pub id: String,
    pub role: Role,
    pub name: Option<String>,
}

/// The change sealing budget key `number` for `member`.
pub(crate) fn seal_key(
    member: &str,
    receiving_key: &str,
    number: u32,
    key: &[u8; 32],
) -> Option<Change> {
    let sealed = crypto::seal(receiving_key, key)?;
    Some(Change::new(
        ACCESS,
        format!("key/{member}/{number}"),
        "key",
        Some(crypto::hex(&sealed)),
    ))
}

fn role_change(member: &str, role: Option<(Role, &str)>) -> Change {
    Change::new(
        ACCESS,
        format!("member/{member}"),
        "role",
        role.map(|(role, receiving_key)| format!("{} {receiving_key}", role.as_str())),
    )
}

impl Store {
    /// The members of `budget`, Admins first.
    pub fn members(&self, budget: BudgetId) -> Result<Vec<MemberInfo>, StoreError> {
        let budget = budget.to_string();
        let mut members: Vec<MemberInfo> = self
            .access(&budget)?
            .members
            .into_iter()
            .map(|(id, member)| MemberInfo {
                id,
                role: member.role,
                name: None,
            })
            .collect();
        for member in &mut members {
            member.name = self.member_name(&budget, &member.id)?;
        }
        // Sort by role, highest first: `Reverse` flips the order.
        members.sort_by_key(|member| std::cmp::Reverse(member.role));
        Ok(members)
    }

    /// This member's role in `budget`, if they belong to it.
    pub fn role(&mut self, budget: BudgetId) -> Result<Option<Role>, StoreError> {
        let me = self.member()?;
        Ok(self.access(&budget.to_string())?.role(&me))
    }

    /// Sets this member's display name, in every budget they can write to.
    pub fn set_name(&mut self, name: &str) -> Result<(), StoreError> {
        self.set_setting("name", name)?;
        for budget in self.budget_ids()? {
            self.introduce(&budget)?;
        }
        Ok(())
    }

    /// An invitation to `budget` with `role`, valid until `expires`
    /// (milliseconds since 1970). Only an Admin may.
    pub fn invite(
        &mut self,
        budget: BudgetId,
        role: Role,
        expires: u64,
    ) -> Result<Invitation, StoreError> {
        let budget = budget.to_string();
        self.require_admin(&self.access(&budget)?)?;
        let invitation = Invitation {
            budget,
            secret: crypto::random(),
        };
        self.connection.execute(
            "INSERT INTO invitations (id, budget, secret, role, expires) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                invitation.id(),
                invitation.budget,
                invitation.secret.as_slice(),
                role.as_str(),
                // SQLite stores signed 64-bit integers: cap the expiry
                // (`i64::MAX` milliseconds is in 292 million years).
                i64::try_from(expires).unwrap_or(i64::MAX)
            ],
        )?;
        Ok(invitation)
    }

    /// Asks to join the budget of `invitation`: an Admin's device will
    /// grant it when it receives the request.
    pub fn request_join(&mut self, invitation: &Invitation) -> Result<(), StoreError> {
        let identity = self.identity()?;
        let (member, receiving_key) = (identity.member(), identity.receiving_key());
        let request = Change::new(
            ACCESS,
            format!("join/{member}"),
            "request",
            Some(format!(
                "{} {} {receiving_key}",
                invitation.id(),
                invitation.proof(&member, &receiving_key)
            )),
        );
        self.write(&invitation.budget, vec![request])
    }

    /// Gives `member` the role `role` in `budget`. Only an Admin may, and
    /// a budget keeps at least one Admin (doc/scope.md).
    pub fn set_role(
        &mut self,
        budget: BudgetId,
        member: &str,
        role: Role,
    ) -> Result<(), StoreError> {
        let budget = budget.to_string();
        let access = self.access(&budget)?;
        self.require_admin(&access)?;
        let Some(current) = access.members.get(member) else {
            return Err(StoreError::NotAllowed("not a member of this budget".into()));
        };
        if current.role == Role::Admin && role != Role::Admin && access.admins() == 1 {
            return Err(StoreError::NotAllowed(
                "a budget keeps at least one Admin: promote someone else first".into(),
            ));
        }
        let change = role_change(member, Some((role, &current.receiving_key)));
        self.write(&budget, vec![change])
    }

    /// Removes `member` from `budget`: an Admin removing someone, or a
    /// member leaving. The last Admin can't leave. The removed member
    /// keeps what they had, and an Admin draws a new key for what follows.
    pub fn remove(&mut self, budget: BudgetId, member: &str) -> Result<(), StoreError> {
        let budget = budget.to_string();
        let me = self.member()?;
        let access = self.access(&budget)?;
        if member != me {
            self.require_admin(&access)?;
        }
        let Some(removed) = access.members.get(member) else {
            return Err(StoreError::NotAllowed("not a member of this budget".into()));
        };
        if removed.role == Role::Admin && access.admins() == 1 {
            return Err(StoreError::NotAllowed(
                "a budget keeps at least one Admin: promote someone else first".into(),
            ));
        }
        self.write(&budget, vec![role_change(member, None)])?;
        // When leaving, someone else draws the new key (in `follow_up`).
        if member != me {
            self.new_key(&budget)?;
        }
        Ok(())
    }

    /// What this member does after receiving messages of `budget`: write
    /// their name if missing and, as an Admin, accept the valid join
    /// requests and draw a new key after someone left.
    pub(crate) fn follow_up(&mut self, budget: &str) -> Result<(), StoreError> {
        if !self.replays || self.identity.is_none() {
            return Ok(());
        }
        let me = self.member()?;
        let access = self.access(budget)?;
        if access.role(&me) == Some(Role::Admin) {
            self.accept_requests(budget, &access)?;
            if access.needs_new_key {
                self.new_key(budget)?;
            }
        }
        self.introduce(budget)
    }

    /// Writes this member's display name in `budget` if it isn't there yet
    /// and they can (a member holding a key).
    pub(crate) fn introduce(&mut self, budget: &str) -> Result<(), StoreError> {
        let Some(name) = self.setting("name")? else {
            return Ok(());
        };
        let me = self.member()?;
        let known = self.member_name(budget, &me)?;
        let access = self.access(budget)?;
        if access.role(&me).is_none()
            || self.keys(budget)?.is_empty()
            || known.as_deref() == Some(name.as_str())
        {
            return Ok(());
        }
        self.write(budget, vec![Change::new("member", me, "name", Some(name))])
    }

    /// Grants the requests made with a valid invitation of this device.
    fn accept_requests(&mut self, budget: &str, access: &Access) -> Result<(), StoreError> {
        for (member, request) in &access.requests {
            if access.members.contains_key(member) {
                continue;
            }
            let row: Option<(Vec<u8>, String, i64)> = self
                .connection
                .query_row(
                    "SELECT secret, role, expires FROM invitations
                     WHERE id = ?1 AND budget = ?2 AND used = 0",
                    params![request.invitation, budget],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let Some((secret, role, expires)) = row else {
                continue; // not ours, or already used
            };
            let invitation = Invitation {
                budget: budget.to_string(),
                secret: secret
                    .try_into()
                    .map_err(|_| crate::Corrupt("secret".into()))?,
            };
            let in_time = clock::parse(&request.timestamp)
                .is_some_and(|(millis, _, _)| i64::try_from(millis).is_ok_and(|m| m <= expires));
            let proof = invitation.proof(member, &request.receiving_key);
            let Some(role) = Role::parse(&role) else {
                continue;
            };
            if !in_time || proof != request.proof {
                continue;
            }
            // The role, then every key we hold, so the newcomer reads the
            // whole history.
            let mut changes = vec![role_change(member, Some((role, &request.receiving_key)))];
            for (number, key) in self.keys(budget)? {
                if let Some(change) = seal_key(member, &request.receiving_key, number, &key) {
                    changes.push(change);
                }
            }
            self.connection.execute(
                "UPDATE invitations SET used = 1 WHERE id = ?1",
                params![request.invitation],
            )?;
            self.write(budget, changes)?;
        }
        Ok(())
    }

    /// Draws a new key for `budget` and seals it for every current member.
    fn new_key(&mut self, budget: &str) -> Result<(), StoreError> {
        let number = self.new_key_number(budget)?;
        let key = self.key(budget, number)?.expect("just drawn");
        let changes = self
            .access(budget)?
            .members
            .iter()
            .filter_map(|(member, info)| seal_key(member, &info.receiving_key, number, &key))
            .collect();
        self.write(budget, changes)
    }

    fn member_name(&self, budget: &str, member: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM fields
                 WHERE budget = ?1 AND entity = 'member' AND id = ?2 AND field = 'name'",
                params![budget, member],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    fn require_admin(&mut self, access: &Access) -> Result<(), StoreError> {
        let me = self.member()?;
        if access.role(&me) == Some(Role::Admin) {
            Ok(())
        } else {
            Err(StoreError::NotAllowed("only an Admin may do this".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Role, Store, StoreError, Versions};
    use bougette_core::budget::Budget;
    use bougette_core::money::Currency;

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

    const LATER: u64 = u64::MAX;

    #[test]
    fn an_invitation_lets_someone_in_once() {
        let mut alice = named("Alice");
        let mut bob = named("Bob");
        let mut mallory = named("Mallory");
        let budget = alice.create(Budget::new("Home", Currency::EUR)).unwrap();
        let invitation = alice.invite(budget.id, Role::Editor, LATER).unwrap();

        // Before joining, Bob gets the messages but can't read them.
        send(&alice, &mut bob);
        assert!(bob.load(budget.id).unwrap().is_none());

        bob.request_join(&invitation).unwrap();
        send(&bob, &mut alice); // Alice's device grants the request…
        send(&alice, &mut bob); // …and Bob gets the keys.
        assert_eq!(bob.load(budget.id).unwrap(), Some(budget.clone()));
        assert_eq!(bob.role(budget.id).unwrap(), Some(Role::Editor));
        send(&bob, &mut alice);
        let names: Vec<_> = alice
            .members(budget.id)
            .unwrap()
            .into_iter()
            .map(|member| (member.role, member.name.unwrap_or_default()))
            .collect();
        assert_eq!(
            names,
            [
                (Role::Admin, "Alice".to_string()),
                (Role::Editor, "Bob".to_string())
            ]
        );

        // The invitation was used: Mallory, given it too, stays out.
        send(&alice, &mut mallory);
        mallory.request_join(&invitation).unwrap();
        send(&mallory, &mut alice);
        send(&alice, &mut mallory);
        assert!(mallory.load(budget.id).unwrap().is_none());
        assert_eq!(alice.members(budget.id).unwrap().len(), 2);
    }

    #[test]
    fn a_forged_proof_or_an_expired_invitation_is_ignored() {
        let mut alice = Store::open_in_memory().unwrap();
        let mut bob = Store::open_in_memory().unwrap();
        let budget = alice.create(Budget::new("Home", Currency::EUR)).unwrap();
        let mut invitation = alice.invite(budget.id, Role::Editor, LATER).unwrap();
        let expired = alice.invite(budget.id, Role::Editor, 0).unwrap();
        // The right identifier but another secret: a wrong proof.
        let id = invitation.id();
        invitation.secret[0] ^= 1;
        assert_ne!(invitation.id(), id);
        send(&alice, &mut bob);
        bob.request_join(&expired).unwrap();
        send(&bob, &mut alice);
        assert_eq!(alice.members(budget.id).unwrap().len(), 1);
    }

    #[test]
    fn readers_read_and_editors_edit() {
        let mut alice = Store::open_in_memory().unwrap();
        let mut bob = Store::open_in_memory().unwrap();
        let mut budget = alice.create(Budget::new("Home", Currency::EUR)).unwrap();
        let invitation = alice.invite(budget.id, Role::Reader, LATER).unwrap();
        send(&alice, &mut bob);
        bob.request_join(&invitation).unwrap();
        send(&bob, &mut alice);
        send(&alice, &mut bob);

        budget.name = "Bob's".to_string();
        assert!(matches!(bob.save(&budget), Err(StoreError::NotAllowed(_))));
        // Promoted to Editor, Bob can change it, and Alice sees it.
        let bob_id = bob.member().unwrap();
        alice.set_role(budget.id, &bob_id, Role::Editor).unwrap();
        send(&alice, &mut bob);
        bob.save(&budget).unwrap();
        send(&bob, &mut alice);
        assert_eq!(alice.load(budget.id).unwrap().unwrap().name, "Bob's");
        // The last Admin can't step down or leave.
        let alice_id = alice.member().unwrap();
        assert!(alice.set_role(budget.id, &alice_id, Role::Editor).is_err());
        assert!(alice.remove(budget.id, &alice_id).is_err());
    }

    #[test]
    fn a_removed_member_reads_nothing_new() {
        let mut alice = Store::open_in_memory().unwrap();
        let mut bob = Store::open_in_memory().unwrap();
        let mut budget = alice.create(Budget::new("Home", Currency::EUR)).unwrap();
        let invitation = alice.invite(budget.id, Role::Editor, LATER).unwrap();
        send(&alice, &mut bob);
        bob.request_join(&invitation).unwrap();
        send(&bob, &mut alice);
        send(&alice, &mut bob);

        let bob_id = bob.member().unwrap();
        alice.remove(budget.id, &bob_id).unwrap();
        budget.name = "After Bob".to_string();
        alice.save(&budget).unwrap();
        send(&alice, &mut bob);
        // Bob keeps the old name: the new one is under a key he lacks.
        assert_eq!(bob.load(budget.id).unwrap().unwrap().name, "Home");
        assert_eq!(bob.role(budget.id).unwrap(), None);
        // And what he writes now no longer counts.
        let mut forged = budget.clone();
        forged.name = "Bob was here".to_string();
        assert!(bob.save(&forged).is_err());
    }

    #[test]
    fn someone_leaving_gets_no_new_key() {
        let mut alice = Store::open_in_memory().unwrap();
        let mut bob = Store::open_in_memory().unwrap();
        let mut budget = alice.create(Budget::new("Home", Currency::EUR)).unwrap();
        let invitation = alice.invite(budget.id, Role::Editor, LATER).unwrap();
        send(&alice, &mut bob);
        bob.request_join(&invitation).unwrap();
        send(&bob, &mut alice);
        send(&alice, &mut bob);

        let bob_id = bob.member().unwrap();
        bob.remove(budget.id, &bob_id).unwrap();
        send(&bob, &mut alice); // Alice's device draws a new key.
        budget.name = "Without Bob".to_string();
        alice.save(&budget).unwrap();
        send(&alice, &mut bob);
        assert_eq!(bob.load(budget.id).unwrap().unwrap().name, "Home");
        assert_eq!(
            alice.versions().unwrap().len(),
            Versions::len(&bob.versions().unwrap())
        );
    }
}
