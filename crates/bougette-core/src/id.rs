//! Identifiers that stay unique across devices.

/// Defines an identifier type wrapping a UUID.
///
/// `macro_rules!` defines a macro: code that writes code at compile time.
/// Unlike the C preprocessor it works on syntax, not text, and is
/// hygienic (its variables can't clash with the caller's). Here it saves
/// writing the same newtype three times. `$name:ident` is a parameter that
/// must be an identifier, and `$(#[$doc:meta])*` copies the doc comment.
macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        ///
        /// A "newtype": a struct around a single value. It costs nothing at
        /// run time but stops the compiler from accepting one kind of
        /// identifier where another is expected, which raw Uuids would not.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub uuid::Uuid);

        impl $name {
            /// A new, random identifier (UUID version 7: it starts with a
            /// timestamp, so identifiers sort by creation time).
            pub fn new() -> $name {
                $name(uuid::Uuid::now_v7())
            }
        }

        // Clippy wants a type with a `new()` taking no argument to
        // implement `Default` too, the trait behind `T::default()` (like a
        // default constructor). Both are the same here.
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                // Delegate to Uuid's own Display: `{}` on the inner value.
                write!(f, "{}", self.0)
            }
        }
    };
}

// Each call expands to a full type definition, as if written out here.
id_type!(
    /// The identifier of a line.
    LineId
);
id_type!(
    /// The identifier of a transaction.
    TransactionId
);
id_type!(
    /// The identifier of a pot.
    PotId
);
