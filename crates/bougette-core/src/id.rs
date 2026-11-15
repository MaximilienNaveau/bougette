//! Identifiers that stay unique across devices.

use std::fmt;

use uuid::Uuid;

/// The identifier of a line.
///
/// A "newtype": a struct around a single value. It costs nothing at run
/// time but stops the compiler from accepting a TransactionId where a
/// LineId is expected, which two raw Uuids would not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineId(pub Uuid);

/// The identifier of a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransactionId(pub Uuid);

impl LineId {
    /// A new, random identifier (UUID version 7).
    pub fn new() -> LineId {
        LineId(Uuid::now_v7())
    }
}

impl TransactionId {
    /// A new, random identifier (UUID version 7).
    pub fn new() -> TransactionId {
        TransactionId(Uuid::now_v7())
    }
}

// Clippy wants a type with a `new()` taking no argument to implement
// `Default` too, the trait behind `T::default()` (like a default
// constructor). Both are the same here.
impl Default for LineId {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LineId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Delegate to Uuid's own Display: `{}` on the inner value.
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
