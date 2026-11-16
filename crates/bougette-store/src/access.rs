//! Who may do what in a budget: its members, their roles, its keys and
//! the requests to join it, rebuilt by applying the access changes in
//! timestamp order (ADR 0005). An invalid change is ignored, never an
//! error: a member's change counts or not depending on their role *when
//! they made it*.
//!
//! The access changes (entity `access`), by identifier:
//! - `origin`: the creation. Valid only from the creator, the member whose
//!   key, hashed with the change's nonce, gives the budget's identifier.
//!   They become its first Admin. Value: `NONCE RECEIVING_KEY`.
//! - `member/M`: M's role, `ROLE RECEIVING_KEY`, or nothing when M leaves.
//!   Valid from an Admin, or from M leaving.
//! - `key/M/N`: budget key number N, sealed for M. Valid from an Admin.
//! - `join/M`: M asks to join with an invitation: `INVITATION PROOF
//!   RECEIVING_KEY`. Valid from M only.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::crypto;
use crate::message::Change;

/// What a member may do in a budget (GLOSSARY.md: Role).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// Reads everything, changes nothing (except their own name).
    Reader,
    /// Changes lines and transactions.
    Editor,
    /// Also manages members and roles.
    Admin,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Reader => "reader",
            Role::Editor => "editor",
            Role::Admin => "admin",
        }
    }

    pub fn parse(text: &str) -> Option<Role> {
        match text {
            "reader" => Some(Role::Reader),
            "editor" => Some(Role::Editor),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }
}

/// A member of a budget, as access changes describe them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub role: Role,
    /// The key their budget keys are sealed for.
    pub receiving_key: String,
}

/// A request to join a budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinRequest {
    /// When it was made: an invitation has to be valid then.
    pub timestamp: String,
    pub invitation: String,
    pub proof: String,
    pub receiving_key: String,
}

/// The access of one budget, as of the changes applied so far.
#[derive(Debug, Clone, Default)]
pub struct Access {
    /// Whether the creation was seen: before it, nothing is valid.
    created: bool,
    /// The members by identifier.
    pub members: BTreeMap<String, Member>,
    /// The sealed budget keys, by member and key number.
    pub sealed: BTreeMap<(String, u32), Vec<u8>>,
    /// The key numbers, the latest announced last: new changes are
    /// encrypted with the latest one a device holds.
    pub keys: Vec<u32>,
    /// The pending and past join requests, by member.
    pub requests: BTreeMap<String, JoinRequest>,
    /// Whether a member left after the latest key was announced: an Admin
    /// then draws a new key they don't get.
    pub needs_new_key: bool,
}

impl Access {
    pub fn role(&self, member: &str) -> Option<Role> {
        self.members.get(member).map(|member| member.role)
    }

    pub fn admins(&self) -> usize {
        self.members
            .values()
            .filter(|member| member.role == Role::Admin)
            .count()
    }

    /// Whether `author` may make `change`, a content change (not access).
    pub fn may_write(&self, author: &str, change: &Change) -> bool {
        match self.role(author) {
            Some(Role::Admin | Role::Editor) => true,
            // A Reader may still set their own display name.
            Some(Role::Reader) => change.entity == "member" && change.id == author,
            None => false,
        }
    }

    /// Applies an access change made by `author` at `timestamp` in budget
    /// `budget`; returns whether it was valid.
    pub fn apply(&mut self, budget: &str, author: &str, timestamp: &str, change: &Change) -> bool {
        let words: Vec<&str> = change
            .value
            .as_deref()
            .unwrap_or_default()
            .split(' ')
            .collect();
        let is_admin = self.role(author) == Some(Role::Admin);
        // Match on the identifier's parts and the value's words at once.
        let parts: Vec<&str> = change.id.split('/').collect();
        match (parts.as_slice(), change.field.as_str(), words.as_slice()) {
            (["origin"], "origin", [nonce, receiving_key]) => {
                let creator = crypto::from_hex(nonce)
                    .map(|nonce| crypto::budget_id(author, &nonce).to_string());
                if self.created || creator.as_deref() != Some(budget) {
                    return false;
                }
                self.created = true;
                self.add(author, Role::Admin, receiving_key);
            }
            (["member", member], "role", [role, receiving_key]) if is_admin => {
                let Some(role) = Role::parse(role) else {
                    return false;
                };
                self.add(member, role, receiving_key);
            }
            (["member", member], "role", [""]) if is_admin || *member == author => {
                if self.members.remove(*member).is_some() {
                    self.needs_new_key = true;
                }
            }
            (["key", member, number], "key", [sealed]) if is_admin => {
                let (Ok(number), Some(sealed)) = (number.parse(), crypto::from_hex(sealed)) else {
                    return false;
                };
                self.sealed.insert((member.to_string(), number), sealed);
                self.keys.retain(|known| *known != number);
                self.keys.push(number);
                self.needs_new_key = false;
            }
            (["join", member], "request", [invitation, proof, receiving_key])
                if *member == author && self.created =>
            {
                self.requests.insert(
                    author.to_string(),
                    JoinRequest {
                        timestamp: timestamp.to_string(),
                        invitation: invitation.to_string(),
                        proof: proof.to_string(),
                        receiving_key: receiving_key.to_string(),
                    },
                );
            }
            _ => return false,
        }
        true
    }

