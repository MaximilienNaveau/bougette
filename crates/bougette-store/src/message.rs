//! Messages: one change of one field, signed by its author's member key.
//! The content is encrypted with a key of its budget, except changes of
//! *access* (who belongs to the budget, with which role, the sealed budget
//! keys, join requests): the server must read those to know whom to pass
//! the budget on to (ADR 0005).

use crate::bytes::{Reader, Writer};
use crate::crypto::{self, Identity};

/// The entity of access changes, the only ones sent in clear.
pub const ACCESS: &str = "access";

/// One change of one field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub entity: String,
    pub id: String,
    pub field: String,
    /// The new value; `None` for an empty optional field.
    pub value: Option<String>,
}

impl Change {
    // `impl Into<String>`: accepts `&str` or `String`, as in the core.
    pub fn new(
        entity: impl Into<String>,
        id: impl Into<String>,
        field: impl Into<String>,
        value: Option<String>,
    ) -> Change {
        Change {
            entity: entity.into(),
            id: id.into(),
            field: field.into(),
            value,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::default();
        writer
            .text(&self.entity)
            .text(&self.id)
            .text(&self.field)
            .optional(self.value.as_deref());
        writer.0
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Change> {
        let mut reader = Reader(bytes);
        let change = Change {
            entity: reader.text()?,
            id: reader.text()?,
            field: reader.text()?,
            value: reader.optional()?,
        };
        // `then_some`: `Some(change)` if the condition holds, else `None`.
        reader.done().then_some(change)
    }
}

/// What a message carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// An access change, in clear.
    Clear(Change),
    /// Any other change, encrypted with the budget key `key`.
    Sealed { key: u32, bytes: Vec<u8> },
}

/// A change as stored and exchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// When and on which device it was made (see `clock`). Unique.
    pub timestamp: String,
    pub budget: String,
    /// The member who made it: their identifier (public key).
    pub author: String,
    pub body: Body,
    /// The author's signature of everything above.
    pub signature: Vec<u8>,
}

impl Message {
    /// A message by `identity`, encrypted with `key` (its number and
    /// bytes) unless it is an access change.
    pub fn new(
        identity: &Identity,
        timestamp: String,
        budget: String,
        change: Change,
        key: Option<(u32, &[u8; 32])>,
    ) -> Option<Message> {
        let body = if change.entity == ACCESS {
            Body::Clear(change)
        } else {
            // `?` on an Option: no key, no message.
            let (number, key) = key?;
            Body::Sealed {
                key: number,
                bytes: crypto::encrypt(key, &context(&timestamp, &budget), &change.to_bytes()),
            }
        };
        let mut message = Message {
            timestamp,
            budget,
            author: identity.member(),
            body,
            signature: Vec::new(),
        };
        message.signature = identity.sign(&message.signed_bytes());
        Some(message)
    }

    /// Whether the author really wrote it, and it has the right shape: an
    /// access change in clear, nothing else.
    pub fn is_authentic(&self) -> bool {
        let shape = match &self.body {
            Body::Clear(change) => change.entity == ACCESS,
            Body::Sealed { .. } => true,
        };
        shape && crypto::verify(&self.author, &self.signed_bytes(), &self.signature)
    }

    /// The change it carries, decrypted with `key` if sealed. `None` if
    /// the key is missing or wrong, or if a sealed change claims to be an
    /// access change (those must be readable by the server).
    // `impl Fn(u32) -> Option<[u8; 32]>`: any function giving the key of
    // each number, e.g. a lookup in a map.
    pub fn change(&self, key: impl Fn(u32) -> Option<[u8; 32]>) -> Option<Change> {
        match &self.body {
            Body::Clear(change) => Some(change.clone()),
            Body::Sealed { key: number, bytes } => {
                let plain = crypto::decrypt(
                    &key(*number)?,
                    &context(&self.timestamp, &self.budget),
                    bytes,
                )?;
                Change::from_bytes(&plain).filter(|change| change.entity != ACCESS)
            }
        }
    }

    /// The body as stored: the encoded change, or the ciphertext, with the
    /// key number for the latter.
    pub fn stored_body(&self) -> (Option<u32>, Vec<u8>) {
        match &self.body {
            Body::Clear(change) => (None, change.to_bytes()),
            Body::Sealed { key, bytes } => (Some(*key), bytes.clone()),
        }
    }

    /// The reverse of `stored_body`.
    pub fn body_from(key: Option<u32>, bytes: Vec<u8>) -> Option<Body> {
        Some(match key {
            None => Body::Clear(Change::from_bytes(&bytes)?),
            Some(key) => Body::Sealed { key, bytes },
        })
    }

    /// What the signature covers: every field, each length-prefixed so
    /// that no two different messages give the same bytes.
    fn signed_bytes(&self) -> Vec<u8> {
        let (key, body) = self.stored_body();
        let mut writer = Writer::default();
        writer
            .text("bougette message 1")
            .text(&self.timestamp)
            .text(&self.budget)
            .text(&self.author);
        match key {
            None => writer.u32(0),
            Some(key) => writer.u32(1).u32(key),
        };
        writer.bytes(&body);
        writer.0
    }
}

/// What a sealed change is bound to: copied into another message, it
/// wouldn't decrypt.
fn context(timestamp: &str, budget: &str) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.text(timestamp).text(budget);
    writer.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rent() -> Change {
        Change::new("line", "l1", "name", Some("Rent".to_string()))
    }

    #[test]
    fn sealed_changes_need_the_key() {
        let alice = Identity::generate();
        let key: [u8; 32] = crypto::random();
        let message =
            Message::new(&alice, "t1".into(), "b".into(), rent(), Some((7, &key))).unwrap();
        assert!(message.is_authentic());
        assert!(matches!(message.body, Body::Sealed { key: 7, .. }));
        assert_eq!(message.change(|_| Some(key)), Some(rent()));
        assert_eq!(message.change(|_| None), None);
        assert_eq!(message.change(|_| Some(crypto::random())), None);
        // Without a key, a content change can't be written at all.
        assert!(Message::new(&alice, "t1".into(), "b".into(), rent(), None).is_none());
    }

    #[test]
    fn tampering_breaks_the_signature() {
        let alice = Identity::generate();
        let grant = Change::new(ACCESS, "member/x", "role", Some("admin k".to_string()));
        let message = Message::new(&alice, "t1".into(), "b".into(), grant, None).unwrap();
        assert!(matches!(message.body, Body::Clear(_)));
        assert!(message.is_authentic());
        let mut forged = message.clone();
        forged.author = Identity::generate().member();
        assert!(!forged.is_authentic());
        let mut moved = message;
        moved.budget = "other".into();
        assert!(!moved.is_authentic());
    }
}