    fn add(&mut self, member: &str, role: Role, receiving_key: &str) {
        self.members.insert(
            member.to_string(),
            Member {
                role,
                receiving_key: receiving_key.to_string(),
            },
        );
    }
}

/// The value of an `origin` change for a budget created by `identity`,
/// with the budget's identifier.
pub fn origin(identity: &crypto::Identity) -> (Uuid, Change) {
    let nonce: [u8; 16] = crypto::random();
    let id = crypto::budget_id(&identity.member(), &nonce);
    let change = Change::new(
        crate::message::ACCESS,
        "origin",
        "origin",
        Some(format!(
            "{} {}",
            crypto::hex(&nonce),
            identity.receiving_key()
        )),
    );
    (id, change)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::Identity;
    use crate::message::ACCESS;

    fn grant(member: &Identity, role: &str) -> Change {
        Change::new(
            ACCESS,
            format!("member/{}", member.member()),
            "role",
            Some(format!("{role} {}", member.receiving_key())),
        )
    }

    #[test]
    fn only_the_creator_starts_a_budget_and_only_admins_grant() {
        let alice = Identity::generate();
        let bob = Identity::generate();
        let mallory = Identity::generate();
        let (id, origin) = origin(&alice);
        let budget = id.to_string();
        let mut access = Access::default();
        // Mallory can't claim Alice's budget, and nothing counts before
        // its creation.
        assert!(!access.apply(&budget, &mallory.member(), "t0", &origin));
        assert!(!access.apply(&budget, &mallory.member(), "t0", &grant(&mallory, "admin")));
        assert!(access.apply(&budget, &alice.member(), "t1", &origin));
        assert_eq!(access.role(&alice.member()), Some(Role::Admin));
        // A second origin, even Alice's, is ignored.
        assert!(!access.apply(&budget, &alice.member(), "t2", &origin));

        assert!(access.apply(&budget, &alice.member(), "t3", &grant(&bob, "editor")));
        // An Editor can't grant.
        assert!(!access.apply(&budget, &bob.member(), "t4", &grant(&mallory, "admin")));
        assert_eq!(access.role(&mallory.member()), None);
        let name = Change::new("member", bob.member(), "name", Some("Bob".into()));
        assert!(access.may_write(&bob.member(), &name));
        assert!(!access.may_write(&mallory.member(), &name));

        // Bob leaves on his own: a new key is then needed.
        let leave = Change::new(ACCESS, format!("member/{}", bob.member()), "role", None);
        assert!(access.apply(&budget, &bob.member(), "t5", &leave));
        assert_eq!(access.role(&bob.member()), None);
        assert!(access.needs_new_key);
    }

    #[test]
    fn join_requests_come_from_the_joiner() {
        let alice = Identity::generate();
        let bob = Identity::generate();
        let (id, origin) = origin(&alice);
        let budget = id.to_string();
        let mut access = Access::default();
        access.apply(&budget, &alice.member(), "t1", &origin);
        let request = Change::new(
            ACCESS,
            format!("join/{}", bob.member()),
            "request",
            Some(format!("inv proof {}", bob.receiving_key())),
        );
        assert!(!access.apply(&budget, &alice.member(), "t2", &request));
        assert!(access.apply(&budget, &bob.member(), "t2", &request));
        assert_eq!(access.requests[&bob.member()].invitation, "inv");
    }
}
